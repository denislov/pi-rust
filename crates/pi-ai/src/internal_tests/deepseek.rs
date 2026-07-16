use super::support;

use pi_ai::model::lookup_model;
use pi_ai::protocol::{
    AssistantMessageEvent, ContentBlock, Context, Message, StopReason, StreamOptions,
};
use pi_ai::providers::deepseek::convert::build_request;
use pi_ai::providers::deepseek::stream::response_to_events;
use pi_ai::providers::deepseek::wire::ChatCompletionResponse;
use pi_ai::registry::env::env_api_key;
use support::EnvGuard;

#[test]
fn deepseek_model_is_available() {
    let model = lookup_model("deepseek-v4-flash").unwrap();

    assert_eq!(model.provider, "deepseek");
    assert_eq!(model.api, "openai-completions");
    assert_eq!(model.base_url, "https://api.deepseek.com");
}

#[test]
fn deepseek_api_key_uses_deepseek_env_var() {
    let env = EnvGuard::new(&["DEEPSEEK_API_KEY"]);
    env.set("DEEPSEEK_API_KEY", "sk-deepseek-test");

    assert_eq!(
        env_api_key("deepseek"),
        Some("sk-deepseek-test".to_string())
    );
}

#[test]
fn build_request_maps_system_and_user_text() {
    let model = lookup_model("deepseek-v4-flash").unwrap();
    let ctx = Context {
        system_prompt: Some("Be brief.".into()),
        messages: vec![Message::User {
            content: vec![ContentBlock::Text {
                text: "Say hi".into(),
                text_signature: None,
            }],
        }],
        tools: None,
    };
    let opts = Some(StreamOptions {
        max_tokens: Some(128),
        temperature: Some(0.2),
        ..Default::default()
    });

    let request = build_request(&model, &ctx, &opts);

    assert_eq!(request.model, "deepseek-v4-flash");
    assert!(!request.stream);
    assert_eq!(request.max_tokens, Some(128));
    assert_eq!(request.temperature, Some(0.2));
    assert_eq!(request.messages.len(), 2);
    assert_eq!(request.messages[0].role, "system");
    assert_eq!(request.messages[0].content, "Be brief.");
    assert_eq!(request.messages[1].role, "user");
    assert_eq!(request.messages[1].content, "Say hi");
}

#[test]
fn response_to_events_maps_text_done_and_usage() {
    let model = lookup_model("deepseek-v4-flash").unwrap();
    let response: ChatCompletionResponse = serde_json::from_value(serde_json::json!({
        "id": "chatcmpl-test",
        "created": 1710000000,
        "model": "deepseek-v4-flash",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "ok"
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 3,
            "completion_tokens": 1,
            "total_tokens": 4
        }
    }))
    .unwrap();

    let events = response_to_events(response, &model);

    assert!(matches!(events[0], AssistantMessageEvent::Start { .. }));
    assert!(events.iter().any(|event| matches!(
        event,
        AssistantMessageEvent::TextDelta { delta, .. } if delta == "ok"
    )));

    let last = events.last().unwrap();
    match last {
        AssistantMessageEvent::Done { reason, message } => {
            assert_eq!(*reason, StopReason::Stop);
            assert_eq!(message.response_id.as_deref(), Some("chatcmpl-test"));
            assert_eq!(message.provider.as_deref(), Some("deepseek"));
            assert_eq!(message.usage.input, 3);
            assert_eq!(message.usage.output, 1);
            assert_eq!(message.usage.total_tokens, 4);
        }
        other => panic!("expected Done, got {other:?}"),
    }
}

#[test]
fn response_to_events_maps_length_finish_reason() {
    let model = lookup_model("deepseek-v4-flash").unwrap();
    let response: ChatCompletionResponse = serde_json::from_value(serde_json::json!({
        "id": "chatcmpl-length",
        "created": 1710000000,
        "model": "deepseek-v4-flash",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "partial"
            },
            "finish_reason": "length"
        }],
        "usage": {
            "prompt_tokens": 3,
            "completion_tokens": 1,
            "total_tokens": 4
        }
    }))
    .unwrap();

    let events = response_to_events(response, &model);

    match events.last().unwrap() {
        AssistantMessageEvent::Done { reason, message } => {
            assert_eq!(*reason, StopReason::Length);
            assert_eq!(message.stop_reason, StopReason::Length);
        }
        other => panic!("expected Done, got {other:?}"),
    }
}
// Internal DeepSeek provider tests.
