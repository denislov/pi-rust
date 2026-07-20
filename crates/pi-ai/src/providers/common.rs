use async_stream::stream;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

use crate::protocol::json::parse_streaming_json;
use crate::protocol::{AssistantMessage, AssistantMessageEvent, StopReason};

use crate::model::Model;
use crate::protocol::stream::EventStream;
use crate::transport::sse::iterate_sse;

pub enum SseEventResult {
    Continue(Vec<AssistantMessageEvent>),
    ProviderDone(Vec<AssistantMessageEvent>),
    ProviderError {
        events: Vec<AssistantMessageEvent>,
        reason: StopReason,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SseTransportTerminal {
    DoneMarker,
    Eof,
}

pub(super) fn start_once(
    started: &mut bool,
    partial: &mut AssistantMessage,
    response_id: String,
    response_model: String,
) -> Option<AssistantMessageEvent> {
    if *started {
        return None;
    }
    partial.response_id = Some(response_id);
    partial.response_model = Some(response_model);
    partial.timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    *started = true;
    Some(AssistantMessageEvent::Start {
        content_index: None,
        partial: partial.clone(),
    })
}

#[derive(Default)]
pub(super) struct ToolArgumentAssembler {
    values: HashMap<u32, String>,
}

impl ToolArgumentAssembler {
    pub(super) fn append(&mut self, provider_index: u32, delta: &str) -> serde_json::Value {
        let value = self.values.entry(provider_index).or_default();
        value.push_str(delta);
        parse_streaming_json(value)
    }

    pub(super) fn finish(&self, provider_index: u32) -> serde_json::Value {
        parse_streaming_json(
            self.values
                .get(&provider_index)
                .map(String::as_str)
                .unwrap_or(""),
        )
    }
}

#[derive(Default)]
pub(super) struct ProviderTerminalLatch {
    observed: bool,
}

impl ProviderTerminalLatch {
    pub(super) fn observe(&mut self) {
        self.observed = true;
    }

    pub(super) fn accept(&self, terminal: SseTransportTerminal) -> Result<(), String> {
        match terminal {
            SseTransportTerminal::DoneMarker if self.observed => Ok(()),
            SseTransportTerminal::DoneMarker => {
                Err("received [DONE] before a usable finish reason".into())
            }
            SseTransportTerminal::Eof => {
                Err("stream ended before the required [DONE] marker".into())
            }
        }
    }
}

pub trait SseEventHandler: Send + 'static {
    fn handle_event(
        &mut self,
        data: &str,
        partial: &mut AssistantMessage,
        model: &Model,
    ) -> Result<SseEventResult, String>;

    fn finish(
        &mut self,
        partial: &mut AssistantMessage,
        model: &Model,
    ) -> Result<Vec<AssistantMessageEvent>, String>;

    fn accept_transport_terminal(&self, terminal: SseTransportTerminal) -> Result<(), String> {
        Err(match terminal {
            SseTransportTerminal::DoneMarker => {
                "provider protocol does not accept a [DONE] terminal marker".to_string()
            }
            SseTransportTerminal::Eof => {
                "provider stream ended before a terminal event".to_string()
            }
        })
    }
}

pub fn process_sse<E, H: SseEventHandler>(
    body: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: Model,
    cancel: Option<CancellationToken>,
    mut handler: H,
    api_name: &str,
) -> EventStream
where
    E: std::fmt::Display + Send + 'static,
{
    let api_name = api_name.to_string();
    Box::pin(stream! {
        let mut partial = AssistantMessage::empty(&api_name, &model.id);
        partial.provider = Some(model.provider.clone());

        let sse = iterate_sse(body);
        futures::pin_mut!(sse);

        loop {
            let next_event = match cancel.as_ref() {
                Some(token) => tokio::select! {
                    biased;
                    _ = token.cancelled() => {
                        partial.stop_reason = StopReason::Aborted;
                        partial.error_message = Some(format!(
                            "{} stream cancelled for provider {} model {}",
                            api_name, model.provider, model.id
                        ));
                        yield AssistantMessageEvent::Error {
                            reason: StopReason::Aborted,
                            message: partial.clone(),
                        };
                        return;
                    }
                    event = sse.next() => event,
                },
                None => sse.next().await,
            };

            let sse_event = match next_event {
                Some(Ok(e)) => e,
                Some(Err(e)) => {
                    partial.stop_reason = StopReason::Error;
                    partial.error_message = Some(format!(
                        "{} stream error for provider {} model {}: {}",
                        api_name, model.provider, model.id, e
                    ));
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        message: partial.clone(),
                    };
                    return;
                }
                None => break,
            };

            if sse_event.data == "[DONE]" {
                if let Err(error) = handler
                    .accept_transport_terminal(SseTransportTerminal::DoneMarker)
                {
                    yield terminal_error(&mut partial, &api_name, &model, error);
                    return;
                }
                match handler.finish(&mut partial, &model) {
                    Ok(events) => {
                        for event in events {
                            yield event;
                        }
                        yield terminal_event(partial, &api_name, &model);
                    }
                    Err(error) => yield terminal_error(&mut partial, &api_name, &model, error),
                }
                return;
            }

            match handler.handle_event(&sse_event.data, &mut partial, &model) {
                Ok(SseEventResult::Continue(events)) => {
                    for event in events {
                        yield event;
                    }
                }
                Ok(SseEventResult::ProviderDone(events)) => {
                    for event in events {
                        yield event;
                    }
                    match handler.finish(&mut partial, &model) {
                        Ok(events) => {
                            for event in events {
                                yield event;
                            }
                            yield terminal_event(partial, &api_name, &model);
                        }
                        Err(error) => {
                            yield terminal_error(&mut partial, &api_name, &model, error)
                        }
                    }
                    return;
                }
                Ok(SseEventResult::ProviderError {
                    events,
                    reason,
                    message,
                }) => {
                    for event in events {
                        yield event;
                    }
                    partial.stop_reason = reason.clone();
                    partial.error_message = Some(format!(
                        "{} provider {} model {} failed: {}",
                        api_name, model.provider, model.id, message
                    ));
                    yield AssistantMessageEvent::Error {
                        reason,
                        message: partial.clone(),
                    };
                    return;
                }
                Err(error) => {
                    yield terminal_error(&mut partial, &api_name, &model, error);
                    return;
                }
            }
        }

        if let Err(error) = handler.accept_transport_terminal(SseTransportTerminal::Eof) {
            yield terminal_error(&mut partial, &api_name, &model, error);
            return;
        }
        match handler.finish(&mut partial, &model) {
            Ok(events) => {
                for event in events {
                    yield event;
                }
                yield terminal_event(partial, &api_name, &model);
            }
            Err(error) => yield terminal_error(&mut partial, &api_name, &model, error),
        }
    })
}

fn terminal_event(
    mut message: AssistantMessage,
    api_name: &str,
    model: &Model,
) -> AssistantMessageEvent {
    match &message.stop_reason {
        StopReason::Stop | StopReason::Length | StopReason::ToolUse => {
            AssistantMessageEvent::Done {
                reason: message.stop_reason.clone(),
                message,
            }
        }
        StopReason::Error | StopReason::Aborted => {
            if message.error_message.is_none() {
                message.error_message = Some(format!(
                    "{} provider {} model {} ended with {:?}",
                    api_name, model.provider, model.id, message.stop_reason
                ));
            }
            AssistantMessageEvent::Error {
                reason: message.stop_reason.clone(),
                message,
            }
        }
    }
}

fn terminal_error(
    partial: &mut AssistantMessage,
    api_name: &str,
    model: &Model,
    error: impl std::fmt::Display,
) -> AssistantMessageEvent {
    partial.stop_reason = StopReason::Error;
    partial.error_message = Some(format!(
        "{} protocol error for provider {} model {}: {}",
        api_name, model.provider, model.id, error
    ));
    AssistantMessageEvent::Error {
        reason: StopReason::Error,
        message: partial.clone(),
    }
}

/// Normalize a tool-call id to match the `^[a-zA-Z0-9_-]{1,64}$` pattern.
/// If the id is already valid, return as-is. Otherwise sanitize and truncate.
/// When `replacement` is Some(c), invalid chars are replaced with `c`;
/// when None, invalid chars are removed.
pub fn normalize_tool_call_id(id: &str, replacement: Option<char>) -> String {
    let is_valid = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if is_valid {
        return id.to_string();
    }

    let sanitized: String = match replacement {
        Some(replacement) => id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    replacement
                }
            })
            .collect(),
        None => id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect(),
    };

    if sanitized.len() > 64 {
        sanitized[..64].to_string()
    } else if sanitized.is_empty() {
        "tool_0".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_id_passes_through() {
        assert_eq!(normalize_tool_call_id("toolu_01", None), "toolu_01");
        assert_eq!(normalize_tool_call_id("call-abc-123", None), "call-abc-123");
    }

    #[test]
    fn invalid_id_filtered() {
        let result = normalize_tool_call_id("tool*use!001", None);
        assert!(!result.contains('*'));
        assert!(!result.contains('!'));
    }

    #[test]
    fn invalid_id_replaced() {
        let result = normalize_tool_call_id("tool*use!001", Some('_'));
        assert_eq!(result, "tool_use_001");
    }

    #[test]
    fn empty_id_returns_placeholder() {
        assert_eq!(normalize_tool_call_id("!!!", None), "tool_0");
    }

    #[test]
    fn long_id_truncated() {
        let long = "a".repeat(100);
        let result = normalize_tool_call_id(&long, None);
        assert_eq!(result.len(), 64);
    }
}
