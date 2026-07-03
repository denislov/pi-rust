use crate::coding_session::CodingAgentEvent;
use crate::protocol::types::{
    CompactionProtocolResult, CompactionReason, ProtocolEvent, ToolExecutionResult,
};
use pi_agent_core::session::{StoredAgentMessage, StoredUsage, StoredUsageCost};
use pi_ai::types::{AssistantMessage, AssistantMessageEvent, ContentBlock, StopReason};

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
                let (content_index, message) = self.append_assistant_text(text);
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
                        content_index,
                        delta: text.clone(),
                        partial: message,
                    },
                });
                events
            }
            CodingAgentEvent::AssistantThinkingDelta { text, .. } => {
                let (content_index, message) = self.append_assistant_thinking(text);
                let mut events = Vec::new();
                if !self.assistant_open {
                    self.assistant_open = true;
                    events.push(ProtocolEvent::MessageStart {
                        message: stored_assistant(&message),
                    });
                }
                events.push(ProtocolEvent::MessageUpdate {
                    message: stored_assistant(&message),
                    assistant_message_event: AssistantMessageEvent::ThinkingDelta {
                        content_index,
                        delta: text.clone(),
                        partial: message,
                    },
                });
                events
            }
            CodingAgentEvent::AssistantMessageCompleted { final_text, .. } => {
                let mut message = self.ensure_assistant();
                if message.content.is_empty() && !final_text.is_empty() {
                    message.content = text_content(final_text);
                }
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
            } => Self::compaction_events(
                CompactionReason::Threshold,
                summary,
                first_kept_message_id,
                *tokens_before,
            ),
            CodingAgentEvent::SessionCompactionCompleted {
                summary,
                first_kept_message_id,
                tokens_before,
                ..
            } => Self::compaction_events(
                CompactionReason::Manual,
                summary,
                first_kept_message_id,
                *tokens_before,
            ),
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
            | CodingAgentEvent::DefaultAgentProfileChanged { .. }
            | CodingAgentEvent::AgentInvocationStarted { .. }
            | CodingAgentEvent::AgentInvocationCompleted { .. }
            | CodingAgentEvent::AgentInvocationFailed { .. }
            | CodingAgentEvent::AgentInvocationAborted { .. }
            | CodingAgentEvent::AgentTeamStarted { .. }
            | CodingAgentEvent::AgentTeamMemberStarted { .. }
            | CodingAgentEvent::AgentTeamMemberCompleted { .. }
            | CodingAgentEvent::AgentTeamCompleted { .. }
            | CodingAgentEvent::AgentTeamFailed { .. }
            | CodingAgentEvent::AgentTeamAborted { .. }
            | CodingAgentEvent::DelegationRequested { .. }
            | CodingAgentEvent::DelegationRejected { .. }
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

    fn append_assistant_text(&mut self, text: &str) -> (u32, AssistantMessage) {
        let mut message = self.ensure_assistant();
        let content_index = append_text_content(&mut message, text);
        self.current_assistant = Some(message.clone());
        (content_index, message)
    }

    fn append_assistant_thinking(&mut self, text: &str) -> (u32, AssistantMessage) {
        let mut message = self.ensure_assistant();
        let content_index = append_thinking_content(&mut message, text);
        self.current_assistant = Some(message.clone());
        (content_index, message)
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

    fn compaction_events(
        reason: CompactionReason,
        summary: &str,
        first_kept_message_id: &str,
        tokens_before: u32,
    ) -> Vec<ProtocolEvent> {
        vec![
            ProtocolEvent::CompactionStart { reason },
            ProtocolEvent::CompactionEnd {
                reason,
                result: Some(CompactionProtocolResult {
                    summary: summary.to_owned(),
                    first_kept_message_id: first_kept_message_id.to_owned(),
                    tokens_before,
                    details: None,
                }),
                aborted: false,
                will_retry: false,
                error_message: None,
            },
        ]
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

fn append_text_content(message: &mut AssistantMessage, text: &str) -> u32 {
    let last_index = message.content.len().saturating_sub(1) as u32;
    match message.content.last_mut() {
        Some(ContentBlock::Text { text: existing, .. }) => {
            existing.push_str(text);
            last_index
        }
        _ => {
            let index = message.content.len() as u32;
            message.content.push(ContentBlock::Text {
                text: text.to_string(),
                text_signature: None,
            });
            index
        }
    }
}

fn append_thinking_content(message: &mut AssistantMessage, text: &str) -> u32 {
    let last_index = message.content.len().saturating_sub(1) as u32;
    match message.content.last_mut() {
        Some(ContentBlock::Thinking { thinking, .. }) => {
            thinking.push_str(text);
            last_index
        }
        _ => {
            let index = message.content.len() as u32;
            message.content.push(ContentBlock::Thinking {
                thinking: text.to_string(),
                thinking_signature: None,
                redacted: None,
            });
            index
        }
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
