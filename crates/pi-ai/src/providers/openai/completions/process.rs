use async_stream::stream;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::collections::HashMap;
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
        let mut partial = AssistantMessage::empty("openai-completions", &model.id);
        partial.provider = Some(model.provider.clone());
        let mut text_content_index: Option<u32> = None;
        let mut tool_index_map: HashMap<u32, u32> = HashMap::new();
        let mut tool_args_acc: HashMap<u32, String> = HashMap::new();
        let mut finish_reason: Option<String> = None;
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

            if sse_event.data == "[DONE]" {
                break;
            }

            let chunk: wire::ChatCompletionChunk = match serde_json::from_str(&sse_event.data) {
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
                partial.response_id = Some(chunk.id.clone());
                partial.response_model = Some(chunk.model.clone());
                partial.timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                yield AssistantMessageEvent::Start { content_index: None, partial: partial.clone() };
                first_event = false;
            }

            for choice in chunk.choices {
                if let Some(fr) = choice.finish_reason.as_deref() {
                    if !fr.is_empty() && fr != "null" {
                        finish_reason = choice.finish_reason.clone();
                    }
                }

                if let Some(text_delta) = &choice.delta.content {
                    if !text_delta.is_empty() {
                        if text_content_index.is_none() {
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
                        let ci = text_content_index.unwrap();
                        if let Some(ContentBlock::Text { text, .. }) = partial.content.get_mut(ci as usize) {
                            text.push_str(text_delta);
                        }
                        yield AssistantMessageEvent::TextDelta {
                            content_index: ci,
                            delta: text_delta.clone(),
                            partial: partial.clone(),
                        };
                    }
                }

                if let Some(tc_deltas) = &choice.delta.tool_calls {
                    for tc in tc_deltas {
                        let openai_idx = tc.index.unwrap_or(0);
                        let content_pos = match tool_index_map.get(&openai_idx) {
                            Some(&pos) => pos,
                            None => {
                                let pos = partial.content.len() as u32;
                                tool_index_map.insert(openai_idx, pos);
                                partial.content.push(ContentBlock::ToolCall {
                                    id: tc.id.clone().unwrap_or_default(),
                                    name: String::new(),
                                    arguments: serde_json::json!({}),
                                    thought_signature: None,
                                });
                                yield AssistantMessageEvent::ToolcallStart {
                                    content_index: pos,
                                    partial: partial.clone(),
                                };
                                pos
                            }
                        };

                        if let Some(ref id) = tc.id {
                            if let Some(ContentBlock::ToolCall {
                                id: block_id, ..
                            }) = partial.content.get_mut(content_pos as usize)
                            {
                                *block_id = id.clone();
                            }
                        }
                        if let Some(ref func) = tc.function {
                            if let Some(ref name) = func.name {
                                if let Some(ContentBlock::ToolCall {
                                    name: block_name,
                                    id: block_id,
                                    ..
                                }) = partial.content.get_mut(content_pos as usize)
                                {
                                    *block_name = name.clone();
                                    if let Some(ref id) = tc.id {
                                        *block_id = id.clone();
                                    }
                                }
                            }
                            if let Some(ref args) = func.arguments {
                                let acc = tool_args_acc.entry(openai_idx).or_default();
                                acc.push_str(args);
                                let parsed = parse_streaming_json(acc);
                                if let Some(ContentBlock::ToolCall {
                                    arguments, ..
                                }) = partial.content.get_mut(content_pos as usize)
                                {
                                    *arguments = parsed;
                                }
                                yield AssistantMessageEvent::ToolcallDelta {
                                    content_index: content_pos,
                                    delta: args.clone(),
                                    partial: partial.clone(),
                                };
                            }
                        }
                    }
                }
            }

            if let Some(usage) = &chunk.usage {
                partial.usage = map_usage(usage, &model);
            }
        }

        if let Some(ci) = text_content_index {
            yield AssistantMessageEvent::TextEnd {
                content_index: ci,
                partial: partial.clone(),
            };
        }

        for (oi, pos) in &tool_index_map {
            let acc = tool_args_acc.get(oi).map(|s| s.as_str()).unwrap_or("");
            let parsed = parse_streaming_json(acc);
            if let Some(ContentBlock::ToolCall { arguments, .. }) =
                partial.content.get_mut(*pos as usize)
            {
                *arguments = parsed;
            }
            yield AssistantMessageEvent::ToolcallEnd {
                content_index: *pos,
                partial: partial.clone(),
            };
        }

        partial.stop_reason = map_finish_reason(finish_reason.as_deref());

        if partial.usage.total_tokens == 0 {
            partial.usage = Usage::default();
        }

        yield AssistantMessageEvent::Done {
            reason: partial.stop_reason.clone(),
            message: partial.clone(),
        };
    })
}

fn map_finish_reason(reason: Option<&str>) -> StopReason {
    match reason {
        Some("stop") | None => StopReason::Stop,
        Some("length") => StopReason::Length,
        Some("tool_calls") => StopReason::ToolUse,
        Some("content_filter") => StopReason::Error,
        _ => StopReason::Stop,
    }
}

fn map_usage(usage: &wire::ChatUsage, model: &Model) -> Usage {
    let cache_tokens = usage
        .prompt_tokens_details
        .as_ref()
        .map(|d| d.cached_tokens)
        .unwrap_or(0);

    let mut result = Usage {
        input: usage.prompt_tokens,
        output: usage.completion_tokens,
        cache_read: cache_tokens,
        cache_write: 0,
        total_tokens: if usage.total_tokens == 0 {
            usage.prompt_tokens + usage.completion_tokens
        } else {
            usage.total_tokens
        },
        cost: Cost::default(),
    };
    calculate_cost(model, &mut result);
    result
}
