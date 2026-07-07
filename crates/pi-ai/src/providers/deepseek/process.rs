use super::wire::ChatCompletionResponse;
use crate::models::calculate_cost;
use crate::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Cost, Model, StopReason, Usage,
};

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

    events.push(AssistantMessageEvent::Done {
        reason: partial.stop_reason.clone(),
        message: partial,
    });
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
