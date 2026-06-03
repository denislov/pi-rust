use bytes::Bytes;
use async_stream::stream;
use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use crate::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Cost, Model, StopReason, Usage,
};
use crate::util::json_repair::parse_streaming_json;
use crate::models::calculate_cost;
use super::sse::iterate_sse;
use super::wire;
use super::convert::map_stop_reason;

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
        let mut _block_id: Option<String> = None;
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
                        error: "cancelled".into(),
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
                        error: e,
                    };
                    return;
                }
                None => break,
            };

            let wire_event: wire::StreamEvent = match serde_json::from_str(&sse_event.data) {
                Ok(v) => v,
                Err(e) => {
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        error: format!("SSE parse error: {}", e),
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
                        yield AssistantMessageEvent::Start { partial: partial.clone() };
                        first_event = false;
                    }
                }

                wire::StreamEvent::ContentBlockStart { index: _index, content_block } => {
                    match content_block {
                        wire::ContentBlockStart::Text { text } => {
                            block_type = Some("text".into());
                            accumulated_text = text.clone();
                            let mut p = partial.clone();
                            p.content.push(ContentBlock::Text {
                                text: text.clone(),
                                text_signature: None,
                            });
                            yield AssistantMessageEvent::TextStart { partial: p };
                            if !accumulated_text.is_empty() {
                                yield AssistantMessageEvent::TextDelta {
                                    delta: text,
                                    partial: partial.clone(),
                                };
                            }
                        }
                        wire::ContentBlockStart::Thinking { thinking } => {
                            block_type = Some("thinking".into());
                            accumulated_thinking = thinking.clone();
                            let mut p = partial.clone();
                            p.content.push(ContentBlock::Thinking {
                                thinking: thinking.clone(),
                                thinking_signature: None,
                                redacted: None,
                            });
                            yield AssistantMessageEvent::ThinkingStart { partial: p };
                            if !accumulated_thinking.is_empty() {
                                yield AssistantMessageEvent::ThinkingDelta {
                                    delta: thinking,
                                    partial: partial.clone(),
                                };
                            }
                        }
                        wire::ContentBlockStart::RedactedThinking { .. } => {
                            block_type = Some("thinking".into());
                            let mut p = partial.clone();
                            p.content.push(ContentBlock::Thinking {
                                thinking: String::new(),
                                thinking_signature: None,
                                redacted: Some(true),
                            });
                            yield AssistantMessageEvent::ThinkingStart { partial: p };
                        }
                        wire::ContentBlockStart::ToolUse { id, name } => {
                            block_type = Some("tool_use".into());
                            _block_id = Some(id.clone());
                            accumulated_tool_args.clear();
                            let mut p = partial.clone();
                            p.content.push(ContentBlock::ToolCall {
                                id: id.clone(),
                                name: name.clone(),
                                arguments: serde_json::json!({}),
                                thought_signature: None,
                            });
                            yield AssistantMessageEvent::ToolcallStart { partial: p };
                        }
                    }
                }

                wire::StreamEvent::ContentBlockDelta { index: _, delta } => {
                    match delta {
                        wire::ContentBlockDelta::TextDelta { text } => {
                            if let Some(ContentBlock::Text { text: t, .. }) =
                                partial.content.last_mut()
                            {
                                t.push_str(&text);
                            }
                            accumulated_text.push_str(&text);
                            yield AssistantMessageEvent::TextDelta {
                                delta: text,
                                partial: partial.clone(),
                            };
                        }
                        wire::ContentBlockDelta::ThinkingDelta { thinking } => {
                            if let Some(ContentBlock::Thinking { thinking: t, .. }) =
                                partial.content.last_mut()
                            {
                                t.push_str(&thinking);
                            }
                            accumulated_thinking.push_str(&thinking);
                            yield AssistantMessageEvent::ThinkingDelta {
                                delta: thinking,
                                partial: partial.clone(),
                            };
                        }
                        wire::ContentBlockDelta::SignatureDelta { signature } => {
                            if block_type.as_deref() == Some("thinking") {
                                pending_thinking_signature = Some(signature);
                            } else {
                                pending_text_signature = Some(signature);
                            }
                        }
                        wire::ContentBlockDelta::InputJsonDelta { partial_json } => {
                            accumulated_tool_args.push_str(&partial_json);
                            let parsed = parse_streaming_json(&accumulated_tool_args);
                            if let Some(ContentBlock::ToolCall { arguments, .. }) =
                                partial.content.last_mut()
                            {
                                *arguments = parsed;
                            }
                            yield AssistantMessageEvent::ToolcallDelta {
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
                                partial.content.last_mut()
                            {
                                *text_signature = pending_text_signature.take();
                            }
                            yield AssistantMessageEvent::TextEnd { partial: partial.clone() };
                        }
                        Some("thinking") => {
                            if let Some(ContentBlock::Thinking { thinking_signature, .. }) =
                                partial.content.last_mut()
                            {
                                *thinking_signature = pending_thinking_signature.take();
                            }
                            yield AssistantMessageEvent::ThinkingEnd { partial: partial.clone() };
                        }
                        Some("tool_use") => {
                            if let Some(ContentBlock::ToolCall { thought_signature, .. }) =
                                partial.content.last_mut()
                            {
                                *thought_signature = pending_thought_signature.take();
                            }
                            yield AssistantMessageEvent::ToolcallEnd { partial: partial.clone() };
                        }
                        _ => {}
                    }
                    block_type = None;
                    _block_id = None;
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
            error: "stream ended without message_stop".into(),
        };
    })
}
