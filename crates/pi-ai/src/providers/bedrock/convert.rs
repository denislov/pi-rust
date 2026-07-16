use super::wire;
use crate::model::Model;
use crate::protocol::{ContentBlock, Context, Message, StreamOptions, Tool};
use crate::providers::common::normalize_tool_call_id;

const EMPTY_TEXT_PLACEHOLDER: &str = "<empty>";

pub fn build_request(
    model: &Model,
    ctx: &Context,
    opts: &Option<StreamOptions>,
) -> wire::ConverseStreamRequest {
    let cache_retention = resolve_cache_retention(opts);
    wire::ConverseStreamRequest {
        model_id: model.id.clone(),
        messages: Some(convert_messages(&ctx.messages, model, cache_retention)),
        system: build_system_prompt(ctx.system_prompt.as_deref(), model, cache_retention),
        inference_config: Some(wire::InferenceConfig {
            max_tokens: opts
                .as_ref()
                .and_then(|o| o.max_tokens)
                .or(Some(model.max_tokens)),
            temperature: opts.as_ref().and_then(|o| o.temperature),
        }),
        tool_config: convert_tool_config(
            ctx.tools.as_deref(),
            opts.as_ref().and_then(|o| o.tool_choice.as_ref()),
        ),
        additional_model_request_fields: opts
            .as_ref()
            .and_then(|o| o.thinking.as_ref())
            .filter(|t| t.enabled)
            .map(|t| {
                serde_json::json!({
                    "thinking": {
                        "type": "enabled",
                        "budget_tokens": t.budget_tokens,
                    }
                })
            }),
    }
}

fn resolve_cache_retention(opts: &Option<StreamOptions>) -> CacheRetention {
    match opts.as_ref().and_then(|o| o.cache_retention.as_ref()) {
        Some(value) if value == "none" => CacheRetention::None,
        Some(value) if value == "long" => CacheRetention::Long,
        _ => CacheRetention::Short,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CacheRetention {
    None,
    Short,
    Long,
}

fn cache_point(retention: CacheRetention) -> wire::CachePoint {
    wire::CachePoint {
        cache_type: "DEFAULT".into(),
        ttl: (retention == CacheRetention::Long).then(|| "ONE_HOUR".into()),
    }
}

fn build_system_prompt(
    system_prompt: Option<&str>,
    model: &Model,
    retention: CacheRetention,
) -> Option<Vec<wire::SystemBlock>> {
    let system_prompt = system_prompt?;
    let mut blocks = vec![wire::SystemBlock {
        text: Some(system_prompt.to_string()),
        cache_point: None,
    }];
    if retention != CacheRetention::None && supports_prompt_caching(model) {
        blocks.push(wire::SystemBlock {
            text: None,
            cache_point: Some(cache_point(retention)),
        });
    }
    Some(blocks)
}

fn convert_messages(
    messages: &[Message],
    model: &Model,
    retention: CacheRetention,
) -> Vec<wire::BedrockMessage> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < messages.len() {
        match &messages[i] {
            Message::User { content } => result.push(wire::BedrockMessage {
                role: "user".into(),
                content: convert_content(content, model),
            }),
            Message::Assistant { content } => {
                let content = convert_assistant_content(content, model);
                if !content.is_empty() {
                    result.push(wire::BedrockMessage {
                        role: "assistant".into(),
                        content,
                    });
                }
            }
            Message::ToolResult { .. } => {
                let mut content = Vec::new();
                while i < messages.len() {
                    let Message::ToolResult {
                        tool_call_id,
                        is_error,
                        content: result_content,
                        ..
                    } = &messages[i]
                    else {
                        break;
                    };
                    content.push(wire::ContentBlock::ToolResult(wire::ToolResultBlock {
                        tool_use_id: normalize_tool_call_id(tool_call_id, Some('_')),
                        content: convert_tool_result_content(result_content, model),
                        status: if is_error.unwrap_or(false) {
                            "error".into()
                        } else {
                            "success".into()
                        },
                    }));
                    i += 1;
                }
                i -= 1;
                result.push(wire::BedrockMessage {
                    role: "user".into(),
                    content,
                });
            }
        }
        i += 1;
    }

    if retention != CacheRetention::None
        && supports_prompt_caching(model)
        && let Some(last) = result.last_mut()
        && last.role == "user"
    {
        last.content
            .push(wire::ContentBlock::CachePoint(cache_point(retention)));
    }

    result
}

fn convert_content(blocks: &[ContentBlock], model: &Model) -> Vec<wire::ContentBlock> {
    let mut converted = Vec::new();
    for block in blocks {
        match block {
            ContentBlock::Text { text, .. } if !text.trim().is_empty() => {
                converted.push(wire::ContentBlock::Text(text.clone()));
            }
            ContentBlock::Image { data, mime_type } => {
                converted.push(wire::ContentBlock::Image(wire::ImageBlock {
                    format: image_format(mime_type),
                    source: wire::ImageSource {
                        bytes: data.clone(),
                    },
                }));
            }
            ContentBlock::ToolCall {
                id,
                name,
                arguments,
                ..
            } => converted.push(wire::ContentBlock::ToolUse(wire::ToolUseBlock {
                tool_use_id: normalize_tool_call_id(id, Some('_')),
                name: name.clone(),
                input: arguments.clone(),
            })),
            ContentBlock::Thinking {
                thinking,
                thinking_signature,
                ..
            } if !thinking.trim().is_empty() => {
                if supports_thinking_signature(model) {
                    converted.push(wire::ContentBlock::ReasoningContent(
                        wire::ReasoningContentBlock {
                            reasoning_text: wire::ReasoningText {
                                text: thinking.clone(),
                                signature: thinking_signature.clone(),
                            },
                        },
                    ));
                } else {
                    converted.push(wire::ContentBlock::Text(thinking.clone()));
                }
            }
            _ => {}
        }
    }
    if converted.is_empty() {
        converted.push(wire::ContentBlock::Text(EMPTY_TEXT_PLACEHOLDER.into()));
    }
    converted
}

fn convert_assistant_content(blocks: &[ContentBlock], model: &Model) -> Vec<wire::ContentBlock> {
    convert_content(blocks, model)
}

fn convert_tool_result_content(
    blocks: &[ContentBlock],
    _model: &Model,
) -> Vec<wire::ToolResultContentBlock> {
    let mut converted = Vec::new();
    for block in blocks {
        match block {
            ContentBlock::Text { text, .. } if !text.trim().is_empty() => {
                converted.push(wire::ToolResultContentBlock::Text(text.clone()));
            }
            ContentBlock::Image { data, mime_type } => {
                converted.push(wire::ToolResultContentBlock::Image(wire::ImageBlock {
                    format: image_format(mime_type),
                    source: wire::ImageSource {
                        bytes: data.clone(),
                    },
                }));
            }
            _ => {}
        }
    }
    if converted.is_empty() {
        converted.push(wire::ToolResultContentBlock::Text(
            EMPTY_TEXT_PLACEHOLDER.into(),
        ));
    }
    converted
}

fn convert_tool_config(
    tools: Option<&[Tool]>,
    tool_choice: Option<&serde_json::Value>,
) -> Option<wire::ToolConfig> {
    let tools = tools?;
    if tools.is_empty() || tool_choice == Some(&serde_json::json!("none")) {
        return None;
    }
    let bedrock_tools = tools
        .iter()
        .map(|tool| wire::BedrockTool {
            tool_spec: wire::ToolSpec {
                name: tool.name.clone(),
                description: tool.description.clone(),
                input_schema: wire::InputSchema {
                    json: tool.parameters.clone(),
                },
            },
        })
        .collect();

    Some(wire::ToolConfig {
        tools: bedrock_tools,
        tool_choice: map_tool_choice(tool_choice),
    })
}

fn map_tool_choice(choice: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    match choice {
        Some(value) if value == "auto" => Some(serde_json::json!({"auto": {}})),
        Some(value) if value == "any" => Some(serde_json::json!({"any": {}})),
        Some(value) if value.get("type").and_then(|v| v.as_str()) == Some("tool") => {
            let name = value.get("name")?.as_str()?;
            Some(serde_json::json!({"tool": {"name": name}}))
        }
        _ => None,
    }
}

fn image_format(mime_type: &str) -> String {
    mime_type.strip_prefix("image/").unwrap_or(mime_type).into()
}

fn supports_prompt_caching(model: &Model) -> bool {
    let id = model.id.to_lowercase();
    let name = model.name.to_lowercase();
    id.contains("claude") || name.contains("claude")
}

fn supports_thinking_signature(model: &Model) -> bool {
    supports_prompt_caching(model)
}
