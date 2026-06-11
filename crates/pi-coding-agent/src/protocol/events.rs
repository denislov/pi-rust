use crate::protocol::types::{
    CompactionProtocolResult, CompactionReason, ProtocolEvent, ToolExecutionResult,
};
use pi_agent_core::session::{StoredAgentMessage, StoredUsage, StoredUsageCost};
use pi_agent_core::{AgentEvent, AgentToolResult};
use pi_ai::types::{AssistantMessage, AssistantMessageEvent, ContentBlock, StopReason};
use std::collections::HashMap;

pub struct ProtocolEventAdapter {
    api: String,
    provider: String,
    model: String,
    messages: Vec<StoredAgentMessage>,
    current_assistant: Option<AssistantMessage>,
    current_tool_results: Vec<StoredAgentMessage>,
    tool_args: HashMap<String, serde_json::Value>,
    assistant_open: bool,
}

impl ProtocolEventAdapter {
    pub fn new(api: String, model: String) -> Self {
        Self::new_with_provider(api, String::new(), model)
    }

    pub fn new_with_provider(api: String, provider: String, model: String) -> Self {
        Self {
            api,
            provider,
            model,
            messages: Vec::new(),
            current_assistant: None,
            current_tool_results: Vec::new(),
            tool_args: HashMap::new(),
            assistant_open: false,
        }
    }

    pub fn push(&mut self, event: &AgentEvent) -> Vec<ProtocolEvent> {
        match event {
            AgentEvent::TurnStart { .. } => {
                let mut events = self.finish_current_turn();
                events.push(ProtocolEvent::TurnStart);
                events
            }
            AgentEvent::LlmEvent(event) => self.push_llm_event(event),
            AgentEvent::ToolCallStart {
                tool_call_id,
                tool_name,
            } => vec![ProtocolEvent::ToolExecutionStart {
                tool_call_id: tool_call_id.clone(),
                tool_name: tool_name.clone(),
                args: self
                    .tool_args
                    .get(tool_call_id)
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            }],
            AgentEvent::ToolCallEnd {
                tool_call_id,
                tool_name,
                result,
            } => self.push_tool_call_end(tool_call_id, tool_name, result),
            AgentEvent::AgentDone { message } => {
                self.current_assistant = Some(message.clone());
                let mut events = self.finish_current_turn();
                events.push(ProtocolEvent::AgentEnd {
                    messages: self.messages.clone(),
                });
                events
            }
            AgentEvent::AgentError { error } => {
                let message = stored_error_assistant(&self.api, &self.provider, &self.model, error);
                self.messages.push(message.clone());
                vec![
                    ProtocolEvent::MessageStart {
                        message: message.clone(),
                    },
                    ProtocolEvent::MessageEnd {
                        message: message.clone(),
                    },
                    ProtocolEvent::TurnEnd {
                        message,
                        tool_results: Vec::new(),
                    },
                    ProtocolEvent::AgentEnd {
                        messages: self.messages.clone(),
                    },
                ]
            }
            AgentEvent::SessionCompacted {
                summary,
                first_kept_message_id,
                tokens_before,
                details,
            } => vec![
                ProtocolEvent::CompactionStart {
                    reason: CompactionReason::Threshold,
                },
                ProtocolEvent::CompactionEnd {
                    reason: CompactionReason::Threshold,
                    result: Some(CompactionProtocolResult {
                        summary: summary.clone(),
                        first_kept_message_id: first_kept_message_id.clone(),
                        tokens_before: *tokens_before,
                        details: details.clone(),
                    }),
                    aborted: false,
                    will_retry: false,
                    error_message: None,
                },
            ],
        }
    }

    fn push_llm_event(&mut self, event: &AssistantMessageEvent) -> Vec<ProtocolEvent> {
        match event {
            AssistantMessageEvent::Start { partial, .. } => {
                self.current_assistant = Some(partial.clone());
                self.assistant_open = true;
                vec![ProtocolEvent::MessageStart {
                    message: stored_assistant(partial),
                }]
            }
            AssistantMessageEvent::Done { message, .. }
            | AssistantMessageEvent::Error { message, .. } => {
                self.current_assistant = Some(message.clone());
                self.record_tool_args(message);
                if self.assistant_open {
                    Vec::new()
                } else {
                    self.assistant_open = true;
                    vec![ProtocolEvent::MessageStart {
                        message: stored_assistant(message),
                    }]
                }
            }
            _ => {
                let Some(partial) = assistant_event_partial(event) else {
                    return Vec::new();
                };
                self.current_assistant = Some(partial.clone());
                self.record_tool_args(partial);

                let mut events = Vec::new();
                if !self.assistant_open {
                    self.assistant_open = true;
                    events.push(ProtocolEvent::MessageStart {
                        message: stored_assistant(partial),
                    });
                }
                events.push(ProtocolEvent::MessageUpdate {
                    message: stored_assistant(partial),
                    assistant_message_event: event.clone(),
                });
                events
            }
        }
    }

    fn push_tool_call_end(
        &mut self,
        tool_call_id: &str,
        tool_name: &str,
        result: &AgentToolResult,
    ) -> Vec<ProtocolEvent> {
        let tool_result = StoredAgentMessage::ToolResult {
            tool_call_id: tool_call_id.to_string(),
            tool_name: tool_name.to_string(),
            content: result.content.clone(),
            is_error: result.is_error,
            timestamp: 0,
        };
        self.current_tool_results.push(tool_result.clone());

        vec![
            ProtocolEvent::ToolExecutionEnd {
                tool_call_id: tool_call_id.to_string(),
                tool_name: tool_name.to_string(),
                result: ToolExecutionResult {
                    content: result.content.clone(),
                    terminate: result.terminate,
                },
                is_error: result.is_error,
            },
            ProtocolEvent::MessageStart {
                message: tool_result.clone(),
            },
            ProtocolEvent::MessageEnd {
                message: tool_result,
            },
        ]
    }

    fn finish_current_turn(&mut self) -> Vec<ProtocolEvent> {
        let Some(message) = self.current_assistant.take() else {
            return Vec::new();
        };

        let stored = stored_assistant(&message);
        if !self.messages.contains(&stored) {
            self.messages.push(stored.clone());
        }
        for tool_result in &self.current_tool_results {
            if !self.messages.contains(tool_result) {
                self.messages.push(tool_result.clone());
            }
        }

        let events = vec![
            ProtocolEvent::MessageEnd {
                message: stored.clone(),
            },
            ProtocolEvent::TurnEnd {
                message: stored,
                tool_results: self.current_tool_results.clone(),
            },
        ];
        self.current_tool_results.clear();
        self.assistant_open = false;
        events
    }

    fn record_tool_args(&mut self, message: &AssistantMessage) {
        for block in &message.content {
            if let ContentBlock::ToolCall { id, arguments, .. } = block {
                self.tool_args.insert(id.clone(), arguments.clone());
            }
        }
    }
}

fn assistant_event_partial(event: &AssistantMessageEvent) -> Option<&AssistantMessage> {
    match event {
        AssistantMessageEvent::Start { partial, .. }
        | AssistantMessageEvent::TextStart { partial, .. }
        | AssistantMessageEvent::TextDelta { partial, .. }
        | AssistantMessageEvent::TextEnd { partial, .. }
        | AssistantMessageEvent::ThinkingStart { partial, .. }
        | AssistantMessageEvent::ThinkingDelta { partial, .. }
        | AssistantMessageEvent::ThinkingEnd { partial, .. }
        | AssistantMessageEvent::ToolcallStart { partial, .. }
        | AssistantMessageEvent::ToolcallDelta { partial, .. }
        | AssistantMessageEvent::ToolcallEnd { partial, .. } => Some(partial),
        AssistantMessageEvent::Done { message, .. }
        | AssistantMessageEvent::Error { message, .. } => Some(message),
    }
}

fn stored_assistant(message: &AssistantMessage) -> StoredAgentMessage {
    StoredAgentMessage::Assistant {
        content: message.content.clone(),
        api: message.api.clone(),
        provider: message.provider.clone().unwrap_or_default(),
        model: message.model.clone(),
        response_model: message.response_model.clone(),
        response_id: message.response_id.clone(),
        usage: StoredUsage {
            input: message.usage.input,
            output: message.usage.output,
            cache_read: message.usage.cache_read,
            cache_write: message.usage.cache_write,
            total: message.usage.total_tokens,
            cost: StoredUsageCost {
                input: message.usage.cost.input,
                output: message.usage.cost.output,
                cache_read: message.usage.cost.cache_read,
                cache_write: message.usage.cost.cache_write,
            },
        },
        stop_reason: message.stop_reason.clone(),
        error_message: message.error_message.clone(),
        timestamp: message.timestamp,
    }
}

fn stored_error_assistant(
    api: &str,
    provider: &str,
    model: &str,
    error: &str,
) -> StoredAgentMessage {
    StoredAgentMessage::Assistant {
        content: Vec::new(),
        api: api.to_string(),
        provider: provider.to_string(),
        model: model.to_string(),
        response_model: None,
        response_id: None,
        usage: StoredUsage::default(),
        stop_reason: StopReason::Error,
        error_message: Some(error.to_string()),
        timestamp: 0,
    }
}
