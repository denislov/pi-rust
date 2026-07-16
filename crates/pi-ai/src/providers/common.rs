use async_stream::stream;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use crate::protocol::{AssistantMessage, AssistantMessageEvent, StopReason};

use crate::model::Model;
use crate::protocol::stream::EventStream;
use crate::transport::sse::iterate_sse;

pub enum SseEventResult {
    Continue(Vec<AssistantMessageEvent>),
    Done(Vec<AssistantMessageEvent>),
}

pub trait SseEventHandler: Send + 'static {
    fn handle_event(
        &mut self,
        data: &str,
        partial: &mut AssistantMessage,
        model: &Model,
    ) -> Result<SseEventResult, String>;

    fn finalize(&self, partial: &mut AssistantMessage, model: &Model)
    -> Vec<AssistantMessageEvent>;
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
            if let Some(ref token) = cancel
                && token.is_cancelled() {
                    partial.stop_reason = StopReason::Aborted;
                    partial.error_message = Some("cancelled".into());
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Aborted,
                        message: partial.clone(),
                    };
                    return;
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

            match handler.handle_event(&sse_event.data, &mut partial, &model) {
                Ok(SseEventResult::Continue(events)) => {
                    for event in events {
                        yield event;
                    }
                }
                Ok(SseEventResult::Done(events)) => {
                    for event in events {
                        yield event;
                    }
                    return;
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

        for event in handler.finalize(&mut partial, &model) {
            yield event;
        }

        yield AssistantMessageEvent::Done {
            reason: partial.stop_reason.clone(),
            message: partial,
        };
    })
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
