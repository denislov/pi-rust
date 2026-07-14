use super::ProductEventSequence;
use super::client_projection::{ClientConnectionId, ClientDraftKind, UiSnapshot};
use super::context::{CodingAgentCapabilities, CodingAgentSessionView};
use super::error::CodingSessionError;
use super::event_service::{EventService, ProductEventReceiver, ProductEventRecovery};
use super::public_event::{CodingAgentProductEvent, CodingAgentProductEventTerminalStatus};
use crate::protocol::version::ProtocolFamilyVersion;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use super::snapshot_coordinator::{
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
    Accepted,
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
        self.coordinator.enqueue_prompt_control(
            &self.handle(),
            &self.operation_id,
            control_id,
            kind,
            text,
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

    pub fn follow_up(
        &self,
        control_id: CodingAgentControlId,
        text: impl Into<String>,
    ) -> Result<CodingAgentControlReceipt, CodingAgentControlRejection> {
        self.submit(control_id, CodingAgentControlKind::FollowUp, text.into())
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
    pub(crate) shared: Arc<Mutex<super::SubmissionLeaseLifecycle>>,
}

impl Drop for CodingAgentSubmissionLease {
    fn drop(&mut self) {
        let mut lifecycle = self.shared.lock().unwrap();
        if matches!(*lifecycle, super::SubmissionLeaseLifecycle::Prepared) {
            *lifecycle = super::SubmissionLeaseLifecycle::Abandoned;
        }
    }
}

impl CodingAgentSubmissionLease {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodingAgentSnapshotCursor {
    pub last_event_sequence: u64,
    pub capability_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentSnapshot {
    pub cursor: CodingAgentSnapshotCursor,
    pub version: ProtocolFamilyVersion,
    pub session: CodingAgentSessionView,
    pub capabilities: CodingAgentCapabilities,
    pub active_operation: Option<String>,
    pub drafts: Vec<CodingAgentDraft>,
    pub submitted_operation: Option<CodingAgentSubmittedOperation>,
}

#[derive(Debug, Clone)]
pub struct CodingAgentClientConnection {
    coordinator: Arc<SnapshotCoordinator>,
    event_service: EventService,
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

    pub fn prompt_control(&self, operation_id: impl Into<String>) -> CodingAgentPromptControl {
        CodingAgentPromptControl {
            client_id: self.client_id.clone(),
            generation: self.generation,
            operation_id: operation_id.into(),
            coordinator: self.coordinator.clone(),
        }
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
                events: boundary
                    .replay
                    .into_iter()
                    .map(CodingAgentProductEvent::from_internal)
                    .collect(),
                cursor: CodingAgentSnapshotCursor {
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

    pub fn set_prompt_draft(
        &self,
        id: CodingAgentDraftId,
        text: impl Into<String>,
    ) -> Result<(), CodingSessionError> {
        self.coordinator
            .set_prompt_draft(
                &self.handle(),
                Some(DraftRecord {
                    id: id.0,
                    kind: ClientDraftKind::Prompt,
                    text: text.into(),
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
                    text: draft.text,
                },
            )
            .map_err(|error| match error {
                ClientRegistryError::QueueCapacityExceeded { .. } => {
                    CodingAgentMutationRejection::QueueCapacity
                }
                ClientRegistryError::Lifecycle(
                    super::error::CodingAgentLifecycleRejection::Detached,
                ) => CodingAgentMutationRejection::Detached,
                ClientRegistryError::Lifecycle(
                    super::error::CodingAgentLifecycleRejection::StaleGeneration,
                )
                | ClientRegistryError::StaleClient => CodingAgentMutationRejection::StaleGeneration,
                ClientRegistryError::Lifecycle(
                    super::error::CodingAgentLifecycleRejection::RuntimeShutDown,
                ) => CodingAgentMutationRejection::RuntimeShutDown,
                _ => CodingAgentMutationRejection::InvalidInput,
            })
    }

    /// Prepare admission provenance; ordinary execution remains on `CodingAgentSession::run`.
    pub fn prepare_submission(
        &self,
        session: &mut super::CodingAgentSession,
        draft_id: CodingAgentDraftId,
        operation: &super::CodingAgentOperation,
    ) -> Result<CodingAgentSubmissionLease, CodingSessionError> {
        let handle = self.handle();
        let descriptor = operation.descriptor();
        let prompt_fingerprint = operation.submission_fingerprint();
        let expected_prompt_draft =
            if descriptor.submitted_kind == super::operation_control::OperationKind::Prompt {
                let Some((_, text)) = prompt_fingerprint.as_ref() else {
                    return Err(CodingSessionError::Input {
                        message: "Prompt submission preparation requires a text invocation".into(),
                    });
                };
                self.coordinator
                    .validate_prompt_draft(&handle, &draft_id.0, text)
                    .map_err(|error| registry_error(&self.client_id, error))?;
                Some(DraftRecord {
                    id: draft_id.0,
                    kind: ClientDraftKind::Prompt,
                    text: text.clone(),
                })
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
                    reason: super::error::CodingAgentLifecycleRejection::RuntimeShutDown
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
                                reason: super::error::CodingAgentLifecycleRejection::RuntimeShutDown
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
                    super::public_event::CodingAgentProductEventKind::Runtime(
                        super::public_event::CodingAgentRuntimeProductEvent::ShutDown
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
                    super::event::ProductEventSequence::new(sequence),
                )
                .is_ok()
        }) {
            return Ok(());
        }
        match self.ensure_live() {
            Ok(()) => Ok(()),
            Err(CodingSessionError::Lifecycle {
                reason: super::error::CodingAgentLifecycleRejection::RuntimeShutDown,
            }) if matches!(
                delivery,
                CodingAgentReconnectDelivery::Event(event)
                    if matches!(
                        event.event(),
                        super::public_event::CodingAgentProductEventKind::Runtime(
                            super::public_event::CodingAgentRuntimeProductEvent::ShutDown
                        )
                    )
            ) =>
            {
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn project_event(&mut self, event: super::ProductEvent) -> CodingAgentReconnectDelivery {
        self.last_sequence = event.sequence().get();
        CodingAgentReconnectDelivery::Event(CodingAgentProductEvent::from_internal(event))
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
        self.inner
            .recv()
            .await
            .map(CodingAgentProductEvent::from_internal)
    }

    pub fn try_recv(&mut self) -> Result<Option<CodingAgentProductEvent>, CodingSessionError> {
        self.inner
            .try_recv()
            .map(|event| event.map(CodingAgentProductEvent::from_internal))
    }
}

#[cfg(test)]
mod product_event_projection_tests {
    use std::sync::Arc;

    use pi_agent_core::AgentResources;
    use pi_ai::providers::faux::FauxProvider;
    use pi_ai::types::{Model, ModelCost, ModelInput};

    use super::*;
    use crate::coding_session::public_event::{
        CodingAgentProductEvent, CodingAgentProductEventKind, CodingAgentRuntimeProductEvent,
    };
    use crate::coding_session::{
        CodingAgentEvent, CodingAgentOperation, CodingAgentSession, CodingAgentSessionOptions,
        CodingAgentShutdownOutcome, CodingSessionError, PromptTurnOptions,
    };
    use crate::prompt_options::PromptRunOptions;
    use crate::runtime::{PromptInvocation, SessionRunOptions};

    fn model(api: &str) -> Model {
        Model {
            id: "shutdown-lag-model".into(),
            name: "Shutdown Lag Model".into(),
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

    fn prompt_options(api: &str) -> PromptTurnOptions {
        PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
            prompt: "force real reconnect lag".into(),
            model: model(api),
            api_key: None,
            auth_diagnostics: Vec::new(),
            system_prompt: Some("test".into()),
            max_turns: Some(1),
            tools: Vec::new(),
            register_builtins: false,
            session: Some(SessionRunOptions::disabled(".".into())),
            session_target: None,
            session_name: None,
            thinking_level: None,
            tool_execution: None,
            resources: AgentResources::default(),
            settings: None,
            invocation: PromptInvocation::Text("force real reconnect lag".into()),
        })
    }

    #[allow(dead_code)]
    async fn receiver_returns_authoritative_typed_event(
        receiver: &mut CodingAgentProductEventReceiver,
    ) -> CodingAgentProductEvent {
        receiver.recv().await.unwrap()
    }

    #[tokio::test]
    async fn lagged_reconnect_after_shutdown_recovers_then_delivers_runtime_shutdown_and_closes() {
        let api = "projection-real-shutdown-lag";
        let _provider = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::simple_text("lagged response")),
        );
        let mut session = CodingAgentSession::non_persistent_with_event_capacities_for_tests(
            CodingAgentSessionOptions::new(),
            1,
            64,
        )
        .await
        .unwrap();
        let async_connection = session
            .connect(CodingAgentClientId::new("async-lag-client"))
            .unwrap();
        let try_connection = session
            .connect(CodingAgentClientId::new("try-lag-client"))
            .unwrap();
        let CodingAgentReconnect::Replayed {
            receiver: mut async_receiver,
            ..
        } = async_connection.reconnect(0).unwrap()
        else {
            panic!("empty cursor must establish async live delivery")
        };
        let CodingAgentReconnect::Replayed {
            receiver: mut try_receiver,
            ..
        } = try_connection.reconnect(0).unwrap()
        else {
            panic!("empty cursor must establish try_recv live delivery")
        };

        session
            .run(CodingAgentOperation::Prompt(prompt_options(api)))
            .await
            .unwrap();
        assert_eq!(
            session.shutdown().await.unwrap(),
            CodingAgentShutdownOutcome::ShutDown
        );

        let CodingAgentReconnectDelivery::FreshSnapshotRequired(async_recovery) =
            async_receiver.recv().await.unwrap()
        else {
            panic!("async receiver must take the real lag recovery path")
        };
        let async_boundary = async_recovery.fresh_cursor.last_event_sequence;
        assert_eq!(
            async_recovery.reason,
            CodingAgentRecoveryReason::LiveReceiverLag
        );
        let CodingAgentReconnectDelivery::Event(async_shutdown) =
            async_receiver.recv().await.unwrap()
        else {
            panic!("async receiver must deliver Runtime.ShutDown after recovery")
        };
        assert_eq!(async_shutdown.sequence(), async_boundary);
        assert!(matches!(
            async_shutdown.event(),
            CodingAgentProductEventKind::Runtime(CodingAgentRuntimeProductEvent::ShutDown)
        ));
        assert_eq!(
            async_receiver.recv().await.unwrap_err(),
            CodingSessionError::Cancelled
        );

        let Some(CodingAgentReconnectDelivery::FreshSnapshotRequired(try_recovery)) =
            try_receiver.try_recv().unwrap()
        else {
            panic!("try_recv receiver must take the real lag recovery path")
        };
        let try_boundary = try_recovery.fresh_cursor.last_event_sequence;
        assert_eq!(
            try_recovery.reason,
            CodingAgentRecoveryReason::LiveReceiverLag
        );
        let Some(CodingAgentReconnectDelivery::Event(try_shutdown)) =
            try_receiver.try_recv().unwrap()
        else {
            panic!("try_recv receiver must deliver Runtime.ShutDown after recovery")
        };
        assert_eq!(try_shutdown.sequence(), try_boundary);
        assert!(matches!(
            try_shutdown.event(),
            CodingAgentProductEventKind::Runtime(CodingAgentRuntimeProductEvent::ShutDown)
        ));
        assert_eq!(
            try_receiver.try_recv().unwrap_err(),
            CodingSessionError::Cancelled
        );
    }

    #[tokio::test]
    async fn receiver_detached_before_phase_a_never_enters_shutdown_drain() {
        let mut session = CodingAgentSession::non_persistent_with_event_capacities_for_tests(
            CodingAgentSessionOptions::new(),
            8,
            16,
        )
        .await
        .unwrap();
        let detached = session
            .connect(CodingAgentClientId::new("pre-phase-a-detached"))
            .unwrap();
        let CodingAgentReconnect::Replayed {
            receiver: mut detached_receiver,
            ..
        } = detached.reconnect(0).unwrap()
        else {
            panic!("detached control must start with a live receiver")
        };
        session.event_service.emit(CodingAgentEvent::Diagnostic {
            operation_id: None,
            message: "queued before detach".into(),
        });
        assert_eq!(
            detached.detach().unwrap(),
            CodingAgentDetachOutcome::Detached
        );

        let attached = session
            .connect(CodingAgentClientId::new("phase-a-participant"))
            .unwrap();
        let cursor = session.event_service.current_product_sequence().get();
        let CodingAgentReconnect::Replayed {
            receiver: mut attached_receiver,
            ..
        } = attached.reconnect(cursor).unwrap()
        else {
            panic!("attached control must establish live delivery")
        };
        session.runtime_shutdown_handle().request_shutdown();
        session.shutdown().await.unwrap();

        assert_eq!(
            detached_receiver.recv().await.unwrap_err(),
            CodingSessionError::Lifecycle {
                reason: super::super::error::CodingAgentLifecycleRejection::Detached
            }
        );
        let Some(CodingAgentReconnectDelivery::Event(shutdown)) =
            attached_receiver.try_recv().unwrap()
        else {
            panic!("Phase-A participant must drain Runtime.ShutDown")
        };
        assert!(matches!(
            shutdown.event(),
            CodingAgentProductEventKind::Runtime(CodingAgentRuntimeProductEvent::ShutDown)
        ));
        assert_eq!(
            attached_receiver.try_recv().unwrap_err(),
            CodingSessionError::Cancelled
        );
    }
}

impl From<UiSnapshot> for CodingAgentSnapshot {
    fn from(snapshot: UiSnapshot) -> Self {
        Self {
            cursor: CodingAgentSnapshotCursor {
                last_event_sequence: snapshot.cursor.last_event_sequence.get(),
                capability_generation: snapshot.cursor.capability_generation.get(),
            },
            version: snapshot.version,
            session: snapshot.session,
            capabilities: snapshot.capabilities,
            active_operation: snapshot
                .active_operation
                .map(|kind| kind.as_str().to_owned()),
            drafts: snapshot
                .client_drafts
                .into_iter()
                .enumerate()
                .map(|(index, draft)| CodingAgentDraft {
                    id: CodingAgentDraftId(index.to_string()),
                    kind: match draft.kind {
                        super::client_projection::ClientDraftKind::Prompt => {
                            CodingAgentDraftKind::Prompt
                        }
                        super::client_projection::ClientDraftKind::Steer => {
                            CodingAgentDraftKind::Steer
                        }
                        super::client_projection::ClientDraftKind::FollowUp => {
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
        SubmittedOperationStatus::Accepted {
            operation_id, kind, ..
        } => CodingAgentSubmittedOperation {
            operation_id,
            kind: kind.as_str().into(),
            status: CodingAgentSubmittedOperationStatus::Accepted,
        },
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
                status: status.into(),
                anchor: match anchor {
                    super::snapshot_coordinator::SubmittedTerminalAnchor::ProductEvent {
                        sequence,
                        durability,
                    } => CodingAgentSubmittedTerminalAnchor::ProductEvent {
                        sequence,
                        durability: match durability {
                            super::snapshot_coordinator::SubmittedEventDurability::Durable => {
                                CodingAgentSubmittedEventDurability::Durable
                            }
                            super::snapshot_coordinator::SubmittedEventDurability::Uncertain => {
                                CodingAgentSubmittedEventDurability::Uncertain
                            }
                        },
                    },
                    super::snapshot_coordinator::SubmittedTerminalAnchor::OutcomeOnly {
                        acknowledgement,
                    } => CodingAgentSubmittedTerminalAnchor::OutcomeOnly { acknowledgement },
                    super::snapshot_coordinator::SubmittedTerminalAnchor::TerminalUncertain {
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

fn registry_error(id: &CodingAgentClientId, error: ClientRegistryError) -> CodingSessionError {
    match error {
        ClientRegistryError::Lifecycle(reason) => CodingSessionError::Lifecycle { reason },
        ClientRegistryError::StaleClient => CodingSessionError::StaleClientConnection {
            client_id: id.as_str().into(),
        },
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
    handle: ClientHandle,
    state: ClientSnapshotState,
) -> CodingAgentClientConnection {
    debug_assert_eq!(handle.id.as_str(), id.as_str());
    CodingAgentClientConnection {
        coordinator,
        event_service,
        client_id: id,
        generation: CodingAgentConnectionGeneration(handle.generation.0),
        snapshot: public_client_snapshot(state),
    }
}
