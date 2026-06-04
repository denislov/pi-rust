use async_stream::stream;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use super::wire;
use crate::models::calculate_cost;
use crate::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Cost, Model, StopReason, Usage,
};
use crate::util::json_repair::parse_streaming_json;
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
        let mut partial = AssistantMessage::empty("openai-responses", &model.id);
        partial.provider = Some(model.provider.clone());
        let mut text_content_index: Option<u32> = None;
        let mut tool_content_index: Option<u32> = None;
        let mut accumulated_tool_args: String = String::new();
        let mut first_event = true;
        let mut response_id: Option<String> = None;
        let mut usage: Option<wire::ResponseUsage> = None;

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

            let event: wire::ResponseStreamEvent = match serde_json::from_str(&sse_event.data) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("SSE parse error for data: {}", sse_event.data);
                    partial.stop_reason = StopReason::Error;
                    partial.error_message = Some(format!("SSE parse error: {}", e));
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        message: partial.clone(),
                    };
                    return;
                }
            };

            match event {
                wire::ResponseStreamEvent::ResponseCreated { response } => {
                    response_id = Some(response.id);
                    if first_event {
                        partial.response_id = response_id.clone();
                        partial.timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        yield AssistantMessageEvent::Start { content_index: None, partial: partial.clone() };
                        first_event = false;
                    }
                }

                wire::ResponseStreamEvent::OutputItemAdded { item } => match item.item_type.as_str() {
                    "function_call" => {
                        if let Some(ci) = tool_content_index {
                            let parsed = parse_streaming_json(&accumulated_tool_args);
                            if let Some(ContentBlock::ToolCall { arguments, .. }) =
                                partial.content.get_mut(ci as usize)
                            {
                                *arguments = parsed;
                            }
                            yield AssistantMessageEvent::ToolcallEnd {
                                content_index: ci,
                                partial: partial.clone(),
                            };
                        }
                        tool_content_index = Some(partial.content.len() as u32);
                        let call_id = item.call_id.unwrap_or(item.id.clone());
                        let call_name = item.name.unwrap_or_default();
                        accumulated_tool_args.clear();
                        partial.content.push(ContentBlock::ToolCall {
                            id: call_id,
                            name: call_name,
                            arguments: serde_json::json!({}),
                            thought_signature: None,
                        });
                        yield AssistantMessageEvent::ToolcallStart {
                            content_index: tool_content_index.unwrap(),
                            partial: partial.clone(),
                        };
                    }
                    _ => {}
                },

                wire::ResponseStreamEvent::ContentPartAdded { .. } => {
                    text_content_index = Some(partial.content.len() as u32);
                    partial.content.push(ContentBlock::Text {
                        text: String::new(),
                        text_signature: None,
                    });
                    yield AssistantMessageEvent::TextStart {
                        content_index: text_content_index.unwrap(),
                        partial: partial.clone(),
                    };
                }

                wire::ResponseStreamEvent::OutputTextDelta { delta } => {
                    if let Some(ci) = text_content_index {
                        if let Some(ContentBlock::Text { text, .. }) = partial.content.get_mut(ci as usize) {
                            text.push_str(&delta);
                        }
                        yield AssistantMessageEvent::TextDelta {
                            content_index: ci,
                            delta: delta.clone(),
                            partial: partial.clone(),
                        };
                    }
                }

                wire::ResponseStreamEvent::FunctionCallArgumentsDelta { delta } => {
                    accumulated_tool_args.push_str(&delta);
                    let parsed = parse_streaming_json(&accumulated_tool_args);
                    if let Some(ci) = tool_content_index {
                        if let Some(ContentBlock::ToolCall { arguments, .. }) =
                            partial.content.get_mut(ci as usize)
                        {
                            *arguments = parsed;
                        }
                        yield AssistantMessageEvent::ToolcallDelta {
                            content_index: ci,
                            delta: delta.clone(),
                            partial: partial.clone(),
                        };
                    }
                }

                wire::ResponseStreamEvent::OutputItemDone { item } => {
                    match item.item_type.as_str() {
                        "message" => {
                            if let Some(ci) = text_content_index {
                                yield AssistantMessageEvent::TextEnd {
                                    content_index: ci,
                                    partial: partial.clone(),
                                };
                                text_content_index = None;
                            }
                        }
                        "function_call" => {
                            if let Some(ci) = tool_content_index {
                                let parsed = parse_streaming_json(&accumulated_tool_args);
                                if let Some(ContentBlock::ToolCall { arguments, .. }) =
                                    partial.content.get_mut(ci as usize)
                                {
                                    *arguments = parsed;
                                }
                                yield AssistantMessageEvent::ToolcallEnd {
                                    content_index: ci,
                                    partial: partial.clone(),
                                };
                                tool_content_index = None;
                            }
                        }
                        _ => {}
                    }
                }

                wire::ResponseStreamEvent::ResponseCompleted { response } => {
                    partial.response_id = response_id.clone();
                    usage = response.usage;
                }
            }
        }

        if let Some(ci) = text_content_index {
            yield AssistantMessageEvent::TextEnd {
                content_index: ci,
                partial: partial.clone(),
            };
        }
        if let Some(ci) = tool_content_index {
            let parsed = parse_streaming_json(&accumulated_tool_args);
            if let Some(ContentBlock::ToolCall { arguments, .. }) = partial.content.get_mut(ci as usize) {
                *arguments = parsed;
            }
            yield AssistantMessageEvent::ToolcallEnd {
                content_index: ci,
                partial: partial.clone(),
            };
        }

        if let Some(u) = usage {
            partial.usage = map_usage(&u, &model);
        }
        let has_tool_calls = partial.content.iter().any(|b| {
            matches!(b, ContentBlock::ToolCall { .. })
        });
        partial.stop_reason = if has_tool_calls {
            StopReason::ToolUse
        } else {
            StopReason::Stop
        };

        yield AssistantMessageEvent::Done {
            reason: partial.stop_reason.clone(),
            message: partial.clone(),
        };
    })
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
