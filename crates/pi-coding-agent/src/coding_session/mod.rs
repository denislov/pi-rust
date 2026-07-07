mod agent_invocation_flow;
mod agent_team_flow;
mod branch_summary_flow;
mod branch_summary_service;
mod capability_service;
mod context;
mod delegation;
mod delegation_confirmation_service;
mod delegation_execution_service;
mod error;
mod event;
mod event_service;
mod export;
mod export_flow;
mod flow_service;
mod manual_compaction_flow;
mod manual_compaction_service;
mod operation;
mod operation_control;
mod plugin_load_flow;
mod plugin_load_service;
mod plugin_service;
mod profiles;
mod prompt;
mod prompt_flow;
mod runtime_service;
mod self_healing_edit_flow;
mod self_healing_edit_service;
mod session_log;
mod session_service;

pub use agent_invocation_flow::{AgentInvocationOptions, AgentInvocationOutcome};
pub use agent_team_flow::{AgentTeamMemberOutcome, AgentTeamOptions, AgentTeamOutcome};
pub use context::{
    CapabilityStatus, CodingAgentCapabilities, CodingAgentSessionOptions,
    CodingAgentSessionSummary, CodingAgentSessionView,
};
pub(crate) use context::{
    CodingAgentSessionDiagnostic, CodingAgentSessionHydration, CodingAgentSessionTranscriptItem,
    CodingAgentSessionTree, CodingAgentSessionUsageSummary,
};
pub use delegation::PendingDelegationConfirmation;
pub use error::CodingSessionError;
pub use event::CodingAgentEvent;
pub use event_service::CodingAgentEventReceiver;
pub use export::{CodingAgentSessionExport, CodingAgentSessionExportItem};
pub(crate) use plugin_load_flow::PluginLoadOutcome;
pub use profiles::{
    AgentProfile, DelegationConfirmationMode, DelegationPolicy, ProfileDiagnostic, ProfileId,
    ProfileKind, ProfileRegistry, ProfileRegistryOptions, ProfileSource, SupervisionPolicy,
    TeamProfile, TeamStrategy, TeamSupervisor,
};
pub use prompt::{
    CodingDiagnostic, CodingDiagnosticSeverity, PromptTurnMode, PromptTurnOptions,
    PromptTurnOutcome,
};
pub use self_healing_edit_flow::{
    SelfHealingEditCheckOutput, SelfHealingEditDiagnostic, SelfHealingEditModelRepairOptions,
    SelfHealingEditOutcome, SelfHealingEditRepairAttempt, SelfHealingEditReplacement,
    SelfHealingEditRequest,
};

use agent_invocation_flow::AgentInvocationContext;
use agent_team_flow::AgentTeamContext;
use branch_summary_service::BranchSummaryService;
use capability_service::CapabilityService;
pub(crate) use delegation::{
    DelegationAuthorizationDecision, PendingDelegationConfirmationQueue,
    PendingDelegationConfirmationState, delegation_lineage_for_request, pending_state_from_replay,
};
use delegation_confirmation_service::DelegationConfirmationService;
use delegation_execution_service::DelegationExecutionService;
use event_service::EventService;
use export_flow::ExportOptions;
use flow_service::FlowService;
use manual_compaction_flow::ManualCompactionOptions;
use manual_compaction_service::ManualCompactionService;
use operation::{Operation, OperationOutcome};
use operation_control::OperationControl;
pub(crate) use operation_control::{OperationKind, PromptControlHandle};
use plugin_load_flow::PluginLoadOptions;
use plugin_load_service::PluginLoadService;
use plugin_service::PluginService;
use prompt::{PromptTurnContext, PromptTurnIds};
use runtime_service::RuntimeService;
pub(crate) use self_healing_edit_flow::{
    ModelSelfHealingEditRepairStrategy, SelfHealingEditContext, SelfHealingEditFlow,
    SelfHealingEditOptions, SelfHealingEditRepairStrategy,
};
use self_healing_edit_service::SelfHealingEditService;
use session_log::event::PersistedDelegationStatus;
use session_log::id::{Clock, IdGenerator, SystemClock, SystemIdGenerator};
use session_service::{
    FinalizedSessionWrite, SessionPersistence, SessionService, TransientSessionState,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::plugins::{
    CommandDefinition, KeybindDefinition, PluginSource, UiActionDefinition, UiDialogDefinition,
};

#[derive(Debug)]
pub struct CodingAgentSession {
    persistence: SessionPersistence,
    runtime_service: RuntimeService,
    flow_service: FlowService,
    event_service: EventService,
    capability_service: CapabilityService,
    plugin_service: PluginService,
    plugin_load_service: PluginLoadService,
    profile_registry: ProfileRegistry,
    default_plugin_load_options: PluginLoadOptions,
    operation_control: OperationControl,
    pending_delegation_confirmations: PendingDelegationConfirmationQueue,
    branch_summary_service: BranchSummaryService,
    delegation_confirmation_service: DelegationConfirmationService,
    delegation_execution_service: DelegationExecutionService,
    manual_compaction_service: ManualCompactionService,
    self_healing_edit_service: SelfHealingEditService,
}

fn default_plugin_load_options(options: &CodingAgentSessionOptions) -> PluginLoadOptions {
    let cwd = options
        .cwd()
        .map(Path::to_path_buf)
        .unwrap_or_else(default_cwd);
    let paths = crate::config::resolve_paths(&cwd);
    PluginLoadOptions::new()
        .with_discovery_root(paths.project_dir.join("plugins"), PluginSource::Project)
        .with_discovery_root(paths.global_dir.join("plugins"), PluginSource::User)
}

fn profile_registry_for_options(
    options: &CodingAgentSessionOptions,
    session_service: Option<&SessionService>,
) -> Result<ProfileRegistry, CodingSessionError> {
    let cwd = options
        .cwd()
        .map(Path::to_path_buf)
        .or_else(|| session_service.and_then(session_cwd))
        .unwrap_or_else(default_cwd);
    let paths = crate::config::resolve_paths(&cwd);
    ProfileRegistry::load(
        ProfileRegistryOptions::new()
            .with_user_root(paths.global_dir)
            .with_project_root(paths.project_dir),
    )
}

fn session_cwd(session_service: &SessionService) -> Option<PathBuf> {
    session_service
        .replay()
        .ok()
        .and_then(|replay| replay.cwd.map(PathBuf::from))
}

fn option_default_agent_profile_id(options: &CodingAgentSessionOptions) -> ProfileId {
    options
        .default_agent_profile_id()
        .cloned()
        .unwrap_or_else(|| ProfileId::from("default"))
}

fn default_cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

impl CodingAgentSession {
    pub async fn create(options: CodingAgentSessionOptions) -> Result<Self, CodingSessionError> {
        let session_service = SessionService::create(&options)?;
        let profile_registry = profile_registry_for_options(&options, Some(&session_service))?;
        Self::from_services(
            session_service,
            default_plugin_load_options(&options),
            profile_registry,
        )
    }

    pub async fn open(options: CodingAgentSessionOptions) -> Result<Self, CodingSessionError> {
        let session_service = SessionService::open(&options)?;
        let profile_registry = profile_registry_for_options(&options, Some(&session_service))?;
        Self::from_services(
            session_service,
            default_plugin_load_options(&options),
            profile_registry,
        )
    }

    pub async fn open_or_create(
        options: CodingAgentSessionOptions,
    ) -> Result<Self, CodingSessionError> {
        let session_service = SessionService::open_or_create(&options)?;
        let profile_registry = profile_registry_for_options(&options, Some(&session_service))?;
        Self::from_services(
            session_service,
            default_plugin_load_options(&options),
            profile_registry,
        )
    }

    pub async fn non_persistent(
        options: CodingAgentSessionOptions,
    ) -> Result<Self, CodingSessionError> {
        if options.session_id().is_some() || options.session_path().is_some() {
            return Err(CodingSessionError::Input {
                message: "non-persistent coding sessions do not accept a session id or path".into(),
            });
        }
        Self::from_transient(
            TransientSessionState::new(option_default_agent_profile_id(&options)),
            default_plugin_load_options(&options),
            profile_registry_for_options(&options, None)?,
        )
    }

    pub fn list(
        options: CodingAgentSessionOptions,
    ) -> Result<Vec<CodingAgentSessionSummary>, CodingSessionError> {
        SessionService::list(&options)
    }

    pub(crate) fn hydrate(
        options: CodingAgentSessionOptions,
    ) -> Result<CodingAgentSessionHydration, CodingSessionError> {
        SessionService::hydrate(&options)
    }

    pub(crate) fn tree_view(
        options: CodingAgentSessionOptions,
    ) -> Result<CodingAgentSessionTree, CodingSessionError> {
        SessionService::tree_view(&options)
    }

    pub(crate) fn clone_session(
        options: CodingAgentSessionOptions,
    ) -> Result<CodingAgentSessionHydration, CodingSessionError> {
        SessionService::open(&options)?
            .clone_current()?
            .hydrated_view()
    }

    pub(crate) fn fork_session(
        options: CodingAgentSessionOptions,
        target_leaf_id: Option<&str>,
    ) -> Result<CodingAgentSessionHydration, CodingSessionError> {
        SessionService::open(&options)?
            .fork_current(target_leaf_id)?
            .hydrated_view()
    }

    pub fn export_session_html(
        options: CodingAgentSessionOptions,
        path: impl AsRef<Path>,
    ) -> Result<PathBuf, CodingSessionError> {
        let session_service = SessionService::open(&options)?;
        let mut context = session_service.export_context(ExportOptions::html(path.as_ref()))?;
        let outcome = FlowService::new().run_export(&mut context)?;
        outcome.path.ok_or_else(|| CodingSessionError::Session {
            message: "export completed without a written html path".into(),
        })
    }

    pub fn export_current_html(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<PathBuf, CodingSessionError> {
        match self.run_sync_operation(Operation::Export(ExportOptions::html(path.as_ref())))? {
            OperationOutcome::Export(outcome) => {
                outcome.path.ok_or_else(|| CodingSessionError::Session {
                    message: "export completed without a written html path".into(),
                })
            }
            OperationOutcome::Prompt(_) => unreachable!("export operation returned prompt outcome"),
            OperationOutcome::ManualCompaction(_) => {
                unreachable!("export operation returned manual compaction outcome")
            }
            OperationOutcome::PluginLoad(_) => {
                unreachable!("export operation returned plugin load outcome")
            }
            OperationOutcome::PluginCommand(_) => {
                unreachable!("export operation returned plugin command outcome")
            }
            OperationOutcome::BranchSummary(_) => {
                unreachable!("export operation returned branch summary outcome")
            }
            OperationOutcome::SelfHealingEdit(_) => {
                unreachable!("export operation returned self-healing edit outcome")
            }
            OperationOutcome::AgentInvocation(_) => {
                unreachable!("export operation returned agent invocation outcome")
            }
            OperationOutcome::AgentTeam(_) => {
                unreachable!("export operation returned agent team outcome")
            }
        }
    }

    pub fn export_current(&self) -> Result<CodingAgentSessionExport, CodingSessionError> {
        match self.run_sync_operation(Operation::Export(ExportOptions::view()))? {
            OperationOutcome::Export(outcome) => Ok(outcome.export),
            OperationOutcome::Prompt(_) => unreachable!("export operation returned prompt outcome"),
            OperationOutcome::ManualCompaction(_) => {
                unreachable!("export operation returned manual compaction outcome")
            }
            OperationOutcome::PluginLoad(_) => {
                unreachable!("export operation returned plugin load outcome")
            }
            OperationOutcome::PluginCommand(_) => {
                unreachable!("export operation returned plugin command outcome")
            }
            OperationOutcome::BranchSummary(_) => {
                unreachable!("export operation returned branch summary outcome")
            }
            OperationOutcome::SelfHealingEdit(_) => {
                unreachable!("export operation returned self-healing edit outcome")
            }
            OperationOutcome::AgentInvocation(_) => {
                unreachable!("export operation returned agent invocation outcome")
            }
            OperationOutcome::AgentTeam(_) => {
                unreachable!("export operation returned agent team outcome")
            }
        }
    }

    pub(crate) fn hydrate_current(
        &self,
    ) -> Result<Option<CodingAgentSessionHydration>, CodingSessionError> {
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => {
                Ok(Some(session_service.hydrated_view()?))
            }
            SessionPersistence::NonPersistent(_) => Ok(None),
        }
    }

    pub(crate) fn fork_current_session(
        &self,
        target_leaf_id: Option<&str>,
    ) -> Result<Self, CodingSessionError> {
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => Self::from_services(
                session_service.fork_current(target_leaf_id)?,
                self.default_plugin_load_options.clone(),
                self.profile_registry.clone(),
            ),
            SessionPersistence::NonPersistent(_) => {
                Err(CodingSessionError::UnsupportedCapability {
                    capability: "fork requires a persistent Rust-native session".into(),
                })
            }
        }
    }

    pub fn subscribe(&self) -> CodingAgentEventReceiver {
        self.event_service.subscribe()
    }

    pub(crate) fn prompt_control_handle(
        &mut self,
    ) -> Result<PromptControlHandle, CodingSessionError> {
        self.operation_control.prompt_control_handle()
    }

    pub fn capabilities(&self) -> CodingAgentCapabilities {
        let plugin_capabilities = self.plugin_service.capabilities();
        let persistent = matches!(self.persistence, SessionPersistence::Persistent(_));
        self.capability_service.capabilities(
            self.operation_control.active(),
            &plugin_capabilities,
            persistent,
        )
    }

    pub fn view(&self) -> CodingAgentSessionView {
        let _ = (
            &self.runtime_service,
            &self.flow_service,
            &self.plugin_service,
        );
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => session_service.view(),
            SessionPersistence::NonPersistent(state) => CodingAgentSessionView {
                session_id: state.runtime_id.clone(),
                default_agent_profile_id: state.default_agent_profile_id.clone(),
            },
        }
    }

    pub fn agent_profiles(&self) -> Vec<AgentProfile> {
        self.profile_registry.agents().cloned().collect()
    }

    pub fn team_profiles(&self) -> Vec<TeamProfile> {
        self.profile_registry.teams().cloned().collect()
    }

    pub fn profile_diagnostics(&self) -> Vec<ProfileDiagnostic> {
        self.profile_registry.diagnostics().to_vec()
    }

    pub fn set_default_agent_profile_id(
        &mut self,
        profile_id: impl Into<ProfileId>,
    ) -> Result<(), CodingSessionError> {
        let profile_id = profile_id.into();
        match &mut self.persistence {
            SessionPersistence::Persistent(session_service) => {
                session_service.set_default_agent_profile_id(profile_id.clone())?;
            }
            SessionPersistence::NonPersistent(state) => {
                state.default_agent_profile_id = profile_id.clone();
            }
        }
        self.event_service
            .emit_default_agent_profile_changed(profile_id);
        Ok(())
    }

    pub fn pending_delegation_confirmations(&self) -> Vec<PendingDelegationConfirmation> {
        let now = SystemClock.now_rfc3339();
        self.delegation_confirmation_service
            .active_views(&self.pending_delegation_confirmations, &now)
    }

    pub async fn approve_delegation_confirmation(
        &mut self,
        operation_id: impl AsRef<str>,
        tool_call_id: impl AsRef<str>,
    ) -> Result<(), CodingSessionError> {
        let operation_id = operation_id.as_ref();
        let tool_call_id = tool_call_id.as_ref();
        let now = SystemClock.now_rfc3339();
        let pending = self.delegation_confirmation_service.active_pending(
            &self.pending_delegation_confirmations,
            operation_id,
            tool_call_id,
            &now,
        )?;
        let operation_kind = match pending.request.target_kind {
            ProfileKind::Agent => OperationKind::AgentInvocation,
            ProfileKind::Team => OperationKind::AgentTeam,
        };
        let _operation = self.operation_control.begin(operation_kind)?;
        let mut ids = SystemIdGenerator;
        let pending = self.delegation_confirmation_service.approve_pending(
            &mut self.persistence,
            &mut self.pending_delegation_confirmations,
            &self.event_service,
            operation_id,
            tool_call_id,
            &now,
            ids.next_operation_id(),
        )?;
        let outcome = match pending.request.target_kind {
            ProfileKind::Agent => {
                self.delegation_execution_service
                    .execute_agent(
                        &self.flow_service,
                        self.profile_registry.clone(),
                        self.plugin_service.clone(),
                        self.event_service.clone(),
                        &pending.request,
                        pending.prompt_options,
                        pending.child_delegation_depth,
                        pending.delegation_lineage,
                    )
                    .await
            }
            ProfileKind::Team => {
                self.delegation_execution_service
                    .execute_team(
                        &self.flow_service,
                        self.profile_registry.clone(),
                        self.plugin_service.clone(),
                        self.event_service.clone(),
                        &pending.request,
                        pending.prompt_options,
                        pending.child_delegation_depth,
                        pending.delegation_lineage,
                    )
                    .await
            }
        };
        self.delegation_confirmation_service.adopt_pending(
            &mut self.persistence,
            &mut self.pending_delegation_confirmations,
            &self.event_service,
            outcome.pending_confirmations,
        )?;
        outcome.execution.map(|_| ())
    }

    pub fn reject_delegation_confirmation(
        &mut self,
        operation_id: impl AsRef<str>,
        tool_call_id: impl AsRef<str>,
        reason: impl Into<String>,
    ) -> Result<(), CodingSessionError> {
        let operation_id = operation_id.as_ref();
        let tool_call_id = tool_call_id.as_ref();
        let now = SystemClock.now_rfc3339();
        self.delegation_confirmation_service.reject_pending(
            &mut self.persistence,
            &mut self.pending_delegation_confirmations,
            &self.event_service,
            operation_id,
            tool_call_id,
            &now,
            reason.into(),
        )
    }

    pub async fn prompt(
        &mut self,
        options: PromptTurnOptions,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        match self.run_operation(Operation::Prompt(options)).await? {
            OperationOutcome::Prompt(outcome) => Ok(outcome),
            OperationOutcome::ManualCompaction(_) => {
                unreachable!("prompt operation returned manual compaction outcome")
            }
            OperationOutcome::PluginLoad(_) => {
                unreachable!("prompt operation returned plugin load outcome")
            }
            OperationOutcome::PluginCommand(_) => {
                unreachable!("prompt operation returned plugin command outcome")
            }
            OperationOutcome::BranchSummary(_) => {
                unreachable!("prompt operation returned branch summary outcome")
            }
            OperationOutcome::SelfHealingEdit(_) => {
                unreachable!("prompt operation returned self-healing edit outcome")
            }
            OperationOutcome::AgentInvocation(_) => {
                unreachable!("prompt operation returned agent invocation outcome")
            }
            OperationOutcome::AgentTeam(_) => {
                unreachable!("prompt operation returned agent team outcome")
            }
            OperationOutcome::Export(_) => unreachable!("prompt operation returned export outcome"),
        }
    }

    pub async fn compact(
        &mut self,
        options: PromptTurnOptions,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        match self
            .run_operation(Operation::ManualCompaction(options))
            .await?
        {
            OperationOutcome::ManualCompaction(outcome) => Ok(outcome),
            OperationOutcome::Prompt(_) => {
                unreachable!("manual compaction operation returned prompt outcome")
            }
            OperationOutcome::PluginLoad(_) => {
                unreachable!("manual compaction operation returned plugin load outcome")
            }
            OperationOutcome::PluginCommand(_) => {
                unreachable!("manual compaction operation returned plugin command outcome")
            }
            OperationOutcome::BranchSummary(_) => {
                unreachable!("manual compaction operation returned branch summary outcome")
            }
            OperationOutcome::SelfHealingEdit(_) => {
                unreachable!("manual compaction operation returned self-healing edit outcome")
            }
            OperationOutcome::AgentInvocation(_) => {
                unreachable!("manual compaction operation returned agent invocation outcome")
            }
            OperationOutcome::AgentTeam(_) => {
                unreachable!("manual compaction operation returned agent team outcome")
            }
            OperationOutcome::Export(_) => {
                unreachable!("manual compaction operation returned export outcome")
            }
        }
    }

    pub async fn self_healing_edit(
        &mut self,
        path: impl Into<String>,
        replacements: Vec<SelfHealingEditReplacement>,
    ) -> Result<SelfHealingEditOutcome, CodingSessionError> {
        self.self_healing_edit_with_options(SelfHealingEditRequest::new(path, replacements))
            .await
    }

    pub async fn self_healing_edit_with_options(
        &mut self,
        request: SelfHealingEditRequest,
    ) -> Result<SelfHealingEditOutcome, CodingSessionError> {
        match self
            .run_operation(Operation::SelfHealingEdit(request))
            .await?
        {
            OperationOutcome::SelfHealingEdit(outcome) => Ok(outcome),
            OperationOutcome::Prompt(_) => {
                unreachable!("self-healing edit operation returned prompt outcome")
            }
            OperationOutcome::ManualCompaction(_) => {
                unreachable!("self-healing edit operation returned manual compaction outcome")
            }
            OperationOutcome::PluginLoad(_) => {
                unreachable!("self-healing edit operation returned plugin load outcome")
            }
            OperationOutcome::PluginCommand(_) => {
                unreachable!("self-healing edit operation returned plugin command outcome")
            }
            OperationOutcome::BranchSummary(_) => {
                unreachable!("self-healing edit operation returned branch summary outcome")
            }
            OperationOutcome::AgentInvocation(_) => {
                unreachable!("self-healing edit operation returned agent invocation outcome")
            }
            OperationOutcome::AgentTeam(_) => {
                unreachable!("self-healing edit operation returned agent team outcome")
            }
            OperationOutcome::Export(_) => {
                unreachable!("self-healing edit operation returned export outcome")
            }
        }
    }

    pub async fn invoke_agent(
        &mut self,
        options: AgentInvocationOptions,
    ) -> Result<AgentInvocationOutcome, CodingSessionError> {
        match self
            .run_operation(Operation::AgentInvocation(options))
            .await?
        {
            OperationOutcome::AgentInvocation(outcome) => Ok(outcome),
            OperationOutcome::Prompt(_) => {
                unreachable!("agent invocation operation returned prompt outcome")
            }
            OperationOutcome::ManualCompaction(_) => {
                unreachable!("agent invocation operation returned manual compaction outcome")
            }
            OperationOutcome::PluginLoad(_) => {
                unreachable!("agent invocation operation returned plugin load outcome")
            }
            OperationOutcome::PluginCommand(_) => {
                unreachable!("agent invocation operation returned plugin command outcome")
            }
            OperationOutcome::BranchSummary(_) => {
                unreachable!("agent invocation operation returned branch summary outcome")
            }
            OperationOutcome::SelfHealingEdit(_) => {
                unreachable!("agent invocation operation returned self-healing edit outcome")
            }
            OperationOutcome::AgentTeam(_) => {
                unreachable!("agent invocation operation returned agent team outcome")
            }
            OperationOutcome::Export(_) => {
                unreachable!("agent invocation operation returned export outcome")
            }
        }
    }

    pub async fn invoke_team(
        &mut self,
        options: AgentTeamOptions,
    ) -> Result<AgentTeamOutcome, CodingSessionError> {
        match self.run_operation(Operation::AgentTeam(options)).await? {
            OperationOutcome::AgentTeam(outcome) => Ok(outcome),
            OperationOutcome::Prompt(_) => {
                unreachable!("agent team operation returned prompt outcome")
            }
            OperationOutcome::ManualCompaction(_) => {
                unreachable!("agent team operation returned manual compaction outcome")
            }
            OperationOutcome::PluginLoad(_) => {
                unreachable!("agent team operation returned plugin load outcome")
            }
            OperationOutcome::PluginCommand(_) => {
                unreachable!("agent team operation returned plugin command outcome")
            }
            OperationOutcome::BranchSummary(_) => {
                unreachable!("agent team operation returned branch summary outcome")
            }
            OperationOutcome::SelfHealingEdit(_) => {
                unreachable!("agent team operation returned self-healing edit outcome")
            }
            OperationOutcome::AgentInvocation(_) => {
                unreachable!("agent team operation returned agent invocation outcome")
            }
            OperationOutcome::Export(_) => {
                unreachable!("agent team operation returned export outcome")
            }
        }
    }

    pub(crate) async fn reload_plugins(&mut self) -> Result<PluginLoadOutcome, CodingSessionError> {
        self.load_plugins(self.default_plugin_load_options.clone())
            .await
    }

    pub(crate) fn plugin_commands(&self) -> Vec<CommandDefinition> {
        self.plugin_service.collect_commands()
    }

    pub(crate) fn plugin_ui_actions(&self) -> Vec<UiActionDefinition> {
        self.plugin_service.collect_ui_actions()
    }

    pub(crate) fn plugin_ui_dialogs(&self) -> Vec<UiDialogDefinition> {
        self.plugin_service.collect_ui_dialogs()
    }

    pub(crate) fn plugin_keybindings(&self) -> Vec<KeybindDefinition> {
        self.plugin_service.collect_keybindings()
    }

    pub(crate) fn run_plugin_command(
        &mut self,
        command_id: &str,
        args: serde_json::Value,
    ) -> Result<String, CodingSessionError> {
        match self.run_sync_operation(Operation::PluginCommand {
            command_id: command_id.to_owned(),
            args,
        })? {
            OperationOutcome::PluginCommand(output) => Ok(output),
            OperationOutcome::Prompt(_) => {
                unreachable!("plugin command operation returned prompt outcome")
            }
            OperationOutcome::ManualCompaction(_) => {
                unreachable!("plugin command operation returned manual compaction outcome")
            }
            OperationOutcome::PluginLoad(_) => {
                unreachable!("plugin command operation returned plugin load outcome")
            }
            OperationOutcome::BranchSummary(_) => {
                unreachable!("plugin command operation returned branch summary outcome")
            }
            OperationOutcome::SelfHealingEdit(_) => {
                unreachable!("plugin command operation returned self-healing edit outcome")
            }
            OperationOutcome::AgentInvocation(_) => {
                unreachable!("plugin command operation returned agent invocation outcome")
            }
            OperationOutcome::AgentTeam(_) => {
                unreachable!("plugin command operation returned agent team outcome")
            }
            OperationOutcome::Export(_) => {
                unreachable!("plugin command operation returned export outcome")
            }
        }
    }

    pub(crate) async fn load_plugins(
        &mut self,
        options: PluginLoadOptions,
    ) -> Result<PluginLoadOutcome, CodingSessionError> {
        match self.run_operation(Operation::PluginLoad(options)).await? {
            OperationOutcome::PluginLoad(outcome) => Ok(outcome),
            OperationOutcome::PluginCommand(_) => {
                unreachable!("plugin load operation returned plugin command outcome")
            }
            OperationOutcome::Prompt(_) => {
                unreachable!("plugin load operation returned prompt outcome")
            }
            OperationOutcome::ManualCompaction(_) => {
                unreachable!("plugin load operation returned manual compaction outcome")
            }
            OperationOutcome::BranchSummary(_) => {
                unreachable!("plugin load operation returned branch summary outcome")
            }
            OperationOutcome::SelfHealingEdit(_) => {
                unreachable!("plugin load operation returned self-healing edit outcome")
            }
            OperationOutcome::AgentInvocation(_) => {
                unreachable!("plugin load operation returned agent invocation outcome")
            }
            OperationOutcome::AgentTeam(_) => {
                unreachable!("plugin load operation returned agent team outcome")
            }
            OperationOutcome::Export(_) => {
                unreachable!("plugin load operation returned export outcome")
            }
        }
    }

    pub async fn summarize_branch(
        &mut self,
        options: PromptTurnOptions,
        source_leaf_id: impl Into<String>,
        target_leaf_id: impl Into<String>,
        custom_instructions: Option<String>,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        match self
            .run_operation(Operation::BranchSummary {
                options,
                source_leaf_id: source_leaf_id.into(),
                target_leaf_id: target_leaf_id.into(),
                custom_instructions,
            })
            .await?
        {
            OperationOutcome::BranchSummary(outcome) => Ok(outcome),
            OperationOutcome::Prompt(_) => {
                unreachable!("branch summary operation returned prompt outcome")
            }
            OperationOutcome::ManualCompaction(_) => {
                unreachable!("branch summary operation returned manual compaction outcome")
            }
            OperationOutcome::PluginLoad(_) => {
                unreachable!("branch summary operation returned plugin load outcome")
            }
            OperationOutcome::PluginCommand(_) => {
                unreachable!("branch summary operation returned plugin command outcome")
            }
            OperationOutcome::SelfHealingEdit(_) => {
                unreachable!("branch summary operation returned self-healing edit outcome")
            }
            OperationOutcome::AgentInvocation(_) => {
                unreachable!("branch summary operation returned agent invocation outcome")
            }
            OperationOutcome::AgentTeam(_) => {
                unreachable!("branch summary operation returned agent team outcome")
            }
            OperationOutcome::Export(_) => {
                unreachable!("branch summary operation returned export outcome")
            }
        }
    }

    pub(crate) async fn summarize_branch_for_navigation(
        &mut self,
        options: PromptTurnOptions,
        source_leaf_id: impl Into<String>,
        target_leaf_id: impl Into<String>,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        self.operation_control.ensure_idle()?;
        let source_leaf_id = source_leaf_id.into();
        let target_leaf_id = target_leaf_id.into();
        if let Some(outcome) = self.branch_summary_service.reused_outcome(
            &self.persistence,
            &options,
            source_leaf_id.as_str(),
            target_leaf_id.as_str(),
        )? {
            return Ok(outcome);
        }

        match self
            .run_operation(Operation::BranchSummary {
                options,
                source_leaf_id,
                target_leaf_id,
                custom_instructions: None,
            })
            .await?
        {
            OperationOutcome::BranchSummary(outcome) => Ok(outcome),
            OperationOutcome::Prompt(_) => {
                unreachable!("branch summary operation returned prompt outcome")
            }
            OperationOutcome::ManualCompaction(_) => {
                unreachable!("branch summary operation returned manual compaction outcome")
            }
            OperationOutcome::PluginLoad(_) => {
                unreachable!("branch summary operation returned plugin load outcome")
            }
            OperationOutcome::PluginCommand(_) => {
                unreachable!("branch summary operation returned plugin command outcome")
            }
            OperationOutcome::SelfHealingEdit(_) => {
                unreachable!("branch summary operation returned self-healing edit outcome")
            }
            OperationOutcome::AgentInvocation(_) => {
                unreachable!("branch summary operation returned agent invocation outcome")
            }
            OperationOutcome::AgentTeam(_) => {
                unreachable!("branch summary operation returned agent team outcome")
            }
            OperationOutcome::Export(_) => {
                unreachable!("branch summary operation returned export outcome")
            }
        }
    }

    fn from_services(
        session_service: SessionService,
        default_plugin_load_options: PluginLoadOptions,
        profile_registry: ProfileRegistry,
    ) -> Result<Self, CodingSessionError> {
        let replay = session_service.replay()?;
        let cwd = replay
            .cwd
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(default_cwd);
        let pending_delegation_confirmations = PendingDelegationConfirmationQueue::from_pending(
            replay
                .pending_delegation_confirmations
                .into_iter()
                .map(|pending| pending_state_from_replay(pending, &cwd))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let event_service = EventService::new();
        event_service.emit_session_opened(session_service.session_id().to_owned());

        Ok(Self {
            persistence: SessionPersistence::Persistent(session_service),
            runtime_service: RuntimeService::new(),
            flow_service: FlowService::new(),
            event_service,
            capability_service: CapabilityService::new(),
            plugin_service: PluginService::new(),
            plugin_load_service: PluginLoadService::new(),
            profile_registry,
            default_plugin_load_options,
            operation_control: OperationControl::new(),
            pending_delegation_confirmations,
            branch_summary_service: BranchSummaryService::new(),
            delegation_confirmation_service: DelegationConfirmationService::new(),
            delegation_execution_service: DelegationExecutionService::new(),
            manual_compaction_service: ManualCompactionService::new(),
            self_healing_edit_service: SelfHealingEditService::new(),
        })
    }

    fn from_transient(
        state: TransientSessionState,
        default_plugin_load_options: PluginLoadOptions,
        profile_registry: ProfileRegistry,
    ) -> Result<Self, CodingSessionError> {
        Ok(Self {
            persistence: SessionPersistence::NonPersistent(state),
            runtime_service: RuntimeService::new(),
            flow_service: FlowService::new(),
            event_service: EventService::new(),
            capability_service: CapabilityService::new(),
            plugin_service: PluginService::new(),
            plugin_load_service: PluginLoadService::new(),
            profile_registry,
            default_plugin_load_options,
            operation_control: OperationControl::new(),
            pending_delegation_confirmations: PendingDelegationConfirmationQueue::default(),
            branch_summary_service: BranchSummaryService::new(),
            delegation_confirmation_service: DelegationConfirmationService::new(),
            delegation_execution_service: DelegationExecutionService::new(),
            manual_compaction_service: ManualCompactionService::new(),
            self_healing_edit_service: SelfHealingEditService::new(),
        })
    }

    fn run_sync_operation(
        &self,
        operation: Operation,
    ) -> Result<OperationOutcome, CodingSessionError> {
        let kind = operation.kind();
        let _operation_guard = self.operation_control.begin(kind)?;

        match operation {
            Operation::Export(options) => self
                .export_current_inner(options)
                .map(OperationOutcome::Export),
            Operation::PluginCommand { command_id, args } => self
                .plugin_service
                .run_command(&command_id, args)
                .map(OperationOutcome::PluginCommand),
            Operation::Prompt(_)
            | Operation::ManualCompaction(_)
            | Operation::PluginLoad(_)
            | Operation::BranchSummary { .. }
            | Operation::SelfHealingEdit(_)
            | Operation::AgentInvocation(_)
            | Operation::AgentTeam(_) => Err(CodingSessionError::UnsupportedCapability {
                capability: format!("{} operation requires async dispatcher", kind.as_str()),
            }),
        }
    }

    fn export_current_inner(
        &self,
        options: ExportOptions,
    ) -> Result<export_flow::ExportOutcome, CodingSessionError> {
        let SessionPersistence::Persistent(session_service) = &self.persistence else {
            return Err(CodingSessionError::UnsupportedCapability {
                capability: "export requires a persistent Rust-native session".into(),
            });
        };
        let mut context = session_service.export_context(options)?;
        self.flow_service.run_export(&mut context)
    }

    async fn run_operation(
        &mut self,
        operation: Operation,
    ) -> Result<OperationOutcome, CodingSessionError> {
        let kind = operation.kind();
        let _operation_guard = self.operation_control.begin(kind)?;

        match operation {
            Operation::Prompt(options) => {
                let result = self.prompt_inner(options).await;
                self.operation_control.clear_prompt_control_receiver();
                result.map(OperationOutcome::Prompt)
            }
            Operation::ManualCompaction(options) => {
                let options = ManualCompactionOptions::from_prompt_turn_options(&options)?;
                let SessionPersistence::Persistent(session_service) = &mut self.persistence else {
                    return Err(CodingSessionError::UnsupportedCapability {
                        capability: "manual compaction without persistent session".into(),
                    });
                };
                self.manual_compaction_service
                    .run_persistent(
                        session_service,
                        &self.flow_service,
                        &self.event_service,
                        options,
                    )
                    .await
                    .map(OperationOutcome::ManualCompaction)
            }
            Operation::PluginLoad(options) => self
                .load_plugins_inner(options)
                .await
                .map(OperationOutcome::PluginLoad),
            Operation::BranchSummary {
                options,
                source_leaf_id,
                target_leaf_id,
                custom_instructions,
            } => {
                let SessionPersistence::Persistent(session_service) = &mut self.persistence else {
                    return Err(CodingSessionError::UnsupportedCapability {
                        capability: "branch summary without persistent session".into(),
                    });
                };
                self.branch_summary_service
                    .run_persistent(
                        session_service,
                        &self.flow_service,
                        &self.event_service,
                        options,
                        source_leaf_id,
                        target_leaf_id,
                        custom_instructions,
                    )
                    .await
                    .map(OperationOutcome::BranchSummary)
            }
            Operation::SelfHealingEdit(request) => {
                let (path, replacements, check_command, repair_attempts, model_repair) =
                    request.into_parts();
                if !repair_attempts.is_empty() && model_repair.is_some() {
                    return Err(CodingSessionError::Input {
                        message:
                            "configure either planned repair attempts or model repair, not both"
                                .into(),
                    });
                }
                let model_repair_policy = self.self_healing_model_repair_policy(model_repair)?;
                let SessionPersistence::Persistent(session_service) = &mut self.persistence else {
                    return Err(CodingSessionError::UnsupportedCapability {
                        capability: "self-healing edit requires a persistent Rust-native session"
                            .into(),
                    });
                };
                let outcome = self
                    .self_healing_edit_service
                    .run_persistent(
                        session_service,
                        &self.flow_service,
                        self.event_service.clone(),
                        path,
                        replacements,
                        check_command,
                        repair_attempts,
                        model_repair_policy,
                    )
                    .await?;
                self.event_service
                    .emit_session_write_events(&outcome.finalized);
                outcome.result.map(OperationOutcome::SelfHealingEdit)
            }
            Operation::AgentInvocation(options) => {
                let result = self.invoke_agent_inner(options).await;
                self.operation_control.clear_prompt_control_receiver();
                result.map(OperationOutcome::AgentInvocation)
            }
            Operation::AgentTeam(options) => self
                .invoke_team_inner(options)
                .await
                .map(OperationOutcome::AgentTeam),
            Operation::Export(_) => Err(CodingSessionError::UnsupportedCapability {
                capability: "export operation requires sync dispatcher".into(),
            }),
            Operation::PluginCommand { .. } => Err(CodingSessionError::UnsupportedCapability {
                capability: "plugin command operation requires sync dispatcher".into(),
            }),
        }
    }

    #[allow(dead_code)]
    async fn load_plugins_inner(
        &mut self,
        options: PluginLoadOptions,
    ) -> Result<PluginLoadOutcome, CodingSessionError> {
        let execution = self
            .plugin_load_service
            .load(
                &mut self.persistence,
                &self.flow_service,
                &self.event_service,
                options,
            )
            .await?;
        if let Some(plugin_service) = execution.loaded_plugin_service {
            self.plugin_service = plugin_service;
        }
        Ok(execution.outcome)
    }

    async fn prompt_inner(
        &mut self,
        options: PromptTurnOptions,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        if options.runtime().is_none() {
            return Err(CodingSessionError::Config {
                message: "prompt turn options do not include a runtime snapshot".into(),
            });
        }
        let options = self.apply_default_agent_profile(options)?;
        let mut context = self.prepare_prompt_context(options)?;
        let operation_id = context.operation_id().to_owned();
        let turn_id = context.turn_id().to_owned();

        self.event_service
            .emit_prompt_started(operation_id, turn_id);
        let mut outcome = match self.flow_service.run_prompt_turn(&mut context).await {
            Ok(outcome) => outcome,
            Err(error) => match context.abort_reason() {
                Some(reason) => {
                    context.finish_abort(reason.to_owned(), context.session_id().map(str::to_owned))
                }
                None => context.finish_failure(error),
            },
        };
        if outcome.is_success() {
            match context.authorize_delegation_requests(0) {
                Ok(decisions) => {
                    let decisions = decisions.to_vec();
                    let prompt_options = context.options().clone();
                    if let Err(error) = self
                        .execute_authorized_delegations(&mut context, &decisions, prompt_options)
                        .await
                    {
                        self.event_service.emit_diagnostic(
                            Some(context.operation_id().to_owned()),
                            format!("delegation execution failed: {error}"),
                        );
                    }
                }
                Err(error) => {
                    outcome = context.finish_failure(error);
                }
            }
        }
        let finalized = match self.finalize_prompt_transaction(&mut context, &outcome) {
            Ok(finalized) => finalized,
            Err(error) => {
                outcome = context.finish_failure(error.clone());
                SessionService::skip_prompt_transaction(
                    context.operation_id().to_owned(),
                    format!("session write finalization failed: {error}"),
                )
            }
        };
        apply_finalized_session_write(&mut outcome, &finalized);

        if !context.live_events_enabled() {
            self.event_service
                .emit_events_before_prompt_outcome(context.coding_events());
        }
        self.event_service.emit_session_write_events(&finalized);
        self.event_service.emit_prompt_outcome(&outcome);
        Ok(outcome)
    }

    async fn invoke_agent_inner(
        &mut self,
        options: AgentInvocationOptions,
    ) -> Result<AgentInvocationOutcome, CodingSessionError> {
        let prompt_control_receiver = self.operation_control.take_prompt_control_receiver();
        let mut context = AgentInvocationContext::new(
            options,
            self.profile_registry.clone(),
            self.plugin_service.clone(),
            self.event_service.clone(),
        );
        if let Some(receiver) = prompt_control_receiver {
            context.set_prompt_control_receiver(receiver);
        }
        self.flow_service.run_agent_invocation(&mut context).await
    }

    async fn invoke_team_inner(
        &mut self,
        options: AgentTeamOptions,
    ) -> Result<AgentTeamOutcome, CodingSessionError> {
        let mut context = AgentTeamContext::new(
            options,
            self.profile_registry.clone(),
            self.plugin_service.clone(),
            self.event_service.clone(),
        );
        self.flow_service.run_agent_team(&mut context).await
    }

    async fn execute_authorized_delegations(
        &mut self,
        context: &mut PromptTurnContext,
        decisions: &[DelegationAuthorizationDecision],
        prompt_options: PromptTurnOptions,
    ) -> Result<(), CodingSessionError> {
        for decision in decisions {
            match decision {
                DelegationAuthorizationDecision::Approved {
                    request,
                    child_delegation_depth,
                } => {
                    self.event_service.emit_delegation_approved(request);
                    let outcome = match request.target_kind {
                        ProfileKind::Agent => {
                            self.delegation_execution_service
                                .execute_agent(
                                    &self.flow_service,
                                    self.profile_registry.clone(),
                                    self.plugin_service.clone(),
                                    self.event_service.clone(),
                                    request,
                                    prompt_options.clone(),
                                    *child_delegation_depth,
                                    delegation_lineage_for_request(&[], request),
                                )
                                .await
                        }
                        ProfileKind::Team => {
                            self.delegation_execution_service
                                .execute_team(
                                    &self.flow_service,
                                    self.profile_registry.clone(),
                                    self.plugin_service.clone(),
                                    self.event_service.clone(),
                                    request,
                                    prompt_options.clone(),
                                    *child_delegation_depth,
                                    delegation_lineage_for_request(&[], request),
                                )
                                .await
                        }
                    };
                    self.delegation_confirmation_service.adopt_pending(
                        &mut self.persistence,
                        &mut self.pending_delegation_confirmations,
                        &self.event_service,
                        outcome.pending_confirmations,
                    )?;
                    match outcome.execution {
                        Ok(execution) => {
                            context.record_delegation_folded_update(
                                request,
                                PersistedDelegationStatus::Completed,
                                Some(execution.child_operation_id),
                                Some(execution.final_text),
                            )?;
                        }
                        Err(error) => {
                            context.record_delegation_folded_update(
                                request,
                                PersistedDelegationStatus::Failed,
                                None,
                                Some(error.to_string()),
                            )?;
                            return Err(error);
                        }
                    }
                }
                DelegationAuthorizationDecision::RequiresConfirmation {
                    request,
                    reason,
                    child_delegation_depth,
                } => {
                    context.record_delegation_folded_update(
                        request,
                        PersistedDelegationStatus::ConfirmationRequired,
                        None,
                        Some(reason.clone()),
                    )?;
                    let pending = PendingDelegationConfirmationState {
                        request: request.clone(),
                        prompt_options: prompt_options.clone(),
                        reason: reason.clone(),
                        requested_at: SystemClock.now_rfc3339(),
                        child_delegation_depth: *child_delegation_depth,
                        delegation_lineage: delegation_lineage_for_request(&[], request),
                    };
                    self.delegation_confirmation_service.queue_pending(
                        &mut self.persistence,
                        &mut self.pending_delegation_confirmations,
                        &self.event_service,
                        pending,
                        true,
                    )?;
                }
                DelegationAuthorizationDecision::Rejected { request, reason } => {
                    self.event_service.emit_delegation_rejected(request, reason);
                    context.record_delegation_folded_update(
                        request,
                        PersistedDelegationStatus::Rejected,
                        None,
                        Some(reason.clone()),
                    )?;
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::type_complexity)]
    fn self_healing_model_repair_policy(
        &self,
        model_repair: Option<SelfHealingEditModelRepairOptions>,
    ) -> Result<Option<(Arc<dyn SelfHealingEditRepairStrategy>, usize)>, CodingSessionError> {
        let Some(model_repair) = model_repair else {
            return Ok(None);
        };
        let (prompt_options, max_attempts) = model_repair.into_parts();
        let prompt_options = self.apply_default_agent_profile(prompt_options)?;
        let runtime =
            prompt_options
                .runtime()
                .cloned()
                .ok_or_else(|| CodingSessionError::Config {
                    message:
                        "self-healing edit model repair options do not include a runtime snapshot"
                            .into(),
                })?;
        Ok(Some((
            Arc::new(ModelSelfHealingEditRepairStrategy::new(runtime)),
            max_attempts,
        )))
    }

    fn apply_default_agent_profile(
        &self,
        mut options: PromptTurnOptions,
    ) -> Result<PromptTurnOptions, CodingSessionError> {
        let profile_id = self.default_agent_profile_id();
        let mut diagnostics = Vec::new();
        let profile = match self.profile_registry.agent(profile_id.as_str()) {
            Some(profile) => profile,
            None => {
                diagnostics.push(CodingDiagnostic::warning(format!(
                    "default agent profile {} could not be resolved; using built-in default profile",
                    profile_id
                )));
                self.profile_registry.agent("default").ok_or_else(|| {
                    CodingSessionError::Config {
                        message: "built-in default agent profile is not available".into(),
                    }
                })?
            }
        };
        options.apply_agent_profile(profile, diagnostics)?;
        Ok(options)
    }

    fn default_agent_profile_id(&self) -> ProfileId {
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => {
                session_service.default_agent_profile_id().clone()
            }
            SessionPersistence::NonPersistent(state) => state.default_agent_profile_id.clone(),
        }
    }

    fn prepare_prompt_context(
        &mut self,
        options: PromptTurnOptions,
    ) -> Result<PromptTurnContext, CodingSessionError> {
        let event_service = self.event_service.clone();
        let prompt_control_receiver = self.operation_control.take_prompt_control_receiver();
        match &mut self.persistence {
            SessionPersistence::Persistent(session_service) => {
                let replay = session_service.replay()?;
                let transaction = session_service.begin_prompt_transaction();
                let operation_id = transaction.operation_id().to_owned();
                let turn_id = transaction.turn_id().to_owned();
                let mut context =
                    PromptTurnContext::new(PromptTurnIds::new(operation_id, turn_id), options);
                context.set_plugin_service(self.plugin_service.clone());
                context.set_session_id(session_service.session_id().to_owned());
                context.set_replay(replay);
                context.set_transaction(transaction);
                if let Some(receiver) = prompt_control_receiver {
                    context.set_prompt_control_receiver(receiver);
                }
                context.enable_live_events(event_service);
                Ok(context)
            }
            SessionPersistence::NonPersistent(state) => {
                let mut ids = SystemIdGenerator;
                let mut context = PromptTurnContext::new(
                    PromptTurnIds::new(ids.next_operation_id(), ids.next_turn_id()),
                    options,
                );
                context.set_plugin_service(self.plugin_service.clone());
                context
                    .set_non_persistent_session(state.runtime_id.clone(), state.transcript.clone());
                if let Some(receiver) = prompt_control_receiver {
                    context.set_prompt_control_receiver(receiver);
                }
                context.enable_live_events(event_service);
                Ok(context)
            }
        }
    }

    fn finalize_prompt_transaction(
        &mut self,
        context: &mut PromptTurnContext,
        outcome: &PromptTurnOutcome,
    ) -> Result<FinalizedSessionWrite, CodingSessionError> {
        let operation_id = context.operation_id().to_owned();
        let transaction = context.take_transaction();
        match &mut self.persistence {
            SessionPersistence::Persistent(session_service) => {
                session_service.finalize_prompt_transaction(transaction, operation_id, outcome)
            }
            SessionPersistence::NonPersistent(state) => {
                Ok(state.finalize_prompt_transaction(context, outcome))
            }
        }
    }

    #[cfg(test)]
    fn persistent_session_service(&self) -> &SessionService {
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => session_service,
            SessionPersistence::NonPersistent(_) => {
                panic!("expected persistent coding agent session")
            }
        }
    }
}

fn apply_finalized_session_write(
    outcome: &mut PromptTurnOutcome,
    finalized: &FinalizedSessionWrite,
) {
    outcome.apply_success_session_write_metadata(
        finalized.session_id.clone(),
        finalized.leaf_id.clone(),
    );
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::{Arc, Mutex},
    };

    use async_stream::stream;
    use pi_agent_core::{AgentResources, AgentTool, AgentToolOutput};
    use pi_ai::providers::faux::{FauxProvider, FauxResponse, FauxToolCall};
    use pi_ai::registry::ApiProvider;
    use pi_ai::stream::EventStream;
    use pi_ai::types::{
        AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Message, Model, ModelCost,
        ModelInput, StopReason, StreamOptions,
    };
    use tokio::sync::oneshot;

    use super::delegation::delegation_runtime_seed_from_prompt_options;
    use super::plugin_load_flow::{PluginLoadCandidate, PluginLoadManifest, PluginLoadOptions};
    use super::*;
    use crate::coding_session::session_log::event::{
        PersistedContentBlock, SessionEventData, SessionEventEnvelope,
    };
    use crate::coding_session::session_log::replay::{MessageStatus, TranscriptItem};
    use crate::plugins::{
        PluginError, PluginId, PluginMetadata, PluginRegistry, PluginSource, ToolProvider,
        ToolRegistrationHost,
    };
    use crate::prompt_options::PromptRunOptions;
    use crate::runtime::{PromptInvocation, SessionRunOptions};

    fn model(api: &str) -> Model {
        Model {
            id: "test-model".into(),
            name: "Test Model".into(),
            api: api.into(),
            provider: "test".into(),
            base_url: String::new(),
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost::default(),
            context_window: 0,
            max_tokens: 0,
            headers: None,
            compat: None,
        }
    }

    fn prompt_options(api: &str, prompt: &str) -> PromptTurnOptions {
        prompt_options_with_tools(api, prompt, Vec::new())
    }

    fn prompt_options_with_tools(
        api: &str,
        prompt: &str,
        tools: Vec<AgentTool>,
    ) -> PromptTurnOptions {
        PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
            prompt: prompt.into(),
            model: model(api),
            api_key: None,
            auth_diagnostics: Vec::new(),
            system_prompt: Some("system".into()),
            max_turns: Some(2),
            tools,
            register_builtins: false,
            session: Some(SessionRunOptions::disabled(".".into())),
            session_target: None,
            session_name: None,
            thinking_level: None,
            tool_execution: None,
            resources: AgentResources::default(),
            settings: None,
            invocation: PromptInvocation::Text(prompt.into()),
        })
    }

    #[tokio::test]
    async fn stale_persistent_delegation_confirmation_is_not_restored_as_pending() {
        let temp = tempfile::tempdir().unwrap();
        let store = session_log::store::SessionLogStore::new(temp.path());
        let handle = store
            .create_session(session_log::store::CreateSessionOptions::new(
                "sess_stale_delegation_confirmation",
                "2026-01-01T00:00:00Z",
            ))
            .unwrap();
        let runtime_seed = delegation_runtime_seed_from_prompt_options(
            &prompt_options("stale-delegation-api", "plan feature"),
            1,
            &[],
        )
        .unwrap();
        store
            .append_events(
                &handle,
                &[
                    SessionEventEnvelope::new(
                        "sess_stale_delegation_confirmation",
                        "evt_1",
                        "2026-01-01T00:00:00Z",
                        SessionEventData::SessionCreated {
                            cwd: Some(".".to_string()),
                        },
                    ),
                    SessionEventEnvelope::new(
                        "sess_stale_delegation_confirmation",
                        "evt_2",
                        "2026-01-01T00:00:00Z",
                        SessionEventData::DelegationConfirmationRequested {
                            source_operation_id: "op_parent".to_string(),
                            turn_id: "turn_parent".to_string(),
                            tool_call_id: "tool_delegate_agent".to_string(),
                            requesting_profile_id: ProfileId::from("delegating-planner"),
                            target_kind: ProfileKind::Agent,
                            target_id: ProfileId::from("coder"),
                            task: "implement parser".to_string(),
                            reason: "delegation policy requires confirmation".to_string(),
                            runtime_seed,
                        },
                    )
                    .with_operation_id("op_parent")
                    .with_turn_id("turn_parent"),
                    SessionEventEnvelope::new(
                        "sess_stale_delegation_confirmation",
                        "evt_3",
                        "2026-01-01T00:00:01Z",
                        SessionEventData::OperationCommitted { new_leaf_id: None },
                    )
                    .with_operation_id("op_parent")
                    .with_turn_id("turn_parent"),
                ],
            )
            .unwrap();
        let replay = store.replay_session(&handle).unwrap();
        assert_eq!(replay.pending_delegation_confirmations.len(), 1);

        let mut session = CodingAgentSession::open(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_stale_delegation_confirmation")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();

        assert!(session.pending_delegation_confirmations().is_empty());
        let error = session
            .approve_delegation_confirmation("op_parent", "tool_delegate_agent")
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("pending delegation confirmation not found"),
            "{error}"
        );
    }

    #[test]
    fn delegation_runtime_seed_strips_model_headers() {
        let mut runtime_model = model("delegation-seed-api");
        runtime_model.headers = Some(serde_json::json!({
            "authorization": "Bearer secret",
            "x-model": "metadata",
        }));
        let options = PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
            prompt: "plan".into(),
            model: runtime_model,
            api_key: Some("secret-key".into()),
            auth_diagnostics: Vec::new(),
            system_prompt: Some("system".into()),
            max_turns: Some(2),
            tools: Vec::new(),
            register_builtins: false,
            session: Some(SessionRunOptions::disabled(".".into())),
            session_target: None,
            session_name: None,
            thinking_level: None,
            tool_execution: None,
            resources: AgentResources::default(),
            settings: None,
            invocation: PromptInvocation::Text("plan".into()),
        });

        let seed = delegation_runtime_seed_from_prompt_options(&options, 1, &[]).unwrap();

        assert_eq!(seed.model.id, "test-model");
        assert!(seed.model.headers.is_none());
    }

    fn compact_options(api: &str, custom_instructions: Option<&str>) -> PromptTurnOptions {
        PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
            prompt: String::new(),
            model: model(api),
            api_key: None,
            auth_diagnostics: Vec::new(),
            system_prompt: Some("system".into()),
            max_turns: Some(2),
            tools: Vec::new(),
            register_builtins: false,
            session: Some(SessionRunOptions::disabled(".".into())),
            session_target: None,
            session_name: None,
            thinking_level: None,
            tool_execution: None,
            resources: AgentResources::default(),
            settings: None,
            invocation: PromptInvocation::Compact {
                custom_instructions: custom_instructions.map(str::to_owned),
            },
        })
    }

    fn echo_tool() -> AgentTool {
        AgentTool {
            name: "echo".into(),
            description: "echoes input".into(),
            parameters: serde_json::json!({"type": "object"}),
            execution_mode: None,
            execute: Arc::new(|args, _on_update| {
                let text = args
                    .get("text")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_owned();
                Box::pin(async move {
                    Ok(AgentToolOutput::new(vec![ContentBlock::Text {
                        text: format!("echo: {text}"),
                        text_signature: None,
                    }]))
                })
            }),
        }
    }

    struct SessionPluginToolProvider;

    impl ToolProvider for SessionPluginToolProvider {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new(
                PluginId::new("session-plugin-tool"),
                "Session Plugin Tool",
                "1.0.0",
                PluginSource::FirstParty,
            )
        }

        fn tools(&self, _host: &ToolRegistrationHost) -> Result<Vec<AgentTool>, PluginError> {
            Ok(vec![AgentTool::new_text(
                "plugin_echo",
                "echoes plugin input",
                serde_json::json!({"type": "object"}),
                |_args| async { Ok("plugin echo".to_owned()) },
            )])
        }
    }

    struct RecordingProvider {
        contexts: Arc<Mutex<Vec<Context>>>,
        response: String,
    }

    impl RecordingProvider {
        fn new(contexts: Arc<Mutex<Vec<Context>>>, response: impl Into<String>) -> Self {
            Self {
                contexts,
                response: response.into(),
            }
        }
    }

    impl ApiProvider for RecordingProvider {
        fn stream(&self, model: &Model, ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
            self.contexts.lock().unwrap().push(ctx);
            let model_id = model.id.clone();
            let response = self.response.clone();
            Box::pin(stream! {
                let mut message = AssistantMessage::empty("recording", &model_id);
                message.provider = Some("recording".into());
                message.content.push(ContentBlock::Text {
                    text: response,
                    text_signature: None,
                });
                yield AssistantMessageEvent::Done {
                    reason: StopReason::Stop,
                    message,
                };
            })
        }
    }

    struct BlockingTwoTurnProvider {
        contexts: Arc<Mutex<Vec<Context>>>,
        first_started: Mutex<Option<oneshot::Sender<()>>>,
        release_first: Mutex<Option<oneshot::Receiver<()>>>,
    }

    impl BlockingTwoTurnProvider {
        fn new(
            contexts: Arc<Mutex<Vec<Context>>>,
            first_started: oneshot::Sender<()>,
            release_first: oneshot::Receiver<()>,
        ) -> Self {
            Self {
                contexts,
                first_started: Mutex::new(Some(first_started)),
                release_first: Mutex::new(Some(release_first)),
            }
        }
    }

    impl ApiProvider for BlockingTwoTurnProvider {
        fn stream(&self, model: &Model, ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
            let call_index = {
                let mut contexts = self.contexts.lock().unwrap();
                contexts.push(ctx);
                contexts.len()
            };
            let first_release = if call_index == 1 {
                if let Some(started) = self.first_started.lock().unwrap().take() {
                    let _ = started.send(());
                }
                self.release_first.lock().unwrap().take()
            } else {
                None
            };
            let model_id = model.id.clone();
            Box::pin(stream! {
                if let Some(release) = first_release {
                    let _ = release.await;
                }
                let text = if call_index == 1 { "first" } else { "second" };
                let mut message = AssistantMessage::empty("blocking", &model_id);
                message.provider = Some("blocking".into());
                message.content.push(ContentBlock::Text {
                    text: text.into(),
                    text_signature: None,
                });
                yield AssistantMessageEvent::Done {
                    reason: StopReason::Stop,
                    message,
                };
            })
        }
    }

    struct AbortableProvider {
        started: Mutex<Option<oneshot::Sender<()>>>,
    }

    impl AbortableProvider {
        fn new(started: oneshot::Sender<()>) -> Self {
            Self {
                started: Mutex::new(Some(started)),
            }
        }
    }

    impl ApiProvider for AbortableProvider {
        fn stream(&self, model: &Model, _ctx: Context, opts: Option<StreamOptions>) -> EventStream {
            if let Some(started) = self.started.lock().unwrap().take() {
                let _ = started.send(());
            }
            let model_id = model.id.clone();
            let cancel = opts.and_then(|opts| opts.cancel);
            Box::pin(stream! {
                if let Some(cancel) = cancel {
                    cancel.cancelled().await;
                }
                let mut message = AssistantMessage::empty("abortable", &model_id);
                message.provider = Some("abortable".into());
                message.stop_reason = StopReason::Aborted;
                yield AssistantMessageEvent::Done {
                    reason: StopReason::Aborted,
                    message,
                };
            })
        }
    }

    #[tokio::test]
    async fn load_plugins_updates_session_runtime_and_emits_capability_events() {
        let api = "coding-session-plugin-load-owner";
        let contexts = Arc::new(Mutex::new(Vec::new()));
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(RecordingProvider::new(contexts.clone(), "plugin loaded")),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_plugin_load_owner")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let mut registry = PluginRegistry::new();
        registry.register_tool_provider(Arc::new(SessionPluginToolProvider));
        let options = PluginLoadOptions::new()
            .with_candidate(PluginLoadCandidate::new(
                PluginLoadManifest::new(
                    "session-plugin",
                    "Session Plugin",
                    "1.0.0",
                    PluginSource::FirstParty,
                ),
                registry,
            ))
            .with_candidate(PluginLoadCandidate::new(
                PluginLoadManifest::new("", "Invalid Plugin", "1.0.0", PluginSource::Project),
                PluginRegistry::new(),
            ));
        let mut events = session.subscribe();

        let outcome = session.load_plugins(options).await.unwrap();

        assert_eq!(outcome.loaded_plugin_ids, vec!["session-plugin"]);
        assert_eq!(outcome.diagnostics.len(), 1);
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert!(
            emitted_events.iter().any(|event| matches!(
                event,
                CodingAgentEvent::Diagnostic { message, .. }
                    if message.contains("plugin id must not be empty")
            )),
            "{emitted_events:#?}"
        );
        assert!(
            emitted_events
                .iter()
                .any(|event| matches!(event, CodingAgentEvent::CapabilityChanged))
        );

        session
            .prompt(prompt_options(api, "use plugin"))
            .await
            .unwrap();

        let contexts = contexts.lock().unwrap();
        let tools = contexts[0].tools.as_ref().unwrap();
        assert!(tools.iter().any(|tool| tool.name == "plugin_echo"));
    }

    #[tokio::test]
    async fn load_plugins_records_persistent_plugin_load_events() {
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_plugin_load_events")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let mut registry = PluginRegistry::new();
        registry.register_tool_provider(Arc::new(SessionPluginToolProvider));
        let options = PluginLoadOptions::new()
            .with_candidate(PluginLoadCandidate::new(
                PluginLoadManifest::new(
                    "session-plugin",
                    "Session Plugin",
                    "1.0.0",
                    PluginSource::FirstParty,
                ),
                registry,
            ))
            .with_candidate(PluginLoadCandidate::new(
                PluginLoadManifest::new("", "Invalid Plugin", "1.0.0", PluginSource::Project),
                PluginRegistry::new(),
            ));

        session.load_plugins(options).await.unwrap();

        let event_log = std::fs::read_to_string(
            temp.path()
                .join("sess_plugin_load_events")
                .join("events.jsonl"),
        )
        .unwrap();
        let events = event_log
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        let kinds = events
            .iter()
            .map(|event| event["kind"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(kinds.contains(&"plugin.load.completed"), "{event_log}");
        assert!(kinds.contains(&"operation.committed"), "{event_log}");
        let plugin_event = events
            .iter()
            .find(|event| event["kind"] == "plugin.load.completed")
            .unwrap();
        assert_eq!(
            plugin_event["data"]["loaded_plugin_ids"],
            serde_json::json!(["session-plugin"])
        );
        assert_eq!(plugin_event["data"]["diagnostics"][0]["plugin_id"], "");
        assert!(
            plugin_event["data"]["diagnostics"][0]["message"]
                .as_str()
                .unwrap()
                .contains("plugin id must not be empty")
        );
    }

    #[tokio::test]
    async fn reload_plugins_discovers_default_project_and_user_roots() {
        let env = crate::test_support::EnvGuard::new(&["PI_RUST_DIR"]);
        let temp = tempfile::tempdir().unwrap();
        let cwd = temp.path().join("project");
        let global = temp.path().join("global");
        let project_plugin = cwd.join(".pi-rust/plugins/project-lua");
        let user_plugin = global.join("plugins/user-lua");
        fs::create_dir_all(&project_plugin).unwrap();
        fs::create_dir_all(&user_plugin).unwrap();
        fs::write(
            project_plugin.join("plugin.toml"),
            r#"
id = "project-lua"
name = "Project Lua"
version = "0.1.0"
runtime = "lua"
"#,
        )
        .unwrap();
        fs::write(
            user_plugin.join("plugin.toml"),
            r#"
id = "user-lua"
name = "User Lua"
version = "0.1.0"
runtime = "lua"
"#,
        )
        .unwrap();
        env.set_pi_rust_dir(&global);
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_cwd(&cwd)
                .with_session_id("sess_plugin_reload_defaults")
                .with_session_log_root(temp.path().join("sessions")),
        )
        .await
        .unwrap();
        let mut events = session.subscribe();

        let outcome = session.reload_plugins().await.unwrap();

        assert!(outcome.loaded_plugin_ids.is_empty());
        assert_eq!(outcome.diagnostics.len(), 2);
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.plugin_id.as_deref() == Some("project-lua"))
        );
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.plugin_id.as_deref() == Some("user-lua"))
        );
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert_eq!(
            emitted_events
                .iter()
                .filter(|event| matches!(event, CodingAgentEvent::Diagnostic { .. }))
                .count(),
            2
        );
        assert!(
            emitted_events
                .iter()
                .any(|event| matches!(event, CodingAgentEvent::CapabilityChanged))
        );
    }

    #[tokio::test]
    async fn prompt_abort_control_returns_aborted_outcome_and_records_operation_abort() {
        let api = "coding-session-abort-control";
        let (started_tx, started_rx) = oneshot::channel();
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(AbortableProvider::new(started_tx)),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_prompt_abort_control")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let handle = session.prompt_control_handle().unwrap();

        let mut prompt = Box::pin(session.prompt(prompt_options(api, "hello")));
        tokio::select! {
            started = started_rx => started.unwrap(),
            result = &mut prompt => panic!("prompt finished before provider blocked: {result:?}"),
        }
        handle.abort("user cancelled").unwrap();

        let outcome = prompt.await.unwrap();

        assert!(
            matches!(
                outcome,
                PromptTurnOutcome::Aborted {
                    ref reason,
                    session_id: Some(ref session_id),
                    ..
                } if reason == "user cancelled" && session_id == "sess_prompt_abort_control"
            ),
            "got {outcome:?}"
        );
        let event_log = std::fs::read_to_string(
            temp.path()
                .join("sess_prompt_abort_control")
                .join("events.jsonl"),
        )
        .unwrap();
        assert!(event_log.contains("\"kind\":\"operation.aborted\""));
        assert!(event_log.contains("user cancelled"));
    }

    #[tokio::test]
    async fn prompt_uses_owner_issued_follow_up_control_handle() {
        let api = "coding-session-follow-up-control";
        let contexts = Arc::new(Mutex::new(Vec::new()));
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(BlockingTwoTurnProvider::new(
                contexts.clone(),
                started_tx,
                release_rx,
            )),
        );
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let handle = session.prompt_control_handle().unwrap();

        let mut prompt = Box::pin(session.prompt(prompt_options(api, "hello")));
        tokio::select! {
            started = started_rx => started.unwrap(),
            result = &mut prompt => panic!("prompt finished before provider blocked: {result:?}"),
        }
        handle.follow_up("continue from session owner").unwrap();
        release_tx.send(()).unwrap();

        let outcome = prompt.await.unwrap();

        assert!(matches!(
            outcome,
            PromptTurnOutcome::Success { final_text, .. } if final_text == "second"
        ));
        let contexts = contexts.lock().unwrap();
        assert_eq!(contexts.len(), 2);
        assert!(
            contexts[1].messages.iter().any(|message| matches!(
                message,
                Message::User { content }
                    if content.iter().any(|block| matches!(
                        block,
                        ContentBlock::Text { text, .. } if text == "continue from session owner"
                    ))
            )),
            "{:#?}",
            contexts[1].messages
        );
    }

    #[tokio::test]
    async fn run_operation_agent_team_uses_guard_and_preserves_input_error() {
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let operation = Operation::AgentTeam(AgentTeamOptions::new(
            "team",
            "",
            PromptTurnOptions::new(PromptInvocation::Text("task".into())),
        ));

        let error = session.run_operation(operation).await.unwrap_err();

        assert_eq!(error.code(), "input");
        assert!(
            error
                .to_string()
                .contains("agent team invocation requires a non-empty task"),
            "{error}"
        );
        assert_eq!(session.operation_control.active(), None);
    }

    #[tokio::test]
    async fn run_sync_operation_export_uses_guard_and_preserves_persistence_error() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let operation = Operation::Export(ExportOptions::view());

        let error = session.run_sync_operation(operation).unwrap_err();

        assert_eq!(error.code(), "unsupported_capability");
        assert_eq!(
            error.to_string(),
            "unsupported capability: export requires a persistent Rust-native session"
        );
        assert_eq!(session.operation_control.active(), None);
    }

    #[tokio::test]
    async fn run_sync_operation_plugin_command_uses_guard_and_preserves_plugin_error() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let operation = Operation::PluginCommand {
            command_id: "missing.command".into(),
            args: serde_json::Value::Null,
        };

        let error = session.run_sync_operation(operation).unwrap_err();

        assert_eq!(error.code(), "plugin");
        assert_eq!(
            error.to_string(),
            "plugin error: plugin command not found: missing.command"
        );
        assert_eq!(session.operation_control.active(), None);
    }

    #[tokio::test]
    async fn run_operation_agent_invocation_uses_guard_and_preserves_input_error() {
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let _handle = session.operation_control.prompt_control_handle().unwrap();
        let operation = Operation::AgentInvocation(AgentInvocationOptions::new(
            "helper",
            "",
            PromptTurnOptions::new(PromptInvocation::Text("task".into())),
        ));

        let error = session.run_operation(operation).await.unwrap_err();

        assert_eq!(error.code(), "input");
        assert!(
            error
                .to_string()
                .contains("agent invocation requires a non-empty task"),
            "{error}"
        );
        assert_eq!(session.operation_control.active(), None);
        assert!(session.operation_control.prompt_control_handle().is_ok());
    }

    #[tokio::test]
    async fn run_operation_self_healing_edit_uses_guard_and_preserves_persistence_error() {
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let operation = Operation::SelfHealingEdit(SelfHealingEditRequest::new(
            "src/lib.rs",
            vec![SelfHealingEditReplacement::new("old", "new")],
        ));

        let error = session.run_operation(operation).await.unwrap_err();

        assert_eq!(error.code(), "unsupported_capability");
        assert!(
            error
                .to_string()
                .contains("self-healing edit requires a persistent Rust-native session"),
            "{error}"
        );
        assert_eq!(session.operation_control.active(), None);
    }

    #[tokio::test]
    async fn run_operation_branch_summary_uses_branch_summary_guard_and_preserves_persistence_error()
     {
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let operation = Operation::BranchSummary {
            options: PromptTurnOptions::new(PromptInvocation::Text("summarize".into())),
            source_leaf_id: "source_leaf".into(),
            target_leaf_id: "target_leaf".into(),
            custom_instructions: None,
        };

        let error = session.run_operation(operation).await.unwrap_err();

        assert_eq!(error.code(), "unsupported_capability");
        assert!(
            error
                .to_string()
                .contains("branch summary without persistent session"),
            "{error}"
        );
        assert_eq!(session.operation_control.active(), None);
    }

    #[tokio::test]
    async fn run_operation_plugin_load_uses_plugin_load_guard_and_returns_outcome() {
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let operation = Operation::PluginLoad(PluginLoadOptions::new());

        let outcome = session.run_operation(operation).await.unwrap();

        let OperationOutcome::PluginLoad(outcome) = outcome else {
            panic!("expected plugin load outcome");
        };
        assert!(outcome.loaded_plugin_ids.is_empty());
        assert!(outcome.diagnostics.is_empty());
        assert!(!outcome.capability_changed);
        assert_eq!(session.operation_control.active(), None);
    }

    #[tokio::test]
    async fn run_operation_manual_compaction_uses_compact_guard_and_preserves_config_error() {
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let operation =
            Operation::ManualCompaction(PromptTurnOptions::new(PromptInvocation::Compact {
                custom_instructions: None,
            }));

        let error = session.run_operation(operation).await.unwrap_err();

        assert_eq!(error.code(), "config");
        assert!(
            error
                .to_string()
                .contains("compact operation options do not include a runtime snapshot"),
            "{error}"
        );
        assert_eq!(session.operation_control.active(), None);
    }

    #[tokio::test]
    async fn run_operation_prompt_uses_prompt_guard_and_preserves_prompt_error() {
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let operation = Operation::Prompt(PromptTurnOptions::new(PromptInvocation::Text(
            "hello".into(),
        )));

        let error = session.run_operation(operation).await.unwrap_err();

        assert_eq!(error.code(), "config");
        assert!(error.to_string().contains("runtime snapshot"), "{error}");
        assert_eq!(session.operation_control.active(), None);
    }

    #[tokio::test]
    async fn prompt_runs_flow_and_commits_session_events() {
        let api = "coding-session-prompt";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::simple_text("session answer")),
        );
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_prompt")
            .with_session_log_root(temp.path());
        let mut session = CodingAgentSession::create(options.clone()).await.unwrap();
        let mut events = session.subscribe();

        let outcome = session.prompt(prompt_options(api, "hello")).await.unwrap();

        let leaf_id = match &outcome {
            PromptTurnOutcome::Success {
                final_text,
                session_id: Some(session_id),
                leaf_id: Some(leaf_id),
                ..
            } if final_text == "session answer" && session_id == "sess_prompt" => leaf_id.clone(),
            other => panic!("expected successful prompt with committed leaf, got {other:?}"),
        };
        assert!(leaf_id.starts_with("leaf_"));
        assert!(matches!(
            events.try_recv().unwrap(),
            Some(CodingAgentEvent::PromptStarted { .. })
        ));
        assert!(matches!(
            events.try_recv().unwrap(),
            Some(CodingAgentEvent::AgentTurnStarted { .. })
        ));
        let remaining_events =
            std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert_event_order(
            &remaining_events,
            &[
                "session_write_pending",
                "session_write_committed",
                "prompt_completed",
            ],
        );
        assert_eq!(
            remaining_events
                .iter()
                .filter(|event| matches!(event, CodingAgentEvent::PromptCompleted { .. }))
                .count(),
            1
        );

        let replay = session.persistent_session_service().replay().unwrap();
        assert_eq!(replay.active_leaf_id.as_deref(), Some(leaf_id.as_str()));
        assert!(matches!(
            replay.transcript.as_slice(),
            [
                TranscriptItem::UserInput {
                    turn_id,
                    text,
                },
                TranscriptItem::AssistantMessage {
                    content,
                    status: MessageStatus::Completed,
                    ..
                },
            ] if turn_id == outcome_turn_id(&outcome)
                && text == "hello"
                && content == &vec![PersistedContentBlock::Text {
                    text: "session answer".into(),
                }]
        ));
        let event_log =
            std::fs::read_to_string(temp.path().join("sess_prompt/events.jsonl")).unwrap();
        assert!(!event_log.contains("\"message.delta\""));
        assert!(event_log.contains("\"kind\":\"message.completed\""));
        assert!(event_log.contains("\"content\""));
        let committed_leaf = event_log
            .lines()
            .filter_map(|line| serde_json::from_str::<SessionEventEnvelope>(line).ok())
            .find_map(|event| match event.data {
                SessionEventData::OperationCommitted {
                    new_leaf_id: Some(leaf_id),
                } => Some(leaf_id),
                _ => None,
            })
            .unwrap();
        assert_eq!(committed_leaf, leaf_id);
        let hydrated = session.hydrate_current().unwrap().unwrap();
        assert_eq!(
            hydrated.summary.active_leaf_id.as_deref(),
            Some(leaf_id.as_str())
        );
        let summaries = CodingAgentSession::list(options).unwrap();
        assert_eq!(
            summaries[0].active_leaf_id.as_deref(),
            Some(leaf_id.as_str())
        );
        assert_eq!(session.view().session_id, "sess_prompt");
    }

    #[tokio::test]
    async fn prompt_requires_runtime_backed_options() {
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_prompt_missing_runtime")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();

        let error = session
            .prompt(PromptTurnOptions::new(PromptInvocation::Text(
                "hello".into(),
            )))
            .await
            .unwrap_err();

        assert_eq!(error.code(), "config");
        assert!(error.to_string().contains("runtime snapshot"));
        assert!(
            session
                .persistent_session_service()
                .replay()
                .unwrap()
                .transcript
                .is_empty()
        );
        assert!(
            session
                .hydrate_current()
                .unwrap()
                .unwrap()
                .summary
                .active_leaf_id
                .is_none()
        );
    }

    #[tokio::test]
    async fn non_persistent_constructor_does_not_create_session_files() {
        let temp = tempfile::tempdir().unwrap();
        let session = CodingAgentSession::non_persistent(
            CodingAgentSessionOptions::new().with_session_log_root(temp.path()),
        )
        .await
        .unwrap();

        assert!(session.view().session_id.starts_with("runtime_sess_"));
        assert!(std::fs::read_dir(temp.path()).unwrap().next().is_none());
    }

    #[tokio::test]
    async fn non_persistent_prompt_emits_skipped_write_before_completion() {
        let api = "coding-session-non-persistent-prompt";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::simple_text("transient answer")),
        );
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let mut events = session.subscribe();

        let outcome = session.prompt(prompt_options(api, "hello")).await.unwrap();

        assert!(matches!(
            &outcome,
            PromptTurnOutcome::Success {
                final_text,
                session_id: None,
                leaf_id: None,
                ..
            } if final_text == "transient answer"
        ));
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert_event_order(
            &emitted_events,
            &["session_write_skipped", "prompt_completed"],
        );
        assert!(emitted_events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::SessionWriteSkipped { reason, .. }
                if reason == "session persistence disabled"
        )));
    }

    #[tokio::test]
    async fn non_persistent_prompt_hydrates_owner_lifetime_transcript() {
        let first_api = "coding-session-non-persistent-first";
        let second_api = "coding-session-non-persistent-second";
        let contexts = Arc::new(Mutex::new(Vec::new()));
        let _provider_guard = crate::test_support::ProviderGuard::register_many(vec![
            (
                first_api.to_string(),
                Arc::new(FauxProvider::simple_text("first answer")),
            ),
            (
                second_api.to_string(),
                Arc::new(RecordingProvider::new(
                    Arc::clone(&contexts),
                    "second answer",
                )),
            ),
        ]);
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();

        session
            .prompt(prompt_options(first_api, "first question"))
            .await
            .unwrap();

        session
            .prompt(prompt_options(second_api, "second question"))
            .await
            .unwrap();

        let contexts = contexts.lock().unwrap();
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].messages.len(), 3);
        assert!(matches!(
            &contexts[0].messages[0],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "first question".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &contexts[0].messages[1],
            Message::Assistant { content }
                if content == &vec![ContentBlock::Text {
                    text: "first answer".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &contexts[0].messages[2],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "second question".into(),
                    text_signature: None,
                }]
        ));
    }

    #[tokio::test]
    async fn prompt_does_not_duplicate_failure_event_from_agent_error() {
        let api = "coding-session-prompt-error";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("partial", StopReason::Error),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_prompt_error")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let mut events = session.subscribe();

        let outcome = session.prompt(prompt_options(api, "hello")).await.unwrap();

        assert!(matches!(outcome, PromptTurnOutcome::Failed { .. }));
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert_event_order(
            &emitted_events,
            &[
                "session_write_pending",
                "session_write_committed",
                "prompt_failed",
            ],
        );
        assert_eq!(
            emitted_events
                .iter()
                .filter(|event| matches!(event, CodingAgentEvent::PromptFailed { .. }))
                .count(),
            1
        );
        assert!(
            session
                .persistent_session_service()
                .replay()
                .unwrap()
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("operation")
                    && diagnostic.message.contains("failed"))
        );
    }

    #[tokio::test]
    async fn branch_summary_persistent_session_records_model_summary() {
        let api = "coding-session-branch-summary";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("root answer", StopReason::Stop),
                FauxProvider::text_call("branch answer", StopReason::Stop),
                FauxProvider::text_call("model branch summary", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_branch_summary_owner")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let root_leaf = match session
            .prompt(prompt_options(api, "root question"))
            .await
            .unwrap()
        {
            PromptTurnOutcome::Success {
                leaf_id: Some(leaf_id),
                ..
            } => leaf_id,
            other => panic!("expected root prompt success, got {other:?}"),
        };
        let branch_leaf = match session
            .prompt(prompt_options(api, "branch question"))
            .await
            .unwrap()
        {
            PromptTurnOutcome::Success {
                leaf_id: Some(leaf_id),
                ..
            } => leaf_id,
            other => panic!("expected branch prompt success, got {other:?}"),
        };
        let mut events = session.subscribe();

        let outcome = session
            .summarize_branch(
                prompt_options(api, ""),
                branch_leaf.clone(),
                root_leaf.clone(),
                Some("keep branch decisions".into()),
            )
            .await
            .unwrap();

        assert!(matches!(
            &outcome,
            PromptTurnOutcome::Success {
                final_text,
                session_id: Some(session_id),
                leaf_id: Some(_),
                ..
            } if final_text.contains("model branch summary")
                && session_id == "sess_branch_summary_owner"
        ));
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert_event_order(
            &emitted_events,
            &["session_write_pending", "session_write_committed"],
        );
        let replay = session.persistent_session_service().replay().unwrap();
        assert!(matches!(
            replay.transcript.last(),
            Some(TranscriptItem::BranchSummary {
                summary,
                source_leaf_id,
                target_leaf_id,
            }) if summary.contains("model branch summary")
                && source_leaf_id == &branch_leaf
                && target_leaf_id == &root_leaf
        ));
        let event_log =
            std::fs::read_to_string(temp.path().join("sess_branch_summary_owner/events.jsonl"))
                .unwrap();
        assert!(event_log.contains("branch.summary.created"));
    }

    #[tokio::test]
    async fn branch_summary_navigation_reuses_existing_summary_without_rewriting_session() {
        let api = "coding-session-branch-summary-navigation-reuse";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("root answer", StopReason::Stop),
                FauxProvider::text_call("branch answer", StopReason::Stop),
                FauxProvider::text_call("model branch summary", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_branch_summary_navigation_reuse")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let root_leaf = match session
            .prompt(prompt_options(api, "root question"))
            .await
            .unwrap()
        {
            PromptTurnOutcome::Success {
                leaf_id: Some(leaf_id),
                ..
            } => leaf_id,
            other => panic!("expected root prompt success, got {other:?}"),
        };
        let branch_leaf = match session
            .prompt(prompt_options(api, "branch question"))
            .await
            .unwrap()
        {
            PromptTurnOutcome::Success {
                leaf_id: Some(leaf_id),
                ..
            } => leaf_id,
            other => panic!("expected branch prompt success, got {other:?}"),
        };
        session
            .summarize_branch(
                prompt_options(api, ""),
                branch_leaf.clone(),
                root_leaf.clone(),
                None,
            )
            .await
            .unwrap();
        let event_log_path = temp
            .path()
            .join("sess_branch_summary_navigation_reuse/events.jsonl");
        let event_log_before = std::fs::read_to_string(&event_log_path).unwrap();
        let summary_count_before = event_log_before.matches("branch.summary.created").count();
        let mut events = session.subscribe();

        let outcome = session
            .summarize_branch_for_navigation(
                prompt_options(api, ""),
                branch_leaf.clone(),
                root_leaf.clone(),
            )
            .await
            .unwrap();

        assert!(matches!(
            &outcome,
            PromptTurnOutcome::Success {
                final_text,
                session_id: Some(session_id),
                leaf_id: Some(active_leaf),
                ..
            } if final_text.contains("model branch summary")
                && session_id == "sess_branch_summary_navigation_reuse"
                && active_leaf.as_str() == branch_leaf.as_str()
        ));
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert!(emitted_events.is_empty(), "{emitted_events:#?}");
        let event_log_after = std::fs::read_to_string(&event_log_path).unwrap();
        assert_eq!(event_log_after, event_log_before);
        assert_eq!(summary_count_before, 1);
        assert_eq!(event_log_after.matches("branch.summary.created").count(), 1);
    }

    #[tokio::test]
    async fn compact_persistent_session_records_events_and_replays_summary() {
        let api = "coding-session-compact";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("first answer", StopReason::Stop),
                FauxProvider::text_call("summary from compact", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_compact")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        session
            .prompt(prompt_options(api, "first question"))
            .await
            .unwrap();
        let mut events = session.subscribe();

        let outcome = session
            .compact(compact_options(api, Some("keep decisions")))
            .await
            .unwrap();

        assert!(matches!(
            &outcome,
            PromptTurnOutcome::Success {
                final_text,
                session_id: Some(session_id),
                leaf_id: Some(_),
                ..
            } if final_text == "summary from compact" && session_id == "sess_compact"
        ));
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert_event_order(
            &emitted_events,
            &[
                "session_write_pending",
                "session_compaction_completed",
                "session_write_committed",
                "prompt_completed",
            ],
        );
        assert!(emitted_events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::SessionCompactionCompleted {
                summary,
                tokens_before,
                ..
            } if summary == "summary from compact" && *tokens_before > 0
        )));

        let replay = session.persistent_session_service().replay().unwrap();
        assert!(matches!(
            replay.transcript.as_slice(),
            [
                TranscriptItem::CompactionSummary {
                    summary,
                    first_kept_message_id,
                    tokens_before,
                },
                TranscriptItem::AssistantMessage {
                    content,
                    status: MessageStatus::Completed,
                    ..
                },
            ] if summary == "summary from compact"
                && first_kept_message_id.starts_with("msg_")
                && *tokens_before > 0
                && content == &vec![PersistedContentBlock::Text {
                    text: "first answer".into(),
                }]
        ));
        let event_log =
            std::fs::read_to_string(temp.path().join("sess_compact/events.jsonl")).unwrap();
        assert!(event_log.contains("session.compaction.started"));
        assert!(event_log.contains("session.compaction.completed"));
    }

    #[tokio::test]
    async fn compact_summary_failure_records_failure_without_folding_replay() {
        let api = "coding-session-compact-summary-failure";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("first answer", StopReason::Stop),
                FauxProvider::text_call("", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_compact_failure")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let prompt_outcome = session
            .prompt(prompt_options(api, "first question"))
            .await
            .unwrap();
        let active_leaf_before = match prompt_outcome {
            PromptTurnOutcome::Success {
                leaf_id: Some(leaf_id),
                ..
            } => leaf_id,
            other => panic!("expected prompt success, got {other:?}"),
        };
        let mut events = session.subscribe();

        let outcome = session.compact(compact_options(api, None)).await.unwrap();

        assert!(matches!(
            &outcome,
            PromptTurnOutcome::Failed { error, .. }
                if error.code() == "provider" && error.to_string().contains("empty summary")
        ));
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert_event_order(
            &emitted_events,
            &[
                "session_write_pending",
                "session_write_committed",
                "prompt_failed",
            ],
        );
        let replay = session.persistent_session_service().replay().unwrap();
        assert_eq!(
            replay.active_leaf_id.as_deref(),
            Some(active_leaf_before.as_str())
        );
        assert!(
            replay
                .transcript
                .iter()
                .all(|item| !matches!(item, TranscriptItem::CompactionSummary { .. }))
        );
        assert!(matches!(
            replay.transcript.as_slice(),
            [
                TranscriptItem::UserInput { text, .. },
                TranscriptItem::AssistantMessage { content, .. },
                TranscriptItem::Diagnostic { message, .. },
            ] if text == "first question"
                && content == &vec![PersistedContentBlock::Text {
                    text: "first answer".into(),
                }]
                && message.contains("empty summary")
        ));
        let event_log =
            std::fs::read_to_string(temp.path().join("sess_compact_failure/events.jsonl")).unwrap();
        assert!(event_log.contains("session.compaction.started"));
        assert!(event_log.contains("operation.failed"));
        assert!(!event_log.contains("session.compaction.completed"));
    }

    #[tokio::test]
    async fn prompt_hydrates_replayed_transcript_when_opening_session() {
        let first_api = "coding-session-hydrate-first";
        let second_api = "coding-session-hydrate-second";
        let contexts = Arc::new(Mutex::new(Vec::new()));
        let _provider_guard = crate::test_support::ProviderGuard::register_many(vec![
            (
                first_api.to_string(),
                Arc::new(FauxProvider::simple_text("first answer")),
            ),
            (
                second_api.to_string(),
                Arc::new(RecordingProvider::new(
                    Arc::clone(&contexts),
                    "second answer",
                )),
            ),
        ]);
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_hydrate")
            .with_session_log_root(temp.path());
        let mut created = CodingAgentSession::create(options.clone()).await.unwrap();
        created
            .prompt(prompt_options(first_api, "first question"))
            .await
            .unwrap();
        let mut opened = CodingAgentSession::open(options).await.unwrap();

        let outcome = opened
            .prompt(prompt_options(second_api, "second question"))
            .await
            .unwrap();

        assert!(matches!(
            outcome,
            PromptTurnOutcome::Success { final_text, .. } if final_text == "second answer"
        ));
        let contexts = contexts.lock().unwrap();
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].messages.len(), 3);
        assert!(matches!(
            &contexts[0].messages[0],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "first question".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &contexts[0].messages[1],
            Message::Assistant { content }
                if content == &vec![ContentBlock::Text {
                    text: "first answer".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &contexts[0].messages[2],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "second question".into(),
                    text_signature: None,
                }]
        ));
    }

    #[tokio::test]
    async fn prompt_hydrates_replayed_tool_calls_when_opening_session() {
        let first_api = "coding-session-hydrate-tool-first";
        let second_api = "coding-session-hydrate-tool-second";
        let contexts = Arc::new(Mutex::new(Vec::new()));
        let _provider_guard = crate::test_support::ProviderGuard::register_many(vec![
            (
                first_api.to_string(),
                Arc::new(FauxProvider::with_call_queue(vec![
                    FauxProvider::single_call(
                        vec![FauxResponse {
                            text_deltas: vec!["I will use echo.".into()],
                            thinking_deltas: Vec::new(),
                            tool_calls: vec![FauxToolCall {
                                id: "toolu_1".into(),
                                name: "echo".into(),
                                deltas: Vec::new(),
                                final_arguments: serde_json::json!({"text": "hi"}),
                            }],
                        }],
                        StopReason::ToolUse,
                    ),
                    FauxProvider::text_call("tool final", StopReason::Stop),
                ])),
            ),
            (
                second_api.to_string(),
                Arc::new(RecordingProvider::new(
                    Arc::clone(&contexts),
                    "second answer",
                )),
            ),
        ]);
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_tool_hydrate")
            .with_session_log_root(temp.path());
        let mut created = CodingAgentSession::create(options.clone()).await.unwrap();
        created
            .prompt(prompt_options_with_tools(
                first_api,
                "use the tool",
                vec![echo_tool()],
            ))
            .await
            .unwrap();
        let mut opened = CodingAgentSession::open(options).await.unwrap();

        opened
            .prompt(prompt_options(second_api, "continue"))
            .await
            .unwrap();

        let contexts = contexts.lock().unwrap();
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].messages.len(), 5);
        assert!(matches!(
            &contexts[0].messages[0],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "use the tool".into(),
                    text_signature: None,
                }]
        ));
        let tool_call_id = match &contexts[0].messages[1] {
            Message::Assistant { content } => match content.as_slice() {
                [
                    ContentBlock::Text { text, .. },
                    ContentBlock::ToolCall {
                        id,
                        name,
                        arguments,
                        ..
                    },
                ] => {
                    assert_eq!(text, "I will use echo.");
                    assert_eq!(name, "echo");
                    assert_eq!(arguments, &serde_json::json!({"text": "hi"}));
                    id.clone()
                }
                other => panic!("unexpected assistant content: {other:?}"),
            },
            other => panic!("unexpected hydrated assistant message: {other:?}"),
        };
        assert!(matches!(
            &contexts[0].messages[2],
            Message::ToolResult {
                tool_call_id: result_tool_call_id,
                tool_name: Some(tool_name),
                is_error: Some(false),
                content,
            } if result_tool_call_id == &tool_call_id
                && tool_name == "echo"
                && content == &vec![ContentBlock::Text {
                    text: "echo: hi".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &contexts[0].messages[3],
            Message::Assistant { content }
                if content == &vec![ContentBlock::Text {
                    text: "tool final".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &contexts[0].messages[4],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "continue".into(),
                    text_signature: None,
                }]
        ));
    }

    #[tokio::test]
    async fn export_current_html_writes_rust_native_session_transcript() {
        let api = "coding-session-export-html";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::single_call(
                    vec![FauxResponse {
                        text_deltas: vec!["I will use echo.".into()],
                        thinking_deltas: Vec::new(),
                        tool_calls: vec![FauxToolCall {
                            id: "toolu_export".into(),
                            name: "echo".into(),
                            deltas: Vec::new(),
                            final_arguments: serde_json::json!({"text": "<hi>"}),
                        }],
                    }],
                    StopReason::ToolUse,
                ),
                FauxProvider::text_call("tool final <done>", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_export_html")
            .with_session_log_root(temp.path());
        let mut session = CodingAgentSession::create(options).await.unwrap();
        session
            .prompt(prompt_options_with_tools(
                api,
                "use <tool>",
                vec![echo_tool()],
            ))
            .await
            .unwrap();
        let output = temp.path().join("exports/session.html");

        let exported = session.export_current_html(&output).unwrap();

        assert_eq!(exported, output);
        let html = std::fs::read_to_string(&exported).unwrap();
        assert!(html.contains("<!doctype html>"), "{html}");
        assert!(html.contains("sess_export_html"), "{html}");
        assert!(html.contains("use &lt;tool&gt;"), "{html}");
        assert!(html.contains("I will use echo."), "{html}");
        assert!(html.contains("Tool: echo"), "{html}");
        assert!(html.contains("&lt;hi&gt;"), "{html}");
        assert!(html.contains("echo: &lt;hi&gt;"), "{html}");
        assert!(html.contains("tool final &lt;done&gt;"), "{html}");
    }

    #[tokio::test]
    async fn export_current_html_rejects_jsonl_target() {
        let temp = tempfile::tempdir().unwrap();
        let session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_export_jsonl")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let output = temp.path().join("session.jsonl");

        let error = session.export_current_html(&output).unwrap_err();

        assert_eq!(error.code(), "input");
        assert_eq!(
            error.to_string(),
            "invalid input: JSONL session export is no longer supported"
        );
        assert!(!output.exists());
    }

    #[tokio::test]
    async fn export_current_html_uses_export_operation_boundary() {
        let temp = tempfile::tempdir().unwrap();
        let session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_export_busy")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let _operation = session
            .operation_control
            .begin(OperationKind::Prompt)
            .unwrap();
        let output = temp.path().join("session.html");

        let error = session.export_current_html(&output).unwrap_err();

        assert_eq!(error.code(), "busy");
        assert_eq!(error.to_string(), "busy: prompt");
        assert!(!output.exists());
    }

    fn outcome_turn_id(outcome: &PromptTurnOutcome) -> &str {
        match outcome {
            PromptTurnOutcome::Success { turn_id, .. } => turn_id,
            _ => panic!("expected success outcome"),
        }
    }

    fn assert_event_order(events: &[CodingAgentEvent], expected: &[&str]) {
        let observed = events.iter().map(event_kind).collect::<Vec<_>>();
        let mut next_index = 0;
        for kind in observed {
            if next_index < expected.len() && kind == expected[next_index] {
                next_index += 1;
            }
        }
        assert_eq!(
            next_index,
            expected.len(),
            "did not observe event order {expected:?}"
        );
    }

    fn event_kind(event: &CodingAgentEvent) -> &'static str {
        match event {
            CodingAgentEvent::SessionOpened { .. } => "session_opened",
            CodingAgentEvent::DefaultAgentProfileChanged { .. } => "default_agent_profile_changed",
            CodingAgentEvent::AgentInvocationStarted { .. } => "agent_invocation_started",
            CodingAgentEvent::AgentInvocationCompleted { .. } => "agent_invocation_completed",
            CodingAgentEvent::AgentInvocationFailed { .. } => "agent_invocation_failed",
            CodingAgentEvent::AgentInvocationAborted { .. } => "agent_invocation_aborted",
            CodingAgentEvent::AgentTeamStarted { .. } => "agent_team_started",
            CodingAgentEvent::AgentTeamMemberStarted { .. } => "agent_team_member_started",
            CodingAgentEvent::AgentTeamMemberCompleted { .. } => "agent_team_member_completed",
            CodingAgentEvent::AgentTeamCompleted { .. } => "agent_team_completed",
            CodingAgentEvent::AgentTeamFailed { .. } => "agent_team_failed",
            CodingAgentEvent::AgentTeamAborted { .. } => "agent_team_aborted",
            CodingAgentEvent::SelfHealingEditStarted { .. } => "self_healing_edit_started",
            CodingAgentEvent::SelfHealingEditRepairAttempted { .. } => {
                "self_healing_edit_repair_attempted"
            }
            CodingAgentEvent::SelfHealingEditCompleted { .. } => "self_healing_edit_completed",
            CodingAgentEvent::SelfHealingEditFailed { .. } => "self_healing_edit_failed",
            CodingAgentEvent::DelegationRequested { .. } => "delegation_requested",
            CodingAgentEvent::DelegationRejected { .. } => "delegation_rejected",
            CodingAgentEvent::DelegationApproved { .. } => "delegation_approved",
            CodingAgentEvent::DelegationConfirmationRequired { .. } => {
                "delegation_confirmation_required"
            }
            CodingAgentEvent::DelegationStarted { .. } => "delegation_started",
            CodingAgentEvent::DelegationCompleted { .. } => "delegation_completed",
            CodingAgentEvent::DelegationFailed { .. } => "delegation_failed",
            CodingAgentEvent::SessionWritePending { .. } => "session_write_pending",
            CodingAgentEvent::SessionWriteCommitted { .. } => "session_write_committed",
            CodingAgentEvent::SessionWriteSkipped { .. } => "session_write_skipped",
            CodingAgentEvent::PromptStarted { .. } => "prompt_started",
            CodingAgentEvent::AgentTurnStarted { .. } => "agent_turn_started",
            CodingAgentEvent::ProviderRequestStarted { .. } => "provider_request_started",
            CodingAgentEvent::AssistantMessageStarted { .. } => "assistant_message_started",
            CodingAgentEvent::AssistantMessageDelta { .. } => "assistant_message_delta",
            CodingAgentEvent::AssistantThinkingDelta { .. } => "assistant_thinking_delta",
            CodingAgentEvent::AssistantMessageCompleted { .. } => "assistant_message_completed",
            CodingAgentEvent::ToolCallStarted { .. } => "tool_call_started",
            CodingAgentEvent::ToolCallUpdated { .. } => "tool_call_updated",
            CodingAgentEvent::ToolCallCompleted { .. } => "tool_call_completed",
            CodingAgentEvent::ToolCallFailed { .. } => "tool_call_failed",
            CodingAgentEvent::RuntimeCompactionCompleted { .. } => "runtime_compaction_completed",
            CodingAgentEvent::SessionCompactionCompleted { .. } => "session_compaction_completed",
            CodingAgentEvent::PromptCompleted { .. } => "prompt_completed",
            CodingAgentEvent::PromptFailed { .. } => "prompt_failed",
            CodingAgentEvent::PromptAborted { .. } => "prompt_aborted",
            CodingAgentEvent::Diagnostic { .. } => "diagnostic",
            CodingAgentEvent::CapabilityChanged => "capability_changed",
        }
    }
}
