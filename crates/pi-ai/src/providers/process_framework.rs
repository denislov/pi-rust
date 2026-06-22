use async_stream::stream;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use crate::stream::EventStream;
use crate::types::{AssistantMessage, AssistantMessageEvent, Model, StopReason};
use crate::util::sse::iterate_sse;

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
