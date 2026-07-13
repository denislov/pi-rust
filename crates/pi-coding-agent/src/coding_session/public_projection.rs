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
    ClientGeneration, ClientHandle, ClientRegistryError, ClientSnapshotState, DraftRecord,
    SnapshotCoordinator, SubmittedOperationStatus,
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
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodingAgentSubmittedOperation {
    pub operation_id: String,
    pub kind: String,
    pub status: CodingAgentSubmittedOperationStatus,
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
                    coordinator: self.coordinator.clone(),
                    client_id: self.client_id.clone(),
                    handle: self.handle(),
                    last_sequence: boundary.replayed_through.get(),
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
                ClientRegistryError::StaleClient => CodingAgentMutationRejection::NotOwner,
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
        let Some((kind, text)) = operation.submission_fingerprint() else {
            return Err(CodingSessionError::Input {
                message: "submission preparation requires a text Prompt operation".into(),
            });
        };
        let handle = self.handle();
        self.coordinator
            .validate_prompt_draft(&handle, &draft_id.0, &text)
            .map_err(|error| registry_error(&self.client_id, error))?;
        let shared = session.install_submission_lease(handle, draft_id.0, kind, text)?;
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
    coordinator: Arc<SnapshotCoordinator>,
    client_id: CodingAgentClientId,
    handle: ClientHandle,
    last_sequence: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CodingAgentReconnectDelivery {
    Event(CodingAgentProductEvent),
    FreshSnapshotRequired(CodingAgentFreshSnapshotRecovery),
}

impl CodingAgentReconnectReceiver {
    pub async fn recv(&mut self) -> Result<CodingAgentReconnectDelivery, CodingSessionError> {
        match self.inner.recv().await {
            Ok(event) => Ok(self.project_event(event)),
            Err(CodingSessionError::EventStreamLag { .. }) => self.project_live_lag(),
            Err(error) => Err(error),
        }
    }

    pub fn try_recv(&mut self) -> Result<Option<CodingAgentReconnectDelivery>, CodingSessionError> {
        match self.inner.try_recv() {
            Ok(Some(event)) => Ok(Some(self.project_event(event))),
            Ok(None) => Ok(None),
            Err(CodingSessionError::EventStreamLag { .. }) => self.project_live_lag().map(Some),
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
    use super::CodingAgentProductEventReceiver;
    use crate::coding_session::public_event::CodingAgentProductEvent;

    #[allow(dead_code)]
    async fn receiver_returns_authoritative_typed_event(
        receiver: &mut CodingAgentProductEventReceiver,
    ) -> CodingAgentProductEvent {
        receiver.recv().await.unwrap()
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
        SubmittedOperationStatus::Accepted { operation_id, kind } => {
            CodingAgentSubmittedOperation {
                operation_id,
                kind: kind.as_str().into(),
                status: CodingAgentSubmittedOperationStatus::Accepted,
            }
        }
        SubmittedOperationStatus::Running { operation_id, kind } => CodingAgentSubmittedOperation {
            operation_id,
            kind: kind.as_str().into(),
            status: CodingAgentSubmittedOperationStatus::Running,
        },
        SubmittedOperationStatus::Terminal {
            operation_id,
            kind,
            status,
            ..
        } => CodingAgentSubmittedOperation {
            operation_id,
            kind: kind.as_str().into(),
            status: CodingAgentSubmittedOperationStatus::Terminal {
                status: status.into(),
            },
        },
    });
    snapshot
}

fn registry_error(id: &CodingAgentClientId, error: ClientRegistryError) -> CodingSessionError {
    match error {
        ClientRegistryError::StaleClient => CodingSessionError::StaleClientConnection {
            client_id: id.as_str().into(),
        },
        ClientRegistryError::ClientCapacityExceeded { limit } => {
            CodingSessionError::ClientCapacityExceeded { limit }
        }
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
