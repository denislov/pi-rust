use super::wire;
use crate::models::calculate_cost;
use crate::providers::process_framework::{SseEventHandler, SseEventResult, process_sse};
use crate::stream::EventStream;
use crate::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Cost, Model, StopReason, Usage,
};
use crate::util::json_repair::parse_streaming_json;
use bytes::Bytes;
use futures::Stream;
use tokio_util::sync::CancellationToken;

pub fn process<E>(
    body: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: Model,
    cancel: Option<CancellationToken>,
) -> EventStream
where
    E: std::fmt::Display + Send + 'static,
{
    process_with_api_name(body, model, cancel, "openai-responses")
}

pub fn process_with_api_name<E>(
    body: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: Model,
    cancel: Option<CancellationToken>,
    api_name: &str,
) -> EventStream
where
    E: std::fmt::Display + Send + 'static,
{
    process_sse(body, model, cancel, ResponsesHandler::default(), api_name)
}

#[derive(Default)]
struct ResponsesHandler {
    first_event: bool,
    text_content_index: Option<u32>,
    tool_content_index: Option<u32>,
    accumulated_tool_args: String,
    response_id: Option<String>,
    usage: Option<wire::ResponseUsage>,
}

impl SseEventHandler for ResponsesHandler {
    fn handle_event(
        &mut self,
        data: &str,
        partial: &mut AssistantMessage,
        _model: &Model,
    ) -> Result<SseEventResult, String> {
        let event: wire::ResponseStreamEvent =
            serde_json::from_str(data).map_err(|e| format!("SSE parse error: {}", e))?;

        let mut events = Vec::new();

        match event {
            wire::ResponseStreamEvent::ResponseCreated { response } => {
                self.response_id = Some(response.id);
                if !self.first_event {
                    partial.response_id = self.response_id.clone();
                    partial.timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    events.push(AssistantMessageEvent::Start {
                        content_index: None,
                        partial: partial.clone(),
                    });
                    self.first_event = true;
                }
            }

            wire::ResponseStreamEvent::OutputItemAdded { item } => match item.item_type.as_str() {
                "function_call" => {
                    if let Some(ci) = self.tool_content_index {
                        let parsed = parse_streaming_json(&self.accumulated_tool_args);
                        if let Some(ContentBlock::ToolCall { arguments, .. }) =
                            partial.content.get_mut(ci as usize)
                        {
                            *arguments = parsed;
                        }
                        events.push(AssistantMessageEvent::ToolcallEnd {
                            content_index: ci,
                            partial: partial.clone(),
                        });
                    }
                    self.tool_content_index = Some(partial.content.len() as u32);
                    let call_id = item.call_id.unwrap_or(item.id.clone());
                    let call_name = item.name.unwrap_or_default();
                    self.accumulated_tool_args.clear();
                    partial.content.push(ContentBlock::ToolCall {
                        id: call_id,
                        name: call_name,
                        arguments: serde_json::json!({}),
                        thought_signature: None,
                    });
                    events.push(AssistantMessageEvent::ToolcallStart {
                        content_index: self.tool_content_index.unwrap(),
                        partial: partial.clone(),
                    });
                }
                _ => {}
            },

            wire::ResponseStreamEvent::ContentPartAdded { .. } => {
                self.text_content_index = Some(partial.content.len() as u32);
                partial.content.push(ContentBlock::Text {
                    text: String::new(),
                    text_signature: None,
                });
                events.push(AssistantMessageEvent::TextStart {
                    content_index: self.text_content_index.unwrap(),
                    partial: partial.clone(),
                });
            }

            wire::ResponseStreamEvent::OutputTextDelta { delta } => {
                if let Some(ci) = self.text_content_index {
                    if let Some(ContentBlock::Text { text, .. }) =
                        partial.content.get_mut(ci as usize)
                    {
                        text.push_str(&delta);
                    }
                    events.push(AssistantMessageEvent::TextDelta {
                        content_index: ci,
                        delta: delta.clone(),
                        partial: partial.clone(),
                    });
                }
            }

            wire::ResponseStreamEvent::FunctionCallArgumentsDelta { delta } => {
                self.accumulated_tool_args.push_str(&delta);
                let parsed = parse_streaming_json(&self.accumulated_tool_args);
                if let Some(ci) = self.tool_content_index {
                    if let Some(ContentBlock::ToolCall { arguments, .. }) =
                        partial.content.get_mut(ci as usize)
                    {
                        *arguments = parsed;
                    }
                    events.push(AssistantMessageEvent::ToolcallDelta {
                        content_index: ci,
                        delta: delta.clone(),
                        partial: partial.clone(),
                    });
                }
            }

            wire::ResponseStreamEvent::OutputItemDone { item } => match item.item_type.as_str() {
                "message" => {
                    if let Some(ci) = self.text_content_index {
                        events.push(AssistantMessageEvent::TextEnd {
                            content_index: ci,
                            partial: partial.clone(),
                        });
                        self.text_content_index = None;
                    }
                }
                "function_call" => {
                    if let Some(ci) = self.tool_content_index {
                        let parsed = parse_streaming_json(&self.accumulated_tool_args);
                        if let Some(ContentBlock::ToolCall { arguments, .. }) =
                            partial.content.get_mut(ci as usize)
                        {
                            *arguments = parsed;
                        }
                        events.push(AssistantMessageEvent::ToolcallEnd {
                            content_index: ci,
                            partial: partial.clone(),
                        });
                        self.tool_content_index = None;
                    }
                }
                _ => {}
            },

            wire::ResponseStreamEvent::ResponseCompleted { response } => {
                partial.response_id = self.response_id.clone();
                self.usage = response.usage;
            }
        }

        Ok(SseEventResult::Continue(events))
    }

    fn finalize(
        &self,
        partial: &mut AssistantMessage,
        model: &Model,
    ) -> Vec<AssistantMessageEvent> {
        let mut events = Vec::new();

        if let Some(ci) = self.text_content_index {
            events.push(AssistantMessageEvent::TextEnd {
                content_index: ci,
                partial: partial.clone(),
            });
        }
        if let Some(ci) = self.tool_content_index {
            let parsed = parse_streaming_json(&self.accumulated_tool_args);
            if let Some(ContentBlock::ToolCall { arguments, .. }) =
                partial.content.get_mut(ci as usize)
            {
                *arguments = parsed;
            }
            events.push(AssistantMessageEvent::ToolcallEnd {
                content_index: ci,
                partial: partial.clone(),
            });
        }

        if let Some(u) = &self.usage {
            partial.usage = map_usage(u, model);
        }
        let has_tool_calls = partial
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolCall { .. }));
        partial.stop_reason = if has_tool_calls {
            StopReason::ToolUse
        } else {
            StopReason::Stop
        };

        events
    }
}

fn map_usage(u: &wire::ResponseUsage, model: &Model) -> Usage {
    let mut result = Usage {
        input: u.input_tokens,
        output: u.output_tokens,
        cache_read: 0,
        cache_write: 0,
        total_tokens: if u.total_tokens == 0 {
            u.input_tokens + u.output_tokens
        } else {
            u.total_tokens
        },
        cost: Cost::default(),
    };
    calculate_cost(model, &mut result);
    result
}
