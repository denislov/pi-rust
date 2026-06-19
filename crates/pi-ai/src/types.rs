use serde::{Deserialize, Serialize};

// ── Content blocks ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        text_signature: Option<String>,
    },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        thinking_signature: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        redacted: Option<bool>,
    },
    #[serde(rename = "image")]
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    #[serde(rename = "toolCall")]
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        thought_signature: Option<String>,
    },
}

// ── Messages ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "role")]
pub enum Message {
    #[serde(rename = "user")]
    User { content: Vec<ContentBlock> },
    #[serde(rename = "assistant")]
    Assistant { content: Vec<ContentBlock> },
    #[serde(rename = "toolResult")]
    ToolResult {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName", skip_serializing_if = "Option::is_none")]
        tool_name: Option<String>,
        #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
        content: Vec<ContentBlock>,
    },
}

// ── Usage & cost ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Cost {
    pub input: f64,
    pub output: f64,
    #[serde(rename = "cacheRead")]
    pub cache_read: f64,
    #[serde(rename = "cacheWrite")]
    pub cache_write: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Usage {
    pub input: u32,
    pub output: u32,
    #[serde(rename = "cacheRead")]
    pub cache_read: u32,
    #[serde(rename = "cacheWrite")]
    pub cache_write: u32,
    #[serde(rename = "totalTokens")]
    pub total_tokens: u32,
    pub cost: Cost,
}

// ── Stop reason ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StopReason {
    Stop,
    Length,
    #[serde(rename = "toolUse")]
    ToolUse,
    Error,
    Aborted,
}

// ── Assistant message (response-side) ───────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssistantMessage {
    pub content: Vec<ContentBlock>,
    pub api: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub model: String,
    #[serde(rename = "responseModel", skip_serializing_if = "Option::is_none")]
    pub response_model: Option<String>,
    #[serde(rename = "responseId", skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,
    pub usage: Usage,
    #[serde(rename = "stopReason")]
    pub stop_reason: StopReason,
    #[serde(rename = "errorMessage", skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Vec<AssistantMessageDiagnostic>>,
    pub timestamp: u64,
}

impl AssistantMessage {
    pub fn empty(api: &str, model: &str) -> Self {
        Self {
            content: Vec::new(),
            api: api.to_string(),
            provider: None,
            model: model.to_string(),
            response_model: None,
            response_id: None,
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            error_message: None,
            diagnostics: None,
            timestamp: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiagnosticErrorInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssistantMessageDiagnostic {
    #[serde(rename = "type")]
    pub diagnostic_type: String,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<DiagnosticErrorInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

// ── Streaming events ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum AssistantMessageEvent {
    #[serde(rename = "start")]
    Start {
        #[serde(rename = "contentIndex", skip_serializing_if = "Option::is_none")]
        content_index: Option<u32>,
        partial: AssistantMessage,
    },
    #[serde(rename = "text_start")]
    TextStart {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        partial: AssistantMessage,
    },
    #[serde(rename = "text_delta")]
    TextDelta {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        delta: String,
        partial: AssistantMessage,
    },
    #[serde(rename = "text_end")]
    TextEnd {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        partial: AssistantMessage,
    },
    #[serde(rename = "thinking_start")]
    ThinkingStart {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        partial: AssistantMessage,
    },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        delta: String,
        partial: AssistantMessage,
    },
    #[serde(rename = "thinking_end")]
    ThinkingEnd {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        partial: AssistantMessage,
    },
    #[serde(rename = "toolcall_start")]
    ToolcallStart {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        partial: AssistantMessage,
    },
    #[serde(rename = "toolcall_delta")]
    ToolcallDelta {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        delta: String,
        partial: AssistantMessage,
    },
    #[serde(rename = "toolcall_end")]
    ToolcallEnd {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        partial: AssistantMessage,
    },
    #[serde(rename = "done")]
    Done {
        reason: StopReason,
        message: AssistantMessage,
    },
    #[serde(rename = "error")]
    Error {
        reason: StopReason,
        message: AssistantMessage,
    },
}

// ── Context, tools, models ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Context {
    #[serde(rename = "systemPrompt", skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub api: String,
    pub provider: String,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    pub reasoning: bool,
    #[serde(rename = "thinkingLevelMap", skip_serializing_if = "Option::is_none")]
    pub thinking_level_map: Option<serde_json::Value>,
    pub input: Vec<ModelInput>,
    pub cost: ModelCost,
    #[serde(rename = "contextWindow")]
    pub context_window: u32,
    #[serde(rename = "maxTokens")]
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compat: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModelInput {
    Text,
    Image,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ModelCost {
    pub input: f64,
    pub output: f64,
    #[serde(rename = "cacheRead")]
    pub cache_read: f64,
    #[serde(rename = "cacheWrite")]
    pub cache_write: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreamOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(rename = "maxTokens", skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(rename = "apiKey", skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(rename = "cacheRetention", skip_serializing_if = "Option::is_none")]
    pub cache_retention: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
    #[serde(rename = "toolChoice", skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(rename = "azureApiVersion", skip_serializing_if = "Option::is_none")]
    pub azure_api_version: Option<String>,
    #[serde(rename = "azureResourceName", skip_serializing_if = "Option::is_none")]
    pub azure_resource_name: Option<String>,
    #[serde(rename = "azureBaseUrl", skip_serializing_if = "Option::is_none")]
    pub azure_base_url: Option<String>,
    #[serde(
        rename = "azureDeploymentName",
        skip_serializing_if = "Option::is_none"
    )]
    pub azure_deployment_name: Option<String>,
    #[serde(rename = "bedrockRegion", skip_serializing_if = "Option::is_none")]
    pub bedrock_region: Option<String>,
    #[serde(rename = "bedrockProfile", skip_serializing_if = "Option::is_none")]
    pub bedrock_profile: Option<String>,
    #[serde(rename = "bedrockBearerToken", skip_serializing_if = "Option::is_none")]
    pub bedrock_bearer_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Value>,
    #[serde(skip)]
    pub cancel: Option<tokio_util::sync::CancellationToken>,
    #[serde(rename = "timeoutMs", skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(rename = "maxRetries", skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
    #[serde(rename = "maxRetryDelayMs", skip_serializing_if = "Option::is_none")]
    pub max_retry_delay_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThinkingConfig {
    pub enabled: bool,
    #[serde(rename = "budgetTokens", skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
}

// ── Tests: serde roundtrip ──────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_block_text_roundtrip() {
        let cb = ContentBlock::Text {
            text: "hello".into(),
            text_signature: None,
        };
        let json = serde_json::to_string(&cb).unwrap();
        assert_eq!(json, r#"{"type":"text","text":"hello"}"#);
        let back: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cb);
    }

    #[test]
    fn content_block_toolcall_roundtrip() {
        let cb = ContentBlock::ToolCall {
            id: "toolu_01".into(),
            name: "read".into(),
            arguments: serde_json::json!({"path": "/x"}),
            thought_signature: None,
        };
        let json = serde_json::to_string(&cb).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "toolCall");
        assert_eq!(parsed["id"], "toolu_01");
        let back: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cb);
    }

    #[test]
    fn message_user_roundtrip() {
        let msg = Message::User {
            content: vec![ContentBlock::Text {
                text: "hi".into(),
                text_signature: None,
            }],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""role":"user""#));
        let back: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn message_tool_result_roundtrip() {
        let msg = Message::ToolResult {
            tool_call_id: "call_1".into(),
            tool_name: Some("read".into()),
            is_error: Some(false),
            content: vec![ContentBlock::Text {
                text: "ok".into(),
                text_signature: None,
            }],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""toolCallId":"call_1""#));
        assert!(json.contains(r#""toolName":"read""#));
        let back: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn event_done_roundtrip() {
        let ev = AssistantMessageEvent::Done {
            reason: StopReason::Stop,
            message: AssistantMessage::empty("anthropic-messages", "claude-sonnet-4-5"),
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains(r#""type":"done""#));
        let back: AssistantMessageEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, AssistantMessageEvent::Done { .. }));
    }

    #[test]
    fn event_error_roundtrip() {
        let mut msg = AssistantMessage::empty("test", "test");
        msg.error_message = Some("fail".into());
        msg.stop_reason = StopReason::Error;
        let ev = AssistantMessageEvent::Error {
            reason: StopReason::Error,
            message: msg,
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains(r#""type":"error""#));
        let back: AssistantMessageEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, AssistantMessageEvent::Error { .. }));
    }

    #[test]
    fn text_delta_has_snake_case_tag() {
        let ev = AssistantMessageEvent::TextDelta {
            content_index: 0,
            delta: "hi".into(),
            partial: AssistantMessage::empty("test", "test"),
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains(r#""type":"text_delta""#));
        assert!(json.contains(r#""contentIndex":0"#));
    }

    #[test]
    fn toolcall_delta_has_snake_case_tag() {
        let ev = AssistantMessageEvent::ToolcallDelta {
            content_index: 1,
            delta: "{}".into(),
            partial: AssistantMessage::empty("test", "test"),
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains(r#""type":"toolcall_delta""#));
    }

    #[test]
    fn stop_reason_serde() {
        assert_eq!(
            serde_json::to_string(&StopReason::Stop).unwrap(),
            r#""stop""#
        );
        assert_eq!(
            serde_json::to_string(&StopReason::ToolUse).unwrap(),
            r#""toolUse""#
        );
        let sr: StopReason = serde_json::from_str(r#""toolUse""#).unwrap();
        assert_eq!(sr, StopReason::ToolUse);
    }

    #[test]
    fn stream_options_serializes_retry_fields() {
        let opts = StreamOptions {
            timeout_ms: Some(30000),
            max_retries: Some(3),
            max_retry_delay_ms: Some(5000),
            ..Default::default()
        };
        let json = serde_json::to_string(&opts).unwrap();
        assert!(json.contains(r#""timeoutMs":30000"#));
        assert!(json.contains(r#""maxRetries":3"#));
        assert!(json.contains(r#""maxRetryDelayMs":5000"#));
    }

    #[test]
    fn stream_options_skips_none_retry_fields() {
        let opts = StreamOptions::default();
        let json = serde_json::to_string(&opts).unwrap();
        assert!(!json.contains("timeoutMs"));
        assert!(!json.contains("maxRetries"));
        assert!(!json.contains("maxRetryDelayMs"));
    }

    #[test]
    fn model_serde_camelcase() {
        let m = Model {
            id: "claude-sonnet-4-5".into(),
            name: "Claude Sonnet 4.5".into(),
            api: "anthropic-messages".into(),
            provider: "anthropic".into(),
            base_url: "https://api.anthropic.com".into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost {
                input: 3.0,
                output: 15.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 200000,
            max_tokens: 8192,
            headers: None,
            compat: None,
        };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains(r#""baseUrl""#));
        assert!(json.contains(r#""contextWindow""#));
        assert!(json.contains(r#""maxTokens""#));
    }
}
