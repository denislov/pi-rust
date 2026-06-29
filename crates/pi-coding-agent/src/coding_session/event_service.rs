#![allow(dead_code)]

use tokio::sync::broadcast;

use pi_agent_core::AgentEvent;
use pi_ai::types::{AssistantMessageEvent, ContentBlock};

use super::{CodingAgentEvent, CodingSessionError};

const EVENT_CHANNEL_CAPACITY: usize = 128;

#[derive(Debug)]
pub(crate) struct EventService {
    sender: broadcast::Sender<CodingAgentEvent>,
}

impl EventService {
    pub(crate) fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self { sender }
    }

    pub(crate) fn emit(&self, event: CodingAgentEvent) {
        let _ = self.sender.send(event);
    }

    pub(crate) fn emit_agent_event(
        &self,
        context: &AgentEventMappingContext,
        event: &AgentEvent,
    ) -> Vec<CodingAgentEvent> {
        let events = map_agent_event(context, event);
        for event in &events {
            self.emit(event.clone());
        }
        events
    }

    pub(crate) fn subscribe(&self) -> CodingAgentEventReceiver {
        CodingAgentEventReceiver {
            inner: self.sender.subscribe(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentEventMappingContext {
    operation_id: String,
    turn_id: String,
    assistant_message_id: Option<String>,
}

impl AgentEventMappingContext {
    pub(crate) fn new(operation_id: impl Into<String>, turn_id: impl Into<String>) -> Self {
        Self {
            operation_id: operation_id.into(),
            turn_id: turn_id.into(),
            assistant_message_id: None,
        }
    }

    pub(crate) fn with_assistant_message_id(mut self, message_id: impl Into<String>) -> Self {
        self.assistant_message_id = Some(message_id.into());
        self
    }
}

pub(crate) fn map_agent_event(
    context: &AgentEventMappingContext,
    event: &AgentEvent,
) -> Vec<CodingAgentEvent> {
    match event {
        AgentEvent::TurnStart { turn } => vec![CodingAgentEvent::AgentTurnStarted {
            operation_id: context.operation_id.clone(),
            turn_id: context.turn_id.clone(),
            agent_turn: *turn,
        }],
        AgentEvent::BeforeProviderRequest { request } => {
            vec![CodingAgentEvent::ProviderRequestStarted {
                operation_id: context.operation_id.clone(),
                turn_id: context.turn_id.clone(),
                provider: request.model.provider.clone(),
                model: request.model.id.clone(),
            }]
        }
        AgentEvent::LlmEvent(event) => map_assistant_event(context, event),
        AgentEvent::ToolCallStart {
            tool_call_id,
            tool_name,
            arguments,
        } => vec![CodingAgentEvent::ToolCallStarted {
            operation_id: context.operation_id.clone(),
            turn_id: context.turn_id.clone(),
            tool_call_id: tool_call_id.clone(),
            name: tool_name.clone(),
            arguments_json: arguments.to_string(),
        }],
        AgentEvent::ToolCallUpdate {
            tool_call_id,
            tool_name,
            update,
        } => vec![CodingAgentEvent::ToolCallUpdated {
            operation_id: context.operation_id.clone(),
            turn_id: context.turn_id.clone(),
            tool_call_id: tool_call_id.clone(),
            name: tool_name.clone(),
            message: content_blocks_text(&update.content),
        }],
        AgentEvent::ToolCallEnd {
            tool_call_id,
            tool_name,
            result,
        } if result.is_error => vec![CodingAgentEvent::ToolCallFailed {
            operation_id: context.operation_id.clone(),
            turn_id: context.turn_id.clone(),
            tool_call_id: tool_call_id.clone(),
            name: tool_name.clone(),
            message: content_blocks_text(&result.content),
        }],
        AgentEvent::ToolCallEnd {
            tool_call_id,
            tool_name,
            result,
        } => vec![CodingAgentEvent::ToolCallCompleted {
            operation_id: context.operation_id.clone(),
            turn_id: context.turn_id.clone(),
            tool_call_id: tool_call_id.clone(),
            name: tool_name.clone(),
            summary: content_blocks_text(&result.content),
        }],
        AgentEvent::AgentDone { message } => {
            vec![CodingAgentEvent::AssistantMessageCompleted {
                operation_id: context.operation_id.clone(),
                turn_id: context.turn_id.clone(),
                message_id: context.assistant_message_id.clone(),
                final_text: content_blocks_text(&message.content),
            }]
        }
        AgentEvent::AgentError { error } => vec![CodingAgentEvent::PromptFailed {
            operation_id: context.operation_id.clone(),
            error: CodingSessionError::Provider {
                message: error.clone(),
            },
        }],
        AgentEvent::SessionCompacted {
            summary,
            first_kept_message_id,
            tokens_before,
            details: _,
        } => vec![CodingAgentEvent::RuntimeCompactionCompleted {
            operation_id: context.operation_id.clone(),
            turn_id: context.turn_id.clone(),
            summary: summary.clone(),
            first_kept_message_id: first_kept_message_id.clone(),
            tokens_before: *tokens_before,
        }],
    }
}

fn map_assistant_event(
    context: &AgentEventMappingContext,
    event: &AssistantMessageEvent,
) -> Vec<CodingAgentEvent> {
    match event {
        AssistantMessageEvent::Start { .. } | AssistantMessageEvent::TextStart { .. } => {
            vec![CodingAgentEvent::AssistantMessageStarted {
                operation_id: context.operation_id.clone(),
                turn_id: context.turn_id.clone(),
                message_id: context.assistant_message_id.clone(),
            }]
        }
        AssistantMessageEvent::TextDelta { delta, .. } => {
            vec![CodingAgentEvent::AssistantMessageDelta {
                operation_id: context.operation_id.clone(),
                turn_id: context.turn_id.clone(),
                message_id: context.assistant_message_id.clone(),
                text: delta.clone(),
            }]
        }
        AssistantMessageEvent::Error { message, .. } => vec![CodingAgentEvent::PromptFailed {
            operation_id: context.operation_id.clone(),
            error: CodingSessionError::Provider {
                message: message
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "assistant stream failed".into()),
            },
        }],
        AssistantMessageEvent::Done { .. }
        | AssistantMessageEvent::TextEnd { .. }
        | AssistantMessageEvent::ThinkingStart { .. }
        | AssistantMessageEvent::ThinkingDelta { .. }
        | AssistantMessageEvent::ThinkingEnd { .. }
        | AssistantMessageEvent::ToolcallStart { .. }
        | AssistantMessageEvent::ToolcallDelta { .. }
        | AssistantMessageEvent::ToolcallEnd { .. } => Vec::new(),
    }
}

fn content_blocks_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text, .. } => text.clone(),
            ContentBlock::Thinking { thinking, .. } => thinking.clone(),
            ContentBlock::Image { mime_type, .. } => format!("[image:{mime_type}]"),
            ContentBlock::ToolCall { name, .. } => format!("[tool_call:{name}]"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug)]
pub struct CodingAgentEventReceiver {
    inner: broadcast::Receiver<CodingAgentEvent>,
}

impl CodingAgentEventReceiver {
    pub async fn recv(&mut self) -> Result<CodingAgentEvent, CodingSessionError> {
        self.inner.recv().await.map_err(map_recv_error)
    }

    pub fn try_recv(&mut self) -> Result<Option<CodingAgentEvent>, CodingSessionError> {
        match self.inner.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(broadcast::error::TryRecvError::Empty) => Ok(None),
            Err(broadcast::error::TryRecvError::Closed) => Err(CodingSessionError::Cancelled),
            Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                Err(CodingSessionError::Resource {
                    message: format!("event receiver lagged by {skipped} events"),
                })
            }
        }
    }
}

fn map_recv_error(error: broadcast::error::RecvError) -> CodingSessionError {
    match error {
        broadcast::error::RecvError::Closed => CodingSessionError::Cancelled,
        broadcast::error::RecvError::Lagged(skipped) => CodingSessionError::Resource {
            message: format!("event receiver lagged by {skipped} events"),
        },
    }
}

#[cfg(test)]
mod tests {
    use pi_agent_core::{AgentEvent, AgentToolOutput, AgentToolResult, ProviderRequestSnapshot};
    use pi_ai::types::{
        AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Model, ModelCost,
        ModelInput, StopReason, StreamOptions,
    };
    use serde_json::json;

    use super::*;

    fn mapping_context() -> AgentEventMappingContext {
        AgentEventMappingContext::new("op_1", "turn_1").with_assistant_message_id("msg_1")
    }

    fn model() -> Model {
        Model {
            id: "test-model".into(),
            name: "Test Model".into(),
            api: "messages".into(),
            provider: "test-provider".into(),
            base_url: String::new(),
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost::default(),
            context_window: 0,
            max_tokens: 0,
            headers: None,
            compat: None,
        }
    }

    fn assistant_message(text: &str) -> AssistantMessage {
        let mut message = AssistantMessage::empty("messages", "test-model");
        message.content.push(ContentBlock::Text {
            text: text.into(),
            text_signature: None,
        });
        message
    }

    #[test]
    fn maps_turn_and_provider_request_events() {
        let context = mapping_context();
        assert_eq!(
            map_agent_event(&context, &AgentEvent::TurnStart { turn: 3 }),
            vec![CodingAgentEvent::AgentTurnStarted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                agent_turn: 3,
            }]
        );

        let event = AgentEvent::BeforeProviderRequest {
            request: ProviderRequestSnapshot {
                model: model(),
                context: Context {
                    system_prompt: None,
                    messages: Vec::new(),
                    tools: None,
                },
                stream_options: StreamOptions::default(),
            },
        };

        assert_eq!(
            map_agent_event(&context, &event),
            vec![CodingAgentEvent::ProviderRequestStarted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                provider: "test-provider".into(),
                model: "test-model".into(),
            }]
        );
    }

    #[test]
    fn maps_assistant_stream_and_done_events() {
        let context = mapping_context();
        let partial = AssistantMessage::empty("messages", "test-model");

        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::LlmEvent(AssistantMessageEvent::TextStart {
                    content_index: 0,
                    partial: partial.clone(),
                }),
            ),
            vec![CodingAgentEvent::AssistantMessageStarted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
            }]
        );
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::LlmEvent(AssistantMessageEvent::TextDelta {
                    content_index: 0,
                    delta: "hi".into(),
                    partial,
                }),
            ),
            vec![CodingAgentEvent::AssistantMessageDelta {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: "hi".into(),
            }]
        );
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::AgentDone {
                    message: assistant_message("done"),
                },
            ),
            vec![CodingAgentEvent::AssistantMessageCompleted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                final_text: "done".into(),
            }]
        );
    }

    #[test]
    fn maps_tool_lifecycle_events() {
        let context = mapping_context();
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::ToolCallStart {
                    tool_call_id: "tool_1".into(),
                    tool_name: "read".into(),
                    arguments: json!({"path": "Cargo.toml"}),
                },
            ),
            vec![CodingAgentEvent::ToolCallStarted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                name: "read".into(),
                arguments_json: r#"{"path":"Cargo.toml"}"#.into(),
            }]
        );
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::ToolCallUpdate {
                    tool_call_id: "tool_1".into(),
                    tool_name: "read".into(),
                    update: AgentToolOutput::new(vec![ContentBlock::Text {
                        text: "running".into(),
                        text_signature: None,
                    }]),
                },
            ),
            vec![CodingAgentEvent::ToolCallUpdated {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                name: "read".into(),
                message: "running".into(),
            }]
        );
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::ToolCallEnd {
                    tool_call_id: "tool_1".into(),
                    tool_name: "read".into(),
                    result: AgentToolResult::ok(vec![ContentBlock::Text {
                        text: "ok".into(),
                        text_signature: None,
                    }]),
                },
            ),
            vec![CodingAgentEvent::ToolCallCompleted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                name: "read".into(),
                summary: "ok".into(),
            }]
        );
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::ToolCallEnd {
                    tool_call_id: "tool_1".into(),
                    tool_name: "read".into(),
                    result: AgentToolResult::error("missing"),
                },
            ),
            vec![CodingAgentEvent::ToolCallFailed {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                name: "read".into(),
                message: "missing".into(),
            }]
        );
    }

    #[test]
    fn maps_error_and_compaction_events() {
        let context = mapping_context();
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::AgentError {
                    error: "provider failed".into(),
                },
            ),
            vec![CodingAgentEvent::PromptFailed {
                operation_id: "op_1".into(),
                error: CodingSessionError::Provider {
                    message: "provider failed".into(),
                },
            }]
        );

        let mut message = AssistantMessage::empty("messages", "test-model");
        message.error_message = Some("stream failed".into());
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::LlmEvent(AssistantMessageEvent::Error {
                    reason: StopReason::Error,
                    message,
                }),
            ),
            vec![CodingAgentEvent::PromptFailed {
                operation_id: "op_1".into(),
                error: CodingSessionError::Provider {
                    message: "stream failed".into(),
                },
            }]
        );

        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::SessionCompacted {
                    summary: "short".into(),
                    first_kept_message_id: "msg_kept".into(),
                    tokens_before: 42,
                    details: None,
                },
            ),
            vec![CodingAgentEvent::RuntimeCompactionCompleted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                summary: "short".into(),
                first_kept_message_id: "msg_kept".into(),
                tokens_before: 42,
            }]
        );
    }

    #[tokio::test]
    async fn event_service_emits_mapped_agent_events() {
        let service = EventService::new();
        let mut receiver = service.subscribe();
        let context = mapping_context();

        let mapped = service.emit_agent_event(
            &context,
            &AgentEvent::LlmEvent(AssistantMessageEvent::TextDelta {
                content_index: 0,
                delta: "hi".into(),
                partial: AssistantMessage::empty("messages", "test-model"),
            }),
        );

        assert_eq!(mapped.len(), 1);
        assert_eq!(
            receiver.recv().await.unwrap(),
            CodingAgentEvent::AssistantMessageDelta {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: "hi".into(),
            }
        );
    }
}
