use super::wire::ChatCompletionResponse;
use crate::model::Model;
use crate::model::calculate_cost;
use crate::protocol::stream::EventStream;
use crate::protocol::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Cost, StopReason, Usage,
};
use async_stream::stream;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

pub fn process<E>(
    mut body: impl Stream<Item = Result<Bytes, E>> + Send + Unpin + 'static,
    model: Model,
    cancel: Option<CancellationToken>,
) -> EventStream
where
    E: std::fmt::Display + Send + 'static,
{
    Box::pin(stream! {
        let mut bytes = Vec::new();
        loop {
            let next = match cancel.as_ref() {
                Some(token) => tokio::select! {
                    biased;
                    _ = token.cancelled() => {
                        let mut message = AssistantMessage::empty("deepseek-chat-completions", &model.id);
                        message.provider = Some(model.provider.clone());
                        message.stop_reason = StopReason::Aborted;
                        message.error_message = Some("DeepSeek response body read cancelled".into());
                        yield AssistantMessageEvent::Error {
                            reason: StopReason::Aborted,
                            message,
                        };
                        return;
                    }
                    item = body.next() => item,
                },
                None => body.next().await,
            };
            match next {
                Some(Ok(chunk)) => bytes.extend_from_slice(&chunk),
                Some(Err(error)) => {
                    let mut message = AssistantMessage::empty("deepseek-chat-completions", &model.id);
                    message.provider = Some(model.provider.clone());
                    message.stop_reason = StopReason::Error;
                    message.error_message = Some(format!("DeepSeek response read error: {error}"));
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        message,
                    };
                    return;
                }
                None => break,
            }
        }
        let response: ChatCompletionResponse = match serde_json::from_slice(&bytes) {
            Ok(response) => response,
            Err(error) => {
                let mut message = AssistantMessage::empty("deepseek-chat-completions", &model.id);
                message.provider = Some(model.provider.clone());
                message.stop_reason = StopReason::Error;
                message.error_message = Some(format!("DeepSeek response parse error: {error}"));
                yield AssistantMessageEvent::Error {
                    reason: StopReason::Error,
                    message,
                };
                return;
            }
        };
        for event in response_to_events(response, &model) {
            yield event;
        }
    })
}

pub fn response_to_events(
    response: ChatCompletionResponse,
    model: &Model,
) -> Vec<AssistantMessageEvent> {
    let mut partial = AssistantMessage::empty("deepseek-chat-completions", &model.id);
    partial.provider = Some("deepseek".into());
    partial.response_id = Some(response.id);
    partial.response_model = Some(response.model);
    partial.timestamp = response.created;

    let mut events = vec![AssistantMessageEvent::Start {
        content_index: None,
        partial: partial.clone(),
    }];

    let Some(choice) = response.choices.into_iter().next() else {
        partial.stop_reason = StopReason::Error;
        partial.error_message = Some("DeepSeek response did not contain a choice".into());
        events.push(AssistantMessageEvent::Error {
            reason: StopReason::Error,
            message: partial,
        });
        return events;
    };

    if let Some(reasoning) = choice.message.reasoning_content
        && !reasoning.is_empty()
    {
        partial.content.push(ContentBlock::Thinking {
            thinking: reasoning.clone(),
            thinking_signature: None,
            redacted: None,
        });
        events.push(AssistantMessageEvent::ThinkingStart {
            content_index: 0,
            partial: partial.clone(),
        });
        events.push(AssistantMessageEvent::ThinkingDelta {
            content_index: 0,
            delta: reasoning,
            partial: partial.clone(),
        });
        events.push(AssistantMessageEvent::ThinkingEnd {
            content_index: 0,
            partial: partial.clone(),
        });
    }

    if let Some(text) = choice.message.content
        && !text.is_empty()
    {
        let content_index = partial.content.len() as u32;
        partial.content.push(ContentBlock::Text {
            text: text.clone(),
            text_signature: None,
        });
        events.push(AssistantMessageEvent::TextStart {
            content_index,
            partial: partial.clone(),
        });
        events.push(AssistantMessageEvent::TextDelta {
            content_index,
            delta: text,
            partial: partial.clone(),
        });
        events.push(AssistantMessageEvent::TextEnd {
            content_index,
            partial: partial.clone(),
        });
    }

    partial.stop_reason = map_finish_reason(choice.finish_reason.as_deref());
    partial.usage = response_usage(response.usage, model);

    if matches!(
        partial.stop_reason,
        StopReason::Stop | StopReason::Length | StopReason::ToolUse
    ) {
        events.push(AssistantMessageEvent::Done {
            reason: partial.stop_reason.clone(),
            message: partial,
        });
    } else {
        partial.error_message = Some(format!(
            "DeepSeek returned unsupported finish reason {:?}",
            choice.finish_reason
        ));
        events.push(AssistantMessageEvent::Error {
            reason: StopReason::Error,
            message: partial,
        });
    }
    events
}

fn map_finish_reason(reason: Option<&str>) -> StopReason {
    match reason {
        Some("stop") | None => StopReason::Stop,
        Some("length") => StopReason::Length,
        Some("tool_calls") => StopReason::ToolUse,
        _ => StopReason::Error,
    }
}

fn response_usage(usage: super::wire::ChatUsage, model: &Model) -> Usage {
    let mut result = Usage {
        input: usage.prompt_tokens,
        output: usage.completion_tokens,
        cache_read: usage.prompt_cache_hit_tokens.unwrap_or(0),
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
