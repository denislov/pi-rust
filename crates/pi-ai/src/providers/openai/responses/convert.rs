use super::wire;
use crate::types::{ContentBlock, Context, Message, Model, StreamOptions};

pub fn build_request(
    model: &Model,
    ctx: &Context,
    opts: &Option<StreamOptions>,
) -> wire::ResponseCreateRequest {
    let tools = ctx.tools.as_ref().map(|tools| {
        tools
            .iter()
            .map(|t| wire::ResponseTool {
                tool_type: "function".to_string(),
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.parameters.clone(),
            })
            .collect()
    });

    let max_tokens = opts.as_ref().and_then(|o| o.max_tokens);

    let temperature = opts.as_ref().and_then(|o| o.temperature);

    wire::ResponseCreateRequest {
        model: model.id.clone(),
        instructions: ctx.system_prompt.clone(),
        input: ctx.messages.iter().map(convert_message).collect(),
        tools,
        max_output_tokens: max_tokens,
        temperature,
        tool_choice: opts.as_ref().and_then(|o| o.tool_choice.clone()),
        stream: true,
    }
}

fn convert_message(msg: &Message) -> wire::ResponseInputItem {
    match msg {
        Message::User { content } => wire::ResponseInputItem::Message {
            role: "user".to_string(),
            content: serde_json::json!(
                content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text, .. } => Some(serde_json::json!({
                            "type": "input_text",
                            "text": text,
                        })),
                        ContentBlock::Image { data, mime_type } => Some(serde_json::json!({
                            "type": "input_image",
                            "image_url": format!("data:{};base64,{}", mime_type, data),
                        })),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            ),
        },
        Message::Assistant { content } => {
            let mut items: Vec<serde_json::Value> = Vec::new();
            for b in content {
                match b {
                    ContentBlock::Text { text, .. } => {
                        items.push(serde_json::json!({
                            "type": "output_text",
                            "text": text,
                        }));
                    }
                    ContentBlock::ToolCall {
                        id,
                        name,
                        arguments,
                        ..
                    } => {
                        items.push(serde_json::json!({
                            "type": "function_call",
                            "id": id,
                            "name": name,
                            "arguments": arguments.to_string(),
                        }));
                    }
                    _ => {}
                }
            }
            wire::ResponseInputItem::Message {
                role: "assistant".to_string(),
                content: serde_json::Value::Array(items),
            }
        }
        Message::ToolResult {
            tool_call_id,
            content,
            ..
        } => {
            let output = content_to_text(content);
            wire::ResponseInputItem::FunctionCallOutput {
                call_id: tool_call_id.clone(),
                output,
            }
        }
    }
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
