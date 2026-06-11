use pi_agent_core::session::StoredAgentMessage;
use pi_agent_core::{QueueMode, ThinkingLevel};
use pi_ai::types::{AssistantMessageEvent, ContentBlock, Model};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Display;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type")]
pub enum ProtocolEvent {
    #[serde(rename = "agent_start")]
    AgentStart,
    #[serde(rename = "turn_start")]
    TurnStart,
    #[serde(rename = "message_start")]
    MessageStart { message: StoredAgentMessage },
    #[serde(rename = "message_update")]
    MessageUpdate {
        message: StoredAgentMessage,
        #[serde(rename = "assistantMessageEvent")]
        assistant_message_event: AssistantMessageEvent,
    },
    #[serde(rename = "message_end")]
    MessageEnd { message: StoredAgentMessage },
    #[serde(rename = "tool_execution_start")]
    ToolExecutionStart {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        args: serde_json::Value,
    },
    #[serde(rename = "tool_execution_end")]
    ToolExecutionEnd {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        result: ToolExecutionResult,
        #[serde(rename = "isError")]
        is_error: bool,
    },
    #[serde(rename = "turn_end")]
    TurnEnd {
        message: StoredAgentMessage,
        #[serde(rename = "toolResults")]
        tool_results: Vec<StoredAgentMessage>,
    },
    #[serde(rename = "queue_update")]
    QueueUpdate {
        steering: Vec<String>,
        #[serde(rename = "followUp")]
        follow_up: Vec<String>,
    },
    #[serde(rename = "compaction_start")]
    CompactionStart { reason: CompactionReason },
    #[serde(rename = "compaction_end")]
    CompactionEnd {
        reason: CompactionReason,
        result: Option<CompactionProtocolResult>,
        aborted: bool,
        #[serde(rename = "willRetry")]
        will_retry: bool,
        #[serde(rename = "errorMessage", skip_serializing_if = "Option::is_none")]
        error_message: Option<String>,
    },
    #[serde(rename = "agent_end")]
    AgentEnd { messages: Vec<StoredAgentMessage> },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ToolExecutionResult {
    pub content: Vec<ContentBlock>,
    pub terminate: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CompactionReason {
    Manual,
    Threshold,
    Overflow,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CompactionProtocolResult {
    pub summary: String,
    #[serde(rename = "firstKeptMessageId")]
    pub first_kept_message_id: String,
    #[serde(rename = "tokensBefore")]
    pub tokens_before: u32,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum RpcCommand {
    #[serde(rename = "prompt")]
    Prompt {
        id: Option<String>,
        message: String,
        images: Option<Vec<ContentBlock>>,
        #[serde(rename = "streamingBehavior")]
        streaming_behavior: Option<StreamingBehavior>,
    },
    #[serde(rename = "steer")]
    Steer {
        id: Option<String>,
        message: String,
        images: Option<Vec<ContentBlock>>,
    },
    #[serde(rename = "follow_up")]
    FollowUp {
        id: Option<String>,
        message: String,
        images: Option<Vec<ContentBlock>>,
    },
    #[serde(rename = "abort")]
    Abort { id: Option<String> },
    #[serde(rename = "new_session")]
    NewSession {
        id: Option<String>,
        #[serde(rename = "parentSession")]
        parent_session: Option<String>,
    },
    #[serde(rename = "get_state")]
    GetState { id: Option<String> },
    #[serde(rename = "set_thinking_level")]
    SetThinkingLevel {
        id: Option<String>,
        #[serde(deserialize_with = "deserialize_from_display")]
        level: ThinkingLevel,
    },
    #[serde(rename = "set_steering_mode")]
    SetSteeringMode {
        id: Option<String>,
        #[serde(deserialize_with = "deserialize_from_display")]
        mode: QueueMode,
    },
    #[serde(rename = "set_follow_up_mode")]
    SetFollowUpMode {
        id: Option<String>,
        #[serde(deserialize_with = "deserialize_from_display")]
        mode: QueueMode,
    },
    #[serde(rename = "compact")]
    Compact {
        id: Option<String>,
        #[serde(rename = "customInstructions")]
        custom_instructions: Option<String>,
    },
    #[serde(rename = "set_auto_compaction")]
    SetAutoCompaction { id: Option<String>, enabled: bool },
    #[serde(rename = "get_session_stats")]
    GetSessionStats { id: Option<String> },
    #[serde(rename = "get_last_assistant_text")]
    GetLastAssistantText { id: Option<String> },
    #[serde(rename = "set_session_name")]
    SetSessionName { id: Option<String>, name: String },
    #[serde(rename = "get_messages")]
    GetMessages { id: Option<String> },
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum StreamingBehavior {
    #[serde(rename = "steer")]
    Steer,
    #[serde(rename = "followUp")]
    FollowUp,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RpcSessionState {
    pub model: Option<Model>,
    #[serde(rename = "thinkingLevel", serialize_with = "serialize_display")]
    pub thinking_level: ThinkingLevel,
    #[serde(rename = "isStreaming")]
    pub is_streaming: bool,
    #[serde(rename = "isCompacting")]
    pub is_compacting: bool,
    #[serde(rename = "steeringMode", serialize_with = "serialize_display")]
    pub steering_mode: QueueMode,
    #[serde(rename = "followUpMode", serialize_with = "serialize_display")]
    pub follow_up_mode: QueueMode,
    #[serde(rename = "sessionFile", skip_serializing_if = "Option::is_none")]
    pub session_file: Option<String>,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "sessionName", skip_serializing_if = "Option::is_none")]
    pub session_name: Option<String>,
    #[serde(rename = "autoCompactionEnabled")]
    pub auto_compaction_enabled: bool,
    #[serde(rename = "messageCount")]
    pub message_count: usize,
    #[serde(rename = "pendingMessageCount")]
    pub pending_message_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RpcResponse {
    #[serde(rename = "type")]
    pub response_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub command: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl RpcResponse {
    pub fn success(
        id: Option<String>,
        command: impl Into<String>,
        data: Option<serde_json::Value>,
    ) -> Self {
        Self {
            response_type: "response",
            id,
            command: command.into(),
            success: true,
            data,
            error: None,
        }
    }

    pub fn error(id: Option<String>, command: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            response_type: "response",
            id,
            command: command.into(),
            success: false,
            data: None,
            error: Some(error.into()),
        }
    }
}

fn serialize_display<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: Display,
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}

fn deserialize_from_display<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: Display,
{
    let value = String::deserialize(deserializer)?;
    value.parse().map_err(serde::de::Error::custom)
}
