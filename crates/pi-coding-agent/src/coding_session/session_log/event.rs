use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::manifest::{EVENT_SCHEMA, EVENT_VERSION};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SessionEventEnvelope {
    pub schema: String,
    pub version: u32,
    pub session_id: String,
    pub event_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leaf_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_event_id: Option<String>,
    pub created_at: String,
    #[serde(flatten)]
    pub data: SessionEventData,
}

impl SessionEventEnvelope {
    pub(crate) fn new(
        session_id: impl Into<String>,
        event_id: impl Into<String>,
        created_at: impl Into<String>,
        data: SessionEventData,
    ) -> Self {
        Self {
            schema: EVENT_SCHEMA.into(),
            version: EVENT_VERSION,
            session_id: session_id.into(),
            event_id: event_id.into(),
            operation_id: None,
            turn_id: None,
            branch_id: None,
            leaf_id: None,
            parent_event_id: None,
            created_at: created_at.into(),
            data,
        }
    }

    pub(crate) fn with_operation_id(mut self, operation_id: impl Into<String>) -> Self {
        self.operation_id = Some(operation_id.into());
        self
    }

    pub(crate) fn with_turn_id(mut self, turn_id: impl Into<String>) -> Self {
        self.turn_id = Some(turn_id.into());
        self
    }

    pub(crate) fn with_branch_id(mut self, branch_id: impl Into<String>) -> Self {
        self.branch_id = Some(branch_id.into());
        self
    }

    pub(crate) fn with_leaf_id(mut self, leaf_id: impl Into<String>) -> Self {
        self.leaf_id = Some(leaf_id.into());
        self
    }

    pub(crate) fn with_parent_event_id(mut self, parent_event_id: impl Into<String>) -> Self {
        self.parent_event_id = Some(parent_event_id.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub(crate) enum SessionEventData {
    #[serde(rename = "session.created")]
    SessionCreated { cwd: Option<String> },
    #[serde(rename = "session.cloned")]
    SessionCloned {
        source_session_id: String,
        source_leaf_id: String,
    },
    #[serde(rename = "session.forked")]
    SessionForked {
        source_session_id: String,
        source_leaf_id: String,
    },
    #[serde(rename = "session.compaction.started")]
    SessionCompactionStarted {
        first_kept_message_id: String,
        tokens_before: u32,
    },
    #[serde(rename = "session.compaction.completed")]
    SessionCompactionCompleted {
        summary: String,
        first_kept_message_id: String,
        tokens_before: u32,
    },
    #[serde(rename = "operation.started")]
    OperationStarted { operation: OperationKind },
    #[serde(rename = "operation.committed")]
    OperationCommitted { new_leaf_id: Option<String> },
    #[serde(rename = "operation.aborted")]
    OperationAborted { reason: String },
    #[serde(rename = "operation.failed")]
    OperationFailed { error_code: String, message: String },
    #[serde(rename = "turn.started")]
    TurnStarted {},
    #[serde(rename = "turn.input.recorded")]
    TurnInputRecorded { content: Vec<PersistedContentBlock> },
    #[serde(rename = "message.started")]
    MessageStarted {
        message_id: String,
        role: PersistedRole,
    },
    #[serde(rename = "message.completed")]
    MessageCompleted {
        message_id: String,
        content: Vec<PersistedContentBlock>,
        finish_reason: Option<String>,
    },
    #[serde(rename = "message.cancelled")]
    MessageCancelled { message_id: String, reason: String },
    #[serde(rename = "tool.call.started")]
    ToolCallStarted {
        tool_call_id: String,
        name: String,
        arguments: Value,
    },
    #[serde(rename = "tool.call.updated")]
    ToolCallUpdated {
        tool_call_id: String,
        message: String,
    },
    #[serde(rename = "tool.call.completed")]
    ToolCallCompleted {
        tool_call_id: String,
        result: PersistedToolResult,
    },
    #[serde(rename = "tool.call.failed")]
    ToolCallFailed {
        tool_call_id: String,
        message: String,
    },
    #[serde(rename = "tool.call.cancelled")]
    ToolCallCancelled {
        tool_call_id: String,
        reason: String,
    },
    #[serde(rename = "diagnostic.emitted")]
    DiagnosticEmitted {
        level: DiagnosticLevel,
        message: String,
    },
    #[serde(rename = "metadata.updated")]
    MetadataUpdated { key: String, value: Value },
    #[serde(rename = "active_leaf.changed")]
    ActiveLeafChanged { leaf_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub(crate) enum OperationKind {
    Prompt,
    ManualCompaction,
    BranchSummary,
    Export,
    PluginLoad,
    Other { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PersistedRole {
    User,
    Assistant,
    Tool,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub(crate) enum PersistedContentBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        thinking_signature: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        redacted: Option<bool>,
    },
    Image {
        mime_type: String,
        data: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub(crate) enum PersistedToolResult {
    Text { text: String },
    Json { value: Value },
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiagnosticLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn event_envelope_serializes_kind_and_data_at_top_level() {
        let envelope = SessionEventEnvelope::new(
            "sess_1",
            "evt_1",
            "2026-06-29T00:00:00Z",
            SessionEventData::TurnStarted {},
        )
        .with_operation_id("op_1")
        .with_turn_id("turn_1")
        .with_branch_id("branch_1")
        .with_leaf_id("leaf_1")
        .with_parent_event_id("evt_0");

        let value = serde_json::to_value(&envelope).unwrap();
        assert_eq!(value["schema"], EVENT_SCHEMA);
        assert_eq!(value["version"], EVENT_VERSION);
        assert_eq!(value["session_id"], "sess_1");
        assert_eq!(value["event_id"], "evt_1");
        assert_eq!(value["operation_id"], "op_1");
        assert_eq!(value["turn_id"], "turn_1");
        assert_eq!(value["branch_id"], "branch_1");
        assert_eq!(value["leaf_id"], "leaf_1");
        assert_eq!(value["parent_event_id"], "evt_0");
        assert_eq!(value["created_at"], "2026-06-29T00:00:00Z");
        assert_eq!(value["kind"], "turn.started");
        assert_eq!(value["data"], json!({}));

        let decoded: SessionEventEnvelope = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, envelope);
    }

    #[test]
    fn session_event_data_variants_keep_stable_kind_names() {
        let cases = [
            (
                SessionEventData::SessionCreated { cwd: None },
                "session.created",
            ),
            (
                SessionEventData::SessionCloned {
                    source_session_id: "sess_source".into(),
                    source_leaf_id: "leaf_source".into(),
                },
                "session.cloned",
            ),
            (
                SessionEventData::SessionForked {
                    source_session_id: "sess_source".into(),
                    source_leaf_id: "leaf_source".into(),
                },
                "session.forked",
            ),
            (
                SessionEventData::SessionCompactionStarted {
                    first_kept_message_id: "msg_1".into(),
                    tokens_before: 1200,
                },
                "session.compaction.started",
            ),
            (
                SessionEventData::SessionCompactionCompleted {
                    summary: "summary".into(),
                    first_kept_message_id: "msg_1".into(),
                    tokens_before: 1200,
                },
                "session.compaction.completed",
            ),
            (
                SessionEventData::OperationStarted {
                    operation: OperationKind::Prompt,
                },
                "operation.started",
            ),
            (
                SessionEventData::OperationCommitted { new_leaf_id: None },
                "operation.committed",
            ),
            (
                SessionEventData::OperationAborted {
                    reason: "user".into(),
                },
                "operation.aborted",
            ),
            (
                SessionEventData::OperationFailed {
                    error_code: "session".into(),
                    message: "failed".into(),
                },
                "operation.failed",
            ),
            (SessionEventData::TurnStarted {}, "turn.started"),
            (
                SessionEventData::TurnInputRecorded {
                    content: vec![PersistedContentBlock::Text {
                        text: "hello".into(),
                    }],
                },
                "turn.input.recorded",
            ),
            (
                SessionEventData::MessageStarted {
                    message_id: "msg_1".into(),
                    role: PersistedRole::Assistant,
                },
                "message.started",
            ),
            (
                SessionEventData::MessageCompleted {
                    message_id: "msg_1".into(),
                    content: vec![PersistedContentBlock::Text { text: "hi".into() }],
                    finish_reason: None,
                },
                "message.completed",
            ),
            (
                SessionEventData::MessageCancelled {
                    message_id: "msg_1".into(),
                    reason: "abort".into(),
                },
                "message.cancelled",
            ),
            (
                SessionEventData::ToolCallStarted {
                    tool_call_id: "tool_1".into(),
                    name: "read".into(),
                    arguments: json!({"path": "src/lib.rs"}),
                },
                "tool.call.started",
            ),
            (
                SessionEventData::ToolCallUpdated {
                    tool_call_id: "tool_1".into(),
                    message: "running".into(),
                },
                "tool.call.updated",
            ),
            (
                SessionEventData::ToolCallCompleted {
                    tool_call_id: "tool_1".into(),
                    result: PersistedToolResult::Text { text: "ok".into() },
                },
                "tool.call.completed",
            ),
            (
                SessionEventData::ToolCallFailed {
                    tool_call_id: "tool_1".into(),
                    message: "failed".into(),
                },
                "tool.call.failed",
            ),
            (
                SessionEventData::ToolCallCancelled {
                    tool_call_id: "tool_1".into(),
                    reason: "abort".into(),
                },
                "tool.call.cancelled",
            ),
            (
                SessionEventData::DiagnosticEmitted {
                    level: DiagnosticLevel::Info,
                    message: "note".into(),
                },
                "diagnostic.emitted",
            ),
            (
                SessionEventData::MetadataUpdated {
                    key: "model".into(),
                    value: json!("test"),
                },
                "metadata.updated",
            ),
            (
                SessionEventData::ActiveLeafChanged {
                    leaf_id: "leaf_1".into(),
                },
                "active_leaf.changed",
            ),
        ];

        for (event, expected_kind) in cases {
            let value = serde_json::to_value(event).unwrap();
            assert_eq!(value["kind"], expected_kind);
            assert!(value.get("data").is_some());
        }
    }
}
