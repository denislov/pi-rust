use super::wire::{ChatCompletionRequest, ChatMessage};
use crate::model::Model;
use crate::protocol::{ContentBlock, Context, Message, StreamOptions};
use serde_json::json;

pub fn build_request(
    model: &Model,
    ctx: &Context,
    opts: &Option<StreamOptions>,
) -> ChatCompletionRequest {
    let mut messages = Vec::new();
    if let Some(system_prompt) = &ctx.system_prompt {
        messages.push(ChatMessage {
            role: "system".into(),
            content: system_prompt.clone(),
        });
    }

    messages.extend(ctx.messages.iter().filter_map(convert_message));

    let (thinking, reasoning_effort) =
        opts.as_ref()
            .and_then(|o| o.thinking.as_ref())
            .map_or((None, None), |tc| {
                if tc.enabled {
                    // Map pi thinking level to provider value via thinkingLevelMap
                    let effort = tc.effort.as_deref().and_then(|level| {
                        model
                            .thinking_level_map
                            .as_ref()
                            .and_then(|map| map.resolve(level))
                            .or(Some(level.to_string()))
                    });
                    (Some(json!({ "type": "enabled" })), effort)
                } else {
                    (None, None)
                }
            });

    ChatCompletionRequest {
        model: model.id.clone(),
        messages,
        max_tokens: opts
            .as_ref()
            .and_then(|opts| opts.max_tokens)
            .or(Some(model.max_tokens)),
        temperature: opts.as_ref().and_then(|opts| opts.temperature),
        stream: false,
        thinking,
        reasoning_effort,
    }
}

fn convert_message(message: &Message) -> Option<ChatMessage> {
    match message {
        Message::User { content } => Some(ChatMessage {
            role: "user".into(),
            content: content_to_text(content),
        }),
        Message::Assistant { content } => Some(ChatMessage {
            role: "assistant".into(),
            content: content_to_text(content),
        }),
        Message::ToolResult { content, .. } => Some(ChatMessage {
            role: "user".into(),
            content: content_to_text(content),
        }),
    }
    .filter(|message| !message.content.is_empty())
}

fn content_to_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
