use crate::events::ProductEventSequence;
use crate::events::{CodingAgentProductEvent, CodingAgentProductEventTerminalStatus};
use crate::protocol::version::{ProtocolFamilyVersion, UI_SNAPSHOT_PROTOCOL_VERSION};
use crate::runtime::capability::{CapabilityRevocationPolicy, InstalledCapabilityGeneration};
use crate::runtime::client::context::{
    UiContextProjection, UiDelegationProjection, UiFileChangeProjection, UiOperationProjection,
    UiOperationStatus, UiTurnUsageProjection, UiUsageProjection,
};
use crate::runtime::client::state::{ClientConnectionId, ClientDraftKind, UiSnapshot};
use crate::runtime::control::OperationControl;
use crate::runtime::error::CodingSessionError;
use crate::runtime::facade::context::{CodingAgentCapabilities, CodingAgentSessionView};
use crate::services::event::{EventService, ProductEventReceiver, ProductEventRecovery};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::runtime::snapshot::{
    ClientDetachOutcome, ClientGeneration, ClientHandle, ClientRegistryError, ClientSnapshotState,
    DraftRecord, SnapshotCoordinator, SubmittedOperationStatus,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CodingAgentClientId(String);

impl CodingAgentClientId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CodingAgentConnectionGeneration(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CodingAgentDraftId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgentDraftKind {
    Prompt,
    Steer,
    FollowUp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodingAgentDraft {
    pub id: CodingAgentDraftId,
    pub kind: CodingAgentDraftKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodingAgentSubmittedOperationStatus {
    Running,
    Terminal {
        status: CodingAgentProductEventTerminalStatus,
        anchor: CodingAgentSubmittedTerminalAnchor,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodingAgentSubmittedOperation {
    pub operation_id: String,
    pub kind: String,
    pub status: CodingAgentSubmittedOperationStatus,
}

/// Result of ending one connection generation without stopping runtime work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgentDetachOutcome {
    Detached,
    AlreadyDetached,
    StaleGeneration,
}

/// Result of draining and closing the product runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgentShutdownOutcome {
    ShutDown,
    AlreadyShutDown,
}

/// Cloneable Phase A authority that can stop new work while the unique owner is moved.
#[derive(Debug, Clone)]
pub struct CodingAgentRuntimeShutdownHandle {
    pub(crate) coordinator: Arc<SnapshotCoordinator>,
}

impl CodingAgentRuntimeShutdownHandle {
    /// Idempotently close admission and control without waiting, aborting, or publishing events.
    pub fn request_shutdown(&self) {
        self.coordinator.request_shutdown();
    }
}

/// Privileged runtime control for installing a new capability generation and
/// requesting cancellation of work admitted under older generations.
#[derive(Debug, Clone)]
pub struct CodingAgentCapabilityControl {
    pub(crate) coordinator: Arc<SnapshotCoordinator>,
    pub(crate) operation_control: OperationControl,
    pub(crate) event_service: EventService,
    pub(crate) authorization_service: crate::services::authorization::AuthorizationService,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodingAgentCapabilityRevocationOutcome {
    pub generation: u64,
    pub cancellation_requested_operation_ids: Vec<String>,
}

impl CodingAgentCapabilityControl {
    pub fn revoke_older_operations(&self) -> CodingAgentCapabilityRevocationOutcome {
        let generation = self.coordinator.install_next_capability_generation();
        let cancellation_requested_operation_ids = self
            .operation_control
            .cancel_capability_generations_before(generation);
        for operation_id in &cancellation_requested_operation_ids {
            self.authorization_service.cancel_operation(
                operation_id,
                "tool authorization cancelled by capability revocation",
            );
        }
        self.event_service
            .emit_capability_changed(InstalledCapabilityGeneration {
                generation,
                revocation: CapabilityRevocationPolicy::RequestCancelOlderOperations,
                cancellation_requested_operation_ids: cancellation_requested_operation_ids.clone(),
            });
        CodingAgentCapabilityRevocationOutcome {
            generation: generation.get(),
            cancellation_requested_operation_ids,
        }
    }
}

/// Public durability evidence for a root terminal event.
///
/// This deliberately omits session identifiers and pending-write internals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgentSubmittedEventDurability {
    Durable,
    Uncertain,
}

/// Opaque identity used to acknowledge an outcome-only submission.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CodingAgentOutcomeAcknowledgementId(String);

impl CodingAgentOutcomeAcknowledgementId {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Recovery disposition when no authoritative root terminal event was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgentTerminalUncertainty {
    RecoveryRequired,
}

/// Exact public evidence that makes a submitted operation terminal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CodingAgentSubmittedTerminalAnchor {
    ProductEvent {
        sequence: u64,
        durability: CodingAgentSubmittedEventDurability,
    },
    OutcomeOnly {
        acknowledgement: CodingAgentOutcomeAcknowledgementId,
    },
    TerminalUncertain {
        operation_id: String,
        recovery: CodingAgentTerminalUncertainty,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgentRecoveryReason {
    RetainedHistoryGap,
    LiveReceiverLag,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodingAgentFreshSnapshotRecovery {
    pub requested_sequence: u64,
    pub oldest_available_sequence: u64,
    pub fresh_cursor: CodingAgentSnapshotCursor,
    pub reason: CodingAgentRecoveryReason,
    pub snapshot: Box<CodingAgentSnapshot>,
}

#[derive(Debug)]
pub enum CodingAgentReconnect {
    Replayed {
        events: Vec<CodingAgentProductEvent>,
        cursor: CodingAgentSnapshotCursor,
        receiver: CodingAgentReconnectReceiver,
    },
    FreshSnapshotRequired(CodingAgentFreshSnapshotRecovery),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CodingAgentControlId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgentControlKind {
    Abort,
    Steer,
    FollowUp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodingAgentControlReceipt {
    pub control_id: CodingAgentControlId,
    pub operation_id: String,
    pub kind: CodingAgentControlKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgentControlRejectionReason {
    StaleConnection,
    Detached,
    StaleGeneration,
    RuntimeShutDown,
    NotOwner,
    TargetMismatch,
    TargetNotRunning,
    NoLongerCancellable,
    ControlChannelClosed,
    InvalidInput,
    QueueCapacityExceeded,
    PayloadConflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgentMutationRejection {
    QueueCapacity,
    ReceiptCapacity,
    TargetMismatch,
    TargetNotRunning,
    PayloadConflict,
    NotOwner,
    Detached,
    StaleGeneration,
    RuntimeShutDown,
    InvalidInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodingAgentControlRejection {
    pub control_id: CodingAgentControlId,
    pub operation_id: String,
    pub kind: CodingAgentControlKind,
    pub reason: CodingAgentControlRejectionReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodingAgentOperationControl {
    pub client_id: CodingAgentClientId,
    pub generation: CodingAgentConnectionGeneration,
    pub operation_id: String,
    #[serde(skip, default = "SnapshotCoordinator::new")]
    pub(crate) coordinator: Arc<SnapshotCoordinator>,
}

impl PartialEq for CodingAgentOperationControl {
    fn eq(&self, other: &Self) -> bool {
        self.client_id == other.client_id
            && self.generation == other.generation
            && self.operation_id == other.operation_id
    }
}

impl Eq for CodingAgentOperationControl {}

impl CodingAgentOperationControl {
    pub fn abort(
        &self,
        control_id: CodingAgentControlId,
        reason: impl Into<String>,
    ) -> Result<CodingAgentControlReceipt, CodingAgentControlRejection> {
        self.coordinator.enqueue_control(
            &self.handle(),
            &self.operation_id,
            control_id,
            CodingAgentControlKind::Abort,
            reason.into(),
        )
    }

    fn handle(&self) -> ClientHandle {
        ClientHandle {
            id: internal_client_id(&self.client_id),
            generation: ClientGeneration(self.generation.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodingAgentPromptControl {
    pub client_id: CodingAgentClientId,
    pub generation: CodingAgentConnectionGeneration,
    pub operation_id: String,
    #[serde(skip, default = "SnapshotCoordinator::new")]
    pub(crate) coordinator: Arc<SnapshotCoordinator>,
}

impl PartialEq for CodingAgentPromptControl {
    fn eq(&self, other: &Self) -> bool {
        self.client_id == other.client_id
            && self.generation == other.generation
            && self.operation_id == other.operation_id
    }
}

impl Eq for CodingAgentPromptControl {}

impl CodingAgentPromptControl {
    fn submit(
        &self,
        control_id: CodingAgentControlId,
        kind: CodingAgentControlKind,
        text: String,
    ) -> Result<CodingAgentControlReceipt, CodingAgentControlRejection> {
        self.coordinator
            .enqueue_control(&self.handle(), &self.operation_id, control_id, kind, text)
    }

    fn submit_content(
        &self,
        control_id: CodingAgentControlId,
        kind: CodingAgentControlKind,
        content: Vec<pi_ai::api::conversation::ContentBlock>,
    ) -> Result<CodingAgentControlReceipt, CodingAgentControlRejection> {
        self.coordinator.enqueue_content_control(
            &self.handle(),
            &self.operation_id,
            control_id,
            kind,
            content,
        )
    }

    pub fn abort(
        &self,
        control_id: CodingAgentControlId,
        reason: impl Into<String>,
    ) -> Result<CodingAgentControlReceipt, CodingAgentControlRejection> {
        self.submit(control_id, CodingAgentControlKind::Abort, reason.into())
    }

    pub fn steer(
        &self,
        control_id: CodingAgentControlId,
        text: impl Into<String>,
    ) -> Result<CodingAgentControlReceipt, CodingAgentControlRejection> {
        self.submit(control_id, CodingAgentControlKind::Steer, text.into())
    }

    pub(crate) fn steer_content(
        &self,
        control_id: CodingAgentControlId,
        content: Vec<pi_ai::api::conversation::ContentBlock>,
    ) -> Result<CodingAgentControlReceipt, CodingAgentControlRejection> {
        self.submit_content(control_id, CodingAgentControlKind::Steer, content)
    }

    pub fn follow_up(
        &self,
        control_id: CodingAgentControlId,
        text: impl Into<String>,
    ) -> Result<CodingAgentControlReceipt, CodingAgentControlRejection> {
        self.submit(control_id, CodingAgentControlKind::FollowUp, text.into())
    }

    pub(crate) fn follow_up_content(
        &self,
        control_id: CodingAgentControlId,
        content: Vec<pi_ai::api::conversation::ContentBlock>,
    ) -> Result<CodingAgentControlReceipt, CodingAgentControlRejection> {
        self.submit_content(control_id, CodingAgentControlKind::FollowUp, content)
    }

    pub fn steer_draft(
        &self,
        draft_id: CodingAgentDraftId,
    ) -> Result<CodingAgentControlReceipt, CodingAgentControlRejection> {
        self.submit_draft(draft_id, CodingAgentControlKind::Steer)
    }

    pub fn follow_up_draft(
        &self,
        draft_id: CodingAgentDraftId,
    ) -> Result<CodingAgentControlReceipt, CodingAgentControlRejection> {
        self.submit_draft(draft_id, CodingAgentControlKind::FollowUp)
    }

    fn submit_draft(
        &self,
        draft_id: CodingAgentDraftId,
        kind: CodingAgentControlKind,
    ) -> Result<CodingAgentControlReceipt, CodingAgentControlRejection> {
        self.coordinator.enqueue_prompt_control_draft(
            &self.handle(),
            &self.operation_id,
            draft_id,
            kind,
        )
    }

    fn handle(&self) -> ClientHandle {
        ClientHandle {
            id: internal_client_id(&self.client_id),
            generation: ClientGeneration(self.generation.0),
        }
    }
}

#[derive(Debug)]
pub struct CodingAgentSubmissionLease {
    operation_id: String,
    pub(crate) shared: Arc<Mutex<crate::runtime::facade::SubmissionLeaseLifecycle>>,
}

impl Drop for CodingAgentSubmissionLease {
    fn drop(&mut self) {
        let mut lifecycle = self.shared.lock().unwrap();
        if matches!(
            *lifecycle,
            crate::runtime::facade::SubmissionLeaseLifecycle::Prepared
        ) {
            *lifecycle = crate::runtime::facade::SubmissionLeaseLifecycle::Abandoned;
        }
    }
}

impl CodingAgentSubmissionLease {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingAgentSnapshotCursor {
    pub stream_id: String,
    pub snapshot_protocol_major: u32,
    pub last_event_sequence: u64,
    pub capability_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgentOperationStatus {
    Running,
    Completed,
    Failed,
    Aborted,
    Recovered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingAgentOperationSnapshot {
    pub operation_id: String,
    pub kind: String,
    pub parent_operation_id: Option<String>,
    pub root_operation_id: Option<String>,
    pub status: CodingAgentOperationStatus,
    pub started_sequence: u64,
    pub updated_sequence: u64,
    pub diagnostics: Vec<String>,
    pub failure: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingAgentFileChangeSnapshot {
    pub path: String,
    pub mutation_kind: String,
    pub operation_id: String,
    pub tool_call_id: Option<String>,
    pub updated_sequence: u64,
    pub first_changed_line: Option<usize>,
    pub added_lines: Option<usize>,
    pub removed_lines: Option<usize>,
    pub diff: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingAgentDelegationSnapshot {
    pub tool_call_id: String,
    pub child_operation_id: Option<String>,
    pub target_kind: String,
    pub target_id: String,
    pub task: String,
    pub status: String,
    pub updated_sequence: u64,
    pub summary: Option<String>,
    pub failure: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingAgentTurnUsageSnapshot {
    pub turn_id: String,
    pub input: u32,
    pub output: u32,
    pub cache_read: u32,
    pub cache_write: u32,
    pub context_tokens: Option<u32>,
    pub cost: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingAgentUsageSnapshot {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub cost: Option<f64>,
    pub latest_turn: Option<CodingAgentTurnUsageSnapshot>,
    pub model_id: Option<String>,
    pub context_window: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingAgentContextSnapshot {
    pub operations: Vec<CodingAgentOperationSnapshot>,
    pub changes: Vec<CodingAgentFileChangeSnapshot>,
    pub delegations: Vec<CodingAgentDelegationSnapshot>,
    pub usage: CodingAgentUsageSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodingAgentSnapshot {
    pub cursor: CodingAgentSnapshotCursor,
    pub version: ProtocolFamilyVersion,
    pub session: CodingAgentSessionView,
    pub capabilities: CodingAgentCapabilities,
    pub active_operation: Option<String>,
    pub drafts: Vec<CodingAgentDraft>,
    pub submitted_operation: Option<CodingAgentSubmittedOperation>,
    pub pending_authorizations: Vec<crate::authorization::ToolAuthorizationRequest>,
    pub context: CodingAgentContextSnapshot,
}

#[derive(Debug, Clone)]
pub struct CodingAgentClientConnection {
    coordinator: Arc<SnapshotCoordinator>,
    event_service: EventService,
    authorization_service: crate::services::authorization::AuthorizationService,
    pub client_id: CodingAgentClientId,
    pub generation: CodingAgentConnectionGeneration,
    pub snapshot: CodingAgentSnapshot,
}

impl CodingAgentClientConnection {
    pub(crate) fn handle(&self) -> ClientHandle {
        ClientHandle {
            id: internal_client_id(&self.client_id),
            generation: ClientGeneration(self.generation.0),
        }
    }

    pub fn state(&self) -> Result<CodingAgentSnapshot, CodingSessionError> {
        self.coordinator
            .client_state(&self.handle())
            .map(public_client_snapshot)
            .map_err(|error| registry_error(&self.client_id, error))
    }

    pub fn pending_tool_authorizations(
        &self,
    ) -> Result<Vec<crate::authorization::ToolAuthorizationRequest>, CodingSessionError> {
        Ok(self.state()?.pending_authorizations)
    }

    pub fn decide_tool_authorization(
        &self,
        authorization_id: &str,
        decision: crate::authorization::ToolAuthorizationDecision,
    ) -> Result<(), CodingSessionError> {
        self.coordinator
            .client_state(&self.handle())
            .map_err(|error| registry_error(&self.client_id, error))?;
        self.authorization_service
            .decide(authorization_id, decision)
    }

    pub fn prompt_control(&self, operation_id: impl Into<String>) -> CodingAgentPromptControl {
        CodingAgentPromptControl {
            client_id: self.client_id.clone(),
            generation: self.generation,
            operation_id: operation_id.into(),
            coordinator: self.coordinator.clone(),
        }
    }

    pub fn operation_control(
        &self,
        operation_id: impl Into<String>,
    ) -> CodingAgentOperationControl {
        CodingAgentOperationControl {
            client_id: self.client_id.clone(),
            generation: self.generation,
            operation_id: operation_id.into(),
            coordinator: self.coordinator.clone(),
        }
    }

    pub(crate) fn bind_operation_cancellation(
        &self,
        operation_id: String,
        cancellation: crate::runtime::control::OperationCancellationHandle,
    ) {
        self.coordinator
            .bind_operation_cancellation(self.handle(), operation_id, cancellation);
    }

    pub fn acknowledge(&self, sequence: u64) -> Result<u64, CodingSessionError> {
        self.coordinator
            .acknowledge(&self.handle(), sequence)
            .map_err(|error| registry_error(&self.client_id, error))
    }

    pub fn acknowledge_outcome(
        &self,
        acknowledgement: CodingAgentOutcomeAcknowledgementId,
    ) -> Result<(), CodingSessionError> {
        self.coordinator
            .acknowledge_outcome(&self.handle(), &acknowledgement)
            .map_err(|error| registry_error(&self.client_id, error))
    }

    pub fn detach(&self) -> Result<CodingAgentDetachOutcome, CodingSessionError> {
        self.coordinator
            .detach(&self.handle())
            .map(|outcome| match outcome {
                ClientDetachOutcome::Detached => CodingAgentDetachOutcome::Detached,
                ClientDetachOutcome::AlreadyDetached => CodingAgentDetachOutcome::AlreadyDetached,
                ClientDetachOutcome::StaleGeneration => CodingAgentDetachOutcome::StaleGeneration,
            })
            .map_err(|error| registry_error(&self.client_id, error))
    }

    pub fn reconnect(
        &self,
        requested_after: u64,
    ) -> Result<CodingAgentReconnect, CodingSessionError> {
        match self
            .event_service
            .recovery_boundary_after_for_client(
                &self.handle(),
                ProductEventSequence::new(requested_after),
            )
            .map_err(|error| registry_error(&self.client_id, error))?
        {
            ProductEventRecovery::Ready(boundary) => Ok(CodingAgentReconnect::Replayed {
                events: boundary.replay.into_iter().collect(),
                cursor: CodingAgentSnapshotCursor {
                    stream_id: self.snapshot.cursor.stream_id.clone(),
                    snapshot_protocol_major: UI_SNAPSHOT_PROTOCOL_VERSION.major,
                    last_event_sequence: boundary.replayed_through.get(),
                    capability_generation: boundary.capability_generation,
                },
                receiver: CodingAgentReconnectReceiver {
                    inner: boundary.receiver,
                    lifecycle_receiver: boundary.lifecycle_receiver,
                    lifecycle_epoch: boundary.lifecycle_epoch,
                    coordinator: self.coordinator.clone(),
                    client_id: self.client_id.clone(),
                    handle: self.handle(),
                    last_sequence: boundary.replayed_through.get(),
                    shutdown_delivered: false,
                },
            }),
            ProductEventRecovery::RetainedGap {
                requested_after,
                oldest_available,
            } => {
                let snapshot = self.state()?;
                Ok(CodingAgentReconnect::FreshSnapshotRequired(
                    CodingAgentFreshSnapshotRecovery {
                        requested_sequence: requested_after.get(),
                        oldest_available_sequence: oldest_available.get(),
                        fresh_cursor: snapshot.cursor.clone(),
                        reason: CodingAgentRecoveryReason::RetainedHistoryGap,
                        snapshot: Box::new(snapshot),
                    },
                ))
            }
        }
    }

    pub fn reconnect_from_cursor(
        &self,
        cursor: &CodingAgentSnapshotCursor,
    ) -> Result<CodingAgentReconnect, CodingSessionError> {
        if cursor.stream_id != self.snapshot.cursor.stream_id {
            return Err(CodingSessionError::Input {
                message: format!(
                    "snapshot cursor belongs to stream {}, expected {}",
                    cursor.stream_id, self.snapshot.cursor.stream_id
                ),
            });
        }
        if cursor.snapshot_protocol_major != UI_SNAPSHOT_PROTOCOL_VERSION.major {
            return Err(CodingSessionError::UnsupportedProtocolVersion {
                family: UI_SNAPSHOT_PROTOCOL_VERSION.family.into(),
                requested: format!(
                    "{}.{}.{}",
                    UI_SNAPSHOT_PROTOCOL_VERSION.family, cursor.snapshot_protocol_major, 0
                ),
                supported: UI_SNAPSHOT_PROTOCOL_VERSION.to_string(),
            });
        }
        self.reconnect(cursor.last_event_sequence)
    }

    pub fn set_prompt_draft(
        &self,
        id: CodingAgentDraftId,
        text: impl Into<String>,
    ) -> Result<(), CodingSessionError> {
        let text = text.into();
        self.coordinator
            .set_prompt_draft(
                &self.handle(),
                Some(DraftRecord {
                    id: id.0,
                    kind: ClientDraftKind::Prompt,
                    fingerprint: text.clone(),
                    text,
                }),
            )
            .map_err(|error| registry_error(&self.client_id, error))
    }

    pub(crate) fn set_prompt_operation_draft(
        &self,
        id: CodingAgentDraftId,
        display_text: impl Into<String>,
        operation: &crate::runtime::facade::CodingAgentOperation,
    ) -> Result<(), CodingSessionError> {
        let Some((_, fingerprint)) = operation.submission_fingerprint() else {
            return Err(CodingSessionError::Input {
                message: "prompt draft requires a fingerprintable prompt operation".into(),
            });
        };
        self.coordinator
            .set_prompt_draft(
                &self.handle(),
                Some(DraftRecord {
                    id: id.0,
                    kind: ClientDraftKind::Prompt,
                    text: display_text.into(),
                    fingerprint,
                }),
            )
            .map_err(|error| registry_error(&self.client_id, error))
    }

    pub fn enqueue_control_draft(
        &self,
        draft: CodingAgentDraft,
    ) -> Result<(), CodingAgentMutationRejection> {
        let kind = match draft.kind {
            CodingAgentDraftKind::Steer => ClientDraftKind::Steer,
            CodingAgentDraftKind::FollowUp => ClientDraftKind::FollowUp,
            CodingAgentDraftKind::Prompt => return Err(CodingAgentMutationRejection::InvalidInput),
        };
        self.coordinator
            .enqueue_draft(
                &self.handle(),
                DraftRecord {
                    id: draft.id.0,
                    kind,
                    fingerprint: draft.text.clone(),
                    text: draft.text,
                },
            )
            .map_err(|error| match error {
                ClientRegistryError::QueueCapacityExceeded { .. } => {
                    CodingAgentMutationRejection::QueueCapacity
                }
                ClientRegistryError::Lifecycle(
                    crate::runtime::error::CodingAgentLifecycleRejection::Detached,
                ) => CodingAgentMutationRejection::Detached,
                ClientRegistryError::Lifecycle(
                    crate::runtime::error::CodingAgentLifecycleRejection::StaleGeneration,
                ) => CodingAgentMutationRejection::StaleGeneration,
                ClientRegistryError::Lifecycle(
                    crate::runtime::error::CodingAgentLifecycleRejection::RuntimeShutDown,
                ) => CodingAgentMutationRejection::RuntimeShutDown,
                _ => CodingAgentMutationRejection::InvalidInput,
            })
    }

    pub(crate) fn clear_control_drafts(&self) -> Result<(), CodingSessionError> {
        self.coordinator
            .clear_control_drafts(&self.handle())
            .map_err(|error| registry_error(&self.client_id, error))
    }

    /// Prepare admission provenance for `CodingAgentSession::run` or runtime-owned `submit`.
    pub fn prepare_submission(
        &self,
        session: &mut crate::runtime::facade::CodingAgentSession,
        draft_id: CodingAgentDraftId,
        operation: &crate::runtime::facade::CodingAgentOperation,
    ) -> Result<CodingAgentSubmissionLease, CodingSessionError> {
        let handle = self.handle();
        let descriptor = operation.descriptor();
        let prompt_fingerprint = operation.submission_fingerprint();
        let expected_prompt_draft = if descriptor.submitted_kind
            == crate::runtime::control::OperationKind::Prompt
        {
            let Some((_, fingerprint)) = prompt_fingerprint.as_ref() else {
                return Err(CodingSessionError::Input {
                    message: "prompt submission preparation requires a fingerprintable invocation"
                        .into(),
                });
            };
            Some(
                self.coordinator
                    .validate_prompt_draft(&handle, &draft_id.0, fingerprint)
                    .map_err(|error| registry_error(&self.client_id, error))?,
            )
        } else {
            None
        };
        let shared = session.install_submission_lease(
            handle,
            descriptor,
            prompt_fingerprint,
            expected_prompt_draft,
        )?;
        let operation_id = format!("client:{}:{}", self.client_id.as_str(), self.generation.0);
        Ok(CodingAgentSubmissionLease {
            operation_id,
            shared,
        })
    }
}

#[derive(Debug)]
pub struct CodingAgentReconnectReceiver {
    inner: ProductEventReceiver,
    lifecycle_receiver: tokio::sync::watch::Receiver<u64>,
    lifecycle_epoch: u64,
    coordinator: Arc<SnapshotCoordinator>,
    client_id: CodingAgentClientId,
    handle: ClientHandle,
    last_sequence: u64,
    shutdown_delivered: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CodingAgentReconnectDelivery {
    Event(CodingAgentProductEvent),
    FreshSnapshotRequired(CodingAgentFreshSnapshotRecovery),
}

impl CodingAgentReconnectReceiver {
    pub async fn recv(&mut self) -> Result<CodingAgentReconnectDelivery, CodingSessionError> {
        if self.shutdown_delivered {
            return Err(CodingSessionError::Cancelled);
        }
        if let Err(error) = self.ensure_live()
            && !matches!(
                error,
                CodingSessionError::Lifecycle {
                    reason: crate::runtime::error::CodingAgentLifecycleRejection::RuntimeShutDown
                }
            )
        {
            return Err(error);
        }
        loop {
            tokio::select! {
                biased;
                event = self.inner.recv() => {
                    let delivery = match event {
                        Ok(event) => self.project_event(event),
                        Err(CodingSessionError::EventStreamLag { .. }) => {
                            return self.project_live_lag().and_then(|delivery| self.finish_delivery(delivery));
                        }
                        Err(error) => return Err(error),
                    };
                    return self.finish_delivery(delivery);
                }
                changed = self.lifecycle_receiver.changed() => {
                    changed.map_err(|_| CodingSessionError::Cancelled)?;
                    self.lifecycle_epoch = *self.lifecycle_receiver.borrow_and_update();
                    if let Err(error) = self.ensure_live() {
                        if matches!(
                            error,
                            CodingSessionError::Lifecycle {
                                reason: crate::runtime::error::CodingAgentLifecycleRejection::RuntimeShutDown
                            }
                        ) {
                            return Err(CodingSessionError::Cancelled);
                        }
                        return Err(error);
                    }
                }
            }
        }
    }

    pub fn try_recv(&mut self) -> Result<Option<CodingAgentReconnectDelivery>, CodingSessionError> {
        if self.shutdown_delivered {
            return Err(CodingSessionError::Cancelled);
        }
        let delivery = match self.inner.try_recv() {
            Ok(Some(event)) => {
                let delivery = self.project_event(event);
                self.finish_delivery(delivery).map(Some)
            }
            Ok(None) => {
                self.observe_lifecycle()?;
                Ok(None)
            }
            Err(CodingSessionError::EventStreamLag { .. }) => self
                .project_live_lag()
                .and_then(|delivery| self.finish_delivery(delivery))
                .map(Some),
            Err(error) => Err(error),
        }?;
        Ok(delivery)
    }

    fn finish_delivery(
        &mut self,
        delivery: CodingAgentReconnectDelivery,
    ) -> Result<CodingAgentReconnectDelivery, CodingSessionError> {
        self.ensure_delivery_live(&delivery)?;
        if matches!(
            delivery,
            CodingAgentReconnectDelivery::Event(ref event)
                if matches!(
                    event.event(),
                    crate::events::CodingAgentProductEventKind::Runtime(
                        crate::events::CodingAgentRuntimeProductEvent::ShutDown
                    )
                )
        ) {
            self.shutdown_delivered = true;
        }
        Ok(delivery)
    }

    fn observe_lifecycle(&mut self) -> Result<(), CodingSessionError> {
        if self.lifecycle_receiver.has_changed().unwrap_or(true) {
            self.lifecycle_epoch = *self.lifecycle_receiver.borrow_and_update();
        }
        self.ensure_live()
    }

    fn ensure_live(&self) -> Result<(), CodingSessionError> {
        let _ = self.lifecycle_epoch;
        self.coordinator
            .validate_receiver(&self.handle)
            .map_err(|error| registry_error(&self.client_id, error))
    }

    fn ensure_delivery_live(
        &self,
        delivery: &CodingAgentReconnectDelivery,
    ) -> Result<(), CodingSessionError> {
        let delivery_sequence = match delivery {
            CodingAgentReconnectDelivery::Event(event) => Some(event.sequence()),
            CodingAgentReconnectDelivery::FreshSnapshotRequired(recovery)
                if recovery.reason == CodingAgentRecoveryReason::LiveReceiverLag =>
            {
                Some(recovery.fresh_cursor.last_event_sequence)
            }
            CodingAgentReconnectDelivery::FreshSnapshotRequired(_) => None,
        };
        if delivery_sequence.is_some_and(|sequence| {
            self.coordinator
                .validate_receiver_event(
                    &self.handle,
                    crate::events::ProductEventSequence::new(sequence),
                )
                .is_ok()
        }) {
            return Ok(());
        }
        match self.ensure_live() {
            Ok(()) => Ok(()),
            Err(CodingSessionError::Lifecycle {
                reason: crate::runtime::error::CodingAgentLifecycleRejection::RuntimeShutDown,
            }) if matches!(
                delivery,
                CodingAgentReconnectDelivery::Event(event)
                    if matches!(
                        event.event(),
                        crate::events::CodingAgentProductEventKind::Runtime(
                            crate::events::CodingAgentRuntimeProductEvent::ShutDown
                        )
                    )
            ) =>
            {
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn project_event(
        &mut self,
        event: crate::events::ProductEvent,
    ) -> CodingAgentReconnectDelivery {
        self.last_sequence = event.sequence();
        CodingAgentReconnectDelivery::Event(event)
    }

    fn project_live_lag(&self) -> Result<CodingAgentReconnectDelivery, CodingSessionError> {
        let (state, oldest_available_sequence) =
            self.coordinator
                .live_lag_recovery(&self.handle)
                .map_err(|error| registry_error(&self.client_id, error))?;
        let snapshot = public_client_snapshot(state);
        Ok(CodingAgentReconnectDelivery::FreshSnapshotRequired(
            CodingAgentFreshSnapshotRecovery {
                requested_sequence: self.last_sequence,
                oldest_available_sequence,
                fresh_cursor: snapshot.cursor.clone(),
                reason: CodingAgentRecoveryReason::LiveReceiverLag,
                snapshot: Box::new(snapshot),
            },
        ))
    }
}

#[derive(Debug)]
pub struct CodingAgentProductEventReceiver {
    inner: ProductEventReceiver,
}

impl CodingAgentProductEventReceiver {
    pub(crate) fn new(inner: ProductEventReceiver) -> Self {
        Self { inner }
    }

    pub async fn recv(&mut self) -> Result<CodingAgentProductEvent, CodingSessionError> {
        self.inner.recv().await
    }

    pub fn try_recv(&mut self) -> Result<Option<CodingAgentProductEvent>, CodingSessionError> {
        self.inner.try_recv()
    }
}

impl From<UiSnapshot> for CodingAgentSnapshot {
    fn from(snapshot: UiSnapshot) -> Self {
        Self {
            cursor: CodingAgentSnapshotCursor {
                stream_id: snapshot.cursor.stream_id.clone(),
                snapshot_protocol_major: UI_SNAPSHOT_PROTOCOL_VERSION.major,
                last_event_sequence: snapshot.cursor.last_event_sequence.get(),
                capability_generation: snapshot.cursor.capability_generation.get(),
            },
            version: snapshot.version,
            session: snapshot.session,
            capabilities: snapshot.capabilities,
            active_operation: snapshot
                .active_operation
                .map(|kind| kind.as_str().to_owned()),
            pending_authorizations: snapshot.pending_authorizations,
            context: snapshot.context.into(),
            drafts: snapshot
                .client_drafts
                .into_iter()
                .enumerate()
                .map(|(index, draft)| CodingAgentDraft {
                    id: CodingAgentDraftId(index.to_string()),
                    kind: match draft.kind {
                        crate::runtime::client::state::ClientDraftKind::Prompt => {
                            CodingAgentDraftKind::Prompt
                        }
                        crate::runtime::client::state::ClientDraftKind::Steer => {
                            CodingAgentDraftKind::Steer
                        }
                        crate::runtime::client::state::ClientDraftKind::FollowUp => {
                            CodingAgentDraftKind::FollowUp
                        }
                    },
                    text: draft.text,
                })
                .collect(),
            submitted_operation: None,
        }
    }
}

impl From<UiOperationStatus> for CodingAgentOperationStatus {
    fn from(status: UiOperationStatus) -> Self {
        match status {
            UiOperationStatus::Running => Self::Running,
            UiOperationStatus::Completed => Self::Completed,
            UiOperationStatus::Failed => Self::Failed,
            UiOperationStatus::Aborted => Self::Aborted,
            UiOperationStatus::Recovered => Self::Recovered,
        }
    }
}

impl From<UiOperationProjection> for CodingAgentOperationSnapshot {
    fn from(operation: UiOperationProjection) -> Self {
        Self {
            operation_id: operation.operation_id,
            kind: operation.kind,
            parent_operation_id: operation.parent_operation_id,
            root_operation_id: operation.root_operation_id,
            status: operation.status.into(),
            started_sequence: operation.started_sequence,
            updated_sequence: operation.updated_sequence,
            diagnostics: operation.diagnostics,
            failure: operation.failure,
        }
    }
}

impl From<UiFileChangeProjection> for CodingAgentFileChangeSnapshot {
    fn from(change: UiFileChangeProjection) -> Self {
        Self {
            path: change.path,
            mutation_kind: change.mutation_kind,
            operation_id: change.operation_id,
            tool_call_id: change.tool_call_id,
            updated_sequence: change.updated_sequence,
            first_changed_line: change.first_changed_line,
            added_lines: change.added_lines,
            removed_lines: change.removed_lines,
            diff: change.diff,
        }
    }
}

impl From<UiDelegationProjection> for CodingAgentDelegationSnapshot {
    fn from(delegation: UiDelegationProjection) -> Self {
        Self {
            tool_call_id: delegation.tool_call_id,
            child_operation_id: delegation.child_operation_id,
            target_kind: delegation.target_kind,
            target_id: delegation.target_id,
            task: delegation.task,
            status: delegation.status,
            updated_sequence: delegation.updated_sequence,
            summary: delegation.summary,
            failure: delegation.failure,
        }
    }
}

impl From<UiTurnUsageProjection> for CodingAgentTurnUsageSnapshot {
    fn from(usage: UiTurnUsageProjection) -> Self {
        Self {
            turn_id: usage.turn_id,
            input: usage.input,
            output: usage.output,
            cache_read: usage.cache_read,
            cache_write: usage.cache_write,
            context_tokens: usage.context_tokens,
            cost: usage.cost,
        }
    }
}

impl From<UiUsageProjection> for CodingAgentUsageSnapshot {
    fn from(usage: UiUsageProjection) -> Self {
        Self {
            input: usage.input,
            output: usage.output,
            cache_read: usage.cache_read,
            cache_write: usage.cache_write,
            cost: usage.cost,
            latest_turn: usage.latest_turn.map(Into::into),
            model_id: usage.model_id,
            context_window: usage.context_window,
        }
    }
}

impl From<UiContextProjection> for CodingAgentContextSnapshot {
    fn from(context: UiContextProjection) -> Self {
        Self {
            operations: context.operations.into_iter().map(Into::into).collect(),
            changes: context.changes.into_iter().map(Into::into).collect(),
            delegations: context.delegations.into_iter().map(Into::into).collect(),
            usage: context.usage.into(),
        }
    }
}

fn public_client_snapshot(state: ClientSnapshotState) -> CodingAgentSnapshot {
    let mut snapshot: CodingAgentSnapshot = state.snapshot.into();
    snapshot.drafts = state
        .drafts
        .into_iter()
        .map(|draft| CodingAgentDraft {
            id: CodingAgentDraftId(draft.id),
            kind: match draft.kind {
                ClientDraftKind::Prompt => CodingAgentDraftKind::Prompt,
                ClientDraftKind::Steer => CodingAgentDraftKind::Steer,
                ClientDraftKind::FollowUp => CodingAgentDraftKind::FollowUp,
            },
            text: draft.text,
        })
        .collect();
    snapshot.submitted_operation = state.submitted_operation.map(|submitted| match submitted {
        SubmittedOperationStatus::Running {
            operation_id, kind, ..
        } => CodingAgentSubmittedOperation {
            operation_id,
            kind: kind.as_str().into(),
            status: CodingAgentSubmittedOperationStatus::Running,
        },
        SubmittedOperationStatus::Terminal {
            operation_id,
            kind,
            anchor,
            status,
            ..
        } => CodingAgentSubmittedOperation {
            operation_id,
            kind: kind.as_str().into(),
            status: CodingAgentSubmittedOperationStatus::Terminal {
                status,
                anchor: match anchor {
                    crate::runtime::snapshot::SubmittedTerminalAnchor::ProductEvent {
                        sequence,
                        durability,
                    } => CodingAgentSubmittedTerminalAnchor::ProductEvent {
                        sequence,
                        durability: match durability {
                            crate::runtime::snapshot::SubmittedEventDurability::Durable => {
                                CodingAgentSubmittedEventDurability::Durable
                            }
                            crate::runtime::snapshot::SubmittedEventDurability::Uncertain => {
                                CodingAgentSubmittedEventDurability::Uncertain
                            }
                        },
                    },
                    crate::runtime::snapshot::SubmittedTerminalAnchor::OutcomeOnly {
                        acknowledgement,
                    } => CodingAgentSubmittedTerminalAnchor::OutcomeOnly { acknowledgement },
                    crate::runtime::snapshot::SubmittedTerminalAnchor::TerminalUncertain {
                        operation_id,
                    } => CodingAgentSubmittedTerminalAnchor::TerminalUncertain {
                        operation_id,
                        recovery: CodingAgentTerminalUncertainty::RecoveryRequired,
                    },
                },
            },
        },
    });
    snapshot
}

fn registry_error(_id: &CodingAgentClientId, error: ClientRegistryError) -> CodingSessionError {
    match error {
        ClientRegistryError::Lifecycle(reason) => CodingSessionError::Lifecycle { reason },
        ClientRegistryError::ClientCapacityExceeded { limit } => {
            CodingSessionError::ClientCapacityExceeded { limit }
        }
        ClientRegistryError::SubmissionDraftMismatch => CodingSessionError::SubmissionDraftMismatch,
        other => CodingSessionError::Input {
            message: other.to_string(),
        },
    }
}

pub(crate) fn internal_client_id(id: &CodingAgentClientId) -> ClientConnectionId {
    ClientConnectionId::new(id.as_str())
}

pub(crate) fn public_client_connection(
    id: CodingAgentClientId,
    coordinator: Arc<SnapshotCoordinator>,
    event_service: EventService,
    authorization_service: crate::services::authorization::AuthorizationService,
    handle: ClientHandle,
    state: ClientSnapshotState,
) -> CodingAgentClientConnection {
    debug_assert_eq!(handle.id.as_str(), id.as_str());
    CodingAgentClientConnection {
        coordinator,
        event_service,
        authorization_service,
        client_id: id,
        generation: CodingAgentConnectionGeneration(handle.generation.0),
        snapshot: public_client_snapshot(state),
    }
}

#[cfg(test)]
#[path = "../../internal_tests/product_event_projection.rs"]
mod product_event_projection_tests;
