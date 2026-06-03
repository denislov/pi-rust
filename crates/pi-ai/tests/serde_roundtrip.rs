use pi_ai::types::*;
use serde_json;

#[test]
fn assistant_message_roundtrip() {
    let msg = AssistantMessage {
        content: vec![ContentBlock::Text { text: "hello".into(), text_signature: Some("sig".into()) }],
        api: "anthropic-messages".into(),
        provider: Some("anthropic".into()),
        model: "claude-sonnet-4-5".into(),
        response_model: Some("claude-sonnet-4-5-20250219".into()),
        response_id: Some("msg_001".into()),
        usage: Usage {
            input: 100, output: 200, cache_read: 50, cache_write: 10,
            total_tokens: 300,
            cost: Cost { input: 0.0003, output: 0.003, cache_read: 0.0, cache_write: 0.0 },
        },
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: 1717000000,
    };
    let json = serde_json::to_string_pretty(&msg).unwrap();
    let back: AssistantMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(back.content, msg.content);
    assert_eq!(back.api, msg.api);
    assert_eq!(back.model, msg.model);
    assert_eq!(back.stop_reason, msg.stop_reason);
    assert_eq!(back.usage.input, msg.usage.input);
    assert_eq!(back.timestamp, msg.timestamp);
}

#[test]
fn event_stream_all_variants_serialize() {
    let msg = AssistantMessage::empty("test", "test-model");

    let events = vec![
        AssistantMessageEvent::Start { partial: msg.clone() },
        AssistantMessageEvent::TextStart { partial: msg.clone() },
        AssistantMessageEvent::TextDelta { delta: "hi".into(), partial: msg.clone() },
        AssistantMessageEvent::TextEnd { partial: msg.clone() },
        AssistantMessageEvent::ThinkingStart { partial: msg.clone() },
        AssistantMessageEvent::ThinkingDelta { delta: "hmm".into(), partial: msg.clone() },
        AssistantMessageEvent::ThinkingEnd { partial: msg.clone() },
        AssistantMessageEvent::ToolcallStart { partial: msg.clone() },
        AssistantMessageEvent::ToolcallDelta { delta: "{}".into(), partial: msg.clone() },
        AssistantMessageEvent::ToolcallEnd { partial: msg.clone() },
        AssistantMessageEvent::Done { reason: StopReason::Stop, message: msg.clone() },
        AssistantMessageEvent::Error { reason: StopReason::Error, error: "oops".into() },
    ];

    for event in &events {
        let json = serde_json::to_string(event).unwrap();
        assert!(json.contains(r#""type""#), "event missing type field: {:?}", json);
    }
}

#[test]
fn context_serialization_matches_pi_format() {
    let ctx = Context {
        system_prompt: Some("Be helpful.".into()),
        messages: vec![
            Message::User { content: vec![ContentBlock::Text { text: "hi".into(), text_signature: None }] },
            Message::Assistant { content: vec![ContentBlock::Text { text: "hello!".into(), text_signature: None }] },
        ],
        tools: None,
    };
    let json = serde_json::to_string(&ctx).unwrap();
    assert!(json.contains(r#""systemPrompt""#));
    assert!(json.contains(r#""role":"user""#));
    assert!(json.contains(r#""role":"assistant""#));
    assert!(json.contains(r#""type":"text""#));
}

#[test]
fn content_block_all_variants_roundtrip() {
    let blocks = vec![
        ContentBlock::Text { text: "hi".into(), text_signature: None },
        ContentBlock::Thinking { thinking: "hmm".into(), thinking_signature: None, redacted: Some(false) },
        ContentBlock::Image { data: "base64data".into(), mime_type: "image/png".into() },
        ContentBlock::ToolCall { id: "t1".into(), name: "f".into(),
            arguments: serde_json::json!({"x": 1}), thought_signature: None },
    ];
    for block in &blocks {
        let json = serde_json::to_string(block).unwrap();
        let back: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(back, *block);
    }
}
