mod connection;
pub(crate) mod context;
mod control;
mod lifecycle;
mod recovery;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
mod view;

pub use crate::events::{
    CodingAgentAgentProductEvent, CodingAgentCapabilityProductEvent,
    CodingAgentDelegationEventContext, CodingAgentDelegationProductEvent,
    CodingAgentDiagnosticProductEvent, CodingAgentImageContent, CodingAgentMessageProductEvent,
    CodingAgentProductEvent, CodingAgentProductEventCapabilityRevocation,
    CodingAgentProductEventCheckOutput, CodingAgentProductEventDeliveryClass,
    CodingAgentProductEventDiagnostic, CodingAgentProductEventDurability,
    CodingAgentProductEventError, CodingAgentProductEventFamily, CodingAgentProductEventKind,
    CodingAgentProductEventProfileKind, CodingAgentProductEventReplacement,
    CodingAgentProductEventTerminalOperation, CodingAgentProductEventTerminalOperationKind,
    CodingAgentProductEventTerminalStatus, CodingAgentProductEventUsage,
    CodingAgentProfileProductEvent, CodingAgentRecoveryResolution, CodingAgentRuntimeProductEvent,
    CodingAgentSessionProductEvent, CodingAgentSessionWriteFailureStatus,
    CodingAgentTeamProductEvent, CodingAgentToolProductEvent, CodingAgentWorkflowProductEvent,
};
#[allow(unused_imports)]
pub(crate) use crate::events::{ProductEvent, ProductEventSequence};
pub use crate::operations::agent_invocation::flow::{
    AgentInvocationOptions, AgentInvocationOutcome,
};
pub use crate::operations::delegation::PendingDelegationConfirmation;
pub use crate::operations::export::{CodingAgentSessionExport, CodingAgentSessionExportItem};
pub use crate::operations::prompt::context::{
    CodingDiagnostic, CodingDiagnosticSeverity, PromptTurnMode, PromptTurnOptions,
    PromptTurnOutcome,
};
pub use crate::operations::self_healing_edit::flow::{
    SelfHealingEditCheckOutput, SelfHealingEditDiagnostic, SelfHealingEditModelRepairOptions,
    SelfHealingEditOutcome, SelfHealingEditRepairAttempt, SelfHealingEditReplacement,
    SelfHealingEditRequest,
};
pub use crate::operations::team_invocation::flow::{
    AgentTeamMemberOutcome, AgentTeamOptions, AgentTeamOutcome,
};
pub use crate::profiles::{
    AgentProfile, DelegationConfirmationMode, DelegationPolicy, ProfileDiagnostic, ProfileId,
    ProfileKind, ProfileRegistry, ProfileRegistryOptions, ProfileSource, SupervisionPolicy,
    TeamProfile, TeamStrategy, TeamSupervisor,
};
pub(crate) use crate::runtime::client::context::{
    UiContextProjection, UiFileChangeProjection, UiOperationProjection,
};
pub use crate::runtime::client::projection::{
    CodingAgentCapabilityControl, CodingAgentCapabilityRevocationOutcome,
    CodingAgentClientConnection, CodingAgentClientId, CodingAgentConnectionGeneration,
    CodingAgentContextSnapshot, CodingAgentControlId, CodingAgentControlKind,
    CodingAgentControlReceipt, CodingAgentControlRejection, CodingAgentControlRejectionReason,
    CodingAgentDelegationSnapshot, CodingAgentDetachOutcome, CodingAgentDraft, CodingAgentDraftId,
    CodingAgentDraftKind, CodingAgentFileChangeSnapshot, CodingAgentFreshSnapshotRecovery,
    CodingAgentMutationRejection, CodingAgentOperationControl, CodingAgentOperationSnapshot,
    CodingAgentOperationStatus, CodingAgentOutcomeAcknowledgementId,
    CodingAgentProductEventReceiver, CodingAgentPromptControl, CodingAgentReconnect,
    CodingAgentReconnectDelivery, CodingAgentReconnectReceiver, CodingAgentRecoveryReason,
    CodingAgentRuntimeShutdownHandle, CodingAgentShutdownOutcome, CodingAgentSnapshot,
    CodingAgentSnapshotCursor, CodingAgentSubmissionLease, CodingAgentSubmittedEventDurability,
    CodingAgentSubmittedOperation, CodingAgentSubmittedOperationStatus,
    CodingAgentSubmittedTerminalAnchor, CodingAgentTerminalUncertainty,
    CodingAgentTurnUsageSnapshot, CodingAgentUsageSnapshot,
};
#[cfg(test)]
pub(crate) use crate::runtime::client::state::{
    ClientConnectionId, ClientDraftKind, UiSnapshotCursor,
};
pub(crate) use crate::runtime::client::state::{ClientDraft, UiSnapshot};
pub use crate::runtime::error::{CodingAgentLifecycleRejection, CodingSessionError};
pub use crate::runtime::execution::CodingAgentOperationTask;
pub use crate::runtime::facade::context::{
    CapabilityStatus, CodingAgentCapabilities, CodingAgentRecoveryPending,
    CodingAgentRecoveryResolutionRequest, CodingAgentRecoveryResolutionResult,
    CodingAgentRecoveryRetryRequest, CodingAgentRecoveryRetryResult, CodingAgentSessionOptions,
    CodingAgentSessionSummary, CodingAgentSessionView,
};
pub(crate) use crate::runtime::facade::context::{
    CodingAgentSessionDiagnostic, CodingAgentSessionHydration, CodingAgentSessionTranscriptItem,
    CodingAgentSessionTree, CodingAgentSessionUsageSummary,
};
pub use crate::runtime::outcome::{
    BranchSummaryReusePolicy, CodingAgentOperation, CodingAgentOperationOutcome,
    CodingAgentPluginDiagnostic, CodingAgentPluginLoadOutcome,
};
pub(crate) use crate::services::event::ProductEventReceiver;

use crate::runtime::client::projection as public_projection;
use crate::runtime::snapshot as snapshot_coordinator;

pub(crate) use crate::operations::delegation::{
    DelegationTargetInventory, PendingDelegationConfirmationQueue,
    PendingDelegationConfirmationState, pending_state_from_replay,
};
use crate::operations::export::flow::ExportOptions;
use crate::operations::plugin_load::flow::PluginLoadOptions;
use crate::runtime::capability::CapabilitySnapshotService;
pub(crate) use crate::runtime::capability::PluginCapabilitySet;
pub use crate::runtime::capability::{FilesystemCapability, ShellCapability};
use crate::runtime::client::service::ClientService;
use crate::runtime::control::{OperationControl, PromptControlCleanup, PromptControlGeneration};
pub(crate) use crate::runtime::control::{OperationKind, PromptControlHandle};
use crate::runtime::intent::{ControlIntent, IntentRouter, QueryIntent};
pub(crate) use crate::runtime::operation::OperationIdempotencyKey;
use crate::runtime::owners::RuntimeHost;
use crate::runtime::snapshot::SnapshotCoordinator;
pub(crate) use crate::runtime::submission::SubmissionLeaseLifecycle;
use crate::services::authorization::AuthorizationService;
use crate::services::capability::CapabilityService;
use crate::services::event::EventService;
use crate::services::flow::FlowService;
use crate::services::plugin::PluginService;
use crate::services::runtime::RuntimeService;
use crate::services::session::{default_cwd, replay_derived_owner_state, session_cwd};
use crate::session::service::{SessionPersistence, SessionService, TransientSessionState};
pub(in crate::runtime) use control::PromptControlCleanupGuard;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::plugins::{
    CommandDefinition, KeybindDefinition, PluginSource, UiActionDefinition, UiDialogDefinition,
};
#[derive(Debug)]
pub struct CodingAgentSession {
    pub(super) runtime_host: RuntimeHost,
}
