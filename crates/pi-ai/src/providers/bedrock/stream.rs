use async_stream::stream;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

use super::wire;
use crate::model::Model;
use crate::model::calculate_cost;
use crate::protocol::json::parse_streaming_json;
use crate::protocol::{AssistantMessage, AssistantMessageEvent, ContentBlock, StopReason};

pub fn process<E>(
    mut body: impl Stream<Item = Result<Bytes, E>> + Send + Unpin + 'static,
    model: Model,
    cancel: Option<CancellationToken>,
) -> crate::protocol::stream::EventStream
where
    E: std::fmt::Display + Send + 'static,
{
    Box::pin(stream! {
        let mut partial = AssistantMessage::empty("bedrock-converse-stream", &model.id);
        partial.provider = Some(model.provider.clone());
        let mut decoder = EventStreamDecoder::default();
        let mut state = ProcessState::default();

        loop {
            let next_chunk = match cancel.as_ref() {
                Some(token) => tokio::select! {
                    biased;
                    _ = token.cancelled() => {
                        partial.stop_reason = StopReason::Aborted;
                        partial.error_message = Some(format!(
                            "Bedrock stream cancelled for provider {} model {}",
                            model.provider, model.id
                        ));
                        yield AssistantMessageEvent::Error {
                            reason: StopReason::Aborted,
                            message: partial.clone(),
                        };
                        return;
                    }
                    chunk = body.next() => chunk,
                },
                None => body.next().await,
            };
            let Some(chunk) = next_chunk else {
                break;
            };

            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(error) => {
                    partial.stop_reason = StopReason::Error;
                    partial.error_message = Some(error.to_string());
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        message: partial.clone(),
                    };
                    return;
                }
            };

            decoder.push(&chunk);
            loop {
                let payload = match decoder.next_payload() {
                    Ok(Some(payload)) => payload,
                    Ok(None) => break,
                    Err(error) => {
                        partial.stop_reason = StopReason::Error;
                        partial.error_message = Some(error);
                        yield AssistantMessageEvent::Error {
                            reason: StopReason::Error,
                            message: partial.clone(),
                        };
                        return;
                    }
                };

                if payload.is_empty() {
                    continue;
                }

                let raw: serde_json::Value = match serde_json::from_slice(&payload) {
                    Ok(value) => value,
                    Err(error) => {
                        partial.stop_reason = StopReason::Error;
                        partial.error_message = Some(format!("Bedrock event parse error: {}", error));
                        yield AssistantMessageEvent::Error {
                            reason: StopReason::Error,
                            message: partial.clone(),
                        };
                        return;
                    }
                };

                if let Some(error) = bedrock_exception_message(&raw) {
                    partial.stop_reason = StopReason::Error;
                    partial.error_message = Some(error);
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        message: partial.clone(),
                    };
                    return;
                }

                let event: wire::ConverseStreamEvent = match serde_json::from_value(raw) {
                    Ok(event) => event,
                    Err(error) => {
                        partial.stop_reason = StopReason::Error;
                        partial.error_message = Some(format!("Bedrock event shape error: {}", error));
                        yield AssistantMessageEvent::Error {
                            reason: StopReason::Error,
                            message: partial.clone(),
                        };
                        return;
                    }
                };

                match handle_event(event, &model, &mut partial, &mut state) {
                    Ok(events) => {
                        for event in events {
                            yield event;
                        }
                    }
                    Err(error) => {
                        partial.stop_reason = StopReason::Error;
                        partial.error_message = Some(error);
                        yield AssistantMessageEvent::Error {
                            reason: StopReason::Error,
                            message: partial.clone(),
                        };
                        return;
                    }
                }
            }
        }

        if decoder.has_buffered_bytes() {
            partial.stop_reason = StopReason::Error;
            partial.error_message = Some("incomplete Bedrock event-stream frame".into());
            yield AssistantMessageEvent::Error {
                reason: StopReason::Error,
                message: partial,
            };
            return;
        }

        if !state.message_stop_observed {
            partial.stop_reason = StopReason::Error;
            partial.error_message = Some(format!(
                "Bedrock stream ended without messageStop for provider {} model {}",
                model.provider, model.id
            ));
            yield AssistantMessageEvent::Error {
                reason: StopReason::Error,
                message: partial,
            };
            return;
        }

        if matches!(partial.stop_reason, StopReason::Error | StopReason::Aborted) {
            if partial.error_message.is_none() {
                partial.error_message = Some(format!(
                    "Bedrock provider {} model {} returned an invalid terminal reason",
                    model.provider, model.id
                ));
            }
            yield AssistantMessageEvent::Error {
                reason: partial.stop_reason.clone(),
                message: partial,
            };
            return;
        }

        let reason = partial.stop_reason.clone();
        yield AssistantMessageEvent::Done {
            reason,
            message: partial,
        };
    })
}

#[derive(Debug, Default)]
struct EventStreamDecoder {
    buffer: Vec<u8>,
}

impl EventStreamDecoder {
    fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    fn has_buffered_bytes(&self) -> bool {
        !self.buffer.is_empty()
    }

    fn next_payload(&mut self) -> Result<Option<Vec<u8>>, String> {
        if self.buffer.len() < 12 {
            return Ok(None);
        }

        let total_len = u32::from_be_bytes([
            self.buffer[0],
            self.buffer[1],
            self.buffer[2],
            self.buffer[3],
        ]) as usize;
        let headers_len = u32::from_be_bytes([
            self.buffer[4],
            self.buffer[5],
            self.buffer[6],
            self.buffer[7],
        ]) as usize;

        if total_len < 16 {
            return Err(format!(
                "invalid Bedrock event-stream frame length: {}",
                total_len
            ));
        }
        if headers_len > total_len.saturating_sub(16) {
            return Err(format!(
                "invalid Bedrock event-stream headers length: {}",
                headers_len
            ));
        }
        if self.buffer.len() < total_len {
            return Ok(None);
        }

        let payload_start = 12 + headers_len;
        let payload_end = total_len - 4;
        let payload = self.buffer[payload_start..payload_end].to_vec();
        self.buffer.drain(..total_len);
        Ok(Some(payload))
    }
}

#[derive(Debug, Default)]
struct ProcessState {
    block_map: HashMap<u32, usize>,
    tool_arg_buffers: HashMap<u32, String>,
    message_stop_observed: bool,
}

fn handle_event(
    event: wire::ConverseStreamEvent,
    model: &Model,
    partial: &mut AssistantMessage,
    state: &mut ProcessState,
) -> Result<Vec<AssistantMessageEvent>, String> {
    let mut events = Vec::new();

    if let Some(message_start) = event.message_start {
        if message_start.role != "assistant" {
            return Err(format!(
                "Unexpected Bedrock message role: {}",
                message_start.role
            ));
        }
        partial.timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        events.push(AssistantMessageEvent::Start {
            content_index: None,
            partial: partial.clone(),
        });
    }

    if let Some(start) = event.content_block_start
        && let Some(tool_use) = start.start.get("toolUse")
    {
        let id = tool_use
            .get("toolUseId")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let name = tool_use
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let index = partial.content.len();
        partial.content.push(ContentBlock::ToolCall {
            id,
            name,
            arguments: serde_json::Value::Object(serde_json::Map::new()),
            thought_signature: None,
        });
        state.block_map.insert(start.content_block_index, index);
        state
            .tool_arg_buffers
            .insert(start.content_block_index, String::new());
        events.push(AssistantMessageEvent::ToolcallStart {
            content_index: index as u32,
            partial: partial.clone(),
        });
    }

    if let Some(delta) = event.content_block_delta {
        let content_block_index = delta.content_block_index;
        if let Some(text) = delta.delta.get("text").and_then(|v| v.as_str()) {
            let (index, started) = ensure_text_block(partial, state, content_block_index);
            if started {
                events.push(AssistantMessageEvent::TextStart {
                    content_index: index as u32,
                    partial: partial.clone(),
                });
            }
            if let Some(ContentBlock::Text {
                text: block_text, ..
            }) = partial.content.get_mut(index)
            {
                block_text.push_str(text);
            }
            events.push(AssistantMessageEvent::TextDelta {
                content_index: index as u32,
                delta: text.to_string(),
                partial: partial.clone(),
            });
        } else if let Some(reasoning) = delta.delta.get("reasoningContent") {
            let (index, started) = ensure_thinking_block(partial, state, content_block_index);
            if started {
                events.push(AssistantMessageEvent::ThinkingStart {
                    content_index: index as u32,
                    partial: partial.clone(),
                });
            }
            let mut text_delta = None;
            if let Some(ContentBlock::Thinking {
                thinking,
                thinking_signature,
                ..
            }) = partial.content.get_mut(index)
            {
                if let Some(text) = reasoning.get("text").and_then(|v| v.as_str()) {
                    thinking.push_str(text);
                    text_delta = Some(text.to_string());
                }
                if let Some(signature) = reasoning.get("signature").and_then(|v| v.as_str()) {
                    match thinking_signature {
                        Some(existing) => existing.push_str(signature),
                        None => *thinking_signature = Some(signature.to_string()),
                    }
                }
            }
            if let Some(delta) = text_delta {
                events.push(AssistantMessageEvent::ThinkingDelta {
                    content_index: index as u32,
                    delta,
                    partial: partial.clone(),
                });
            }
        } else if let Some(tool_use) = delta.delta.get("toolUse")
            && let Some(index) = state.block_map.get(&content_block_index).copied()
        {
            let input = tool_use
                .get("input")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let args = state
                .tool_arg_buffers
                .entry(content_block_index)
                .or_default();
            args.push_str(input);
            if let Some(ContentBlock::ToolCall { arguments, .. }) = partial.content.get_mut(index) {
                *arguments = parse_streaming_json(args);
            }
            events.push(AssistantMessageEvent::ToolcallDelta {
                content_index: index as u32,
                delta: input.to_string(),
                partial: partial.clone(),
            });
        }
    }

    if let Some(stop) = event.content_block_stop
        && let Some(index) = state.block_map.get(&stop.content_block_index).copied()
    {
        if let Some(ContentBlock::ToolCall { arguments, .. }) = partial.content.get_mut(index)
            && let Some(args) = state.tool_arg_buffers.get(&stop.content_block_index)
        {
            *arguments = parse_streaming_json(args);
        }

        match partial.content.get(index) {
            Some(ContentBlock::Text { .. }) => events.push(AssistantMessageEvent::TextEnd {
                content_index: index as u32,
                partial: partial.clone(),
            }),
            Some(ContentBlock::Thinking { .. }) => {
                events.push(AssistantMessageEvent::ThinkingEnd {
                    content_index: index as u32,
                    partial: partial.clone(),
                })
            }
            Some(ContentBlock::ToolCall { .. }) => {
                events.push(AssistantMessageEvent::ToolcallEnd {
                    content_index: index as u32,
                    partial: partial.clone(),
                })
            }
            _ => {}
        }
    }

    if let Some(message_stop) = event.message_stop {
        partial.stop_reason = map_stop_reason(&message_stop.stop_reason);
        state.message_stop_observed = true;
    }

    if let Some(metadata) = event.metadata
        && let Some(usage) = metadata.usage
    {
        partial.usage.input = usage.input_tokens;
        partial.usage.output = usage.output_tokens;
        partial.usage.cache_read = usage.cache_read_input_tokens;
        partial.usage.cache_write = usage.cache_write_input_tokens;
        partial.usage.total_tokens = if usage.total_tokens == 0 {
            usage.input_tokens + usage.output_tokens
        } else {
            usage.total_tokens
        };
        calculate_cost(model, &mut partial.usage);
    }

    Ok(events)
}

fn ensure_text_block(
    partial: &mut AssistantMessage,
    state: &mut ProcessState,
    bedrock_index: u32,
) -> (usize, bool) {
    if let Some(index) = state.block_map.get(&bedrock_index).copied() {
        return (index, false);
    }
    let index = partial.content.len();
    partial.content.push(ContentBlock::Text {
        text: String::new(),
        text_signature: None,
    });
    state.block_map.insert(bedrock_index, index);
    (index, true)
}

fn ensure_thinking_block(
    partial: &mut AssistantMessage,
    state: &mut ProcessState,
    bedrock_index: u32,
) -> (usize, bool) {
    if let Some(index) = state.block_map.get(&bedrock_index).copied() {
        return (index, false);
    }
    let index = partial.content.len();
    partial.content.push(ContentBlock::Thinking {
        thinking: String::new(),
        thinking_signature: None,
        redacted: None,
    });
    state.block_map.insert(bedrock_index, index);
    (index, true)
}

fn map_stop_reason(reason: &str) -> StopReason {
    match reason {
        "end_turn" | "stop_sequence" => StopReason::Stop,
        "max_tokens" | "model_context_window_exceeded" => StopReason::Length,
        "tool_use" => StopReason::ToolUse,
        _ => StopReason::Error,
    }
}

fn bedrock_exception_message(raw: &serde_json::Value) -> Option<String> {
    const KEYS: &[&str] = &[
        "internalServerException",
        "modelStreamErrorException",
        "validationException",
        "throttlingException",
        "serviceUnavailableException",
    ];

    for key in KEYS {
        if let Some(value) = raw.get(*key) {
            if let Some(message) = value.get("message").and_then(|v| v.as_str()) {
                return Some(message.to_string());
            }
            return Some(format!("Bedrock stream error: {}", key));
        }
    }
    None
}
