mod agent_invocation_flow;
mod agent_team_flow;
mod branch_summary_flow;
mod branch_summary_service;
mod capability_service;
mod capability_snapshot;
mod client_projection;
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
mod intent_router;
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
mod public_operation;
mod public_projection;
mod runtime_service;
mod self_healing_edit_flow;
mod self_healing_edit_service;
mod session_log;
mod session_service;

pub use agent_invocation_flow::{AgentInvocationOptions, AgentInvocationOutcome};
pub use agent_team_flow::{AgentTeamMemberOutcome, AgentTeamOptions, AgentTeamOutcome};
#[allow(unused_imports)]
pub(crate) use client_projection::{
    ClientConnection, ClientConnectionId, ClientDraft, ClientDraftKind, SubmittedOperation,
    UiSnapshot, UiSnapshotCursor,
};
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
#[allow(unused_imports)]
pub(crate) use event::{ProductEvent, ProductEventSequence};
pub use event_service::CodingAgentEventReceiver;
pub(crate) use event_service::ProductEventReceiver;
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
pub use public_operation::{
    BranchSummaryReusePolicy, CodingAgentOperation, CodingAgentOperationOutcome,
    CodingAgentPluginDiagnostic, CodingAgentPluginLoadOutcome,
};
pub use public_projection::{
    CodingAgentClientConnection, CodingAgentClientId, CodingAgentProductEvent,
    CodingAgentProductEventReceiver, CodingAgentSnapshot, CodingAgentSnapshotCursor,
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
pub(crate) use capability_snapshot::PluginCapabilitySet;
use capability_snapshot::{
    ActorId, CapabilitySnapshotInput, CapabilitySnapshotService, OperationCapabilitySnapshot,
    SessionReadCapability, SessionWriteCapability,
};
pub use capability_snapshot::{CapabilityRevocationPolicy, FilesystemCapability, ShellCapability};
pub(crate) use delegation::{
    DelegationAuthorizationDecision, PendingDelegationConfirmationQueue,
    PendingDelegationConfirmationState, delegation_lineage_for_request, pending_state_from_replay,
};
use delegation_confirmation_service::DelegationConfirmationService;
use delegation_execution_service::DelegationExecutionService;
use event_service::EventService;
use export_flow::ExportOptions;
use flow_service::FlowService;
use intent_router::{ControlIntent, IntentRouter, QueryIntent};
use manual_compaction_flow::ManualCompactionOptions;
use manual_compaction_service::ManualCompactionService;
pub(crate) use operation::OperationIdempotencyKey;
use operation::{Operation, OperationAdmission, OperationDispatchMode, OperationOutcome};
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
    FinalizedSessionWrite, SessionPersistence, SessionService, StartupRecoveryMarker,
    TransientSessionState,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::plugins::{
    CommandDefinition, KeybindDefinition, PluginSource, UiActionDefinition, UiDialogDefinition,
};
use crate::protocol::version::UI_SNAPSHOT_PROTOCOL_VERSION;

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
    capability_snapshots: CapabilitySnapshotService,
    startup_recovery_markers: Mutex<Vec<StartupRecoveryMarker>>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProductEventReplayHandle {
    event_service: EventService,
}

impl ProductEventReplayHandle {
    fn new(event_service: EventService) -> Self {
        Self { event_service }
    }

    pub(crate) fn product_events_after(
        &self,
        cursor: ProductEventSequence,
    ) -> Result<Vec<ProductEvent>, CodingSessionError> {
        self.event_service.product_events_after(cursor)
    }
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

struct ReplayDerivedOwnerState {
    pending_delegation_confirmations: PendingDelegationConfirmationQueue,
    startup_recovery_markers: Vec<StartupRecoveryMarker>,
}

fn replay_derived_owner_state(
    session_service: &mut SessionService,
) -> Result<ReplayDerivedOwnerState, CodingSessionError> {
    let startup_recovery_markers = session_service.take_startup_recovery_markers();
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
    Ok(ReplayDerivedOwnerState {
        pending_delegation_confirmations,
        startup_recovery_markers,
    })
}

impl CodingAgentSession {
    pub async fn run(
        &mut self,
        operation: CodingAgentOperation,
    ) -> Result<CodingAgentOperationOutcome, CodingSessionError> {
        let operation = operation.into_internal(self.default_plugin_load_options.clone());
        let dispatch_mode = operation.metadata().dispatch_mode;
        let outcome = match dispatch_mode {
            OperationDispatchMode::Async => self.run_operation(operation).await?,
            OperationDispatchMode::SyncReadOnly => self.run_sync_operation(operation)?,
            OperationDispatchMode::SyncMutable => self.run_sync_mut_operation(operation)?,
        };
        Ok(CodingAgentOperationOutcome::from_internal(outcome))
    }

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

    #[cfg(test)]
    pub(crate) async fn non_persistent_with_event_capacity_for_tests(
        options: CodingAgentSessionOptions,
        event_capacity: usize,
    ) -> Result<Self, CodingSessionError> {
        let mut session = Self::non_persistent(options).await?;
        session.event_service = EventService::with_event_capacity_for_tests(event_capacity);
        Ok(session)
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

    #[deprecated(note = "use CodingAgentSession::run instead")]
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
            OperationOutcome::DelegationApproval => {
                unreachable!("export operation returned delegation approval outcome")
            }
            OperationOutcome::DelegationRejection => {
                unreachable!("export operation returned delegation rejection outcome")
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
            OperationOutcome::ForkSession | OperationOutcome::SwitchActiveLeaf => {
                unreachable!("export operation returned navigation outcome")
            }
            OperationOutcome::SetDefaultAgentProfile => {
                unreachable!("export operation returned set default agent profile outcome")
            }
        }
    }

    #[deprecated(note = "use CodingAgentSession::run instead")]
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
            OperationOutcome::DelegationApproval => {
                unreachable!("export operation returned delegation approval outcome")
            }
            OperationOutcome::DelegationRejection => {
                unreachable!("export operation returned delegation rejection outcome")
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
            OperationOutcome::ForkSession | OperationOutcome::SwitchActiveLeaf => {
                unreachable!("export operation returned navigation outcome")
            }
            OperationOutcome::SetDefaultAgentProfile => {
                unreachable!("export operation returned set default agent profile outcome")
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
        let operation = Operation::ForkSession {
            target_leaf_id: target_leaf_id.map(str::to_owned),
        };
        let admission = self.resolve_operation_admission(&operation)?;
        let _operation_permit = IntentRouter::admit_operation(
            &self.operation_control,
            &admission,
            OperationDispatchMode::SyncMutable,
        )?;
        let Operation::ForkSession { target_leaf_id } = operation else {
            unreachable!("operation is ForkSession")
        };

        match &self.persistence {
            SessionPersistence::Persistent(session_service) => Self::from_services(
                session_service.fork_current(target_leaf_id.as_deref())?,
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

    #[deprecated(note = "use subscribe_product_events_public instead")]
    #[allow(deprecated)]
    pub fn subscribe(&self) -> CodingAgentEventReceiver {
        let receiver = self.event_service.subscribe();
        self.emit_pending_startup_recovery_markers();
        receiver
    }

    pub(crate) fn subscribe_product_events(&self) -> ProductEventReceiver {
        let receiver = self.event_service.subscribe_product_events();
        self.emit_pending_startup_recovery_markers();
        receiver
    }

    pub fn subscribe_product_events_public(&self) -> CodingAgentProductEventReceiver {
        CodingAgentProductEventReceiver::new(self.subscribe_product_events())
    }

    fn emit_pending_startup_recovery_markers(&self) {
        let markers = {
            let mut markers = self.startup_recovery_markers.lock().unwrap();
            std::mem::take(&mut *markers)
        };
        for marker in markers {
            self.event_service.emit_operation_recovered(
                marker.operation_id,
                marker.recovery_id,
                marker.reason,
            );
        }
    }

    pub(crate) fn product_event_replay_handle(&self) -> ProductEventReplayHandle {
        self.emit_pending_startup_recovery_markers();
        ProductEventReplayHandle::new(self.event_service.clone())
    }

    pub fn snapshot(&self) -> CodingAgentSnapshot {
        self.ui_snapshot(Vec::new()).into()
    }

    pub fn connect(&self, id: CodingAgentClientId) -> CodingAgentClientConnection {
        let internal_id = public_projection::internal_client_id(&id);
        let (connection, snapshot) = self.connect_client(internal_id, Vec::new());
        public_projection::public_client_connection(id, connection, snapshot)
    }

    #[allow(dead_code)]
    pub(crate) fn ui_snapshot(&self, client_drafts: Vec<ClientDraft>) -> UiSnapshot {
        self.emit_pending_startup_recovery_markers();
        IntentRouter::admit_query(&self.operation_control, QueryIntent::SessionView);
        UiSnapshot::new(
            UiSnapshotCursor {
                last_event_sequence: self.event_service.current_product_sequence(),
                capability_generation: self.capability_snapshots.current_generation(),
            },
            UI_SNAPSHOT_PROTOCOL_VERSION,
            self.view(),
            self.capabilities(),
            self.operation_control.active(),
            client_drafts,
        )
    }

    #[allow(dead_code)]
    pub(crate) fn connect_client(
        &self,
        id: ClientConnectionId,
        client_drafts: Vec<ClientDraft>,
    ) -> (ClientConnection, UiSnapshot) {
        let snapshot = self.ui_snapshot(client_drafts);
        let connection = ClientConnection::new(id, snapshot.clone());
        (connection, snapshot)
    }

    #[allow(dead_code)]
    pub(crate) fn product_events_after(
        &self,
        cursor: ProductEventSequence,
    ) -> Result<Vec<ProductEvent>, CodingSessionError> {
        self.emit_pending_startup_recovery_markers();
        self.event_service.product_events_after(cursor)
    }

    #[cfg(test)]
    pub(crate) fn emit_product_event_for_tests(&self, event: CodingAgentEvent) -> ProductEvent {
        self.event_service.emit(event)
    }

    pub(crate) fn prompt_control_handle(
        &mut self,
    ) -> Result<PromptControlHandle, CodingSessionError> {
        IntentRouter::prompt_control_handle(
            &mut self.operation_control,
            ControlIntent::PromptControl,
        )
    }

    pub fn capabilities(&self) -> CodingAgentCapabilities {
        IntentRouter::admit_query(&self.operation_control, QueryIntent::Capabilities);
        let plugin_capabilities = self.plugin_service.capabilities();
        let persistent = matches!(self.persistence, SessionPersistence::Persistent(_));
        self.capability_service.capabilities(
            self.operation_control.active(),
            &plugin_capabilities,
            persistent,
        )
    }

    pub fn view(&self) -> CodingAgentSessionView {
        IntentRouter::admit_query(&self.operation_control, QueryIntent::SessionView);
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
        IntentRouter::admit_query(&self.operation_control, QueryIntent::AgentProfiles);
        self.profile_registry.agents().cloned().collect()
    }

    pub fn team_profiles(&self) -> Vec<TeamProfile> {
        IntentRouter::admit_query(&self.operation_control, QueryIntent::TeamProfiles);
        self.profile_registry.teams().cloned().collect()
    }

    pub fn profile_diagnostics(&self) -> Vec<ProfileDiagnostic> {
        IntentRouter::admit_query(&self.operation_control, QueryIntent::ProfileDiagnostics);
        self.profile_registry.diagnostics().to_vec()
    }

    pub fn set_default_agent_profile_id(
        &mut self,
        profile_id: impl Into<ProfileId>,
    ) -> Result<(), CodingSessionError> {
        let profile_id = profile_id.into();
        match self.run_sync_mut_operation(Operation::SetDefaultAgentProfile { profile_id })? {
            OperationOutcome::SetDefaultAgentProfile => Ok(()),
            _ => unreachable!("set default agent profile operation returned wrong outcome"),
        }
    }

    pub fn pending_delegation_confirmations(&self) -> Vec<PendingDelegationConfirmation> {
        IntentRouter::admit_query(
            &self.operation_control,
            QueryIntent::PendingDelegationConfirmations,
        );
        let now = SystemClock.now_rfc3339();
        self.delegation_confirmation_service
            .active_views(&self.pending_delegation_confirmations, &now)
    }

    pub async fn approve_delegation_confirmation(
        &mut self,
        operation_id: impl AsRef<str>,
        tool_call_id: impl AsRef<str>,
    ) -> Result<(), CodingSessionError> {
        match self
            .run_operation(Operation::ApproveDelegationConfirmation {
                operation_id: operation_id.as_ref().to_owned(),
                tool_call_id: tool_call_id.as_ref().to_owned(),
            })
            .await?
        {
            OperationOutcome::DelegationApproval => Ok(()),
            OperationOutcome::DelegationRejection => {
                unreachable!("delegation approval operation returned delegation rejection outcome")
            }
            OperationOutcome::Prompt(_) => {
                unreachable!("delegation approval operation returned prompt outcome")
            }
            OperationOutcome::ManualCompaction(_) => {
                unreachable!("delegation approval operation returned manual compaction outcome")
            }
            OperationOutcome::PluginLoad(_) => {
                unreachable!("delegation approval operation returned plugin load outcome")
            }
            OperationOutcome::PluginCommand(_) => {
                unreachable!("delegation approval operation returned plugin command outcome")
            }
            OperationOutcome::BranchSummary(_) => {
                unreachable!("delegation approval operation returned branch summary outcome")
            }
            OperationOutcome::SelfHealingEdit(_) => {
                unreachable!("delegation approval operation returned self-healing edit outcome")
            }
            OperationOutcome::AgentInvocation(_) => {
                unreachable!("delegation approval operation returned agent invocation outcome")
            }
            OperationOutcome::AgentTeam(_) => {
                unreachable!("delegation approval operation returned agent team outcome")
            }
            OperationOutcome::Export(_) => {
                unreachable!("delegation approval operation returned export outcome")
            }
            OperationOutcome::ForkSession | OperationOutcome::SwitchActiveLeaf => {
                unreachable!("delegation approval operation returned navigation outcome")
            }
            OperationOutcome::SetDefaultAgentProfile => {
                unreachable!(
                    "delegation approval operation returned set default agent profile outcome"
                )
            }
        }
    }

    pub fn reject_delegation_confirmation(
        &mut self,
        operation_id: impl AsRef<str>,
        tool_call_id: impl AsRef<str>,
        reason: impl Into<String>,
    ) -> Result<(), CodingSessionError> {
        match self.run_sync_mut_operation(Operation::RejectDelegationConfirmation {
            operation_id: operation_id.as_ref().to_owned(),
            tool_call_id: tool_call_id.as_ref().to_owned(),
            reason: reason.into(),
        })? {
            OperationOutcome::DelegationRejection => Ok(()),
            OperationOutcome::DelegationApproval => {
                unreachable!("delegation rejection operation returned delegation approval outcome")
            }
            OperationOutcome::Prompt(_) => {
                unreachable!("delegation rejection operation returned prompt outcome")
            }
            OperationOutcome::ManualCompaction(_) => {
                unreachable!("delegation rejection operation returned manual compaction outcome")
            }
            OperationOutcome::PluginLoad(_) => {
                unreachable!("delegation rejection operation returned plugin load outcome")
            }
            OperationOutcome::PluginCommand(_) => {
                unreachable!("delegation rejection operation returned plugin command outcome")
            }
            OperationOutcome::BranchSummary(_) => {
                unreachable!("delegation rejection operation returned branch summary outcome")
            }
            OperationOutcome::SelfHealingEdit(_) => {
                unreachable!("delegation rejection operation returned self-healing edit outcome")
            }
            OperationOutcome::AgentInvocation(_) => {
                unreachable!("delegation rejection operation returned agent invocation outcome")
            }
            OperationOutcome::AgentTeam(_) => {
                unreachable!("delegation rejection operation returned agent team outcome")
            }
            OperationOutcome::Export(_) => {
                unreachable!("delegation rejection operation returned export outcome")
            }
            OperationOutcome::ForkSession | OperationOutcome::SwitchActiveLeaf => {
                unreachable!("delegation rejection operation returned navigation outcome")
            }
            OperationOutcome::SetDefaultAgentProfile => {
                unreachable!(
                    "delegation rejection operation returned set default agent profile outcome"
                )
            }
        }
    }

    #[deprecated(note = "use CodingAgentSession::run instead")]
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
            OperationOutcome::DelegationApproval => {
                unreachable!("prompt operation returned delegation approval outcome")
            }
            OperationOutcome::DelegationRejection => {
                unreachable!("prompt operation returned delegation rejection outcome")
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
            OperationOutcome::ForkSession | OperationOutcome::SwitchActiveLeaf => {
                unreachable!("prompt operation returned navigation outcome")
            }
            OperationOutcome::SetDefaultAgentProfile => {
                unreachable!("prompt operation returned set default agent profile outcome")
            }
        }
    }

    #[deprecated(note = "use CodingAgentSession::run instead")]
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
            OperationOutcome::DelegationApproval => {
                unreachable!("manual compaction operation returned delegation approval outcome")
            }
            OperationOutcome::DelegationRejection => {
                unreachable!("manual compaction operation returned delegation rejection outcome")
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
            OperationOutcome::ForkSession | OperationOutcome::SwitchActiveLeaf => {
                unreachable!("manual compaction operation returned navigation outcome")
            }
            OperationOutcome::SetDefaultAgentProfile => {
                unreachable!(
                    "manual compaction operation returned set default agent profile outcome"
                )
            }
        }
    }

    #[allow(deprecated)]
    pub async fn self_healing_edit(
        &mut self,
        path: impl Into<String>,
        replacements: Vec<SelfHealingEditReplacement>,
    ) -> Result<SelfHealingEditOutcome, CodingSessionError> {
        self.self_healing_edit_with_options(SelfHealingEditRequest::new(path, replacements))
            .await
    }

    #[deprecated(note = "use CodingAgentSession::run instead")]
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
            OperationOutcome::DelegationApproval => {
                unreachable!("self-healing edit operation returned delegation approval outcome")
            }
            OperationOutcome::DelegationRejection => {
                unreachable!("self-healing edit operation returned delegation rejection outcome")
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
            OperationOutcome::ForkSession | OperationOutcome::SwitchActiveLeaf => {
                unreachable!("self-healing edit operation returned navigation outcome")
            }
            OperationOutcome::SetDefaultAgentProfile => {
                unreachable!(
                    "self-healing edit operation returned set default agent profile outcome"
                )
            }
        }
    }

    #[deprecated(note = "use CodingAgentSession::run instead")]
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
            OperationOutcome::DelegationApproval => {
                unreachable!("agent invocation operation returned delegation approval outcome")
            }
            OperationOutcome::DelegationRejection => {
                unreachable!("agent invocation operation returned delegation rejection outcome")
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
            OperationOutcome::ForkSession | OperationOutcome::SwitchActiveLeaf => {
                unreachable!("agent invocation operation returned navigation outcome")
            }
            OperationOutcome::SetDefaultAgentProfile => {
                unreachable!(
                    "agent invocation operation returned set default agent profile outcome"
                )
            }
        }
    }

    #[deprecated(note = "use CodingAgentSession::run instead")]
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
            OperationOutcome::DelegationApproval => {
                unreachable!("agent team operation returned delegation approval outcome")
            }
            OperationOutcome::DelegationRejection => {
                unreachable!("agent team operation returned delegation rejection outcome")
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
            OperationOutcome::ForkSession | OperationOutcome::SwitchActiveLeaf => {
                unreachable!("agent team operation returned navigation outcome")
            }
            OperationOutcome::SetDefaultAgentProfile => {
                unreachable!("agent team operation returned set default agent profile outcome")
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
            OperationOutcome::DelegationApproval => {
                unreachable!("plugin command operation returned delegation approval outcome")
            }
            OperationOutcome::DelegationRejection => {
                unreachable!("plugin command operation returned delegation rejection outcome")
            }
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
            OperationOutcome::ForkSession | OperationOutcome::SwitchActiveLeaf => {
                unreachable!("plugin command operation returned navigation outcome")
            }
            OperationOutcome::SetDefaultAgentProfile => {
                unreachable!("plugin command operation returned set default agent profile outcome")
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
            OperationOutcome::DelegationApproval => {
                unreachable!("plugin load operation returned delegation approval outcome")
            }
            OperationOutcome::DelegationRejection => {
                unreachable!("plugin load operation returned delegation rejection outcome")
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
            OperationOutcome::ForkSession | OperationOutcome::SwitchActiveLeaf => {
                unreachable!("plugin load operation returned navigation outcome")
            }
            OperationOutcome::SetDefaultAgentProfile => {
                unreachable!("plugin load operation returned set default agent profile outcome")
            }
        }
    }

    #[deprecated(note = "use CodingAgentSession::run instead")]
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
                reuse_existing: false,
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
            OperationOutcome::DelegationApproval => {
                unreachable!("branch summary operation returned delegation approval outcome")
            }
            OperationOutcome::DelegationRejection => {
                unreachable!("branch summary operation returned delegation rejection outcome")
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
            OperationOutcome::ForkSession | OperationOutcome::SwitchActiveLeaf => {
                unreachable!("branch summary operation returned navigation outcome")
            }
            OperationOutcome::SetDefaultAgentProfile => {
                unreachable!("branch summary operation returned set default agent profile outcome")
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
        let operation = Operation::BranchSummary {
            options,
            source_leaf_id,
            target_leaf_id,
            custom_instructions: None,
            reuse_existing: true,
        };
        let admission = self.resolve_operation_admission(&operation)?;
        let operation_permit = IntentRouter::admit_operation(
            &self.operation_control,
            &admission,
            OperationDispatchMode::Async,
        )?;
        let snapshot = operation_permit.capability_snapshot().clone();
        let Operation::BranchSummary {
            options,
            source_leaf_id,
            target_leaf_id,
            custom_instructions,
            reuse_existing: _,
        } = operation
        else {
            unreachable!("navigation branch summary built a non-branch-summary operation")
        };
        if let Some(outcome) = self.branch_summary_service.reused_outcome(
            &self.persistence,
            &options,
            source_leaf_id.as_str(),
            target_leaf_id.as_str(),
            &snapshot,
        )? {
            drop(operation_permit);
            return Ok(outcome);
        }

        let outcome = self
            .run_branch_summary_admitted(
                options,
                source_leaf_id,
                target_leaf_id,
                custom_instructions,
                &snapshot,
            )
            .await;
        drop(operation_permit);
        outcome
    }

    fn from_services(
        session_service: SessionService,
        default_plugin_load_options: PluginLoadOptions,
        profile_registry: ProfileRegistry,
    ) -> Result<Self, CodingSessionError> {
        let mut session_service = session_service;
        let replay_state = replay_derived_owner_state(&mut session_service)?;
        let event_service = EventService::new();
        event_service.emit_session_opened(session_service.session_id().to_owned());

        let session = Self {
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
            pending_delegation_confirmations: replay_state.pending_delegation_confirmations,
            branch_summary_service: BranchSummaryService::new(),
            delegation_confirmation_service: DelegationConfirmationService::new(),
            delegation_execution_service: DelegationExecutionService::new(),
            manual_compaction_service: ManualCompactionService::new(),
            self_healing_edit_service: SelfHealingEditService::new(),
            capability_snapshots: CapabilitySnapshotService::new(),
            startup_recovery_markers: Mutex::new(replay_state.startup_recovery_markers),
        };

        Ok(session)
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
            capability_snapshots: CapabilitySnapshotService::new(),
            startup_recovery_markers: Mutex::new(Vec::new()),
        })
    }

    fn run_sync_operation(
        &self,
        operation: Operation,
    ) -> Result<OperationOutcome, CodingSessionError> {
        let admission = self.resolve_operation_admission(&operation)?;
        let operation_permit = IntentRouter::admit_operation(
            &self.operation_control,
            &admission,
            OperationDispatchMode::SyncReadOnly,
        )?;

        match operation {
            Operation::Export(options) => self
                .export_current_inner(options, operation_permit.capability_snapshot())
                .map(OperationOutcome::Export),
            Operation::PluginCommand { command_id, args } => self
                .plugin_service
                .run_command_with_capabilities(
                    &command_id,
                    args,
                    &operation_permit.capability_snapshot().plugin,
                )
                .map(OperationOutcome::PluginCommand),
            Operation::RejectDelegationConfirmation { .. } => {
                Err(IntentRouter::unsupported_dispatch(&admission))
            }
            Operation::Prompt(_)
            | Operation::ManualCompaction(_)
            | Operation::PluginLoad(_)
            | Operation::ApproveDelegationConfirmation { .. }
            | Operation::BranchSummary { .. }
            | Operation::SelfHealingEdit(_)
            | Operation::AgentInvocation(_)
            | Operation::AgentTeam(_)
            | Operation::ForkSession { .. }
            | Operation::SwitchActiveLeaf { .. }
            | Operation::SetDefaultAgentProfile { .. } => {
                Err(IntentRouter::unsupported_dispatch(&admission))
            }
        }
    }

    fn run_sync_mut_operation(
        &mut self,
        operation: Operation,
    ) -> Result<OperationOutcome, CodingSessionError> {
        let admission = self.resolve_operation_admission(&operation)?;
        let operation_permit = IntentRouter::admit_operation(
            &self.operation_control,
            &admission,
            OperationDispatchMode::SyncMutable,
        )?;

        match operation {
            Operation::RejectDelegationConfirmation {
                operation_id,
                tool_call_id,
                reason,
            } => {
                let now = SystemClock.now_rfc3339();
                self.delegation_confirmation_service.reject_pending(
                    &mut self.persistence,
                    &mut self.pending_delegation_confirmations,
                    &self.event_service,
                    operation_id.as_str(),
                    tool_call_id.as_str(),
                    &now,
                    reason,
                )?;
                Ok(OperationOutcome::DelegationRejection)
            }
            Operation::ForkSession { target_leaf_id } => {
                let operation_id = operation_permit.capability_snapshot().operation_id.clone();
                let SessionPersistence::Persistent(session_service) = &self.persistence else {
                    return Err(CodingSessionError::UnsupportedCapability {
                        capability: "fork requires a persistent Rust-native session".into(),
                    });
                };
                let mut forked_service = session_service
                    .fork_current_admitted(target_leaf_id.as_deref(), &operation_id)?;
                let forked_session_id = forked_service.session_id().to_owned();
                let replay_state = match replay_derived_owner_state(&mut forked_service) {
                    Ok(replay_state) => replay_state,
                    Err(error) => {
                        return Err(forked_service.cleanup_failed_transition(&operation_id, error));
                    }
                };
                drop(operation_permit);
                self.persistence = SessionPersistence::Persistent(forked_service);
                self.pending_delegation_confirmations =
                    replay_state.pending_delegation_confirmations;
                *self.startup_recovery_markers.lock().unwrap() =
                    replay_state.startup_recovery_markers;
                self.event_service.emit_session_opened(forked_session_id);
                Ok(OperationOutcome::ForkSession)
            }
            Operation::SwitchActiveLeaf { target_leaf_id } => {
                let SessionPersistence::Persistent(session_service) = &mut self.persistence else {
                    return Err(CodingSessionError::UnsupportedCapability {
                        capability:
                            "active leaf navigation requires a persistent Rust-native session"
                                .into(),
                    });
                };
                session_service.switch_active_leaf(
                    &target_leaf_id,
                    &operation_permit.capability_snapshot().operation_id,
                )?;
                Ok(OperationOutcome::SwitchActiveLeaf)
            }
            Operation::SetDefaultAgentProfile { profile_id } => {
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
                let installed = self
                    .capability_snapshots
                    .install_next_generation(CapabilityRevocationPolicy::FutureOnly);
                self.event_service.emit_capability_changed(installed);
                Ok(OperationOutcome::SetDefaultAgentProfile)
            }
            Operation::Export(_) | Operation::PluginCommand { .. } => {
                Err(IntentRouter::unsupported_dispatch(&admission))
            }
            Operation::Prompt(_)
            | Operation::ManualCompaction(_)
            | Operation::PluginLoad(_)
            | Operation::ApproveDelegationConfirmation { .. }
            | Operation::BranchSummary { .. }
            | Operation::SelfHealingEdit(_)
            | Operation::AgentInvocation(_)
            | Operation::AgentTeam(_) => Err(IntentRouter::unsupported_dispatch(&admission)),
        }
    }

    fn export_current_inner(
        &self,
        options: ExportOptions,
        snapshot: &OperationCapabilitySnapshot,
    ) -> Result<export_flow::ExportOutcome, CodingSessionError> {
        SessionReadCapability::require(snapshot.session_read.as_ref())?;
        let SessionPersistence::Persistent(session_service) = &self.persistence else {
            return Err(CodingSessionError::UnsupportedCapability {
                capability: "export requires a persistent Rust-native session".into(),
            });
        };
        let mut context = session_service.export_context(options)?;
        self.flow_service.run_export(&mut context)
    }

    fn resolve_operation_admission(
        &self,
        operation: &Operation,
    ) -> Result<OperationAdmission, CodingSessionError> {
        let metadata = operation.metadata();
        let (kind, admitted_at) = match operation {
            Operation::ApproveDelegationConfirmation {
                operation_id,
                tool_call_id,
            } => {
                let now = SystemClock.now_rfc3339();
                let kind = self.delegation_approval_operation_kind(
                    operation_id.as_str(),
                    tool_call_id.as_str(),
                    &now,
                )?;
                (kind, Some(now))
            }
            _ => (
                operation.static_kind().ok_or_else(|| {
                    CodingSessionError::UnsupportedCapability {
                        capability: "dynamic operation requires async dispatcher".into(),
                    }
                })?,
                None,
            ),
        };
        let operation_id = self.next_operation_admission_id(operation);
        let snapshot = self
            .capability_snapshots
            .snapshot(self.snapshot_input_for_operation(operation_id, kind, operation));
        Ok(OperationAdmission::new(
            kind,
            metadata,
            admitted_at,
            snapshot,
        ))
    }

    fn next_operation_admission_id(&self, _operation: &Operation) -> String {
        let mut ids = SystemIdGenerator;
        ids.next_operation_id()
    }

    fn snapshot_input_for_operation(
        &self,
        operation_id: String,
        kind: OperationKind,
        operation: &Operation,
    ) -> CapabilitySnapshotInput {
        let plugin_capabilities = self.plugin_service.capabilities();
        let default_profile_id = self.default_agent_profile_id();
        let runtime_tools = self.operation_runtime_tool_names(operation);
        let profile_tools = match self.active_agent_profile() {
            Some(profile) if !profile.tools.is_empty() => profile.tools.clone(),
            _ => runtime_tools.clone(),
        };
        CapabilitySnapshotInput {
            operation_id,
            operation_kind: kind,
            actor: ActorId::Client,
            default_profile_id,
            plugin_capabilities,
            persistent_session: matches!(self.persistence, SessionPersistence::Persistent(_)),
            cwd: self.cwd(),
            runtime_tools,
            profile_tools,
        }
    }

    fn operation_runtime_tool_names(&self, operation: &Operation) -> Vec<String> {
        let mut names = self.current_runtime_tool_names();
        let options = match operation {
            Operation::Prompt(options)
            | Operation::ManualCompaction(options)
            | Operation::BranchSummary { options, .. } => Some(options),
            _ => None,
        };
        if let Some(options) = options
            && let Some(runtime) = options.runtime()
        {
            names.extend(runtime.tools().iter().map(|tool| tool.name.clone()));
        }
        names.extend(
            self.plugin_service
                .collect_tools()
                .into_iter()
                .map(|tool| tool.name),
        );
        if let Some(profile) = self.active_agent_profile() {
            names.extend(
                delegation::delegation_tools(Some(&profile.id), Some(&profile.delegation))
                    .into_iter()
                    .map(|tool| tool.name),
            );
        }
        names.sort();
        names.dedup();
        names
    }

    fn cwd(&self) -> Option<PathBuf> {
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => session_cwd(session_service),
            SessionPersistence::NonPersistent(_) => None,
        }
    }

    fn active_agent_profile(&self) -> Option<&AgentProfile> {
        let id = self.default_agent_profile_id();
        self.profile_registry.agent(id.as_str())
    }

    fn current_runtime_tool_names(&self) -> Vec<String> {
        vec![
            "read".into(),
            "write".into(),
            "edit".into(),
            "bash".into(),
            "grep".into(),
            "find".into(),
            "ls".into(),
        ]
    }

    fn delegation_approval_operation_kind(
        &self,
        operation_id: &str,
        tool_call_id: &str,
        now: &str,
    ) -> Result<OperationKind, CodingSessionError> {
        let pending = self.delegation_confirmation_service.active_pending(
            &self.pending_delegation_confirmations,
            operation_id,
            tool_call_id,
            now,
        )?;
        Ok(match pending.request.target_kind {
            ProfileKind::Agent => OperationKind::AgentInvocation,
            ProfileKind::Team => OperationKind::AgentTeam,
        })
    }

    async fn approve_delegation_confirmation_inner(
        &mut self,
        operation_id: String,
        tool_call_id: String,
        now: String,
    ) -> Result<(), CodingSessionError> {
        let mut ids = SystemIdGenerator;
        let pending = self.delegation_confirmation_service.approve_pending(
            &mut self.persistence,
            &mut self.pending_delegation_confirmations,
            &self.event_service,
            operation_id.as_str(),
            tool_call_id.as_str(),
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

    async fn run_operation(
        &mut self,
        operation: Operation,
    ) -> Result<OperationOutcome, CodingSessionError> {
        let admission = self.resolve_operation_admission(&operation)?;
        let operation_permit = IntentRouter::admit_operation(
            &self.operation_control,
            &admission,
            OperationDispatchMode::Async,
        )?;
        let snapshot = operation_permit.capability_snapshot().clone();

        match operation {
            Operation::Prompt(options) => {
                let result = self.prompt_inner(options, &snapshot).await;
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
                        &snapshot,
                    )
                    .await
                    .map(OperationOutcome::ManualCompaction)
            }
            Operation::PluginLoad(options) => self
                .load_plugins_inner(options, &snapshot)
                .await
                .map(OperationOutcome::PluginLoad),
            Operation::BranchSummary {
                options,
                source_leaf_id,
                target_leaf_id,
                custom_instructions,
                reuse_existing,
            } => {
                if reuse_existing
                    && let Some(outcome) = self.branch_summary_service.reused_outcome(
                        &self.persistence,
                        &options,
                        source_leaf_id.as_str(),
                        target_leaf_id.as_str(),
                        operation_permit.capability_snapshot(),
                    )?
                {
                    return Ok(OperationOutcome::BranchSummary(outcome));
                }
                self.run_branch_summary_admitted(
                    options,
                    source_leaf_id,
                    target_leaf_id,
                    custom_instructions,
                    &snapshot,
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
                        &snapshot,
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
            Operation::Export(_)
            | Operation::PluginCommand { .. }
            | Operation::RejectDelegationConfirmation { .. }
            | Operation::ForkSession { .. }
            | Operation::SwitchActiveLeaf { .. }
            | Operation::SetDefaultAgentProfile { .. } => {
                Err(IntentRouter::unsupported_dispatch(&admission))
            }
            Operation::ApproveDelegationConfirmation {
                operation_id,
                tool_call_id,
            } => self
                .approve_delegation_confirmation_inner(
                    operation_id,
                    tool_call_id,
                    admission
                        .admitted_at
                        .expect("delegation approval admission time is resolved"),
                )
                .await
                .map(|_| OperationOutcome::DelegationApproval),
        }
    }

    async fn run_branch_summary_admitted(
        &mut self,
        options: PromptTurnOptions,
        source_leaf_id: String,
        target_leaf_id: String,
        custom_instructions: Option<String>,
        snapshot: &OperationCapabilitySnapshot,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
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
                snapshot,
            )
            .await
    }

    #[allow(dead_code)]
    async fn load_plugins_inner(
        &mut self,
        options: PluginLoadOptions,
        snapshot: &OperationCapabilitySnapshot,
    ) -> Result<PluginLoadOutcome, CodingSessionError> {
        let execution = self
            .plugin_load_service
            .load(
                &mut self.persistence,
                &self.flow_service,
                &self.event_service,
                options,
                snapshot,
            )
            .await?;
        if let Some(plugin_service) = execution.loaded_plugin_service {
            self.plugin_service = plugin_service;
        }
        if execution.outcome.capability_changed {
            let installed = self
                .capability_snapshots
                .install_next_generation(CapabilityRevocationPolicy::FutureOnly);
            self.event_service.emit_capability_changed(installed);
        }
        Ok(execution.outcome)
    }

    async fn prompt_inner(
        &mut self,
        options: PromptTurnOptions,
        snapshot: &OperationCapabilitySnapshot,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        if options.runtime().is_none() {
            return Err(CodingSessionError::Config {
                message: "prompt turn options do not include a runtime snapshot".into(),
            });
        }
        let options = self.apply_default_agent_profile(options)?;
        let mut context = self.prepare_prompt_context(options, snapshot)?;
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
        snapshot: &OperationCapabilitySnapshot,
    ) -> Result<PromptTurnContext, CodingSessionError> {
        let event_service = self.event_service.clone();
        let prompt_control_receiver = self.operation_control.take_prompt_control_receiver();
        match &mut self.persistence {
            SessionPersistence::Persistent(session_service) => {
                let replay = session_service.replay()?;
                let transaction = session_service.begin_prompt_transaction_with_snapshot(snapshot);
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
                context.set_capability_snapshot(snapshot.clone());
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
                context.set_capability_snapshot(snapshot.clone());
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
                let snapshot = context.capability_snapshot().ok_or_else(|| {
                    CodingSessionError::UnsupportedCapability {
                        capability: "prompt session write requires operation capability snapshot"
                            .into(),
                    }
                })?;
                SessionWriteCapability::require(snapshot.session_write.as_ref())?;
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

    #[cfg(test)]
    fn current_capability_generation_for_tests(&self) -> capability_snapshot::CapabilityGeneration {
        self.capability_snapshots.current_generation()
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
#[allow(deprecated)]
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
    use super::prompt::DelegationRequest;
    use super::*;
    use crate::coding_session::session_log::event::{
        PersistedContentBlock, SessionEventData, SessionEventEnvelope,
    };
    use crate::coding_session::session_log::replay::{MessageStatus, TranscriptItem};
    use crate::coding_session::session_log::store::StoreFailurePoint;
    use crate::plugins::{
        CommandDefinition, CommandProvider, CommandRegistrationHost, PluginError, PluginId,
        PluginMetadata, PluginRegistry, PluginSource, ToolProvider, ToolRegistrationHost,
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

    fn pending_delegation_confirmation_state(
        target_kind: ProfileKind,
    ) -> PendingDelegationConfirmationState {
        PendingDelegationConfirmationState {
            request: DelegationRequest {
                operation_id: "op_parent".into(),
                turn_id: "turn_parent".into(),
                tool_call_id: "tool_delegate".into(),
                requesting_profile_id: ProfileId::from("parent"),
                target_kind,
                target_id: ProfileId::from("target"),
                task: "delegate this".into(),
            },
            prompt_options: PromptTurnOptions::new(PromptInvocation::Text("delegated task".into())),
            reason: "requires confirmation".into(),
            requested_at: SystemClock.now_rfc3339(),
            child_delegation_depth: 1,
            delegation_lineage: Vec::new(),
        }
    }

    fn queue_persistent_delegation_confirmation(
        session: &mut CodingAgentSession,
        operation_id: &str,
        tool_call_id: &str,
        target_kind: ProfileKind,
    ) {
        let mut pending = pending_delegation_confirmation_state(target_kind);
        pending.request.operation_id = operation_id.into();
        pending.request.tool_call_id = tool_call_id.into();
        pending.request.target_id = ProfileId::from("default");
        pending.prompt_options = prompt_options(
            "coding-session-canonical-delegation-decision",
            "delegated task",
        );
        session
            .delegation_confirmation_service
            .queue_pending(
                &mut session.persistence,
                &mut session.pending_delegation_confirmations,
                &session.event_service,
                pending,
                true,
            )
            .unwrap();
    }

    #[tokio::test]
    async fn interactive_store_and_pending_delegation_bridge_arms_real_fixtures() {
        let temp = tempfile::tempdir().unwrap();

        let append_options = CodingAgentSessionOptions::new()
            .with_session_id("sess_interactive_append_bridge")
            .with_session_log_root(temp.path());
        let mut append_session = CodingAgentSession::create(append_options).await.unwrap();
        append_session.arm_append_events_failure_for_tests(0);
        let append_error = append_session
            .run(CodingAgentOperation::SetDefaultAgentProfile {
                profile_id: ProfileId::from("default"),
            })
            .await
            .unwrap_err();
        assert_eq!(append_error.code(), "session");

        let manifest_options = CodingAgentSessionOptions::new()
            .with_session_id("sess_interactive_manifest_bridge")
            .with_session_log_root(temp.path());
        let mut manifest_session = CodingAgentSession::create(manifest_options).await.unwrap();
        manifest_session.arm_update_manifest_failure_for_tests(0);
        let manifest_error = manifest_session
            .run(CodingAgentOperation::SetDefaultAgentProfile {
                profile_id: ProfileId::from("default"),
            })
            .await
            .unwrap_err();
        assert_eq!(manifest_error.code(), "partial_commit");

        let pending_options = CodingAgentSessionOptions::new()
            .with_session_id("sess_interactive_pending_bridge")
            .with_session_log_root(temp.path());
        let mut pending_session = CodingAgentSession::create(pending_options.clone())
            .await
            .unwrap();
        pending_session.queue_pending_delegation_for_tests("op_pending", "tool_pending");
        let pending = pending_session.pending_delegation_confirmations();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].operation_id, "op_pending");
        assert_eq!(pending[0].tool_call_id, "tool_pending");

        let reopened = CodingAgentSession::open(pending_options).await.unwrap();
        let reopened_pending = reopened.pending_delegation_confirmations();
        assert_eq!(reopened_pending.len(), 1);
        assert_eq!(reopened_pending[0].operation_id, "op_pending");
        assert_eq!(reopened_pending[0].tool_call_id, "tool_pending");
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
    async fn ui_snapshot_uses_session_view_capabilities_and_event_cursor() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let snapshot = session.ui_snapshot(Vec::new());

        assert_eq!(snapshot.session, session.view());
        assert_eq!(snapshot.capabilities, session.capabilities());
        assert_eq!(
            snapshot.cursor.last_event_sequence,
            session.event_service.current_product_sequence()
        );
        assert_eq!(
            snapshot.cursor.capability_generation,
            session.current_capability_generation_for_tests()
        );
        assert_eq!(snapshot.active_operation, None);
    }

    #[tokio::test]
    async fn connect_client_returns_connection_and_initial_snapshot() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();

        let (connection, snapshot) = session.connect_client(
            ClientConnectionId::new("rpc-primary"),
            vec![ClientDraft::new(ClientDraftKind::Prompt, "hello")],
        );

        assert_eq!(connection.id.as_str(), "rpc-primary");
        assert_eq!(connection.cursor, snapshot.cursor);
        assert_eq!(connection.client_drafts.len(), 1);
    }

    #[tokio::test]
    async fn startup_recovery_product_event_is_visible_to_first_subscriber() {
        let temp = tempfile::tempdir().unwrap();
        let store = session_log::store::SessionLogStore::new(temp.path());
        let handle = store
            .create_session(session_log::store::CreateSessionOptions::new(
                "sess_startup_recovery_projection",
                "2026-07-09T00:00:00Z",
            ))
            .unwrap();
        let started = SessionEventEnvelope::new(
            "sess_startup_recovery_projection",
            "evt_started",
            "2026-07-09T00:00:01Z",
            SessionEventData::OperationStarted {
                operation: crate::coding_session::session_log::event::OperationKind::Prompt,
                runtime_generation: Default::default(),
            },
        )
        .with_operation_id("op_in_doubt");
        store.append_events(&handle, &[started]).unwrap();

        let session = CodingAgentSession::open(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_startup_recovery_projection")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let mut receiver = session.subscribe_product_events();

        let event = receiver
            .try_recv()
            .unwrap()
            .expect("startup recovery should be projected after subscription");
        assert!(matches!(
            event.compatibility_event(),
            CodingAgentEvent::OperationRecovered { operation_id, .. }
                if operation_id == "op_in_doubt"
        ));
    }

    #[tokio::test]
    async fn public_product_event_receiver_maps_internal_product_events() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let mut receiver = session.subscribe_product_events_public();
        session.emit_product_event_for_tests(CodingAgentEvent::Diagnostic {
            operation_id: None,
            message: "public event".into(),
        });

        let event = receiver.recv().await.unwrap();
        assert_eq!(event.sequence, 1);
        assert_eq!(event.family, "Diagnostic");
        assert_eq!(event.kind, "Diagnostic(Diagnostic)");
    }

    #[tokio::test]
    async fn public_product_event_receiver_supports_non_blocking_receive() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let mut receiver = session.subscribe_product_events_public();

        assert_eq!(receiver.try_recv().unwrap(), None);

        session.emit_product_event_for_tests(CodingAgentEvent::Diagnostic {
            operation_id: None,
            message: "public event".into(),
        });
        let event = receiver
            .try_recv()
            .unwrap()
            .expect("emitted event should be available without blocking");
        assert_eq!(event.sequence, 1);
        assert_eq!(event.family, "Diagnostic");
        assert_eq!(event.kind, "Diagnostic(Diagnostic)");
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

    struct SessionPluginCommandProvider;

    impl CommandProvider for SessionPluginCommandProvider {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new(
                PluginId::new("session-plugin-command"),
                "Session Plugin Command",
                "1.0.0",
                PluginSource::FirstParty,
            )
        }

        fn commands(
            &self,
            _host: &CommandRegistrationHost,
        ) -> Result<Vec<CommandDefinition>, PluginError> {
            Ok(vec![CommandDefinition::new(
                "plugin.say_hello",
                "greets from session plugin",
            )])
        }

        fn run_command(
            &self,
            command_id: &str,
            _args: serde_json::Value,
        ) -> Result<String, PluginError> {
            assert_eq!(command_id, "plugin.say_hello");
            Ok("hello".to_owned())
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
                .any(|event| matches!(event, CodingAgentEvent::CapabilityChanged { .. }))
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
                .any(|event| matches!(event, CodingAgentEvent::CapabilityChanged { .. }))
        );
    }

    #[tokio::test]
    async fn set_default_profile_installs_future_capability_generation() {
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_generation_profile")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let first = session.current_capability_generation_for_tests();

        session.set_default_agent_profile_id("reviewer").unwrap();
        let second = session.current_capability_generation_for_tests();

        assert_eq!(first.get() + 1, second.get());
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
    async fn run_sync_operation_export_preserves_persistence_error_without_active_operation() {
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
    async fn run_sync_operation_export_uses_read_only_admission_while_root_busy() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let _guard = session
            .operation_control
            .begin(OperationKind::Prompt)
            .unwrap();

        let error = session
            .run_sync_operation(Operation::Export(ExportOptions::view()))
            .unwrap_err();

        assert_eq!(error.code(), "unsupported_capability");
        assert_eq!(
            error.to_string(),
            "unsupported capability: export requires a persistent Rust-native session"
        );
        assert_eq!(
            session.operation_control.active(),
            Some(OperationKind::Prompt)
        );
    }

    #[tokio::test]
    async fn canonical_run_uses_each_metadata_dispatch_family() {
        let api = "coding-session-canonical-dispatch-families";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("async answer", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_canonical_dispatch_families")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();

        let async_metadata = CodingAgentOperation::Prompt(prompt_options(api, "async prompt"))
            .into_internal(PluginLoadOptions::new())
            .metadata();
        assert_eq!(
            async_metadata.dispatch_mode,
            operation::OperationDispatchMode::Async
        );
        let async_outcome = session
            .run(CodingAgentOperation::Prompt(prompt_options(
                api,
                "async prompt",
            )))
            .await
            .unwrap();
        assert!(matches!(
            async_outcome,
            CodingAgentOperationOutcome::Prompt(PromptTurnOutcome::Success { .. })
        ));

        let read_only_metadata = CodingAgentOperation::ExportCurrent
            .into_internal(PluginLoadOptions::new())
            .metadata();
        assert_eq!(
            read_only_metadata.dispatch_mode,
            operation::OperationDispatchMode::SyncReadOnly
        );
        let read_only_outcome = session
            .run(CodingAgentOperation::ExportCurrent)
            .await
            .unwrap();
        assert!(matches!(
            read_only_outcome,
            CodingAgentOperationOutcome::Export(_)
        ));

        let sync_mut_metadata = CodingAgentOperation::SetDefaultAgentProfile {
            profile_id: ProfileId::from("reviewer"),
        }
        .into_internal(PluginLoadOptions::new())
        .metadata();
        assert_eq!(
            sync_mut_metadata.dispatch_mode,
            operation::OperationDispatchMode::SyncMutable
        );
        let sync_mut_outcome = session
            .run(CodingAgentOperation::SetDefaultAgentProfile {
                profile_id: ProfileId::from("reviewer"),
            })
            .await
            .unwrap();
        assert!(matches!(
            sync_mut_outcome,
            CodingAgentOperationOutcome::DefaultAgentProfileChanged
        ));
        assert_eq!(session.default_agent_profile_id().as_str(), "reviewer");
    }

    #[tokio::test]
    async fn set_default_agent_profile_rejects_while_operation_is_busy() {
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let _guard = session
            .operation_control
            .begin(OperationKind::Prompt)
            .unwrap();

        let error = session
            .set_default_agent_profile_id("agent-main")
            .unwrap_err();

        assert_eq!(error.code(), "busy");
        assert_eq!(
            session.operation_control.active(),
            Some(OperationKind::Prompt)
        );
    }

    #[tokio::test]
    async fn fork_current_session_rejects_while_operation_is_busy() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let _guard = session
            .operation_control
            .begin(OperationKind::Prompt)
            .unwrap();

        let error = session.fork_current_session(None).unwrap_err();

        assert_eq!(error.code(), "busy");
        assert_eq!(
            session.operation_control.active(),
            Some(OperationKind::Prompt)
        );
    }

    #[tokio::test]
    async fn canonical_run_switches_active_leaf() {
        let api = "coding-session-canonical-switch-active-leaf";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("root answer", StopReason::Stop),
                FauxProvider::text_call("branch answer", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_canonical_switch_active_leaf")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let target_leaf_id = match session
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
        session
            .prompt(prompt_options(api, "branch question"))
            .await
            .unwrap();

        let outcome = session
            .run(CodingAgentOperation::SwitchActiveLeaf {
                target_leaf_id: target_leaf_id.clone(),
            })
            .await
            .unwrap();

        assert!(matches!(
            outcome,
            CodingAgentOperationOutcome::ActiveLeafSwitched
        ));
        let hydrated = session.hydrate_current().unwrap().unwrap();
        assert_eq!(
            hydrated.summary.active_leaf_id.as_deref(),
            Some(target_leaf_id.as_str())
        );
        assert_eq!(
            session
                .persistent_session_service()
                .replay()
                .unwrap()
                .active_leaf_id
                .as_deref(),
            Some(target_leaf_id.as_str())
        );
    }

    #[tokio::test]
    async fn canonical_run_forks_current_session() {
        let api = "coding-session-canonical-fork-current-session";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("keep answer", StopReason::Stop),
                FauxProvider::text_call("drop answer", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_canonical_fork_source")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let target_leaf_id = match session
            .prompt(prompt_options(api, "keep prompt"))
            .await
            .unwrap()
        {
            PromptTurnOutcome::Success {
                leaf_id: Some(leaf_id),
                ..
            } => leaf_id,
            other => panic!("expected selected prompt success, got {other:?}"),
        };
        session
            .prompt(prompt_options(api, "drop prompt"))
            .await
            .unwrap();
        let original_session_id = session.persistent_session_service().session_id().to_owned();

        let outcome = session
            .run(CodingAgentOperation::ForkSession {
                target_leaf_id: Some(target_leaf_id.clone()),
            })
            .await
            .unwrap();

        assert!(matches!(
            outcome,
            CodingAgentOperationOutcome::SessionForked
        ));
        let hydrated = session.hydrate_current().unwrap().unwrap();
        assert_ne!(hydrated.summary.session_id, original_session_id);
        assert_eq!(
            hydrated.summary.active_leaf_id.as_deref(),
            Some(target_leaf_id.as_str())
        );
        assert!(hydrated.transcript.iter().any(|item| matches!(
            item,
            CodingAgentSessionTranscriptItem::User { text } if text == "keep prompt"
        )));
        assert!(!hydrated.transcript.iter().any(|item| matches!(
            item,
            CodingAgentSessionTranscriptItem::User { text } if text == "drop prompt"
        )));
        let replay = session.persistent_session_service().replay().unwrap();
        assert_eq!(
            replay.active_leaf_id.as_deref(),
            Some(target_leaf_id.as_str())
        );
        assert!(replay.transcript.iter().any(|item| matches!(
            item,
            TranscriptItem::UserInput { text, .. } if text == "keep prompt"
        )));
        assert!(!replay.transcript.iter().any(|item| matches!(
            item,
            TranscriptItem::UserInput { text, .. } if text == "drop prompt"
        )));
    }

    #[tokio::test]
    async fn canonical_fork_preserves_owner_runtime_and_event_stream() {
        let api = "coding-session-canonical-fork-owner-continuity";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("keep answer", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_canonical_fork_owner_continuity")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let target_leaf_id = match session
            .prompt(prompt_options(api, "keep prompt"))
            .await
            .unwrap()
        {
            PromptTurnOutcome::Success {
                leaf_id: Some(leaf_id),
                ..
            } => leaf_id,
            other => panic!("expected selected prompt success, got {other:?}"),
        };
        let mut registry = PluginRegistry::new();
        registry.register_command_provider(Arc::new(SessionPluginCommandProvider));
        session
            .load_plugins(
                PluginLoadOptions::new().with_candidate(PluginLoadCandidate::new(
                    PluginLoadManifest::new(
                        "session-plugin-command",
                        "Session Plugin Command",
                        "1.0.0",
                        PluginSource::FirstParty,
                    ),
                    registry,
                )),
            )
            .await
            .unwrap();
        session
            .run(CodingAgentOperation::SetDefaultAgentProfile {
                profile_id: ProfileId::from("reviewer"),
            })
            .await
            .unwrap();
        let capability_generation_before = session.current_capability_generation_for_tests();
        let mut events = session.subscribe_product_events();

        session
            .run(CodingAgentOperation::ForkSession {
                target_leaf_id: Some(target_leaf_id),
            })
            .await
            .unwrap();

        assert_eq!(
            session.current_capability_generation_for_tests(),
            capability_generation_before
        );
        let command = session
            .run(CodingAgentOperation::PluginCommand {
                command_id: "plugin.say_hello".into(),
                args: serde_json::Value::Null,
            })
            .await
            .unwrap();
        assert!(matches!(
            command,
            CodingAgentOperationOutcome::PluginCommand(output) if output == "hello"
        ));
        session
            .run(CodingAgentOperation::SetDefaultAgentProfile {
                profile_id: ProfileId::from("default"),
            })
            .await
            .unwrap();

        let emitted = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert!(
            emitted.iter().any(|event| matches!(
                event.compatibility_event(),
                CodingAgentEvent::SessionOpened { session_id }
                    if session_id == &session.view().session_id
            )),
            "pre-fork receiver should observe the forked session transition: {emitted:#?}"
        );
        assert!(
            emitted.iter().any(|event| matches!(
                event.compatibility_event(),
                CodingAgentEvent::DefaultAgentProfileChanged { profile_id }
                    if profile_id == &ProfileId::from("default")
            )),
            "pre-fork receiver should observe post-fork runtime events: {emitted:#?}"
        );
        assert!(
            emitted
                .windows(2)
                .all(|events| events[0].sequence() < events[1].sequence()),
            "product event sequence should stay monotonic across fork: {emitted:#?}"
        );
    }

    #[tokio::test]
    async fn canonical_switch_reports_partial_commit_after_durable_leaf_change() {
        let api = "coding-session-canonical-switch-partial-commit";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("root answer", StopReason::Stop),
                FauxProvider::text_call("branch answer", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_canonical_switch_partial_commit")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let target_leaf_id = match session
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
        session
            .prompt(prompt_options(api, "branch question"))
            .await
            .unwrap();
        let manifest_path = session
            .persistent_session_service()
            .session_dir()
            .join("session.json");
        let mut permissions = std::fs::metadata(&manifest_path).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&manifest_path, permissions).unwrap();

        let error = session
            .run(CodingAgentOperation::SwitchActiveLeaf {
                target_leaf_id: target_leaf_id.clone(),
            })
            .await
            .unwrap_err();

        let mut permissions = std::fs::metadata(&manifest_path).unwrap().permissions();
        permissions.set_readonly(false);
        std::fs::set_permissions(&manifest_path, permissions).unwrap();
        assert!(matches!(
            &error,
            CodingSessionError::PartialCommit { operation_id, .. }
                if operation_id.starts_with("op_")
        ));
        assert_eq!(error.code(), "partial_commit");
        assert_eq!(
            session
                .persistent_session_service()
                .replay()
                .unwrap()
                .active_leaf_id
                .as_deref(),
            Some(target_leaf_id.as_str())
        );
    }

    #[tokio::test]
    async fn run_sync_operation_plugin_command_uses_guard_and_preserves_plugin_error() {
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let mut registry = PluginRegistry::new();
        registry.register_command_provider(Arc::new(SessionPluginCommandProvider));
        session
            .load_plugins(
                PluginLoadOptions::new().with_candidate(PluginLoadCandidate::new(
                    PluginLoadManifest::new(
                        "session-plugin-command",
                        "Session Plugin Command",
                        "1.0.0",
                        PluginSource::FirstParty,
                    ),
                    registry,
                )),
            )
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
    async fn run_sync_operation_plugin_command_remains_guarded_while_root_busy() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let _guard = session
            .operation_control
            .begin(OperationKind::Prompt)
            .unwrap();
        let operation = Operation::PluginCommand {
            command_id: "missing.command".into(),
            args: serde_json::Value::Null,
        };

        let error = session.run_sync_operation(operation).unwrap_err();

        assert_eq!(
            error,
            CodingSessionError::Busy {
                operation: "prompt".into(),
            }
        );
        assert_eq!(
            session.operation_control.active(),
            Some(OperationKind::Prompt)
        );
    }

    #[tokio::test]
    async fn delegation_approval_operation_kind_uses_pending_team_target() {
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        session
            .pending_delegation_confirmations
            .push(pending_delegation_confirmation_state(ProfileKind::Team));
        let now = SystemClock.now_rfc3339();

        let kind = session
            .delegation_approval_operation_kind("op_parent", "tool_delegate", &now)
            .unwrap();

        assert_eq!(kind, OperationKind::AgentTeam);
    }

    #[tokio::test]
    async fn resolve_operation_admission_returns_structured_dynamic_contract() {
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        session
            .pending_delegation_confirmations
            .push(pending_delegation_confirmation_state(ProfileKind::Team));
        let operation = Operation::ApproveDelegationConfirmation {
            operation_id: "op_parent".into(),
            tool_call_id: "tool_delegate".into(),
        };

        let admission = session.resolve_operation_admission(&operation).unwrap();

        assert_eq!(admission.kind, OperationKind::AgentTeam);
        assert_eq!(admission.metadata.static_kind, None);
        assert_eq!(
            admission.metadata.dispatch_mode,
            operation::OperationDispatchMode::Async
        );
        assert!(admission.admitted_at.is_some());
    }

    #[tokio::test]
    async fn resolve_operation_admission_returns_structured_static_contract() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let operation = Operation::RejectDelegationConfirmation {
            operation_id: "op_parent".into(),
            tool_call_id: "tool_delegate".into(),
            reason: "not now".into(),
        };

        let admission = session.resolve_operation_admission(&operation).unwrap();

        assert_eq!(admission.kind, OperationKind::DelegationConfirmation);
        assert_eq!(
            admission.metadata.static_kind,
            Some(OperationKind::DelegationConfirmation)
        );
        assert_eq!(
            admission.metadata.dispatch_mode,
            operation::OperationDispatchMode::SyncMutable
        );
        assert_eq!(admission.admitted_at, None);
    }

    #[tokio::test]
    async fn run_operation_delegation_approval_preserves_missing_pending_before_busy() {
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let _operation = session
            .operation_control
            .begin(OperationKind::Prompt)
            .unwrap();
        let operation = Operation::ApproveDelegationConfirmation {
            operation_id: "missing_op".into(),
            tool_call_id: "missing_tool".into(),
        };

        let error = session.run_operation(operation).await.unwrap_err();

        assert_eq!(error.code(), "input");
        assert!(
            error
                .to_string()
                .contains("pending delegation confirmation not found"),
            "{error}"
        );
        assert_eq!(
            session.operation_control.active(),
            Some(OperationKind::Prompt)
        );
    }

    #[tokio::test]
    async fn reject_delegation_confirmation_reports_busy_before_mutating_pending_confirmation() {
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        session
            .pending_delegation_confirmations
            .push(pending_delegation_confirmation_state(ProfileKind::Agent));
        let _operation = session
            .operation_control
            .begin(OperationKind::Prompt)
            .unwrap();

        let error = session
            .reject_delegation_confirmation("op_parent", "tool_delegate", "not now")
            .unwrap_err();

        assert_eq!(
            error,
            CodingSessionError::Busy {
                operation: "prompt".into(),
            }
        );
        assert_eq!(session.pending_delegation_confirmations().len(), 1);
        assert_eq!(
            session.operation_control.active(),
            Some(OperationKind::Prompt)
        );
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
            reuse_existing: false,
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
    async fn canonical_run_reuses_branch_summary_when_requested() {
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
        let event_log_before = std::fs::read(&event_log_path).unwrap();
        let event_count_before = event_log_before.split(|byte| *byte == b'\n').count();
        let event_log_text_before = String::from_utf8(event_log_before.clone()).unwrap();
        let summary_count_before = event_log_text_before
            .matches("branch.summary.created")
            .count();
        let mut events = session.subscribe_product_events_public();

        let outcome = session
            .run(CodingAgentOperation::BranchSummary {
                options: prompt_options(api, ""),
                source_leaf_id: branch_leaf.clone(),
                target_leaf_id: root_leaf.clone(),
                custom_instructions: None,
                reuse: BranchSummaryReusePolicy::ReuseExisting,
            })
            .await
            .unwrap();

        assert!(matches!(
            &outcome,
            CodingAgentOperationOutcome::BranchSummary(PromptTurnOutcome::Success {
                final_text,
                session_id: Some(session_id),
                leaf_id: Some(active_leaf),
                ..
            }) if final_text.contains("model branch summary")
                && session_id == "sess_branch_summary_navigation_reuse"
                && active_leaf.as_str() == branch_leaf.as_str()
        ));
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert!(emitted_events.is_empty(), "{emitted_events:#?}");
        let event_log_after = std::fs::read(&event_log_path).unwrap();
        assert_eq!(event_log_after, event_log_before);
        assert_eq!(
            event_log_after.split(|byte| *byte == b'\n').count(),
            event_count_before
        );
        assert_eq!(summary_count_before, 1);
        assert_eq!(
            String::from_utf8(event_log_after)
                .unwrap()
                .matches("branch.summary.created")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn canonical_run_preserves_navigation_and_branch_summary_durability() {
        let api = "coding-session-canonical-navigation-durability";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("root answer", StopReason::Stop),
                FauxProvider::text_call("branch answer", StopReason::Stop),
                FauxProvider::text_call("durable branch summary", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let source_options = CodingAgentSessionOptions::new()
            .with_session_id("sess_canonical_navigation_durability")
            .with_session_log_root(temp.path());
        let mut session = CodingAgentSession::create(source_options.clone())
            .await
            .unwrap();
        let root_leaf = match session
            .run(CodingAgentOperation::Prompt(prompt_options(
                api,
                "root question",
            )))
            .await
            .unwrap()
        {
            CodingAgentOperationOutcome::Prompt(PromptTurnOutcome::Success {
                leaf_id: Some(leaf_id),
                ..
            }) => leaf_id,
            other => panic!("expected root prompt success, got {other:?}"),
        };
        let branch_leaf = match session
            .run(CodingAgentOperation::Prompt(prompt_options(
                api,
                "branch question",
            )))
            .await
            .unwrap()
        {
            CodingAgentOperationOutcome::Prompt(PromptTurnOutcome::Success {
                leaf_id: Some(leaf_id),
                ..
            }) => leaf_id,
            other => panic!("expected branch prompt success, got {other:?}"),
        };

        let switch = session
            .run(CodingAgentOperation::SwitchActiveLeaf {
                target_leaf_id: root_leaf.clone(),
            })
            .await
            .unwrap();
        assert!(matches!(
            switch,
            CodingAgentOperationOutcome::ActiveLeafSwitched
        ));
        assert_eq!(
            session
                .hydrate_current()
                .unwrap()
                .unwrap()
                .summary
                .active_leaf_id,
            Some(root_leaf.clone())
        );
        let reopened = CodingAgentSession::open(source_options.clone())
            .await
            .unwrap();
        assert_eq!(
            reopened
                .persistent_session_service()
                .replay()
                .unwrap()
                .active_leaf_id,
            Some(root_leaf.clone())
        );

        let generated = session
            .run(CodingAgentOperation::BranchSummary {
                options: prompt_options(api, ""),
                source_leaf_id: branch_leaf.clone(),
                target_leaf_id: root_leaf.clone(),
                custom_instructions: None,
                reuse: BranchSummaryReusePolicy::AlwaysCreate,
            })
            .await
            .unwrap();
        let expected_summary = match generated {
            CodingAgentOperationOutcome::BranchSummary(PromptTurnOutcome::Success {
                final_text,
                ..
            }) => final_text,
            other => panic!("expected generated branch summary, got {other:?}"),
        };
        let event_log_path = temp
            .path()
            .join("sess_canonical_navigation_durability/events.jsonl");
        let event_log_before_reuse = std::fs::read(&event_log_path).unwrap();
        let mut reuse_events = session.subscribe_product_events_public();
        let reused = session
            .run(CodingAgentOperation::BranchSummary {
                options: prompt_options(api, ""),
                source_leaf_id: branch_leaf.clone(),
                target_leaf_id: root_leaf.clone(),
                custom_instructions: None,
                reuse: BranchSummaryReusePolicy::ReuseExisting,
            })
            .await
            .unwrap();
        assert!(matches!(
            reused,
            CodingAgentOperationOutcome::BranchSummary(PromptTurnOutcome::Success {
                final_text,
                ..
            }) if final_text == expected_summary
        ));
        assert!(reuse_events.try_recv().unwrap().is_none());
        assert_eq!(
            std::fs::read(&event_log_path).unwrap(),
            event_log_before_reuse
        );
        let reopened = CodingAgentSession::open(source_options).await.unwrap();
        assert_eq!(
            reopened
                .persistent_session_service()
                .branch_summary_for(&branch_leaf, &root_leaf)
                .unwrap()
                .as_deref(),
            Some(expected_summary.as_str())
        );

        let capability_generation = session.current_capability_generation_for_tests();
        let mut fork_events = session.subscribe_product_events();
        let source_session_id = session.view().session_id;
        let forked = session
            .run(CodingAgentOperation::ForkSession {
                target_leaf_id: Some(root_leaf.clone()),
            })
            .await
            .unwrap();
        assert!(matches!(forked, CodingAgentOperationOutcome::SessionForked));
        assert_ne!(session.view().session_id, source_session_id);
        assert_eq!(
            session.view().session_id,
            session
                .hydrate_current()
                .unwrap()
                .unwrap()
                .summary
                .session_id
        );
        assert_eq!(
            session.current_capability_generation_for_tests(),
            capability_generation
        );
        let emitted = std::iter::from_fn(|| fork_events.try_recv().unwrap()).collect::<Vec<_>>();
        assert!(emitted.iter().any(|event| matches!(
            event.compatibility_event(),
            CodingAgentEvent::SessionOpened { session_id }
                if session_id == &session.view().session_id
        )));
        assert!(
            emitted
                .windows(2)
                .all(|pair| pair[0].sequence() < pair[1].sequence())
        );
    }

    #[tokio::test]
    async fn canonical_durable_mutations_distinguish_no_commit_partial_commit_and_replay() {
        let api = "coding-session-canonical-mutation-boundaries";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("root answer", StopReason::Stop),
                FauxProvider::text_call("branch answer", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_canonical_mutation_boundaries")
            .with_session_log_root(temp.path());
        let mut session = CodingAgentSession::create(options.clone()).await.unwrap();
        let root_leaf = match session
            .run(CodingAgentOperation::Prompt(prompt_options(
                api,
                "root question",
            )))
            .await
            .unwrap()
        {
            CodingAgentOperationOutcome::Prompt(PromptTurnOutcome::Success {
                leaf_id: Some(leaf_id),
                ..
            }) => leaf_id,
            other => panic!("expected root prompt success, got {other:?}"),
        };
        let branch_leaf = match session
            .run(CodingAgentOperation::Prompt(prompt_options(
                api,
                "branch question",
            )))
            .await
            .unwrap()
        {
            CodingAgentOperationOutcome::Prompt(PromptTurnOutcome::Success {
                leaf_id: Some(leaf_id),
                ..
            }) => leaf_id,
            other => panic!("expected branch prompt success, got {other:?}"),
        };
        let event_log_path = temp
            .path()
            .join("sess_canonical_mutation_boundaries/events.jsonl");
        let manifest_path = temp
            .path()
            .join("sess_canonical_mutation_boundaries/session.json");
        let events_before = std::fs::read(&event_log_path).unwrap();
        let manifest_before = std::fs::read(&manifest_path).unwrap();

        session
            .persistent_session_service()
            .fail_store_after_for_tests(StoreFailurePoint::AppendEvents, 0);
        let error = session
            .run(CodingAgentOperation::SwitchActiveLeaf {
                target_leaf_id: root_leaf.clone(),
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), "session");
        assert_eq!(std::fs::read(&event_log_path).unwrap(), events_before);
        assert_eq!(std::fs::read(&manifest_path).unwrap(), manifest_before);
        assert_eq!(
            session.view().session_id,
            "sess_canonical_mutation_boundaries"
        );
        let reopened = CodingAgentSession::open(options.clone()).await.unwrap();
        assert_eq!(
            reopened
                .persistent_session_service()
                .replay()
                .unwrap()
                .active_leaf_id,
            Some(branch_leaf.clone())
        );

        session
            .persistent_session_service()
            .fail_store_after_for_tests(StoreFailurePoint::UpdateManifest, 0);
        let error = session
            .run(CodingAgentOperation::SwitchActiveLeaf {
                target_leaf_id: root_leaf.clone(),
            })
            .await
            .unwrap_err();
        let operation_id = match &error {
            CodingSessionError::PartialCommit { operation_id, .. } => operation_id,
            other => panic!("expected partial commit, got {other:?}"),
        };
        assert!(!operation_id.is_empty());
        assert_eq!(error.code(), "partial_commit");
        assert_eq!(
            session
                .persistent_session_service()
                .replay()
                .unwrap()
                .active_leaf_id,
            Some(root_leaf.clone())
        );
        let reopened = CodingAgentSession::open(options).await.unwrap();
        assert_eq!(
            reopened
                .persistent_session_service()
                .replay()
                .unwrap()
                .active_leaf_id,
            Some(root_leaf)
        );
    }

    #[tokio::test]
    async fn canonical_run_preserves_plugin_profile_and_delegation_contracts() {
        let api = "coding-session-canonical-delegation-decision";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("delegated result", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_canonical_plugin_profile_delegation")
            .with_session_log_root(temp.path());
        let mut session = CodingAgentSession::create(options.clone()).await.unwrap();
        let mut registry = PluginRegistry::new();
        registry.register_command_provider(Arc::new(SessionPluginCommandProvider));
        session.default_plugin_load_options =
            PluginLoadOptions::new().with_candidate(PluginLoadCandidate::new(
                PluginLoadManifest::new(
                    "canonical-command",
                    "Canonical Command",
                    "1.0.0",
                    PluginSource::FirstParty,
                ),
                registry,
            ));
        let mut events = session.subscribe_product_events();

        let loaded = session.run(CodingAgentOperation::PluginLoad).await.unwrap();
        assert!(matches!(
            loaded,
            CodingAgentOperationOutcome::PluginLoad(CodingAgentPluginLoadOutcome {
                loaded_plugin_ids,
                diagnostics,
                capability_changed: true,
            }) if loaded_plugin_ids == vec!["canonical-command"] && diagnostics.is_empty()
        ));
        let command = session
            .run(CodingAgentOperation::PluginCommand {
                command_id: "plugin.say_hello".into(),
                args: serde_json::Value::Null,
            })
            .await
            .unwrap();
        assert!(matches!(
            command,
            CodingAgentOperationOutcome::PluginCommand(output) if output == "hello"
        ));
        let error = session
            .run(CodingAgentOperation::PluginCommand {
                command_id: "missing.command".into(),
                args: serde_json::Value::Null,
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), "plugin");
        assert_eq!(
            error.to_string(),
            "plugin error: plugin command not found: missing.command"
        );
        assert_eq!(session.operation_control.active(), None);

        let profile = session
            .run(CodingAgentOperation::SetDefaultAgentProfile {
                profile_id: ProfileId::from("reviewer"),
            })
            .await
            .unwrap();
        assert!(matches!(
            profile,
            CodingAgentOperationOutcome::DefaultAgentProfileChanged
        ));
        assert_eq!(session.view().default_agent_profile_id.as_str(), "reviewer");
        let reopened = CodingAgentSession::open(options.clone()).await.unwrap();
        assert_eq!(
            reopened.view().default_agent_profile_id.as_str(),
            "reviewer"
        );

        queue_persistent_delegation_confirmation(
            &mut session,
            "op_reject_contract",
            "tool_reject_contract",
            ProfileKind::Agent,
        );
        let rejected = session
            .run(CodingAgentOperation::RejectDelegation {
                operation_id: "op_reject_contract".into(),
                tool_call_id: "tool_reject_contract".into(),
                reason: "not now".into(),
            })
            .await
            .unwrap();
        assert!(matches!(
            rejected,
            CodingAgentOperationOutcome::DelegationRejected
        ));
        assert!(session.pending_delegation_confirmations().is_empty());

        queue_persistent_delegation_confirmation(
            &mut session,
            "op_approve_contract",
            "tool_approve_contract",
            ProfileKind::Agent,
        );
        let approved = session
            .run(CodingAgentOperation::ApproveDelegation {
                operation_id: "op_approve_contract".into(),
                tool_call_id: "tool_approve_contract".into(),
            })
            .await
            .unwrap();
        assert!(matches!(
            approved,
            CodingAgentOperationOutcome::DelegationApproved
        ));
        assert!(session.pending_delegation_confirmations().is_empty());
        let emitted = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert!(emitted.iter().any(|event| matches!(
            event.compatibility_event(),
            CodingAgentEvent::DelegationRejected { reason, .. } if reason == "not now"
        )));
        assert!(emitted.iter().any(|event| matches!(
            event.compatibility_event(),
            CodingAgentEvent::DelegationApproved { .. }
        )));
        assert!(
            emitted
                .windows(2)
                .all(|pair| pair[0].sequence() < pair[1].sequence())
        );
    }

    #[tokio::test]
    async fn canonical_delegation_decisions_distinguish_no_commit_partial_commit_and_replay() {
        async fn session_with_pending(
            root: &Path,
            session_id: &str,
            operation_id: &str,
            tool_call_id: &str,
        ) -> (CodingAgentSession, CodingAgentSessionOptions) {
            let options = CodingAgentSessionOptions::new()
                .with_session_id(session_id)
                .with_session_log_root(root);
            let mut session = CodingAgentSession::create(options.clone()).await.unwrap();
            queue_persistent_delegation_confirmation(
                &mut session,
                operation_id,
                tool_call_id,
                ProfileKind::Agent,
            );
            (session, options)
        }

        for (decision, failure_point) in [
            ("reject_pre_append", StoreFailurePoint::AppendEvents),
            ("approve_pre_append", StoreFailurePoint::AppendEvents),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let operation_id = format!("op_{decision}");
            let tool_call_id = format!("tool_{decision}");
            let (mut session, options) = session_with_pending(
                temp.path(),
                &format!("sess_{decision}"),
                &operation_id,
                &tool_call_id,
            )
            .await;
            let event_log_path = temp.path().join(format!("sess_{decision}/events.jsonl"));
            let manifest_path = temp.path().join(format!("sess_{decision}/session.json"));
            let events_before = std::fs::read(&event_log_path).unwrap();
            let manifest_before = std::fs::read(&manifest_path).unwrap();
            session
                .persistent_session_service()
                .fail_store_after_for_tests(failure_point, 0);
            let error = if decision.starts_with("reject") {
                session
                    .run(CodingAgentOperation::RejectDelegation {
                        operation_id: operation_id.clone(),
                        tool_call_id: tool_call_id.clone(),
                        reason: "declined".into(),
                    })
                    .await
                    .unwrap_err()
            } else {
                session
                    .run(CodingAgentOperation::ApproveDelegation {
                        operation_id: operation_id.clone(),
                        tool_call_id: tool_call_id.clone(),
                    })
                    .await
                    .unwrap_err()
            };
            assert_eq!(error.code(), "session");
            assert_eq!(session.pending_delegation_confirmations().len(), 1);
            assert_eq!(std::fs::read(&event_log_path).unwrap(), events_before);
            assert_eq!(std::fs::read(&manifest_path).unwrap(), manifest_before);
            assert_eq!(
                CodingAgentSession::open(options)
                    .await
                    .unwrap()
                    .pending_delegation_confirmations()
                    .len(),
                1
            );
        }

        for decision in ["reject_partial_commit", "approve_partial_commit"] {
            let temp = tempfile::tempdir().unwrap();
            let operation_id = format!("op_{decision}");
            let tool_call_id = format!("tool_{decision}");
            let (mut session, options) = session_with_pending(
                temp.path(),
                &format!("sess_{decision}"),
                &operation_id,
                &tool_call_id,
            )
            .await;
            session
                .persistent_session_service()
                .fail_store_after_for_tests(StoreFailurePoint::UpdateManifest, 0);
            let error = if decision.starts_with("reject") {
                session
                    .run(CodingAgentOperation::RejectDelegation {
                        operation_id: operation_id.clone(),
                        tool_call_id: tool_call_id.clone(),
                        reason: "declined".into(),
                    })
                    .await
                    .unwrap_err()
            } else {
                session
                    .run(CodingAgentOperation::ApproveDelegation {
                        operation_id: operation_id.clone(),
                        tool_call_id: tool_call_id.clone(),
                    })
                    .await
                    .unwrap_err()
            };
            assert!(matches!(
                &error,
                CodingSessionError::PartialCommit {
                    operation_id: durable_operation_id,
                    ..
                } if durable_operation_id == &operation_id
            ));
            assert_eq!(error.code(), "partial_commit");
            assert_eq!(session.pending_delegation_confirmations().len(), 1);
            let reopened = CodingAgentSession::open(options).await.unwrap();
            assert!(reopened.pending_delegation_confirmations().is_empty());
            assert!(
                reopened
                    .persistent_session_service()
                    .replay()
                    .unwrap()
                    .pending_delegation_confirmations
                    .is_empty()
            );
        }
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
    async fn export_current_html_uses_read_only_operation_admission_while_root_busy() {
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

        let exported = session.export_current_html(&output).unwrap();

        assert_eq!(exported, output);
        assert!(output.exists());
        assert_eq!(
            session.operation_control.active(),
            Some(OperationKind::Prompt)
        );
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
            CodingAgentEvent::CapabilityChanged { .. } => "capability_changed",
            CodingAgentEvent::OperationRecovered { .. } => "operation_recovered",
        }
    }
}
