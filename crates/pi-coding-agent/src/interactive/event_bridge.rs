use crate::coding_session::CodingAgentEvent;
use pi_agent_core::AgentEvent;
use pi_ai::types::{AssistantMessageEvent, ContentBlock, Usage};

#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    AgentStarted,
    TurnStarted,
    AssistantDelta {
        text: String,
    },
    ThinkingDelta {
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
    ToolUpdated {
        call_id: String,
        result: String,
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
        cache_read: u32,
        cache_write: u32,
        cost: f64,
        /// Estimated context tokens from the last assistant usage;
        /// `None` means unknown (e.g. right after compaction).
        context_tokens: Option<u32>,
    },
}

#[derive(Debug, Default)]
pub struct InteractiveEventBridge {
    total_input: u32,
    total_output: u32,
    total_cache_read: u32,
    total_cache_write: u32,
    total_cost: f64,
    last_context_tokens: Option<u32>,
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
                arguments,
            } => vec![UiEvent::ToolStarted {
                call_id: tool_call_id.clone(),
                name: tool_name.clone(),
                args: arguments.clone(),
            }],
            AgentEvent::ToolCallUpdate {
                tool_call_id,
                update,
                ..
            } => vec![UiEvent::ToolUpdated {
                call_id: tool_call_id.clone(),
                result: tool_output_text(&update.content),
            }],
            AgentEvent::ToolCallEnd {
                tool_call_id,
                result,
                ..
            } => vec![UiEvent::ToolFinished {
                call_id: tool_call_id.clone(),
                result: tool_output_text(&result.content),
                is_error: result.is_error,
            }],
            AgentEvent::AgentDone { message } => {
                let usage = &message.usage;
                self.total_input = self.total_input.saturating_add(usage.input);
                self.total_output = self.total_output.saturating_add(usage.output);
                self.total_cache_read = self.total_cache_read.saturating_add(usage.cache_read);
                self.total_cache_write = self.total_cache_write.saturating_add(usage.cache_write);
                self.total_cost += usage.cost.input
                    + usage.cost.output
                    + usage.cost.cache_read
                    + usage.cost.cache_write;
                let context_tokens = calculate_context_tokens(usage);
                self.last_context_tokens = Some(context_tokens);
                vec![
                    UiEvent::AssistantDone,
                    UiEvent::UsageUpdate {
                        input: self.total_input,
                        output: self.total_output,
                        cache_read: self.total_cache_read,
                        cache_write: self.total_cache_write,
                        cost: self.total_cost,
                        context_tokens: Some(context_tokens),
                    },
                ]
            }
            AgentEvent::AgentError { error } => vec![UiEvent::AgentError {
                error: error.clone(),
            }],
            AgentEvent::SessionCompacted { summary, .. } => {
                // After compaction the context size is unknown until the next
                // LLM response; mirror TS `getContextUsage` returning a null
                // percent so the footer shows "?" until then.
                self.last_context_tokens = None;
                vec![
                    UiEvent::CompactionNotice {
                        summary: summary.clone(),
                    },
                    UiEvent::UsageUpdate {
                        input: self.total_input,
                        output: self.total_output,
                        cache_read: self.total_cache_read,
                        cache_write: self.total_cache_write,
                        cost: self.total_cost,
                        context_tokens: None,
                    },
                ]
            }
        }
    }

    fn handle_llm_event(&mut self, event: &AssistantMessageEvent) -> Vec<UiEvent> {
        match event {
            AssistantMessageEvent::TextDelta { delta, .. } => {
                vec![UiEvent::AssistantDelta {
                    text: delta.clone(),
                }]
            }
            AssistantMessageEvent::ThinkingDelta { delta, .. } => {
                vec![UiEvent::ThinkingDelta {
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

#[derive(Debug, Default)]
pub struct CodingEventBridge {
    total_input: u32,
    total_output: u32,
    total_cache_read: u32,
    total_cache_write: u32,
    total_cost: f64,
}

impl CodingEventBridge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle(&mut self, event: &CodingAgentEvent) -> Vec<UiEvent> {
        match event {
            CodingAgentEvent::AgentTurnStarted { .. } => vec![UiEvent::TurnStarted],
            CodingAgentEvent::AssistantMessageDelta { text, .. } => {
                vec![UiEvent::AssistantDelta { text: text.clone() }]
            }
            CodingAgentEvent::AssistantMessageCompleted { .. } => vec![UiEvent::AssistantDone],
            CodingAgentEvent::ToolCallStarted {
                tool_call_id,
                name,
                arguments_json,
                ..
            } => vec![UiEvent::ToolStarted {
                call_id: tool_call_id.clone(),
                name: name.clone(),
                args: parse_tool_arguments(arguments_json),
            }],
            CodingAgentEvent::ToolCallUpdated {
                tool_call_id,
                message,
                ..
            } => vec![UiEvent::ToolUpdated {
                call_id: tool_call_id.clone(),
                result: message.clone(),
            }],
            CodingAgentEvent::ToolCallCompleted {
                tool_call_id,
                summary,
                ..
            } => vec![UiEvent::ToolFinished {
                call_id: tool_call_id.clone(),
                result: summary.clone(),
                is_error: false,
            }],
            CodingAgentEvent::ToolCallFailed {
                tool_call_id,
                message,
                ..
            } => vec![UiEvent::ToolFinished {
                call_id: tool_call_id.clone(),
                result: message.clone(),
                is_error: true,
            }],
            CodingAgentEvent::RuntimeCompactionCompleted { summary, .. } => vec![
                UiEvent::CompactionNotice {
                    summary: summary.clone(),
                },
                UiEvent::UsageUpdate {
                    input: self.total_input,
                    output: self.total_output,
                    cache_read: self.total_cache_read,
                    cache_write: self.total_cache_write,
                    cost: self.total_cost,
                    context_tokens: None,
                },
            ],
            CodingAgentEvent::PromptFailed { error, .. } => vec![UiEvent::AgentError {
                error: error.to_string(),
            }],
            CodingAgentEvent::PromptAborted { reason, .. } => vec![UiEvent::AgentError {
                error: format!("prompt aborted: {reason}"),
            }],
            CodingAgentEvent::SessionOpened { .. }
            | CodingAgentEvent::SessionWritePending { .. }
            | CodingAgentEvent::SessionWriteCommitted { .. }
            | CodingAgentEvent::SessionWriteSkipped { .. }
            | CodingAgentEvent::PromptStarted { .. }
            | CodingAgentEvent::ProviderRequestStarted { .. }
            | CodingAgentEvent::AssistantMessageStarted { .. }
            | CodingAgentEvent::PromptCompleted { .. }
            | CodingAgentEvent::Diagnostic { .. }
            | CodingAgentEvent::CapabilityChanged => Vec::new(),
        }
    }
}

/// Extract text-only content from tool-result blocks. Tool results never
/// contain `thinking` blocks (those belong to the assistant message), so
/// this is a plain text concatenation.
fn tool_output_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Total context tokens reflected by a usage block.
///
/// Mirrors `calculateContextTokens` in
/// `pi/packages/coding-agent/src/core/compaction/compaction.ts`: prefer the
/// provider-reported `totalTokens`, falling back to the component sum.
fn calculate_context_tokens(usage: &Usage) -> u32 {
    if usage.total_tokens > 0 {
        usage.total_tokens
    } else {
        usage
            .input
            .saturating_add(usage.output)
            .saturating_add(usage.cache_read)
            .saturating_add(usage.cache_write)
    }
}

fn parse_tool_arguments(arguments_json: &str) -> serde_json::Value {
    serde_json::from_str(arguments_json)
        .unwrap_or_else(|_| serde_json::Value::String(arguments_json.to_string()))
}
