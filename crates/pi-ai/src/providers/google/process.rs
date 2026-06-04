use async_stream::stream;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use super::wire;
use crate::models::calculate_cost;
use crate::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Cost, Model, StopReason, Usage,
};
use crate::util::sse::iterate_sse;

pub fn process<E>(
    body: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: Model,
    cancel: Option<CancellationToken>,
) -> crate::stream::EventStream
where
    E: std::fmt::Display + Send + 'static,
{
    Box::pin(stream! {
        let mut partial = AssistantMessage::empty("google-generative-ai", &model.id);
        partial.provider = Some(model.provider.clone());
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

            let response: wire::GenerateContentResponse = match serde_json::from_str(&sse_event.data) {
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

            if first_event {
                partial.timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                yield AssistantMessageEvent::Start { content_index: None, partial: partial.clone() };
                first_event = false;
            }

            for candidate in &response.candidates {
                if let Some(fr) = &candidate.finish_reason {
                    if !fr.is_empty() {
                        partial.stop_reason = map_finish_reason(fr);
                    }
                }

                if let Some(content) = &candidate.content {
                    for part in &content.parts {
                        if let Some(fc) = &part.function_call {
                            let content_index = partial.content.len() as u32;
                            partial.content.push(ContentBlock::ToolCall {
                                id: fc.name.clone(),
                                name: fc.name.clone(),
                                arguments: fc.args.clone(),
                                thought_signature: None,
                            });
                            yield AssistantMessageEvent::ToolcallStart {
                                content_index,
                                partial: partial.clone(),
                            };
                            yield AssistantMessageEvent::ToolcallDelta {
                                content_index,
                                delta: fc.args.to_string(),
                                partial: partial.clone(),
                            };
                            yield AssistantMessageEvent::ToolcallEnd {
                                content_index,
                                partial: partial.clone(),
                            };
                        }

                        if let Some(text) = &part.text {
                            let is_thought = part.thought.unwrap_or(false);
                            let content_index = partial.content.len() as u32;
                            if is_thought {
                                partial.content.push(ContentBlock::Thinking {
                                    thinking: text.clone(),
                                    thinking_signature: None,
                                    redacted: None,
                                });
                                yield AssistantMessageEvent::ThinkingStart {
                                    content_index,
                                    partial: partial.clone(),
                                };
                                yield AssistantMessageEvent::ThinkingDelta {
                                    content_index,
                                    delta: text.clone(),
                                    partial: partial.clone(),
                                };
                                yield AssistantMessageEvent::ThinkingEnd {
                                    content_index,
                                    partial: partial.clone(),
                                };
                            } else {
                                partial.content.push(ContentBlock::Text {
                                    text: text.clone(),
                                    text_signature: None,
                                });
                                yield AssistantMessageEvent::TextStart {
                                    content_index,
                                    partial: partial.clone(),
                                };
                                yield AssistantMessageEvent::TextDelta {
                                    content_index,
                                    delta: text.clone(),
                                    partial: partial.clone(),
                                };
                                yield AssistantMessageEvent::TextEnd {
                                    content_index,
                                    partial: partial.clone(),
                                };
                            }
                        }
                    }
                }
            }

            if let Some(usage) = &response.usage_metadata {
                partial.usage = map_usage(usage, &model);
            }
        }

        partial.stop_reason = partial.stop_reason.clone();

        let has_tool_calls = partial.content.iter().any(|b| {
            matches!(b, ContentBlock::ToolCall { .. })
        });
        if has_tool_calls {
            partial.stop_reason = StopReason::ToolUse;
        }

        yield AssistantMessageEvent::Done {
            reason: partial.stop_reason.clone(),
            message: partial.clone(),
        };
    })
}

fn map_finish_reason(reason: &str) -> StopReason {
    match reason {
        "STOP" => StopReason::Stop,
        "MAX_TOKENS" => StopReason::Length,
        "SAFETY" | "RECITATION" | "OTHER" => StopReason::Error,
        _ => StopReason::Stop,
    }
}

fn map_usage(usage: &wire::UsageMetadata, model: &Model) -> Usage {
    let mut result = Usage {
        input: usage.prompt_token_count,
        output: usage.candidates_token_count,
        cache_read: 0,
        cache_write: 0,
        total_tokens: usage.total_token_count,
        cost: Cost::default(),
    };
    calculate_cost(model, &mut result);
    result
}
