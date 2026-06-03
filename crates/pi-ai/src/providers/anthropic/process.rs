use async_stream::stream;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use super::convert::map_stop_reason;
use super::sse::iterate_sse;
use super::wire;
use crate::models::calculate_cost;
use crate::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Cost, Model, StopReason, Usage,
};
use crate::util::json_repair::parse_streaming_json;

/// Process an SSE body stream into an EventStream.
/// This is the pure, testable core — no reqwest dependency.
pub fn process<E>(
    body: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: Model,
    cancel: Option<CancellationToken>,
) -> crate::stream::EventStream
where
    E: std::fmt::Display + Send + 'static,
{
    Box::pin(stream! {
        let mut partial = AssistantMessage::empty("anthropic-messages", &model.id);
        let mut block_type: Option<String> = None;
        let mut block_index: u32 = 0;
        let mut accumulated_text = String::new();
        let mut accumulated_thinking = String::new();
        let mut accumulated_tool_args = String::new();
        let mut pending_text_signature: Option<String> = None;
        let mut pending_thinking_signature: Option<String> = None;
        let mut pending_thought_signature: Option<String> = None;
        let mut message_usage = wire::MessageUsage::default();
        let mut stop_reason: Option<StopReason> = None;
        let mut first_event = true;

        let sse = iterate_sse(body);
        futures::pin_mut!(sse);

        loop {
            if let Some(ref token) = cancel {
                if token.is_cancelled() {
                    partial.stop_reason = StopReason::Aborted;
                    partial.error_message = Some("cancelled".into());
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Aborted,
                        message: partial.clone(),
                    };
                    return;
                }
            }

            let sse_event = match sse.next().await {
                Some(Ok(e)) => e,
                Some(Err(e)) => {
                    partial.stop_reason = StopReason::Error;
                    partial.error_message = Some(e.clone());
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        message: partial.clone(),
                    };
                    return;
                }
                None => break,
            };

            let wire_event: wire::StreamEvent = match serde_json::from_str(&sse_event.data) {
                Ok(v) => v,
                Err(e) => {
                    partial.stop_reason = StopReason::Error;
                    partial.error_message = Some(format!("SSE parse error: {}", e));
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        message: partial.clone(),
                    };
                    return;
                }
            };

            match wire_event {
                wire::StreamEvent::MessageStart { message } => {
                    partial.response_id = Some(message.id);
                    partial.response_model = Some(message.model);
                    message_usage = message.usage;
                    partial.timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    if first_event {
                        yield AssistantMessageEvent::Start { content_index: None, partial: partial.clone() };
                        first_event = false;
                    }
                }

                wire::StreamEvent::ContentBlockStart { index, content_block } => {
                    block_index = index;
                    match content_block {
                        wire::ContentBlockStart::Text { text } => {
                            block_type = Some("text".into());
                            accumulated_text = text.clone();
                            partial.content.push(ContentBlock::Text {
                                text: text.clone(),
                                text_signature: None,
                            });
                            yield AssistantMessageEvent::TextStart { content_index: index, partial: partial.clone() };
                            if !accumulated_text.is_empty() {
                                yield AssistantMessageEvent::TextDelta {
                                    content_index: index,
                                    delta: text,
                                    partial: partial.clone(),
                                };
                            }
                        }
                        wire::ContentBlockStart::Thinking { thinking } => {
                            block_type = Some("thinking".into());
                            accumulated_thinking = thinking.clone();
                            partial.content.push(ContentBlock::Thinking {
                                thinking: thinking.clone(),
                                thinking_signature: None,
                                redacted: None,
                            });
                            yield AssistantMessageEvent::ThinkingStart { content_index: index, partial: partial.clone() };
                            if !accumulated_thinking.is_empty() {
                                yield AssistantMessageEvent::ThinkingDelta {
                                    content_index: index,
                                    delta: thinking,
                                    partial: partial.clone(),
                                };
                            }
                        }
                        wire::ContentBlockStart::RedactedThinking { .. } => {
                            block_type = Some("thinking".into());
                            partial.content.push(ContentBlock::Thinking {
                                thinking: String::new(),
                                thinking_signature: None,
                                redacted: Some(true),
                            });
                            yield AssistantMessageEvent::ThinkingStart { content_index: index, partial: partial.clone() };
                        }
                        wire::ContentBlockStart::ToolUse { id, name } => {
                            block_type = Some("tool_use".into());
                            accumulated_tool_args.clear();
                            partial.content.push(ContentBlock::ToolCall {
                                id: id.clone(),
                                name: name.clone(),
                                arguments: serde_json::json!({}),
                                thought_signature: None,
                            });
                            yield AssistantMessageEvent::ToolcallStart { content_index: index, partial: partial.clone() };
                        }
                    }
                }

                wire::StreamEvent::ContentBlockDelta { index: _, delta } => {
                    match delta {
                        wire::ContentBlockDelta::TextDelta { text } => {
                            if let Some(ContentBlock::Text { text: t, .. }) =
                                partial.content.get_mut(block_index as usize)
                            {
                                t.push_str(&text);
                            }
                            accumulated_text.push_str(&text);
                            yield AssistantMessageEvent::TextDelta {
                                content_index: block_index,
                                delta: text,
                                partial: partial.clone(),
                            };
                        }
                        wire::ContentBlockDelta::ThinkingDelta { thinking } => {
                            if let Some(ContentBlock::Thinking { thinking: t, .. }) =
                                partial.content.get_mut(block_index as usize)
                            {
                                t.push_str(&thinking);
                            }
                            accumulated_thinking.push_str(&thinking);
                            yield AssistantMessageEvent::ThinkingDelta {
                                content_index: block_index,
                                delta: thinking,
                                partial: partial.clone(),
                            };
                        }
                        wire::ContentBlockDelta::SignatureDelta { signature } => {
                            match block_type.as_deref() {
                                Some("thinking") => pending_thinking_signature = Some(signature),
                                Some("tool_use") => pending_thought_signature = Some(signature),
                                _ => pending_text_signature = Some(signature),
                            }
                        }
                        wire::ContentBlockDelta::InputJsonDelta { partial_json } => {
                            accumulated_tool_args.push_str(&partial_json);
                            let parsed = parse_streaming_json(&accumulated_tool_args);
                            if let Some(ContentBlock::ToolCall { arguments, .. }) =
                                partial.content.get_mut(block_index as usize)
                            {
                                *arguments = parsed;
                            }
                            yield AssistantMessageEvent::ToolcallDelta {
                                content_index: block_index,
                                delta: partial_json,
                                partial: partial.clone(),
                            };
                        }
                    }
                }

                wire::StreamEvent::ContentBlockStop { index: _ } => {
                    match block_type.as_deref() {
                        Some("text") => {
                            if let Some(ContentBlock::Text { text_signature, .. }) =
                                partial.content.get_mut(block_index as usize)
                            {
                                *text_signature = pending_text_signature.take();
                            }
                            yield AssistantMessageEvent::TextEnd { content_index: block_index, partial: partial.clone() };
                        }
                        Some("thinking") => {
                            if let Some(ContentBlock::Thinking { thinking_signature, .. }) =
                                partial.content.get_mut(block_index as usize)
                            {
                                *thinking_signature = pending_thinking_signature.take();
                            }
                            yield AssistantMessageEvent::ThinkingEnd { content_index: block_index, partial: partial.clone() };
                        }
                        Some("tool_use") => {
                            if let Some(ContentBlock::ToolCall { thought_signature, .. }) =
                                partial.content.get_mut(block_index as usize)
                            {
                                *thought_signature = pending_thought_signature.take();
                            }
                            yield AssistantMessageEvent::ToolcallEnd { content_index: block_index, partial: partial.clone() };
                        }
                        _ => {}
                    }
                    block_type = None;
                }

                wire::StreamEvent::MessageDelta { delta, usage } => {
                    stop_reason = delta.stop_reason.as_deref().map(map_stop_reason);
                    message_usage.output_tokens = usage.output_tokens;
                    if let Some(cache_read) = usage.cache_read_input_tokens {
                        message_usage.cache_read_input_tokens = Some(cache_read);
                    }
                    if let Some(cache_write) = usage.cache_creation_input_tokens {
                        message_usage.cache_creation_input_tokens = Some(cache_write);
                    }
                }

                wire::StreamEvent::MessageStop => {
                    let mut usage = Usage {
                        input: message_usage.input_tokens,
                        output: message_usage.output_tokens,
                        cache_read: message_usage.cache_read_input_tokens.unwrap_or(0),
                        cache_write: message_usage.cache_creation_input_tokens.unwrap_or(0),
                        total_tokens: message_usage.input_tokens + message_usage.output_tokens,
                        cost: Cost::default(),
                    };
                    calculate_cost(&model, &mut usage);

                    partial.usage = usage;
                    partial.stop_reason = stop_reason.unwrap_or(StopReason::Stop);
                    partial.provider = Some("anthropic".into());

                    yield AssistantMessageEvent::Done {
                        reason: partial.stop_reason.clone(),
                        message: partial.clone(),
                    };
                    return;
                }

                wire::StreamEvent::Ping => {}
            }
        }

        partial.stop_reason = StopReason::Error;
        partial.error_message = Some("stream ended without message_stop".into());
        yield AssistantMessageEvent::Error {
            reason: StopReason::Error,
            message: partial.clone(),
        };
    })
}
