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
mod prompt_execution;
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
use prompt_execution::apply_finalized_session_write;
use runtime_service::RuntimeService;
use scheduler::OperationScheduler;
pub(crate) use self_healing_edit_flow::{
    ModelSelfHealingEditRepairStrategy, SelfHealingEditContext, SelfHealingEditFlow,
    SelfHealingEditOptions,
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
