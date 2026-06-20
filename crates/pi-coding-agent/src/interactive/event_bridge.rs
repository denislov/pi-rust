use pi_agent_core::AgentEvent;
use pi_ai::types::{AssistantMessageEvent, ContentBlock};

#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    AgentStarted,
    TurnStarted,
    AssistantDelta {
        text: String,
    },
    AssistantDone,
    ToolStarted {
        call_id: String,
        name: String,
        args: serde_json::Value,
    },
    ToolFinished {
        call_id: String,
        result: String,
        is_error: bool,
    },
    AgentError {
        error: String,
    },
    CompactionNotice {
        summary: String,
    },
    UsageUpdate {
        input: u32,
        output: u32,
    },
}

#[derive(Debug, Default)]
pub struct InteractiveEventBridge {
    total_input: u32,
    total_output: u32,
}

impl InteractiveEventBridge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle(&mut self, event: &AgentEvent) -> Vec<UiEvent> {
        match event {
            AgentEvent::TurnStart { .. } => vec![UiEvent::TurnStarted],
            AgentEvent::BeforeProviderRequest { .. } => Vec::new(),
            AgentEvent::LlmEvent(event) => self.handle_llm_event(event),
            AgentEvent::ToolCallStart {
                tool_call_id,
                tool_name,
            } => vec![UiEvent::ToolStarted {
                call_id: tool_call_id.clone(),
                name: tool_name.clone(),
                args: serde_json::Value::Null,
            }],
            AgentEvent::ToolCallEnd {
                tool_call_id,
                result,
                ..
            } => vec![UiEvent::ToolFinished {
                call_id: tool_call_id.clone(),
                result: content_blocks_to_text(&result.content),
                is_error: result.is_error,
            }],
            AgentEvent::AgentDone { message } => {
                self.total_input = self.total_input.saturating_add(message.usage.input);
                self.total_output = self.total_output.saturating_add(message.usage.output);
                vec![
                    UiEvent::AssistantDone,
                    UiEvent::UsageUpdate {
                        input: self.total_input,
                        output: self.total_output,
                    },
                ]
            }
            AgentEvent::AgentError { error } => vec![UiEvent::AgentError {
                error: error.clone(),
            }],
            AgentEvent::SessionCompacted { summary, .. } => vec![UiEvent::CompactionNotice {
                summary: summary.clone(),
            }],
        }
    }

    fn handle_llm_event(&mut self, event: &AssistantMessageEvent) -> Vec<UiEvent> {
        match event {
            AssistantMessageEvent::TextDelta { delta, .. } => {
                vec![UiEvent::AssistantDelta {
                    text: delta.clone(),
                }]
            }
            AssistantMessageEvent::Done { .. } => vec![UiEvent::AssistantDone],
            AssistantMessageEvent::Error { message, .. } => vec![UiEvent::AgentError {
                error: message
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "assistant message error".to_string()),
            }],
            _ => Vec::new(),
        }
    }
}

fn content_blocks_to_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            ContentBlock::Thinking { thinking, .. } => Some(thinking.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
