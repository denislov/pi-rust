use pi_ai::types::*;

fn test_model() -> Model {
    Model {
        id: "claude-haiku-4-5".into(), name: "Haiku".into(),
        api: "anthropic-messages".into(), provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(), reasoning: false,
        input: 1.0, output: 5.0, cache_read: None, cache_write: None,
        context_window: 200000, max_tokens: Some(8192), headers: None,
    }
}

#[test]
fn basic_request_has_system_prompt_with_cache_control() {
    let ctx = Context {
        system_prompt: Some("Be concise.".into()),
        messages: vec![Message::User {
            content: vec![ContentBlock::Text { text: "hi".into(), text_signature: None }],
        }],
        tools: None,
    };
    let req = pi_ai::providers::anthropic::convert::build_request(&test_model(), &ctx, &None);
    let json = serde_json::to_value(&req).unwrap();
    let system = json["system"].as_array().unwrap();
    assert_eq!(system.len(), 1);
    assert_eq!(system[0]["text"], "Be concise.");
    assert_eq!(system[0]["cache_control"]["type"], "ephemeral");
}

#[test]
fn tool_result_coalescing_multiple_results() {
    let msgs = vec![
        Message::ToolResult {
            tool_call_id: "a".into(),
            content: vec![ContentBlock::Text { text: "r1".into(), text_signature: None }],
        },
        Message::ToolResult {
            tool_call_id: "b".into(),
            content: vec![ContentBlock::Text { text: "r2".into(), text_signature: None }],
        },
    ];
    let ctx = Context { system_prompt: None, messages: msgs, tools: None };
    let req = pi_ai::providers::anthropic::convert::build_request(&test_model(), &ctx, &None);
    assert_eq!(req.messages.len(), 1);
    assert_eq!(req.messages[0].role, "user");
}

#[test]
fn image_block_converts_to_anthropic_format() {
    let ctx = Context {
        system_prompt: None,
        messages: vec![Message::User {
            content: vec![ContentBlock::Image {
                data: "base64data".into(),
                mime_type: "image/png".into(),
            }],
        }],
        tools: None,
    };
    let req = pi_ai::providers::anthropic::convert::build_request(&test_model(), &ctx, &None);
    let json = serde_json::to_value(req.messages[0].content.clone()).unwrap();
    let first = &json.as_array().unwrap()[0];
    assert_eq!(first["type"], "image");
    assert_eq!(first["source"]["type"], "base64");
    assert_eq!(first["source"]["media_type"], "image/png");
}

#[test]
fn max_tokens_falls_back_to_model_default() {
    let ctx = Context { system_prompt: None, messages: vec![], tools: None };
    let req = pi_ai::providers::anthropic::convert::build_request(&test_model(), &ctx, &None);
    assert_eq!(req.max_tokens, 8192);
}

#[test]
fn tool_def_converts_parameters_to_input_schema() {
    let ctx = Context {
        system_prompt: None,
        messages: vec![],
        tools: Some(vec![Tool {
            name: "search".into(),
            description: Some("search the web".into()),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        }]),
    };
    let req = pi_ai::providers::anthropic::convert::build_request(&test_model(), &ctx, &None);
    let json = serde_json::to_value(&req).unwrap();
    let tools = json["tools"].as_array().unwrap();
    assert_eq!(tools[0]["name"], "search");
    assert!(tools[0].as_object().unwrap().contains_key("input_schema"));
}
