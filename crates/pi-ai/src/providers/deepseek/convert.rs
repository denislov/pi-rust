use super::wire::{ChatCompletionRequest, ChatMessage};
use crate::types::{ContentBlock, Context, Message, Model, StreamOptions};

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

    ChatCompletionRequest {
        model: model.id.clone(),
        messages,
        max_tokens: opts
            .as_ref()
            .and_then(|opts| opts.max_tokens)
            .or(model.max_tokens),
        temperature: opts.as_ref().and_then(|opts| opts.temperature),
        stream: false,
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
