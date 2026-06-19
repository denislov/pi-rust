use crate::types::{AssistantMessage, AssistantMessageEvent};
use futures::{Stream, StreamExt};
use std::pin::Pin;

pub type EventStream = Pin<Box<dyn Stream<Item = AssistantMessageEvent> + Send>>;

pub async fn complete(mut stream: EventStream) -> Result<AssistantMessage, String> {
    while let Some(event) = stream.next().await {
        match event {
            AssistantMessageEvent::Done { message, .. } => return Ok(message),
            AssistantMessageEvent::Error { message, .. } => {
                return Err(message.error_message.unwrap_or_default());
            }
            _ => continue,
        }
    }
    Err("stream ended without Done event".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ContentBlock, StopReason, Usage};
    use futures::stream;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_event_stream(events: Vec<AssistantMessageEvent>) -> EventStream {
        Box::pin(stream::iter(events))
    }

    fn dummy_message() -> AssistantMessage {
        AssistantMessage {
            content: vec![ContentBlock::Text {
                text: "ok".into(),
                text_signature: None,
            }],
            api: "test".into(),
            provider: None,
            model: "test".into(),
            response_model: None,
            response_id: None,
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            error_message: None,
            diagnostics: None,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    #[tokio::test]
    async fn complete_returns_done_message() {
        let msg = dummy_message();
        let stream = make_event_stream(vec![
            AssistantMessageEvent::Start {
                content_index: None,
                partial: msg.clone(),
            },
            AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message: msg.clone(),
            },
        ]);
        let result = complete(stream).await.unwrap();
        assert_eq!(result, msg);
    }

    #[tokio::test]
    async fn complete_returns_error() {
        let mut err_msg = AssistantMessage::empty("test", "test");
        err_msg.error_message = Some("fail".into());
        err_msg.stop_reason = StopReason::Error;
        let stream = make_event_stream(vec![AssistantMessageEvent::Error {
            reason: StopReason::Error,
            message: err_msg,
        }]);
        let result = complete(stream).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "fail");
    }

    #[tokio::test]
    async fn complete_empty_stream_errors() {
        let stream = make_event_stream(vec![]);
        assert!(complete(stream).await.is_err());
    }
}
