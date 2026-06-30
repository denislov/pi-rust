use std::collections::HashSet;

use serde_json::Value;

use super::event::{
    DiagnosticLevel, OperationKind, PersistedContentBlock, PersistedRole, PersistedToolResult,
    SessionEventData, SessionEventEnvelope,
};
use super::id::{Clock, IdGenerator};
use super::store::{ManifestPatch, SessionHandle, SessionLogStore};
use crate::coding_session::CodingSessionError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionState {
    Open,
    Committed,
    Aborted,
    Failed,
}

impl TransactionState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Committed => "committed",
            Self::Aborted => "aborted",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug)]
pub(crate) struct TurnTransaction<G, C>
where
    G: IdGenerator,
    C: Clock,
{
    store: SessionLogStore,
    handle: SessionHandle,
    ids: G,
    clock: C,
    operation_id: String,
    turn_id: String,
    pending_events: Vec<SessionEventEnvelope>,
    open_messages: HashSet<String>,
    open_tool_calls: HashSet<String>,
    state: TransactionState,
}

impl<G, C> TurnTransaction<G, C>
where
    G: IdGenerator,
    C: Clock,
{
    pub(crate) fn begin(
        store: &SessionLogStore,
        handle: SessionHandle,
        mut ids: G,
        clock: C,
        operation: OperationKind,
    ) -> Self {
        let operation_id = ids.next_operation_id();
        let turn_id = ids.next_turn_id();
        let mut transaction = Self {
            store: store.clone(),
            handle,
            ids,
            clock,
            operation_id,
            turn_id,
            pending_events: Vec::new(),
            open_messages: HashSet::new(),
            open_tool_calls: HashSet::new(),
            state: TransactionState::Open,
        };
        transaction.push_event(SessionEventData::OperationStarted { operation });
        transaction.push_event(SessionEventData::TurnStarted {});
        transaction
    }

    pub(crate) fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub(crate) fn turn_id(&self) -> &str {
        &self.turn_id
    }

    pub(crate) fn pending_events(&self) -> &[SessionEventEnvelope] {
        &self.pending_events
    }

    pub(crate) fn record_user_input(
        &mut self,
        content: Vec<PersistedContentBlock>,
    ) -> Result<(), CodingSessionError> {
        self.ensure_open()?;
        self.push_event(SessionEventData::TurnInputRecorded { content });
        Ok(())
    }

    pub(crate) fn start_assistant_message(&mut self) -> Result<String, CodingSessionError> {
        self.ensure_open()?;
        let message_id = self.ids.next_message_id();
        self.open_messages.insert(message_id.clone());
        self.push_event(SessionEventData::MessageStarted {
            message_id: message_id.clone(),
            role: PersistedRole::Assistant,
        });
        Ok(message_id)
    }

    pub(crate) fn append_assistant_delta(
        &mut self,
        message_id: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<(), CodingSessionError> {
        self.ensure_open()?;
        let message_id = message_id.into();
        self.ensure_message_open(&message_id)?;
        self.push_event(SessionEventData::MessageDelta {
            message_id,
            text: text.into(),
        });
        Ok(())
    }

    pub(crate) fn complete_assistant_message(
        &mut self,
        message_id: impl Into<String>,
        finish_reason: Option<String>,
    ) -> Result<(), CodingSessionError> {
        self.ensure_open()?;
        let message_id = message_id.into();
        self.ensure_message_open(&message_id)?;
        self.open_messages.remove(&message_id);
        self.push_event(SessionEventData::MessageCompleted {
            message_id,
            finish_reason,
        });
        Ok(())
    }

    pub(crate) fn cancel_assistant_message(
        &mut self,
        message_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<(), CodingSessionError> {
        self.ensure_open()?;
        let message_id = message_id.into();
        self.ensure_message_open(&message_id)?;
        self.open_messages.remove(&message_id);
        self.push_event(SessionEventData::MessageCancelled {
            message_id,
            reason: reason.into(),
        });
        Ok(())
    }

    pub(crate) fn record_tool_started(
        &mut self,
        name: impl Into<String>,
        arguments: Value,
    ) -> Result<String, CodingSessionError> {
        self.ensure_open()?;
        let tool_call_id = self.ids.next_tool_call_id();
        self.open_tool_calls.insert(tool_call_id.clone());
        self.push_event(SessionEventData::ToolCallStarted {
            tool_call_id: tool_call_id.clone(),
            name: name.into(),
            arguments,
        });
        Ok(tool_call_id)
    }

    pub(crate) fn record_tool_updated(
        &mut self,
        tool_call_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<(), CodingSessionError> {
        self.ensure_open()?;
        let tool_call_id = tool_call_id.into();
        self.ensure_tool_call_open(&tool_call_id)?;
        self.push_event(SessionEventData::ToolCallUpdated {
            tool_call_id,
            message: message.into(),
        });
        Ok(())
    }

    pub(crate) fn record_tool_completed(
        &mut self,
        tool_call_id: impl Into<String>,
        result: PersistedToolResult,
    ) -> Result<(), CodingSessionError> {
        self.ensure_open()?;
        let tool_call_id = tool_call_id.into();
        self.ensure_tool_call_open(&tool_call_id)?;
        self.open_tool_calls.remove(&tool_call_id);
        self.push_event(SessionEventData::ToolCallCompleted {
            tool_call_id,
            result,
        });
        Ok(())
    }

    pub(crate) fn record_tool_failed(
        &mut self,
        tool_call_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<(), CodingSessionError> {
        self.ensure_open()?;
        let tool_call_id = tool_call_id.into();
        self.ensure_tool_call_open(&tool_call_id)?;
        self.open_tool_calls.remove(&tool_call_id);
        self.push_event(SessionEventData::ToolCallFailed {
            tool_call_id,
            message: message.into(),
        });
        Ok(())
    }

    pub(crate) fn record_tool_cancelled(
        &mut self,
        tool_call_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<(), CodingSessionError> {
        self.ensure_open()?;
        let tool_call_id = tool_call_id.into();
        self.ensure_tool_call_open(&tool_call_id)?;
        self.open_tool_calls.remove(&tool_call_id);
        self.push_event(SessionEventData::ToolCallCancelled {
            tool_call_id,
            reason: reason.into(),
        });
        Ok(())
    }

    pub(crate) fn emit_diagnostic(
        &mut self,
        level: DiagnosticLevel,
        message: impl Into<String>,
    ) -> Result<(), CodingSessionError> {
        self.ensure_open()?;
        self.push_event(SessionEventData::DiagnosticEmitted {
            level,
            message: message.into(),
        });
        Ok(())
    }

    pub(crate) fn record_session_compaction_started(
        &mut self,
        first_kept_message_id: impl Into<String>,
        tokens_before: u32,
    ) -> Result<(), CodingSessionError> {
        self.ensure_open()?;
        self.push_event(SessionEventData::SessionCompactionStarted {
            first_kept_message_id: first_kept_message_id.into(),
            tokens_before,
        });
        Ok(())
    }

    pub(crate) fn record_session_compaction_completed(
        &mut self,
        summary: impl Into<String>,
        first_kept_message_id: impl Into<String>,
        tokens_before: u32,
    ) -> Result<(), CodingSessionError> {
        self.ensure_open()?;
        self.push_event(SessionEventData::SessionCompactionCompleted {
            summary: summary.into(),
            first_kept_message_id: first_kept_message_id.into(),
            tokens_before,
        });
        Ok(())
    }

    pub(crate) fn commit(&mut self, new_leaf_id: Option<String>) -> Result<(), CodingSessionError> {
        self.ensure_open()?;
        self.push_event(SessionEventData::OperationCommitted {
            new_leaf_id: new_leaf_id.clone(),
        });
        self.flush_pending()?;
        self.state = TransactionState::Committed;
        if let Some(leaf_id) = new_leaf_id {
            self.store.update_manifest(
                &self.handle,
                ManifestPatch::new()
                    .updated_at(self.clock.now_rfc3339())
                    .active_leaf_id(Some(leaf_id)),
            )?;
        }
        Ok(())
    }

    pub(crate) fn abort(&mut self, reason: impl Into<String>) -> Result<(), CodingSessionError> {
        self.ensure_open()?;
        let reason = reason.into();
        self.cancel_open_lifecycle_events(&reason);
        self.push_event(SessionEventData::OperationAborted { reason });
        self.flush_pending()?;
        self.state = TransactionState::Aborted;
        Ok(())
    }

    pub(crate) fn fail(
        &mut self,
        error_code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<(), CodingSessionError> {
        self.ensure_open()?;
        let error_code = error_code.into();
        let message = message.into();
        self.cancel_open_lifecycle_events("failed");
        self.push_event(SessionEventData::DiagnosticEmitted {
            level: DiagnosticLevel::Error,
            message: message.clone(),
        });
        self.push_event(SessionEventData::OperationFailed {
            error_code,
            message,
        });
        self.flush_pending()?;
        self.state = TransactionState::Failed;
        Ok(())
    }

    fn push_event(&mut self, data: SessionEventData) {
        let event = SessionEventEnvelope::new(
            self.handle.manifest().session_id.clone(),
            self.ids.next_event_id(),
            self.clock.now_rfc3339(),
            data,
        )
        .with_operation_id(self.operation_id.clone())
        .with_turn_id(self.turn_id.clone());
        self.pending_events.push(event);
    }

    fn flush_pending(&mut self) -> Result<(), CodingSessionError> {
        self.store
            .append_events(&self.handle, &self.pending_events)?;
        self.pending_events.clear();
        Ok(())
    }

    fn cancel_open_lifecycle_events(&mut self, reason: &str) {
        let open_messages = self.open_messages.drain().collect::<Vec<_>>();
        for message_id in open_messages {
            self.push_event(SessionEventData::MessageCancelled {
                message_id,
                reason: reason.to_owned(),
            });
        }

        let open_tool_calls = self.open_tool_calls.drain().collect::<Vec<_>>();
        for tool_call_id in open_tool_calls {
            self.push_event(SessionEventData::ToolCallCancelled {
                tool_call_id,
                reason: reason.to_owned(),
            });
        }
    }

    fn ensure_open(&self) -> Result<(), CodingSessionError> {
        if self.state == TransactionState::Open {
            Ok(())
        } else {
            Err(CodingSessionError::Session {
                message: format!(
                    "turn transaction is already finalized: {}",
                    self.state.as_str()
                ),
            })
        }
    }

    fn ensure_message_open(&self, message_id: &str) -> Result<(), CodingSessionError> {
        if self.open_messages.contains(message_id) {
            Ok(())
        } else {
            Err(CodingSessionError::Session {
                message: format!("assistant message is not open: {message_id}"),
            })
        }
    }

    fn ensure_tool_call_open(&self, tool_call_id: &str) -> Result<(), CodingSessionError> {
        if self.open_tool_calls.contains(tool_call_id) {
            Ok(())
        } else {
            Err(CodingSessionError::Session {
                message: format!("tool call is not open: {tool_call_id}"),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::session_log::id::{DeterministicIdGenerator, FixedClock};
    use crate::coding_session::session_log::store::{CreateSessionOptions, SessionLogStore};

    fn setup() -> (tempfile::TempDir, SessionLogStore, SessionHandle) {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionLogStore::new(temp.path());
        let handle = store
            .create_session(CreateSessionOptions::new("sess_tx", "2026-06-29T00:00:00Z"))
            .unwrap();
        (temp, store, handle)
    }

    fn begin(
        store: &SessionLogStore,
        handle: SessionHandle,
    ) -> TurnTransaction<DeterministicIdGenerator, FixedClock> {
        TurnTransaction::begin(
            store,
            handle,
            DeterministicIdGenerator::new(),
            FixedClock::new("2026-06-29T00:00:01Z"),
            OperationKind::Prompt,
        )
    }

    fn event_kinds(events: &[SessionEventEnvelope]) -> Vec<&'static str> {
        events
            .iter()
            .map(|event| match event.data {
                SessionEventData::OperationStarted { .. } => "operation.started",
                SessionEventData::OperationCommitted { .. } => "operation.committed",
                SessionEventData::OperationAborted { .. } => "operation.aborted",
                SessionEventData::OperationFailed { .. } => "operation.failed",
                SessionEventData::TurnStarted {} => "turn.started",
                SessionEventData::TurnInputRecorded { .. } => "turn.input.recorded",
                SessionEventData::MessageStarted { .. } => "message.started",
                SessionEventData::MessageDelta { .. } => "message.delta",
                SessionEventData::MessageCompleted { .. } => "message.completed",
                SessionEventData::MessageCancelled { .. } => "message.cancelled",
                SessionEventData::ToolCallStarted { .. } => "tool.call.started",
                SessionEventData::ToolCallUpdated { .. } => "tool.call.updated",
                SessionEventData::ToolCallCompleted { .. } => "tool.call.completed",
                SessionEventData::ToolCallFailed { .. } => "tool.call.failed",
                SessionEventData::ToolCallCancelled { .. } => "tool.call.cancelled",
                SessionEventData::DiagnosticEmitted { .. } => "diagnostic.emitted",
                SessionEventData::MetadataUpdated { .. } => "metadata.updated",
                SessionEventData::ActiveLeafChanged { .. } => "active_leaf.changed",
                SessionEventData::SessionCreated { .. } => "session.created",
                SessionEventData::SessionCloned { .. } => "session.cloned",
                SessionEventData::SessionForked { .. } => "session.forked",
                SessionEventData::SessionCompactionStarted { .. } => "session.compaction.started",
                SessionEventData::SessionCompactionCompleted { .. } => {
                    "session.compaction.completed"
                }
            })
            .collect()
    }

    #[test]
    fn commit_appends_pending_events_and_updates_manifest_leaf() {
        let (_temp, store, handle) = setup();
        let mut tx = begin(&store, handle.clone());

        assert_eq!(tx.operation_id(), "op_1");
        assert_eq!(tx.turn_id(), "turn_1");

        tx.record_user_input(vec![PersistedContentBlock::Text {
            text: "hello".into(),
        }])
        .unwrap();
        let message_id = tx.start_assistant_message().unwrap();
        tx.append_assistant_delta(&message_id, "hi").unwrap();
        tx.complete_assistant_message(&message_id, Some("stop".into()))
            .unwrap();
        let tool_call_id = tx
            .record_tool_started("read", serde_json::json!({"path": "src/lib.rs"}))
            .unwrap();
        tx.record_tool_updated(&tool_call_id, "running").unwrap();
        tx.record_tool_completed(
            &tool_call_id,
            PersistedToolResult::Text { text: "ok".into() },
        )
        .unwrap();
        tx.emit_diagnostic(DiagnosticLevel::Info, "note").unwrap();
        tx.commit(Some("leaf_1".into())).unwrap();

        let events = store.read_events(&handle).unwrap();
        assert_eq!(
            event_kinds(&events),
            vec![
                "operation.started",
                "turn.started",
                "turn.input.recorded",
                "message.started",
                "message.delta",
                "message.completed",
                "tool.call.started",
                "tool.call.updated",
                "tool.call.completed",
                "diagnostic.emitted",
                "operation.committed",
            ]
        );
        assert!(matches!(
            events.last().map(|event| &event.data),
            Some(SessionEventData::OperationCommitted {
                new_leaf_id: Some(leaf_id)
            }) if leaf_id == "leaf_1"
        ));

        let opened = store.open_session(handle.session_dir()).unwrap();
        assert_eq!(opened.manifest().active_leaf_id.as_deref(), Some("leaf_1"));
        assert_eq!(opened.manifest().updated_at, "2026-06-29T00:00:01Z");
    }

    #[test]
    fn finalized_transaction_rejects_further_mutation() {
        let (_temp, store, handle) = setup();
        let mut tx = begin(&store, handle);
        tx.commit(None).unwrap();

        let error = tx
            .record_user_input(vec![PersistedContentBlock::Text {
                text: "late".into(),
            }])
            .unwrap_err();

        assert_eq!(error.code(), "session");
        assert!(
            error
                .to_string()
                .contains("turn transaction is already finalized: committed")
        );
    }

    #[test]
    fn abort_cancels_open_lifecycle_events_and_does_not_update_leaf() {
        let (_temp, store, handle) = setup();
        let mut tx = begin(&store, handle.clone());
        let message_id = tx.start_assistant_message().unwrap();
        tx.append_assistant_delta(&message_id, "partial").unwrap();
        let _tool_call_id = tx
            .record_tool_started("bash", serde_json::json!({}))
            .unwrap();

        tx.abort("user cancelled").unwrap();

        let events = store.read_events(&handle).unwrap();
        assert_eq!(
            event_kinds(&events),
            vec![
                "operation.started",
                "turn.started",
                "message.started",
                "message.delta",
                "tool.call.started",
                "message.cancelled",
                "tool.call.cancelled",
                "operation.aborted",
            ]
        );
        assert!(matches!(
            events.last().map(|event| &event.data),
            Some(SessionEventData::OperationAborted { reason }) if reason == "user cancelled"
        ));

        let opened = store.open_session(handle.session_dir()).unwrap();
        assert!(opened.manifest().active_leaf_id.is_none());
    }

    #[test]
    fn fail_emits_error_diagnostic_and_failure_marker() {
        let (_temp, store, handle) = setup();
        let mut tx = begin(&store, handle.clone());
        let message_id = tx.start_assistant_message().unwrap();
        tx.append_assistant_delta(&message_id, "partial").unwrap();

        tx.fail("provider", "stream failed").unwrap();

        let events = store.read_events(&handle).unwrap();
        assert_eq!(
            event_kinds(&events),
            vec![
                "operation.started",
                "turn.started",
                "message.started",
                "message.delta",
                "message.cancelled",
                "diagnostic.emitted",
                "operation.failed",
            ]
        );
        assert!(matches!(
            events.iter().rev().nth(1).map(|event| &event.data),
            Some(SessionEventData::DiagnosticEmitted {
                level: DiagnosticLevel::Error,
                message,
            }) if message == "stream failed"
        ));
        assert!(matches!(
            events.last().map(|event| &event.data),
            Some(SessionEventData::OperationFailed {
                error_code,
                message,
            }) if error_code == "provider" && message == "stream failed"
        ));
    }

    #[test]
    fn completed_message_and_tool_cannot_be_mutated_again() {
        let (_temp, store, handle) = setup();
        let mut tx = begin(&store, handle);
        let message_id = tx.start_assistant_message().unwrap();
        tx.complete_assistant_message(&message_id, None).unwrap();
        let tool_call_id = tx
            .record_tool_started("read", serde_json::json!({}))
            .unwrap();
        tx.record_tool_failed(&tool_call_id, "not found").unwrap();

        let message_error = tx.append_assistant_delta(&message_id, "late").unwrap_err();
        let tool_error = tx
            .record_tool_completed(
                &tool_call_id,
                PersistedToolResult::Text {
                    text: "late".into(),
                },
            )
            .unwrap_err();

        assert_eq!(message_error.code(), "session");
        assert!(
            message_error
                .to_string()
                .contains("assistant message is not open")
        );
        assert_eq!(tool_error.code(), "session");
        assert!(tool_error.to_string().contains("tool call is not open"));
    }

    #[test]
    fn explicit_cancellations_are_not_duplicated_by_abort() {
        let (_temp, store, handle) = setup();
        let mut tx = begin(&store, handle.clone());
        let message_id = tx.start_assistant_message().unwrap();
        tx.cancel_assistant_message(&message_id, "hidden").unwrap();
        let tool_call_id = tx
            .record_tool_started("read", serde_json::json!({}))
            .unwrap();
        tx.record_tool_cancelled(&tool_call_id, "hidden").unwrap();

        tx.abort("user cancelled").unwrap();

        let events = store.read_events(&handle).unwrap();
        assert_eq!(
            event_kinds(&events),
            vec![
                "operation.started",
                "turn.started",
                "message.started",
                "message.cancelled",
                "tool.call.started",
                "tool.call.cancelled",
                "operation.aborted",
            ]
        );
    }
}
