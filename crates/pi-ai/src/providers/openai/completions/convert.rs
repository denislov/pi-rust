use super::wire;
use crate::providers::openai::common::{CompatFlags, resolve_completions_compat};
use crate::types::{ContentBlock, Context, Message, Model, StreamOptions};

pub fn build_request(
    model: &Model,
    ctx: &Context,
    opts: &Option<StreamOptions>,
) -> wire::ChatCompletionRequest {
    let compat = resolve_completions_compat(model);
    let mut messages = Vec::new();

    if let Some(sp) = &ctx.system_prompt {
        let role = if compat.supports_developer_role && model.reasoning {
            "developer"
        } else {
            "system"
        };
        messages.push(wire::ChatMessage {
            role: role.to_string(),
            content: wire::ChatContent::Text(sp.clone()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        });
    }

    messages.extend(ctx.messages.iter().map(|m| convert_message(m, &compat)));

    let mut max_tokens: Option<u32> = None;
    let mut max_completion_tokens: Option<u32> = None;
    let tokens = opts
        .as_ref()
        .and_then(|o| o.max_tokens)
        .unwrap_or(model.max_tokens);
    match compat.max_tokens_field.as_str() {
        "max_tokens" => max_tokens = Some(tokens),
        _ => max_completion_tokens = Some(tokens),
    }

    let temperature = opts.as_ref().and_then(|o| o.temperature);
    let tool_choice = opts.as_ref().and_then(|o| o.tool_choice.clone());

    let tools = ctx.tools.as_ref().map(|tools| {
        tools
            .iter()
            .map(|t| wire::ChatTool {
                tool_type: "function".to_string(),
                function: wire::FunctionDef {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.parameters.clone(),
                },
            })
            .collect()
    });

    let stream_options = if compat.supports_usage_in_streaming {
        Some(wire::StreamOptions {
            include_usage: true,
        })
    } else {
        None
    };

    wire::ChatCompletionRequest {
        model: model.id.clone(),
        messages,
        max_tokens,
        max_completion_tokens,
        temperature,
        tools,
        tool_choice,
        stream: true,
        stream_options,
    }
}

fn convert_message(msg: &Message, _compat: &CompatFlags) -> wire::ChatMessage {
    match msg {
        Message::User { content } => wire::ChatMessage {
            role: "user".to_string(),
            content: convert_user_content(content),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        Message::Assistant { content } => {
            let mut text_parts = Vec::new();
            let mut tool_calls = Vec::new();
            for block in content {
                match block {
                    ContentBlock::Text { text, .. } => {
                        text_parts.push(wire::ChatContentPart::Text { text: text.clone() });
                    }
                    ContentBlock::ToolCall {
                        id,
                        name,
                        arguments,
                        ..
                    } => {
                        tool_calls.push(wire::ToolCall {
                            id: id.clone(),
                            tool_type: "function".to_string(),
                            function: wire::ToolCallFunction {
                                name: name.clone(),
                                arguments: arguments.to_string(),
                            },
                        });
                    }
                    _ => {}
                }
            }
            let content = if text_parts.is_empty() && !tool_calls.is_empty() {
                wire::ChatContent::Text(String::new())
            } else {
                wire::ChatContent::Parts(text_parts)
            };
            wire::ChatMessage {
                role: "assistant".to_string(),
                content,
                name: None,
                tool_calls: if tool_calls.is_empty() {
                    None
                } else {
                    Some(tool_calls)
                },
                tool_call_id: None,
            }
        }
        Message::ToolResult {
            tool_call_id,
            content,
            ..
        } => {
            let text = content_to_text(content);
            wire::ChatMessage {
                role: "tool".to_string(),
                content: wire::ChatContent::Text(text),
                name: None,
                tool_calls: None,
                tool_call_id: Some(tool_call_id.clone()),
            }
        }
    }
}

fn convert_user_content(content: &[ContentBlock]) -> wire::ChatContent {
    let has_image = content
        .iter()
        .any(|b| matches!(b, ContentBlock::Image { .. }));
    if !has_image {
        let text = content_to_text(content);
        return wire::ChatContent::Text(text);
    }

    let parts: Vec<wire::ChatContentPart> = content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text, .. } => {
                Some(wire::ChatContentPart::Text { text: text.clone() })
            }
            ContentBlock::Image { data, mime_type } => {
                let data_url = format!("data:{};base64,{}", mime_type, data);
                Some(wire::ChatContentPart::ImageUrl {
                    image_url: wire::ImageUrl { url: data_url },
                })
            }
            _ => None,
        })
        .collect();

    wire::ChatContent::Parts(parts)
}

fn content_to_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
