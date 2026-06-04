use super::wire;
use crate::types::{ContentBlock, Context, Message, Model, StreamOptions};

/// Normalize a tool-call id to match Anthropic's `^[a-zA-Z0-9_-]{1,64}$`.
/// If the id is already valid, return as-is. Otherwise sanitize and truncate.
pub fn normalize_tool_call_id(id: &str) -> String {
    let is_valid = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if is_valid {
        return id.to_string();
    }
    let sanitized: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    if sanitized.len() > 64 {
        sanitized[..64].to_string()
    } else if sanitized.is_empty() {
        "tool_0".to_string()
    } else {
        sanitized
    }
}

/// Map pi stop-reason string to our StopReason enum.
pub fn map_stop_reason(s: &str) -> crate::types::StopReason {
    match s {
        "end_turn" => crate::types::StopReason::Stop,
        "max_tokens" => crate::types::StopReason::Length,
        "tool_use" => crate::types::StopReason::ToolUse,
        _ => crate::types::StopReason::Error,
    }
}

/// Convert a Context to an Anthropic Request.
pub fn build_request(model: &Model, ctx: &Context, opts: &Option<StreamOptions>) -> wire::Request {
    let max_tokens = opts
        .as_ref()
        .and_then(|o| o.max_tokens)
        .or(Some(model.max_tokens))
        .unwrap_or(4096);

    let system = ctx.system_prompt.as_ref().map(|sp| {
        vec![wire::SystemBlock {
            block_type: "text".into(),
            text: sp.clone(),
            cache_control: Some(wire::CacheControl {
                cache_type: "ephemeral".into(),
            }),
        }]
    });

    let messages = convert_messages(&ctx.messages);

    let tools = ctx.tools.as_ref().map(|tools| {
        tools
            .iter()
            .map(|t| wire::ToolDef {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.parameters.clone(),
            })
            .collect()
    });

    let temperature = opts.as_ref().and_then(|o| o.temperature);

    let thinking = opts.as_ref().and_then(|o| {
        o.thinking
            .as_ref()
            .filter(|t| t.enabled)
            .map(|t| wire::ThinkingConfig {
                think_type: if t.budget_tokens.is_some() {
                    "enabled".into()
                } else {
                    "auto".into()
                },
                budget_tokens: t.budget_tokens,
            })
    });

    let tool_choice = opts.as_ref().and_then(|o| o.tool_choice.clone());

    wire::Request {
        model: model.id.clone(),
        max_tokens,
        messages,
        system,
        tools,
        temperature,
        thinking,
        tool_choice,
        stream: true,
    }
}

/// Convert pi Messages to Anthropic request messages.
/// Handles consecutive ToolResult coalescing into a single user turn.
fn convert_messages(messages: &[Message]) -> Vec<wire::RequestMessage> {
    let mut result: Vec<wire::RequestMessage> = Vec::new();

    for msg in messages {
        match msg {
            Message::User { content } => {
                result.push(wire::RequestMessage {
                    role: "user".into(),
                    content: convert_content(content),
                });
            }
            Message::Assistant { content } => {
                result.push(wire::RequestMessage {
                    role: "assistant".into(),
                    content: convert_content(content),
                });
            }
            Message::ToolResult {
                tool_call_id,
                content,
                ..
            } => {
                let tool_content = serde_json::json!({
                    "type": "tool_result",
                    "tool_use_id": normalize_tool_call_id(tool_call_id),
                    "content": convert_content(content),
                });

                // Coalesce: if the last message is also a user-role, append
                // the tool_result to its content array; otherwise push a new user message.
                if let Some(last) = result.last_mut() {
                    if last.role == "user" {
                        if let Some(arr) = last.content.as_array_mut() {
                            arr.push(tool_content);
                            continue;
                        }
                    }
                }
                result.push(wire::RequestMessage {
                    role: "user".into(),
                    content: serde_json::json!([tool_content]),
                });
            }
        }
    }

    result
}

/// Convert pi ContentBlocks to Anthropic-compatible JSON array.
fn convert_content(blocks: &[ContentBlock]) -> serde_json::Value {
    let items: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| match b {
            ContentBlock::Text { text, .. } => {
                serde_json::json!({ "type": "text", "text": text })
            }
            ContentBlock::Thinking { thinking, .. } => {
                serde_json::json!({ "type": "thinking", "thinking": thinking })
            }
            ContentBlock::Image { data, mime_type } => {
                serde_json::json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": mime_type,
                        "data": data,
                    }
                })
            }
            ContentBlock::ToolCall {
                id,
                name,
                arguments,
                ..
            } => {
                serde_json::json!({
                    "type": "tool_use",
                    "id": normalize_tool_call_id(id),
                    "name": name,
                    "input": arguments,
                })
            }
        })
        .collect();
    serde_json::Value::Array(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ModelCost, ModelInput};

    #[test]
    fn normalize_valid_id_passes_through() {
        assert_eq!(normalize_tool_call_id("toolu_01"), "toolu_01");
        assert_eq!(normalize_tool_call_id("call-abc-123"), "call-abc-123");
    }

    #[test]
    fn normalize_invalid_id_sanitized() {
        let result = normalize_tool_call_id("tool*use!001");
        assert!(!result.contains('*'));
        assert!(!result.contains('!'));
    }

    #[test]
    fn map_stop_reason_end_turn() {
        assert_eq!(map_stop_reason("end_turn"), crate::types::StopReason::Stop);
    }

    #[test]
    fn map_stop_reason_tool_use() {
        assert_eq!(
            map_stop_reason("tool_use"),
            crate::types::StopReason::ToolUse
        );
    }

    #[test]
    fn map_stop_reason_max_tokens() {
        assert_eq!(
            map_stop_reason("max_tokens"),
            crate::types::StopReason::Length
        );
    }

    #[test]
    fn map_stop_reason_unknown() {
        assert_eq!(
            map_stop_reason("weird_reason"),
            crate::types::StopReason::Error
        );
    }

    #[test]
    fn build_basic_request() {
        let model = Model {
            id: "claude-haiku-4-5".into(),
            name: "Haiku".into(),
            api: "anthropic-messages".into(),
            provider: "anthropic".into(),
            base_url: "https://api.anthropic.com".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost { input: 1.0, output: 5.0, cache_read: 0.0, cache_write: 0.0 },
            context_window: 200000,
            max_tokens: 8192,
            headers: None,
            compat: None,
        };
        let ctx = Context {
            system_prompt: Some("Be helpful.".into()),
            messages: vec![Message::User {
                content: vec![ContentBlock::Text {
                    text: "Hello".into(),
                    text_signature: None,
                }],
            }],
            tools: None,
        };
        let req = build_request(&model, &ctx, &None);
        assert_eq!(req.model, "claude-haiku-4-5");
        assert_eq!(req.max_tokens, 8192);
        assert!(req.stream);
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, "user");
        assert!(req.system.is_some());
    }

    #[test]
    fn tool_result_coalescing() {
        let messages = vec![
            Message::ToolResult {
                tool_call_id: "call_1".into(),
                tool_name: None,
                is_error: None,
                content: vec![ContentBlock::Text {
                    text: "result1".into(),
                    text_signature: None,
                }],
            },
            Message::ToolResult {
                tool_call_id: "call_2".into(),
                tool_name: None,
                is_error: None,
                content: vec![ContentBlock::Text {
                    text: "result2".into(),
                    text_signature: None,
                }],
            },
        ];
        let converted = convert_messages(&messages);
        assert_eq!(converted.len(), 1); // coalesced into one user message
        assert_eq!(converted[0].role, "user");
        let content = converted[0].content.as_array().unwrap();
        assert_eq!(content.len(), 2);
    }
}
