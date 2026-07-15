mod agent_invocation_flow;
mod agent_team_flow;
mod branch_summary_flow;
mod branch_summary_service;
mod capability_service;
mod capability_snapshot;
mod client_projection;
mod client_service;
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
mod operation_admission;
mod operation_control;
mod operation_dispatch;
mod operation_submission;
mod plugin_load_flow;
mod plugin_load_service;
mod plugin_service;
mod profiles;
mod prompt;
mod prompt_flow;
mod public_event;
mod public_operation;
mod public_projection;
mod runtime_service;
mod scheduler;
mod self_healing_edit_flow;
mod self_healing_edit_service;
mod session_connection;
mod session_control;
mod session_lifecycle;
mod session_log;
mod session_service;
#[cfg(test)]
mod session_test_support;
#[cfg(test)]
mod session_tests;
mod session_view;
mod snapshot_coordinator;

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
pub use error::{CodingAgentLifecycleRejection, CodingSessionError};
pub use event::CodingAgentEvent;
#[allow(unused_imports)]
pub(crate) use event::{ProductEvent, ProductEventSequence};
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
pub use public_event::{
    CodingAgentAgentProductEvent, CodingAgentCapabilityProductEvent,
    CodingAgentDelegationEventContext, CodingAgentDelegationProductEvent,
    CodingAgentDiagnosticProductEvent, CodingAgentMessageProductEvent, CodingAgentProductEvent,
    CodingAgentProductEventCapabilityRevocation, CodingAgentProductEventCheckOutput,
    CodingAgentProductEventDiagnostic, CodingAgentProductEventDurability,
    CodingAgentProductEventError, CodingAgentProductEventFamily, CodingAgentProductEventKind,
    CodingAgentProductEventProfileKind, CodingAgentProductEventReplacement,
    CodingAgentProductEventTerminalOperation, CodingAgentProductEventTerminalOperationKind,
    CodingAgentProductEventTerminalStatus, CodingAgentProductEventUsage,
    CodingAgentProfileProductEvent, CodingAgentRuntimeProductEvent, CodingAgentSessionProductEvent,
    CodingAgentTeamProductEvent, CodingAgentToolProductEvent, CodingAgentWorkflowProductEvent,
};
pub use public_operation::{
    BranchSummaryReusePolicy, CodingAgentOperation, CodingAgentOperationOutcome,
    CodingAgentPluginDiagnostic, CodingAgentPluginLoadOutcome,
};
pub use public_projection::{
    CodingAgentClientConnection, CodingAgentClientId, CodingAgentConnectionGeneration,
    CodingAgentControlId, CodingAgentControlKind, CodingAgentControlReceipt,
    CodingAgentControlRejection, CodingAgentControlRejectionReason, CodingAgentDetachOutcome,
    CodingAgentDraft, CodingAgentDraftId, CodingAgentDraftKind, CodingAgentFreshSnapshotRecovery,
    CodingAgentMutationRejection, CodingAgentOutcomeAcknowledgementId,
    CodingAgentProductEventReceiver, CodingAgentPromptControl, CodingAgentReconnect,
    CodingAgentReconnectDelivery, CodingAgentReconnectReceiver, CodingAgentRecoveryReason,
    CodingAgentRuntimeShutdownHandle, CodingAgentShutdownOutcome, CodingAgentSnapshot,
    CodingAgentSnapshotCursor, CodingAgentSubmissionLease, CodingAgentSubmittedEventDurability,
    CodingAgentSubmittedOperation, CodingAgentSubmittedOperationStatus,
    CodingAgentSubmittedTerminalAnchor, CodingAgentTerminalUncertainty,
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
use client_service::ClientService;
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
use operation_control::{
    OperationControl, PromptControlCleanup, PromptControlGeneration, PromptControlRegistration,
};
pub(crate) use operation_control::{OperationKind, PromptControlHandle};
pub(crate) use operation_submission::SubmissionLeaseLifecycle;
use operation_submission::{
    PendingSubmissionLease, SubmissionCommitGuard, submitted_terminal_status,
};
use plugin_load_flow::PluginLoadOptions;
use plugin_load_service::PluginLoadService;
use plugin_service::PluginService;
use prompt::{PromptTurnContext, PromptTurnIds};
use runtime_service::RuntimeService;
use scheduler::OperationScheduler;
pub(crate) use self_healing_edit_flow::{
    ModelSelfHealingEditRepairStrategy, SelfHealingEditContext, SelfHealingEditFlow,
    SelfHealingEditOptions, SelfHealingEditRepairStrategy,
};
use self_healing_edit_service::SelfHealingEditService;
use session_control::PromptControlCleanupGuard;
use session_log::event::PersistedDelegationStatus;
use session_log::id::{Clock, IdGenerator, SystemClock, SystemIdGenerator};
use session_service::{
    FinalizedSessionWrite, SessionPersistence, SessionService, StartupRecoveryMarker,
    TransientSessionState,
};
use snapshot_coordinator::SnapshotCoordinator;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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
    capability_snapshots: CapabilitySnapshotService,
    snapshot_coordinator: Arc<SnapshotCoordinator>,
    client_service: ClientService,
    pending_submission: Option<PendingSubmissionLease>,
    startup_recovery_markers: Mutex<Vec<StartupRecoveryMarker>>,
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

fn runtime_service_for_options(options: &CodingAgentSessionOptions) -> RuntimeService {
    options
        .ai_client()
        .cloned()
        .map(RuntimeService::with_ai_client)
        .unwrap_or_else(RuntimeService::new)
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

    async fn approve_delegation_confirmation_inner(
        &mut self,
        operation_id: String,
        tool_call_id: String,
        now: String,
        parent_capability_snapshot: OperationCapabilitySnapshot,
    ) -> Result<(), CodingSessionError> {
        let mut ids = SystemIdGenerator;
        let mut pending = self.delegation_confirmation_service.approve_pending(
            &mut self.persistence,
            &mut self.pending_delegation_confirmations,
            &self.event_service,
            operation_id.as_str(),
            tool_call_id.as_str(),
            &now,
            ids.next_operation_id(),
        )?;
        if let Some(runtime) = pending.prompt_options.runtime_mut() {
            self.runtime_service.install_provider_runtime(runtime);
        }
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
                        Some(parent_capability_snapshot.clone()),
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
                        Some(parent_capability_snapshot),
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
            self.refresh_snapshot_projection();
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
        scheduler_parent_operation_id: String,
    ) -> Result<AgentInvocationOutcome, CodingSessionError> {
        let prompt_control_receiver = self.operation_control.take_prompt_control_receiver();
        let mut context = AgentInvocationContext::new(
            options,
            self.profile_registry.clone(),
            self.plugin_service.clone(),
            self.event_service.clone(),
        )
        .with_scheduler_parent_operation_id(scheduler_parent_operation_id);
        if let Some(receiver) = prompt_control_receiver {
            context.set_prompt_control_receiver(receiver);
        }
        self.flow_service.run_agent_invocation(&mut context).await
    }

    async fn invoke_team_inner(
        &mut self,
        options: AgentTeamOptions,
        scheduler_parent_operation_id: String,
    ) -> Result<AgentTeamOutcome, CodingSessionError> {
        let mut context = AgentTeamContext::new(
            options,
            self.profile_registry.clone(),
            self.plugin_service.clone(),
            self.event_service.clone(),
        )
        .with_scheduler_parent_operation_id(scheduler_parent_operation_id);
        self.flow_service.run_agent_team(&mut context).await
    }

    async fn execute_authorized_delegations(
        &mut self,
        context: &mut PromptTurnContext,
        decisions: &[DelegationAuthorizationDecision],
        prompt_options: PromptTurnOptions,
    ) -> Result<(), CodingSessionError> {
        let parent_capability_snapshot = context.capability_snapshot().cloned();
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
                                    parent_capability_snapshot.clone(),
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
                                    parent_capability_snapshot.clone(),
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
                    PromptTurnIds::new(snapshot.operation_id.clone(), ids.next_turn_id()),
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
