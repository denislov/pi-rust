use std::collections::{HashMap, HashSet};

use super::event::{
    DiagnosticLevel, PersistedContentBlock, PersistedToolResult, SessionEventData,
    SessionEventEnvelope,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionReplay {
    pub(crate) session_id: String,
    pub(crate) cwd: Option<String>,
    pub(crate) active_leaf_id: Option<String>,
    pub(crate) leaves: Vec<ReplayLeaf>,
    pub(crate) transcript: Vec<TranscriptItem>,
    pub(crate) diagnostics: Vec<ReplayDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReplayLeaf {
    pub(crate) leaf_id: String,
    pub(crate) parent_leaf_id: Option<String>,
    pub(crate) transcript_start: usize,
    pub(crate) transcript_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TranscriptItem {
    UserInput {
        turn_id: String,
        text: String,
    },
    AssistantMessage {
        message_id: String,
        content: Vec<PersistedContentBlock>,
        status: MessageStatus,
    },
    ToolCall {
        tool_call_id: String,
        name: String,
        arguments: serde_json::Value,
        status: ToolCallStatus,
        summary: String,
    },
    CompactionSummary {
        summary: String,
        first_kept_message_id: String,
        tokens_before: u32,
    },
    BranchSummary {
        summary: String,
        source_leaf_id: String,
        target_leaf_id: String,
    },
    Diagnostic {
        level: DiagnosticLevel,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessageStatus {
    Started,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolCallStatus {
    Started,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReplayDiagnostic {
    pub(crate) level: DiagnosticLevel,
    pub(crate) message: String,
}

#[derive(Debug, Default)]
struct ReplayBuilder {
    session_id: Option<String>,
    cwd: Option<String>,
    active_leaf_id: Option<String>,
    transcript: Vec<TranscriptItem>,
    leaves: Vec<ReplayLeaf>,
    diagnostics: Vec<ReplayDiagnostic>,
    message_indices: HashMap<String, usize>,
    tool_indices: HashMap<String, usize>,
    operation_kinds: HashMap<String, super::event::OperationKind>,
    operation_transcript_starts: HashMap<String, usize>,
}

pub(crate) fn fold_events(events: &[SessionEventEnvelope]) -> SessionReplay {
    let finalized_operations = finalized_operation_ids(events);
    let incomplete_operations = incomplete_operation_ids(events, &finalized_operations);
    let mut builder = ReplayBuilder::default();

    for event in events {
        builder.observe_session_id(event);
        if let Some(operation_id) = event.operation_id.as_deref()
            && !finalized_operations.contains(operation_id)
        {
            continue;
        }
        builder.apply_event(event);
    }

    for operation_id in incomplete_operations {
        builder.warn(format!(
            "operation {operation_id} has no final marker and was omitted from replay"
        ));
    }

    SessionReplay {
        session_id: builder.session_id.unwrap_or_default(),
        cwd: builder.cwd,
        active_leaf_id: builder.active_leaf_id,
        leaves: builder.leaves,
        transcript: builder.transcript,
        diagnostics: builder.diagnostics,
    }
}

fn finalized_operation_ids(events: &[SessionEventEnvelope]) -> HashSet<String> {
    events
        .iter()
        .filter_map(|event| match event.data {
            SessionEventData::OperationCommitted { .. }
            | SessionEventData::OperationAborted { .. }
            | SessionEventData::OperationFailed { .. } => event.operation_id.clone(),
            _ => None,
        })
        .collect()
}

fn incomplete_operation_ids(
    events: &[SessionEventEnvelope],
    finalized_operations: &HashSet<String>,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut incomplete = Vec::new();
    for event in events {
        let Some(operation_id) = event.operation_id.as_deref() else {
            continue;
        };
        if finalized_operations.contains(operation_id) || !seen.insert(operation_id.to_owned()) {
            continue;
        }
        incomplete.push(operation_id.to_owned());
    }
    incomplete
}

impl ReplayBuilder {
    fn observe_session_id(&mut self, event: &SessionEventEnvelope) {
        match self.session_id.as_deref() {
            None => self.session_id = Some(event.session_id.clone()),
            Some(session_id) if session_id != event.session_id => self.warn(format!(
                "event {} belongs to {}, expected {}",
                event.event_id, event.session_id, session_id
            )),
            Some(_) => {}
        }
    }

    fn apply_event(&mut self, event: &SessionEventEnvelope) {
        match &event.data {
            SessionEventData::SessionCreated { cwd } => {
                self.cwd = cwd.clone();
            }
            SessionEventData::OperationStarted { operation } => {
                if let Some(operation_id) = event.operation_id.as_deref() {
                    self.operation_kinds
                        .insert(operation_id.to_owned(), operation.clone());
                    self.operation_transcript_starts
                        .insert(operation_id.to_owned(), self.transcript.len());
                }
            }
            SessionEventData::SessionCloned { .. }
            | SessionEventData::SessionForked { .. }
            | SessionEventData::SessionCompactionStarted { .. }
            | SessionEventData::TurnStarted {}
            | SessionEventData::MetadataUpdated { .. } => {}
            SessionEventData::SessionCompactionCompleted {
                summary,
                first_kept_message_id,
                tokens_before,
            } => {
                self.apply_compaction_completed(summary, first_kept_message_id, *tokens_before);
            }
            SessionEventData::BranchSummaryCreated {
                summary,
                source_leaf_id,
                target_leaf_id,
            } => {
                self.transcript.push(TranscriptItem::BranchSummary {
                    summary: summary.clone(),
                    source_leaf_id: source_leaf_id.clone(),
                    target_leaf_id: target_leaf_id.clone(),
                });
            }
            SessionEventData::OperationCommitted { new_leaf_id } => {
                if let Some(new_leaf_id) = new_leaf_id {
                    self.record_prompt_leaf(event, new_leaf_id);
                    self.active_leaf_id = Some(new_leaf_id.clone());
                }
            }
            SessionEventData::OperationAborted { reason } => {
                self.warn(format!(
                    "operation {} aborted: {reason}",
                    event.operation_id.as_deref().unwrap_or("<unknown>")
                ));
            }
            SessionEventData::OperationFailed {
                error_code,
                message,
            } => {
                self.diagnostics.push(ReplayDiagnostic {
                    level: DiagnosticLevel::Error,
                    message: format!(
                        "operation {} failed ({error_code}): {message}",
                        event.operation_id.as_deref().unwrap_or("<unknown>")
                    ),
                });
            }
            SessionEventData::TurnInputRecorded { content } => {
                self.transcript.push(TranscriptItem::UserInput {
                    turn_id: event.turn_id.clone().unwrap_or_default(),
                    text: content_blocks_text(content),
                });
            }
            SessionEventData::MessageStarted { message_id, .. } => {
                self.message_indices
                    .insert(message_id.clone(), self.transcript.len());
                self.transcript.push(TranscriptItem::AssistantMessage {
                    message_id: message_id.clone(),
                    content: Vec::new(),
                    status: MessageStatus::Started,
                });
            }
            SessionEventData::MessageCompleted {
                message_id,
                content,
                finish_reason: _,
            } => {
                if self.complete_message(message_id, content.clone()).is_err() {
                    self.warn(format!(
                        "message completion references unknown message: {message_id}"
                    ));
                }
            }
            SessionEventData::MessageCancelled { message_id, .. } => {
                if self
                    .set_message_status(message_id, MessageStatus::Cancelled)
                    .is_err()
                {
                    self.warn(format!(
                        "message cancellation references unknown message: {message_id}"
                    ));
                }
            }
            SessionEventData::ToolCallStarted {
                tool_call_id,
                name,
                arguments,
            } => {
                self.tool_indices
                    .insert(tool_call_id.clone(), self.transcript.len());
                self.transcript.push(TranscriptItem::ToolCall {
                    tool_call_id: tool_call_id.clone(),
                    name: name.clone(),
                    arguments: arguments.clone(),
                    status: ToolCallStatus::Started,
                    summary: String::new(),
                });
            }
            SessionEventData::ToolCallUpdated {
                tool_call_id,
                message,
            } => {
                if let Some(tool) = self.tool_mut(tool_call_id) {
                    if !tool.is_empty() {
                        tool.push('\n');
                    }
                    tool.push_str(message);
                } else {
                    self.warn(format!(
                        "tool update references unknown tool call: {tool_call_id}"
                    ));
                }
            }
            SessionEventData::ToolCallCompleted {
                tool_call_id,
                result,
            } => {
                if self
                    .set_tool_status(tool_call_id, ToolCallStatus::Completed)
                    .is_err()
                {
                    self.warn(format!(
                        "tool completion references unknown tool call: {tool_call_id}"
                    ));
                } else if let Some(summary) = self.tool_mut(tool_call_id) {
                    *summary = tool_result_summary(result);
                }
            }
            SessionEventData::ToolCallFailed {
                tool_call_id,
                message,
            } => {
                if self
                    .set_tool_status(tool_call_id, ToolCallStatus::Failed)
                    .is_err()
                {
                    self.warn(format!(
                        "tool failure references unknown tool call: {tool_call_id}"
                    ));
                } else if let Some(summary) = self.tool_mut(tool_call_id) {
                    *summary = message.clone();
                }
            }
            SessionEventData::ToolCallCancelled {
                tool_call_id,
                reason,
            } => {
                if self
                    .set_tool_status(tool_call_id, ToolCallStatus::Cancelled)
                    .is_err()
                {
                    self.warn(format!(
                        "tool cancellation references unknown tool call: {tool_call_id}"
                    ));
                } else if let Some(summary) = self.tool_mut(tool_call_id) {
                    *summary = reason.clone();
                }
            }
            SessionEventData::DiagnosticEmitted { level, message } => {
                self.diagnostics.push(ReplayDiagnostic {
                    level: level.clone(),
                    message: message.clone(),
                });
                self.transcript.push(TranscriptItem::Diagnostic {
                    level: level.clone(),
                    message: message.clone(),
                });
            }
            SessionEventData::ActiveLeafChanged { leaf_id } => {
                self.active_leaf_id = Some(leaf_id.clone());
            }
        }
    }

    fn record_prompt_leaf(&mut self, event: &SessionEventEnvelope, leaf_id: &str) {
        let Some(operation_id) = event.operation_id.as_deref() else {
            return;
        };
        if self.operation_kinds.get(operation_id) != Some(&super::event::OperationKind::Prompt) {
            return;
        }
        let transcript_start = self
            .operation_transcript_starts
            .get(operation_id)
            .copied()
            .unwrap_or(self.transcript.len());
        self.leaves.push(ReplayLeaf {
            leaf_id: leaf_id.to_owned(),
            parent_leaf_id: self.active_leaf_id.clone(),
            transcript_start,
            transcript_end: self.transcript.len(),
        });
    }

    fn tool_mut(&mut self, tool_call_id: &str) -> Option<&mut String> {
        let index = *self.tool_indices.get(tool_call_id)?;
        match self.transcript.get_mut(index)? {
            TranscriptItem::ToolCall { summary, .. } => Some(summary),
            _ => None,
        }
    }

    fn complete_message(
        &mut self,
        message_id: &str,
        content: Vec<PersistedContentBlock>,
    ) -> Result<(), ()> {
        let index = *self.message_indices.get(message_id).ok_or(())?;
        match self.transcript.get_mut(index).ok_or(())? {
            TranscriptItem::AssistantMessage {
                content: current,
                status,
                ..
            } => {
                *current = content;
                *status = MessageStatus::Completed;
                Ok(())
            }
            _ => Err(()),
        }
    }

    fn set_message_status(&mut self, message_id: &str, status: MessageStatus) -> Result<(), ()> {
        let index = *self.message_indices.get(message_id).ok_or(())?;
        match self.transcript.get_mut(index).ok_or(())? {
            TranscriptItem::AssistantMessage {
                status: current, ..
            } => {
                *current = status;
                Ok(())
            }
            _ => Err(()),
        }
    }

    fn set_tool_status(&mut self, tool_call_id: &str, status: ToolCallStatus) -> Result<(), ()> {
        let index = *self.tool_indices.get(tool_call_id).ok_or(())?;
        match self.transcript.get_mut(index).ok_or(())? {
            TranscriptItem::ToolCall {
                status: current, ..
            } => {
                *current = status;
                Ok(())
            }
            _ => Err(()),
        }
    }

    fn warn(&mut self, message: impl Into<String>) {
        self.diagnostics.push(ReplayDiagnostic {
            level: DiagnosticLevel::Warn,
            message: message.into(),
        });
    }

    fn apply_compaction_completed(
        &mut self,
        summary: &str,
        first_kept_message_id: &str,
        tokens_before: u32,
    ) {
        let Some(first_kept_index) = self
            .transcript
            .iter()
            .position(|item| transcript_item_id(item).as_deref() == Some(first_kept_message_id))
        else {
            self.warn(format!(
                "session compaction references unknown first kept message: {first_kept_message_id}"
            ));
            return;
        };

        let kept = self.transcript.split_off(first_kept_index);
        self.transcript.clear();
        self.transcript.push(TranscriptItem::CompactionSummary {
            summary: summary.to_owned(),
            first_kept_message_id: first_kept_message_id.to_owned(),
            tokens_before,
        });
        self.transcript.extend(kept);
        self.rebuild_indices();
    }

    fn rebuild_indices(&mut self) {
        self.message_indices.clear();
        self.tool_indices.clear();
        for (index, item) in self.transcript.iter().enumerate() {
            match item {
                TranscriptItem::AssistantMessage { message_id, .. } => {
                    self.message_indices.insert(message_id.clone(), index);
                }
                TranscriptItem::ToolCall { tool_call_id, .. } => {
                    self.tool_indices.insert(tool_call_id.clone(), index);
                }
                TranscriptItem::UserInput { .. }
                | TranscriptItem::CompactionSummary { .. }
                | TranscriptItem::BranchSummary { .. }
                | TranscriptItem::Diagnostic { .. } => {}
            }
        }
    }
}

pub(crate) fn transcript_item_id(item: &TranscriptItem) -> Option<String> {
    match item {
        TranscriptItem::UserInput { turn_id, .. } => Some(turn_id.clone()),
        TranscriptItem::AssistantMessage { message_id, .. } => Some(message_id.clone()),
        TranscriptItem::ToolCall { tool_call_id, .. } => Some(tool_call_id.clone()),
        TranscriptItem::CompactionSummary { .. }
        | TranscriptItem::BranchSummary { .. }
        | TranscriptItem::Diagnostic { .. } => None,
    }
}

fn content_blocks_text(content: &[PersistedContentBlock]) -> String {
    content
        .iter()
        .map(|block| match block {
            PersistedContentBlock::Text { text } => text.clone(),
            PersistedContentBlock::Thinking { thinking, .. } => thinking.clone(),
            PersistedContentBlock::Image { mime_type, .. } => format!("[image:{mime_type}]"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tool_result_summary(result: &PersistedToolResult) -> String {
    match result {
        PersistedToolResult::Text { text } => text.clone(),
        PersistedToolResult::Json { value } => value.to_string(),
        PersistedToolResult::Error { message } => message.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::event::{OperationKind, PersistedRole};
    use super::*;

    fn event(
        event_id: &str,
        operation_id: Option<&str>,
        turn_id: Option<&str>,
        data: SessionEventData,
    ) -> SessionEventEnvelope {
        let mut event =
            SessionEventEnvelope::new("sess_replay", event_id, "2026-06-29T00:00:00Z", data);
        if let Some(operation_id) = operation_id {
            event = event.with_operation_id(operation_id);
        }
        if let Some(turn_id) = turn_id {
            event = event.with_turn_id(turn_id);
        }
        event
    }

    fn op_event(event_id: &str, data: SessionEventData) -> SessionEventEnvelope {
        event(event_id, Some("op_1"), Some("turn_1"), data)
    }

    #[test]
    fn committed_operation_folds_transcript_in_event_order() {
        let events = vec![
            op_event(
                "evt_1",
                SessionEventData::OperationStarted {
                    operation: OperationKind::Prompt,
                },
            ),
            op_event("evt_2", SessionEventData::TurnStarted {}),
            op_event(
                "evt_3",
                SessionEventData::TurnInputRecorded {
                    content: vec![PersistedContentBlock::Text {
                        text: "hello".into(),
                    }],
                },
            ),
            op_event(
                "evt_4",
                SessionEventData::MessageStarted {
                    message_id: "msg_1".into(),
                    role: PersistedRole::Assistant,
                },
            ),
            op_event(
                "evt_5",
                SessionEventData::MessageCompleted {
                    message_id: "msg_1".into(),
                    content: vec![PersistedContentBlock::Text { text: "hi".into() }],
                    finish_reason: Some("stop".into()),
                },
            ),
            op_event(
                "evt_7",
                SessionEventData::ToolCallStarted {
                    tool_call_id: "tool_1".into(),
                    name: "read".into(),
                    arguments: serde_json::json!({"path": "src/lib.rs"}),
                },
            ),
            op_event(
                "evt_8",
                SessionEventData::ToolCallCompleted {
                    tool_call_id: "tool_1".into(),
                    result: PersistedToolResult::Text { text: "ok".into() },
                },
            ),
            op_event(
                "evt_9",
                SessionEventData::DiagnosticEmitted {
                    level: DiagnosticLevel::Info,
                    message: "note".into(),
                },
            ),
            op_event(
                "evt_10",
                SessionEventData::OperationCommitted {
                    new_leaf_id: Some("leaf_1".into()),
                },
            ),
        ];

        let replay = fold_events(&events);

        assert_eq!(replay.session_id, "sess_replay");
        assert_eq!(replay.active_leaf_id.as_deref(), Some("leaf_1"));
        assert_eq!(
            replay.transcript,
            vec![
                TranscriptItem::UserInput {
                    turn_id: "turn_1".into(),
                    text: "hello".into(),
                },
                TranscriptItem::AssistantMessage {
                    message_id: "msg_1".into(),
                    content: vec![PersistedContentBlock::Text { text: "hi".into() }],
                    status: MessageStatus::Completed,
                },
                TranscriptItem::ToolCall {
                    tool_call_id: "tool_1".into(),
                    name: "read".into(),
                    arguments: serde_json::json!({"path": "src/lib.rs"}),
                    status: ToolCallStatus::Completed,
                    summary: "ok".into(),
                },
                TranscriptItem::Diagnostic {
                    level: DiagnosticLevel::Info,
                    message: "note".into(),
                },
            ]
        );
        assert_eq!(
            replay.diagnostics,
            vec![ReplayDiagnostic {
                level: DiagnosticLevel::Info,
                message: "note".into(),
            }]
        );
    }

    #[test]
    fn session_created_records_cwd() {
        let events = vec![event(
            "evt_1",
            None,
            None,
            SessionEventData::SessionCreated {
                cwd: Some("/work".into()),
            },
        )];

        let replay = fold_events(&events);

        assert_eq!(replay.cwd.as_deref(), Some("/work"));
    }

    #[test]
    fn incomplete_operation_is_omitted_and_diagnosed() {
        let events = vec![
            op_event(
                "evt_1",
                SessionEventData::OperationStarted {
                    operation: OperationKind::Prompt,
                },
            ),
            op_event(
                "evt_2",
                SessionEventData::TurnInputRecorded {
                    content: vec![PersistedContentBlock::Text {
                        text: "ignored".into(),
                    }],
                },
            ),
        ];

        let replay = fold_events(&events);

        assert!(replay.transcript.is_empty());
        assert_eq!(replay.diagnostics.len(), 1);
        assert_eq!(replay.diagnostics[0].level, DiagnosticLevel::Warn);
        assert!(
            replay.diagnostics[0]
                .message
                .contains("operation op_1 has no final marker")
        );
    }

    #[test]
    fn aborted_operation_marks_open_items_cancelled() {
        let events = vec![
            op_event(
                "evt_1",
                SessionEventData::OperationStarted {
                    operation: OperationKind::Prompt,
                },
            ),
            op_event(
                "evt_2",
                SessionEventData::MessageStarted {
                    message_id: "msg_1".into(),
                    role: PersistedRole::Assistant,
                },
            ),
            op_event(
                "evt_3",
                SessionEventData::ToolCallStarted {
                    tool_call_id: "tool_1".into(),
                    name: "bash".into(),
                    arguments: serde_json::json!({"cmd": "cargo test"}),
                },
            ),
            op_event(
                "evt_5",
                SessionEventData::MessageCancelled {
                    message_id: "msg_1".into(),
                    reason: "abort".into(),
                },
            ),
            op_event(
                "evt_6",
                SessionEventData::ToolCallCancelled {
                    tool_call_id: "tool_1".into(),
                    reason: "abort".into(),
                },
            ),
            op_event(
                "evt_7",
                SessionEventData::OperationAborted {
                    reason: "abort".into(),
                },
            ),
        ];

        let replay = fold_events(&events);

        assert_eq!(
            replay.transcript,
            vec![
                TranscriptItem::AssistantMessage {
                    message_id: "msg_1".into(),
                    content: Vec::new(),
                    status: MessageStatus::Cancelled,
                },
                TranscriptItem::ToolCall {
                    tool_call_id: "tool_1".into(),
                    name: "bash".into(),
                    arguments: serde_json::json!({"cmd": "cargo test"}),
                    status: ToolCallStatus::Cancelled,
                    summary: "abort".into(),
                },
            ]
        );
        assert!(
            replay
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("operation op_1 aborted"))
        );
    }

    #[test]
    fn failed_operation_keeps_error_diagnostic_and_failure_marker() {
        let events = vec![
            op_event(
                "evt_1",
                SessionEventData::OperationStarted {
                    operation: OperationKind::Prompt,
                },
            ),
            op_event(
                "evt_2",
                SessionEventData::DiagnosticEmitted {
                    level: DiagnosticLevel::Error,
                    message: "stream failed".into(),
                },
            ),
            op_event(
                "evt_3",
                SessionEventData::OperationFailed {
                    error_code: "provider".into(),
                    message: "stream failed".into(),
                },
            ),
        ];

        let replay = fold_events(&events);

        assert_eq!(
            replay.transcript,
            vec![TranscriptItem::Diagnostic {
                level: DiagnosticLevel::Error,
                message: "stream failed".into(),
            }]
        );
        assert_eq!(
            replay.diagnostics,
            vec![
                ReplayDiagnostic {
                    level: DiagnosticLevel::Error,
                    message: "stream failed".into(),
                },
                ReplayDiagnostic {
                    level: DiagnosticLevel::Error,
                    message: "operation op_1 failed (provider): stream failed".into(),
                },
            ]
        );
    }

    #[test]
    fn global_active_leaf_event_updates_replay_leaf() {
        let events = vec![event(
            "evt_1",
            None,
            None,
            SessionEventData::ActiveLeafChanged {
                leaf_id: "leaf_global".into(),
            },
        )];

        let replay = fold_events(&events);

        assert_eq!(replay.active_leaf_id.as_deref(), Some("leaf_global"));
    }

    #[test]
    fn committed_prompt_operations_record_leaf_transcript_ranges() {
        let events = vec![
            event(
                "evt_1",
                Some("op_root"),
                Some("turn_root"),
                SessionEventData::OperationStarted {
                    operation: OperationKind::Prompt,
                },
            ),
            event(
                "evt_2",
                Some("op_root"),
                Some("turn_root"),
                SessionEventData::TurnInputRecorded {
                    content: vec![PersistedContentBlock::Text {
                        text: "root prompt".into(),
                    }],
                },
            ),
            event(
                "evt_3",
                Some("op_root"),
                Some("turn_root"),
                SessionEventData::OperationCommitted {
                    new_leaf_id: Some("leaf_root".into()),
                },
            ),
            event(
                "evt_4",
                Some("op_branch"),
                Some("turn_branch"),
                SessionEventData::OperationStarted {
                    operation: OperationKind::Prompt,
                },
            ),
            event(
                "evt_5",
                Some("op_branch"),
                Some("turn_branch"),
                SessionEventData::TurnInputRecorded {
                    content: vec![PersistedContentBlock::Text {
                        text: "branch prompt".into(),
                    }],
                },
            ),
            event(
                "evt_6",
                Some("op_branch"),
                Some("turn_branch"),
                SessionEventData::OperationCommitted {
                    new_leaf_id: Some("leaf_branch".into()),
                },
            ),
            event(
                "evt_7",
                None,
                None,
                SessionEventData::ActiveLeafChanged {
                    leaf_id: "leaf_root".into(),
                },
            ),
            event(
                "evt_8",
                Some("op_alt"),
                Some("turn_alt"),
                SessionEventData::OperationStarted {
                    operation: OperationKind::Prompt,
                },
            ),
            event(
                "evt_9",
                Some("op_alt"),
                Some("turn_alt"),
                SessionEventData::TurnInputRecorded {
                    content: vec![PersistedContentBlock::Text {
                        text: "alternate prompt".into(),
                    }],
                },
            ),
            event(
                "evt_10",
                Some("op_alt"),
                Some("turn_alt"),
                SessionEventData::OperationCommitted {
                    new_leaf_id: Some("leaf_alt".into()),
                },
            ),
        ];

        let replay = fold_events(&events);

        assert_eq!(
            replay.leaves,
            vec![
                ReplayLeaf {
                    leaf_id: "leaf_root".into(),
                    parent_leaf_id: None,
                    transcript_start: 0,
                    transcript_end: 1,
                },
                ReplayLeaf {
                    leaf_id: "leaf_branch".into(),
                    parent_leaf_id: Some("leaf_root".into()),
                    transcript_start: 1,
                    transcript_end: 2,
                },
                ReplayLeaf {
                    leaf_id: "leaf_alt".into(),
                    parent_leaf_id: Some("leaf_root".into()),
                    transcript_start: 2,
                    transcript_end: 3,
                },
            ]
        );
        assert_eq!(replay.active_leaf_id.as_deref(), Some("leaf_alt"));
    }

    #[test]
    fn branch_summary_created_replays_as_transcript_item() {
        let events = vec![
            op_event(
                "evt_1",
                SessionEventData::OperationStarted {
                    operation: OperationKind::BranchSummary,
                },
            ),
            op_event(
                "evt_2",
                SessionEventData::BranchSummaryCreated {
                    summary: "summary of abandoned work".into(),
                    source_leaf_id: "leaf_old".into(),
                    target_leaf_id: "leaf_target".into(),
                },
            ),
            op_event(
                "evt_3",
                SessionEventData::OperationCommitted { new_leaf_id: None },
            ),
        ];

        let replay = fold_events(&events);

        assert_eq!(
            replay.transcript,
            vec![TranscriptItem::BranchSummary {
                summary: "summary of abandoned work".into(),
                source_leaf_id: "leaf_old".into(),
                target_leaf_id: "leaf_target".into(),
            }]
        );
        assert_eq!(replay.active_leaf_id, None);
        assert!(replay.diagnostics.is_empty());
    }

    #[test]
    fn session_compaction_completed_folds_transcript_to_summary_and_kept_tail() {
        let events = vec![
            op_event(
                "evt_1",
                SessionEventData::OperationStarted {
                    operation: OperationKind::Prompt,
                },
            ),
            op_event(
                "evt_2",
                SessionEventData::TurnInputRecorded {
                    content: vec![PersistedContentBlock::Text {
                        text: "old prompt".into(),
                    }],
                },
            ),
            op_event(
                "evt_3",
                SessionEventData::MessageStarted {
                    message_id: "msg_old".into(),
                    role: PersistedRole::Assistant,
                },
            ),
            op_event(
                "evt_4",
                SessionEventData::MessageCompleted {
                    message_id: "msg_old".into(),
                    content: vec![PersistedContentBlock::Text {
                        text: "old answer".into(),
                    }],
                    finish_reason: Some("stop".into()),
                },
            ),
            op_event(
                "evt_6",
                SessionEventData::MessageStarted {
                    message_id: "msg_kept".into(),
                    role: PersistedRole::Assistant,
                },
            ),
            op_event(
                "evt_7",
                SessionEventData::MessageCompleted {
                    message_id: "msg_kept".into(),
                    content: vec![PersistedContentBlock::Text {
                        text: "kept answer".into(),
                    }],
                    finish_reason: Some("stop".into()),
                },
            ),
            op_event(
                "evt_9",
                SessionEventData::SessionCompactionCompleted {
                    summary: "summary of old context".into(),
                    first_kept_message_id: "msg_kept".into(),
                    tokens_before: 1200,
                },
            ),
            op_event(
                "evt_10",
                SessionEventData::OperationCommitted {
                    new_leaf_id: Some("leaf_1".into()),
                },
            ),
        ];

        let replay = fold_events(&events);

        assert_eq!(
            replay.transcript,
            vec![
                TranscriptItem::CompactionSummary {
                    summary: "summary of old context".into(),
                    first_kept_message_id: "msg_kept".into(),
                    tokens_before: 1200,
                },
                TranscriptItem::AssistantMessage {
                    message_id: "msg_kept".into(),
                    content: vec![PersistedContentBlock::Text {
                        text: "kept answer".into(),
                    }],
                    status: MessageStatus::Completed,
                },
            ]
        );
        assert!(replay.diagnostics.is_empty());
    }

    #[test]
    fn session_compaction_completed_warns_when_kept_id_is_unknown() {
        let events = vec![event(
            "evt_1",
            None,
            None,
            SessionEventData::SessionCompactionCompleted {
                summary: "summary".into(),
                first_kept_message_id: "missing".into(),
                tokens_before: 1200,
            },
        )];

        let replay = fold_events(&events);

        assert!(replay.transcript.is_empty());
        assert_eq!(replay.diagnostics.len(), 1);
        assert!(
            replay.diagnostics[0]
                .message
                .contains("unknown first kept message")
        );
    }
}
