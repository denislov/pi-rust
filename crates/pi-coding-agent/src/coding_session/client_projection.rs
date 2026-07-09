#![allow(dead_code)]

use super::CodingAgentCapabilities;
use super::capability_snapshot::CapabilityGeneration;
use super::context::CodingAgentSessionView;
use super::event::ProductEventSequence;
use super::operation_control::OperationKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UiSnapshotCursor {
    pub(crate) last_event_sequence: ProductEventSequence,
    pub(crate) capability_generation: CapabilityGeneration,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UiSnapshot {
    pub(crate) cursor: UiSnapshotCursor,
    pub(crate) session: CodingAgentSessionView,
    pub(crate) capabilities: CodingAgentCapabilities,
    pub(crate) active_operation: Option<OperationKind>,
    pub(crate) client_drafts: Vec<ClientDraft>,
}

impl UiSnapshot {
    pub(crate) fn new(
        cursor: UiSnapshotCursor,
        session: CodingAgentSessionView,
        capabilities: CodingAgentCapabilities,
        active_operation: Option<OperationKind>,
        client_drafts: Vec<ClientDraft>,
    ) -> Self {
        Self {
            cursor,
            session,
            capabilities,
            active_operation,
            client_drafts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClientConnectionId(String);

impl ClientConnectionId {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClientDraftKind {
    Prompt,
    Steer,
    FollowUp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClientDraft {
    pub(crate) kind: ClientDraftKind,
    pub(crate) text: String,
}

impl ClientDraft {
    pub(crate) fn new(kind: ClientDraftKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SubmittedOperation {
    pub(crate) operation_id: String,
    pub(crate) kind: OperationKind,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClientConnection {
    pub(crate) id: ClientConnectionId,
    pub(crate) cursor: UiSnapshotCursor,
    pub(crate) client_drafts: Vec<ClientDraft>,
    pub(crate) submitted_operation: Option<SubmittedOperation>,
}

impl ClientConnection {
    pub(crate) fn new(id: ClientConnectionId, snapshot: UiSnapshot) -> Self {
        Self {
            id,
            cursor: snapshot.cursor,
            client_drafts: snapshot.client_drafts,
            submitted_operation: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::capability_snapshot::CapabilityGeneration;
    use crate::coding_session::context::CodingAgentSessionView;
    use crate::coding_session::event::ProductEventSequence;
    use crate::coding_session::operation_control::OperationKind;
    use crate::coding_session::profiles::ProfileId;
    use crate::coding_session::{CapabilityStatus, CodingAgentCapabilities};

    fn capabilities() -> CodingAgentCapabilities {
        CodingAgentCapabilities {
            prompt: CapabilityStatus::Available,
            abort: CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            },
            steer: CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            },
            follow_up: CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            },
            compact: CapabilityStatus::Available,
            fork: CapabilityStatus::Available,
            clone_session: CapabilityStatus::Available,
            branch_summary: CapabilityStatus::Available,
            switch_session: CapabilityStatus::Unsupported {
                reason: "session switching is not exposed on CodingAgentSession yet".into(),
            },
            export: CapabilityStatus::Available,
            plugin_reload: CapabilityStatus::Available,
            agent_profiles: CapabilityStatus::Available,
            team_profiles: CapabilityStatus::Available,
            delegation: CapabilityStatus::Available,
            self_healing_edit: CapabilityStatus::Available,
            tools: CapabilityStatus::Available,
            shell: CapabilityStatus::Available,
            plugins: CapabilityStatus::Available,
        }
    }

    #[test]
    fn ui_snapshot_carries_cursor_session_and_runtime_state() {
        let snapshot = UiSnapshot::new(
            UiSnapshotCursor {
                last_event_sequence: ProductEventSequence::new(7),
                capability_generation: CapabilityGeneration::new(3),
            },
            CodingAgentSessionView {
                session_id: "sess_ui".into(),
                default_agent_profile_id: ProfileId::from("reviewer"),
            },
            capabilities(),
            Some(OperationKind::Prompt),
            Vec::new(),
        );

        assert_eq!(snapshot.cursor.last_event_sequence.get(), 7);
        assert_eq!(snapshot.cursor.capability_generation.get(), 3);
        assert_eq!(snapshot.session.session_id, "sess_ui");
        assert_eq!(snapshot.active_operation, Some(OperationKind::Prompt));
        assert!(snapshot.client_drafts.is_empty());
    }

    #[test]
    fn client_connection_starts_from_snapshot_cursor() {
        let snapshot = UiSnapshot::new(
            UiSnapshotCursor {
                last_event_sequence: ProductEventSequence::new(11),
                capability_generation: CapabilityGeneration::new(2),
            },
            CodingAgentSessionView {
                session_id: "sess_client".into(),
                default_agent_profile_id: ProfileId::from("default"),
            },
            capabilities(),
            None,
            vec![ClientDraft::new(ClientDraftKind::Prompt, "draft text")],
        );

        let connection = ClientConnection::new(ClientConnectionId::new("rpc-1"), snapshot.clone());

        assert_eq!(connection.id.as_str(), "rpc-1");
        assert_eq!(connection.cursor, snapshot.cursor);
        assert_eq!(connection.client_drafts.len(), 1);
        assert!(connection.submitted_operation.is_none());
    }
}
