use std::pin::Pin;
use std::sync::Arc;
use futures::Stream;
use pi_ai::types::{AssistantMessage, AssistantMessageEvent, ContentBlock, Model, StreamOptions};

// ── AgentMessage ───────────────────────────────────

#[derive(Debug, Clone)]
pub enum AgentMessage {
    UserText {
        message_id: String,
        text: String,
    },
    Assistant {
        message_id: String,
        message: AssistantMessage,
    },
    ToolResult {
        message_id: String,
        tool_call_id: String,
        content: Vec<ContentBlock>,
    },
    SystemPrompt {
        message_id: String,
        text: String,
    },
}

// ── AgentTool ──────────────────────────────────────

pub type ToolFn = Arc<
    dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = Result<Vec<ContentBlock>, String>> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub struct AgentTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub execute: ToolFn,
}

// ── AgentConfig ────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub model: Model,
    pub system_prompt: Option<String>,
    pub max_turns: u32,
    pub stream_options: Option<StreamOptions>,
}

// ── AgentEvent ─────────────────────────────────────

#[derive(Debug)]
pub enum AgentEvent {
    TurnStart { turn: u32 },
    LlmEvent(AssistantMessageEvent),
    ToolCallStart { tool_call_id: String, tool_name: String },
    ToolCallEnd { tool_call_id: String, result: Result<Vec<ContentBlock>, String> },
    AgentDone { message: AssistantMessage },
    AgentError { error: String },
}

// ── AgentStream ────────────────────────────────────

pub type AgentStream = Pin<Box<dyn Stream<Item = AgentEvent> + Send>>;

// ── Unit tests ─────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text_tool() -> AgentTool {
        AgentTool {
            name: "echo".into(),
            description: "echoes input".into(),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
            execute: Arc::new(|args| {
                let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("no text");
                let result: Vec<ContentBlock> = vec![ContentBlock::Text {
                    text: text.to_string(),
                    text_signature: None,
                }];
                Box::pin(async move { Ok(result) })
            }),
        }
    }

    #[test]
    fn agent_message_user_text_constructs() {
        let msg = AgentMessage::UserText {
            message_id: "1".into(),
            text: "hello".into(),
        };
        match &msg {
            AgentMessage::UserText { text, .. } => assert_eq!(text, "hello"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn agent_tool_has_correct_fields() {
        let tool = make_text_tool();
        assert_eq!(tool.name, "echo");
        assert!(tool.description.contains("echoes"));
    }

    #[tokio::test]
    async fn tool_fn_executes() {
        let tool = make_text_tool();
        let result = (tool.execute)(serde_json::json!({"text": "hi"})).await;
        assert!(result.is_ok());
        let blocks = result.unwrap();
        assert_eq!(blocks.len(), 1);
    }
}
