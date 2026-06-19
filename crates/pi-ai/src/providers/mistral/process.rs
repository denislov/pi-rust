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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenBlock {
    Text(u32),
    Thinking(u32),
}

pub fn process<E>(
    body: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: Model,
    cancel: Option<CancellationToken>,
) -> crate::stream::EventStream
where
    E: std::fmt::Display + Send + 'static,
{
    Box::pin(stream! {
        let mut partial = AssistantMessage::empty("mistral-conversations", &model.id);
        partial.provider = Some(model.provider.clone());
        let mut current_block: Option<OpenBlock> = None;
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
                let _choice_index = choice.index;
                if let Some(reason) = choice.finish_reason.as_deref() {
                    if !reason.is_empty() && reason != "null" {
                        finish_reason = choice.finish_reason.clone();
                    }
                }

                if let Some(content) = choice.delta.content {
                    match content {
                        wire::ContentDelta::Text(text) => {
                            if !text.is_empty() {
                                for event in emit_text_delta(&mut partial, &mut current_block, text) {
                                    yield event;
                                }
                            }
                        }
                        wire::ContentDelta::Parts(parts) => {
                            for part in parts {
                                match part {
                                    wire::ContentDeltaPart::Text { text } => {
                                        if !text.is_empty() {
                                            for event in emit_text_delta(&mut partial, &mut current_block, text) {
                                                yield event;
                                            }
                                        }
                                    }
                                    wire::ContentDeltaPart::Thinking { thinking } => {
                                        let text = thinking
                                            .into_iter()
                                            .map(|part| match part {
                                                wire::ThinkingDeltaPart::Text { text } => text,
                                            })
                                            .collect::<Vec<_>>()
                                            .join("");
                                        if !text.is_empty() {
                                            for event in emit_thinking_delta(&mut partial, &mut current_block, text) {
                                                yield event;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(tool_calls) = choice.delta.tool_calls {
                    if let Some(event) = finish_current_block(&partial, &mut current_block) {
                        yield event;
                    }
                    for tool_call in tool_calls {
                        let provider_index = tool_call.index.unwrap_or(0);
                        let content_pos = match tool_index_map.get(&provider_index) {
                            Some(&pos) => pos,
                            None => {
                                let pos = partial.content.len() as u32;
                                tool_index_map.insert(provider_index, pos);
                                partial.content.push(ContentBlock::ToolCall {
                                    id: tool_call.id.clone().unwrap_or_default(),
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

                        let _tool_type = tool_call.tool_type.as_deref();
                        if let Some(id) = tool_call.id {
                            if let Some(ContentBlock::ToolCall { id: block_id, .. }) =
                                partial.content.get_mut(content_pos as usize)
                            {
                                *block_id = id;
                            }
                        }

                        if let Some(function) = tool_call.function {
                            if let Some(name) = function.name {
                                if let Some(ContentBlock::ToolCall { name: block_name, .. }) =
                                    partial.content.get_mut(content_pos as usize)
                                {
                                    *block_name = name;
                                }
                            }
                            if let Some(args) = function.arguments {
                                let acc = tool_args_acc.entry(provider_index).or_default();
                                acc.push_str(&args);
                                let parsed = parse_streaming_json(acc);
                                if let Some(ContentBlock::ToolCall { arguments, .. }) =
                                    partial.content.get_mut(content_pos as usize)
                                {
                                    *arguments = parsed;
                                }
                                yield AssistantMessageEvent::ToolcallDelta {
                                    content_index: content_pos,
                                    delta: args,
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

        if let Some(event) = finish_current_block(&partial, &mut current_block) {
            yield event;
        }

        for (provider_index, content_pos) in &tool_index_map {
            let acc = tool_args_acc
                .get(provider_index)
                .map(String::as_str)
                .unwrap_or("");
            let parsed = parse_streaming_json(acc);
            if let Some(ContentBlock::ToolCall { arguments, .. }) =
                partial.content.get_mut(*content_pos as usize)
            {
                *arguments = parsed;
            }
            yield AssistantMessageEvent::ToolcallEnd {
                content_index: *content_pos,
                partial: partial.clone(),
            };
        }

        partial.stop_reason = map_finish_reason(finish_reason.as_deref());
        yield AssistantMessageEvent::Done {
            reason: partial.stop_reason.clone(),
            message: partial.clone(),
        };
    })
}

fn emit_text_delta(
    partial: &mut AssistantMessage,
    current_block: &mut Option<OpenBlock>,
    text: String,
) -> Vec<AssistantMessageEvent> {
    let mut events = Vec::new();
    if !matches!(current_block, Some(OpenBlock::Text(_))) {
        if let Some(event) = finish_current_block(partial, current_block) {
            events.push(event);
        }
        let content_index = partial.content.len() as u32;
        partial.content.push(ContentBlock::Text {
            text: String::new(),
            text_signature: None,
        });
        *current_block = Some(OpenBlock::Text(content_index));
        events.push(AssistantMessageEvent::TextStart {
            content_index,
            partial: partial.clone(),
        });
    }

    let content_index = match current_block {
        Some(OpenBlock::Text(index)) => *index,
        _ => unreachable!(),
    };
    if let Some(ContentBlock::Text {
        text: block_text, ..
    }) = partial.content.get_mut(content_index as usize)
    {
        block_text.push_str(&text);
    }
    events.push(AssistantMessageEvent::TextDelta {
        content_index,
        delta: text,
        partial: partial.clone(),
    });
    events
}

fn emit_thinking_delta(
    partial: &mut AssistantMessage,
    current_block: &mut Option<OpenBlock>,
    text: String,
) -> Vec<AssistantMessageEvent> {
    let mut events = Vec::new();
    if !matches!(current_block, Some(OpenBlock::Thinking(_))) {
        if let Some(event) = finish_current_block(partial, current_block) {
            events.push(event);
        }
        let content_index = partial.content.len() as u32;
        partial.content.push(ContentBlock::Thinking {
            thinking: String::new(),
            thinking_signature: None,
            redacted: None,
        });
        *current_block = Some(OpenBlock::Thinking(content_index));
        events.push(AssistantMessageEvent::ThinkingStart {
            content_index,
            partial: partial.clone(),
        });
    }

    let content_index = match current_block {
        Some(OpenBlock::Thinking(index)) => *index,
        _ => unreachable!(),
    };
    if let Some(ContentBlock::Thinking { thinking, .. }) =
        partial.content.get_mut(content_index as usize)
    {
        thinking.push_str(&text);
    }
    events.push(AssistantMessageEvent::ThinkingDelta {
        content_index,
        delta: text,
        partial: partial.clone(),
    });
    events
}

fn finish_current_block(
    partial: &AssistantMessage,
    current_block: &mut Option<OpenBlock>,
) -> Option<AssistantMessageEvent> {
    match current_block.take() {
        Some(OpenBlock::Text(content_index)) => Some(AssistantMessageEvent::TextEnd {
            content_index,
            partial: partial.clone(),
        }),
        Some(OpenBlock::Thinking(content_index)) => Some(AssistantMessageEvent::ThinkingEnd {
            content_index,
            partial: partial.clone(),
        }),
        None => None,
    }
}

fn map_finish_reason(reason: Option<&str>) -> StopReason {
    match reason {
        Some("length") | Some("model_length") => StopReason::Length,
        Some("tool_calls") => StopReason::ToolUse,
        Some("error") => StopReason::Error,
        Some("stop") | None => StopReason::Stop,
        _ => StopReason::Stop,
    }
}

fn map_usage(usage: &wire::ChatUsage, model: &Model) -> Usage {
    let mut result = Usage {
        input: usage.prompt_tokens,
        output: usage.completion_tokens,
        cache_read: 0,
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
