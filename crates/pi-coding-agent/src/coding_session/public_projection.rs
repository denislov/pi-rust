use super::client_projection::{ClientConnection, ClientConnectionId, UiSnapshot};
use super::context::{CodingAgentCapabilities, CodingAgentSessionView};
use super::error::CodingSessionError;
use super::event_service::ProductEventReceiver;
use super::public_event::{CodingAgentProductEvent, CodingAgentProductEventTerminalStatus};
use crate::protocol::version::ProtocolFamilyVersion;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

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

#[derive(Debug, Clone, PartialEq)]
pub enum CodingAgentReconnect {
    Replayed {
        events: Vec<CodingAgentProductEvent>,
        cursor: CodingAgentSnapshotCursor,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodingAgentPromptControl {
    pub client_id: CodingAgentClientId,
    pub generation: CodingAgentConnectionGeneration,
    pub operation_id: String,
}

#[derive(Debug)]
pub struct CodingAgentSubmissionLease {
    operation_id: String,
    _not_clone: PhantomData<*mut ()>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentClientConnection {
    pub client_id: CodingAgentClientId,
    pub generation: CodingAgentConnectionGeneration,
    pub snapshot: CodingAgentSnapshot,
}

impl CodingAgentClientConnection {
    /// Prepare admission provenance; ordinary execution remains on `CodingAgentSession::run`.
    pub fn prepare_submission(
        &self,
        _session: &mut super::CodingAgentSession,
        _draft_id: CodingAgentDraftId,
        operation: &super::CodingAgentOperation,
    ) -> Result<CodingAgentSubmissionLease, CodingSessionError> {
        let operation_id = format!("client:{}:{}", self.client_id.as_str(), self.generation.0);
        let _ = operation;
        Ok(CodingAgentSubmissionLease {
            operation_id,
            _not_clone: PhantomData,
        })
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

pub(crate) fn internal_client_id(id: &CodingAgentClientId) -> ClientConnectionId {
    ClientConnectionId::new(id.as_str())
}

pub(crate) fn public_client_connection(
    id: CodingAgentClientId,
    connection: ClientConnection,
    snapshot: UiSnapshot,
) -> CodingAgentClientConnection {
    debug_assert_eq!(connection.id().as_str(), id.as_str());
    CodingAgentClientConnection {
        client_id: id,
        generation: CodingAgentConnectionGeneration(0),
        snapshot: snapshot.into(),
    }
}
