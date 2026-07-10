use super::client_projection::{ClientConnection, ClientConnectionId, UiSnapshot};
use super::context::{CodingAgentCapabilities, CodingAgentSessionView};
use super::error::CodingSessionError;
use super::event::ProductEvent;
use super::event_service::ProductEventReceiver;
use crate::protocol::version::ProtocolFamilyVersion;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CodingAgentClientId(String);

impl CodingAgentClientId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub client_draft_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentClientConnection {
    pub client_id: CodingAgentClientId,
    pub snapshot: CodingAgentSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentProductEvent {
    pub sequence: u64,
    pub family: String,
    pub kind: String,
}

impl From<ProductEvent> for CodingAgentProductEvent {
    fn from(event: ProductEvent) -> Self {
        Self {
            sequence: event.sequence().get(),
            family: format!("{:?}", event.family()),
            kind: format!("{:?}", event.kind()),
        }
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
        self.inner.recv().await.map(CodingAgentProductEvent::from)
    }

    pub fn try_recv(&mut self) -> Result<Option<CodingAgentProductEvent>, CodingSessionError> {
        self.inner
            .try_recv()
            .map(|event| event.map(CodingAgentProductEvent::from))
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
            client_draft_count: snapshot.client_drafts.len(),
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
        snapshot: snapshot.into(),
    }
}
