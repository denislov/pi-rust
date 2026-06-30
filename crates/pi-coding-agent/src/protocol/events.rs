use crate::coding_session::CodingAgentEvent;
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
            AgentEvent::BeforeProviderRequest { .. } => Vec::new(),
            AgentEvent::LlmEvent(event) => self.push_llm_event(event),
            AgentEvent::ToolCallStart {
                tool_call_id,
                tool_name,
                ..
            } => vec![ProtocolEvent::ToolExecutionStart {
                tool_call_id: tool_call_id.clone(),
                tool_name: tool_name.clone(),
                args: self
                    .tool_args
                    .get(tool_call_id)
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            }],
            AgentEvent::ToolCallUpdate {
                tool_call_id,
                tool_name,
                update,
            } => vec![ProtocolEvent::ToolExecutionUpdate {
                tool_call_id: tool_call_id.clone(),
                tool_name: tool_name.clone(),
                result: ToolExecutionResult {
                    content: update.content.clone(),
                    terminate: false,
                    details: update.details.clone(),
                },
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
                    details: result.details.clone(),
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

pub struct CodingProtocolEventAdapter {
    api: String,
    provider: String,
    model: String,
    messages: Vec<StoredAgentMessage>,
    current_assistant: Option<AssistantMessage>,
    current_tool_results: Vec<StoredAgentMessage>,
    assistant_open: bool,
}

impl CodingProtocolEventAdapter {
    pub fn new_with_provider(api: String, provider: String, model: String) -> Self {
        Self {
            api,
            provider,
            model,
            messages: Vec::new(),
            current_assistant: None,
            current_tool_results: Vec::new(),
            assistant_open: false,
        }
    }

    pub fn push(&mut self, event: &CodingAgentEvent) -> Vec<ProtocolEvent> {
        match event {
            CodingAgentEvent::AgentTurnStarted { .. } => {
                let mut events = self.finish_current_turn();
                events.push(ProtocolEvent::TurnStart);
                events
            }
            CodingAgentEvent::ProviderRequestStarted {
                provider, model, ..
            } => {
                self.provider = provider.clone();
                self.model = model.clone();
                Vec::new()
            }
            CodingAgentEvent::AssistantMessageStarted { .. } => {
                if self.assistant_open {
                    return Vec::new();
                }
                let message = self.ensure_assistant();
                self.assistant_open = true;
                vec![ProtocolEvent::MessageStart {
                    message: stored_assistant(&message),
                }]
            }
            CodingAgentEvent::AssistantMessageDelta { text, .. } => {
                let message = self.append_assistant_text(text);
                let mut events = Vec::new();
                if !self.assistant_open {
                    self.assistant_open = true;
                    events.push(ProtocolEvent::MessageStart {
                        message: stored_assistant(&message),
                    });
                }
                events.push(ProtocolEvent::MessageUpdate {
                    message: stored_assistant(&message),
                    assistant_message_event: AssistantMessageEvent::TextDelta {
                        content_index: 0,
                        delta: text.clone(),
                        partial: message,
                    },
                });
                events
            }
            CodingAgentEvent::AssistantMessageCompleted { final_text, .. } => {
                let message = self.assistant_message(final_text);
                let mut events = Vec::new();
                if !self.assistant_open {
                    self.assistant_open = true;
                    events.push(ProtocolEvent::MessageStart {
                        message: stored_assistant(&message),
                    });
                }
                self.current_assistant = Some(message);
                events
            }
            CodingAgentEvent::ToolCallStarted {
                tool_call_id,
                name,
                arguments_json,
                ..
            } => vec![ProtocolEvent::ToolExecutionStart {
                tool_call_id: tool_call_id.clone(),
                tool_name: name.clone(),
                args: serde_json::from_str(arguments_json).unwrap_or(serde_json::Value::Null),
            }],
            CodingAgentEvent::ToolCallUpdated {
                tool_call_id,
                name,
                message,
                ..
            } => vec![ProtocolEvent::ToolExecutionUpdate {
                tool_call_id: tool_call_id.clone(),
                tool_name: name.clone(),
                result: ToolExecutionResult {
                    content: text_content(message),
                    terminate: false,
                    details: None,
                },
            }],
            CodingAgentEvent::ToolCallCompleted {
                tool_call_id,
                name,
                summary,
                ..
            } => self.push_tool_result(tool_call_id, name, summary, false),
            CodingAgentEvent::ToolCallFailed {
                tool_call_id,
                name,
                message,
                ..
            } => self.push_tool_result(tool_call_id, name, message, true),
            CodingAgentEvent::RuntimeCompactionCompleted {
                summary,
                first_kept_message_id,
                tokens_before,
                ..
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
                        details: None,
                    }),
                    aborted: false,
                    will_retry: false,
                    error_message: None,
                },
            ],
            CodingAgentEvent::PromptCompleted { .. } => {
                let mut events = self.finish_current_turn();
                events.push(ProtocolEvent::AgentEnd {
                    messages: self.messages.clone(),
                });
                events
            }
            CodingAgentEvent::PromptFailed { error, .. } => {
                self.push_prompt_failed_message(&error.to_string())
            }
            CodingAgentEvent::PromptAborted { reason, .. } => {
                self.push_prompt_failed_message(reason)
            }
            CodingAgentEvent::SessionOpened { .. }
            | CodingAgentEvent::SessionWritePending { .. }
            | CodingAgentEvent::SessionWriteCommitted { .. }
            | CodingAgentEvent::SessionWriteSkipped { .. }
            | CodingAgentEvent::PromptStarted { .. }
            | CodingAgentEvent::Diagnostic { .. }
            | CodingAgentEvent::CapabilityChanged => Vec::new(),
        }
    }

    fn ensure_assistant(&mut self) -> AssistantMessage {
        if self.current_assistant.is_none() {
            self.current_assistant = Some(self.assistant_message(""));
        }
        self.current_assistant
            .clone()
            .expect("assistant was inserted when missing")
    }

    fn append_assistant_text(&mut self, text: &str) -> AssistantMessage {
        let mut message = self.ensure_assistant();
        append_text_content(&mut message, text);
        self.current_assistant = Some(message.clone());
        message
    }

    fn assistant_message(&self, text: &str) -> AssistantMessage {
        let mut message = AssistantMessage::empty(&self.api, &self.model);
        if !self.provider.is_empty() {
            message.provider = Some(self.provider.clone());
        }
        if !text.is_empty() {
            message.content = text_content(text);
        }
        message
    }

    fn push_tool_result(
        &mut self,
        tool_call_id: &str,
        tool_name: &str,
        text: &str,
        is_error: bool,
    ) -> Vec<ProtocolEvent> {
        let content = text_content(text);
        let tool_result = StoredAgentMessage::ToolResult {
            tool_call_id: tool_call_id.to_string(),
            tool_name: tool_name.to_string(),
            content: content.clone(),
            is_error,
            timestamp: 0,
        };
        self.current_tool_results.push(tool_result.clone());

        vec![
            ProtocolEvent::ToolExecutionEnd {
                tool_call_id: tool_call_id.to_string(),
                tool_name: tool_name.to_string(),
                result: ToolExecutionResult {
                    content,
                    terminate: false,
                    details: None,
                },
                is_error,
            },
            ProtocolEvent::MessageStart {
                message: tool_result.clone(),
            },
            ProtocolEvent::MessageEnd {
                message: tool_result,
            },
        ]
    }

    fn push_prompt_failed_message(&mut self, error: &str) -> Vec<ProtocolEvent> {
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

fn append_text_content(message: &mut AssistantMessage, text: &str) {
    match message.content.last_mut() {
        Some(ContentBlock::Text { text: existing, .. }) => existing.push_str(text),
        _ => message.content.push(ContentBlock::Text {
            text: text.to_string(),
            text_signature: None,
        }),
    }
}

fn text_content(text: &str) -> Vec<ContentBlock> {
    if text.is_empty() {
        Vec::new()
    } else {
        vec![ContentBlock::Text {
            text: text.to_string(),
            text_signature: None,
        }]
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
