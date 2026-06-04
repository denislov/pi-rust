use futures::StreamExt;
use pi_ai::providers::faux::{FauxCall, FauxProvider, FauxResponse, FauxToolCall};
use pi_ai::registry;
use pi_ai::stream::complete;
use pi_ai::types::*;
use std::sync::Arc;

fn faux_model(api: &str) -> Model {
    Model {
        id: "faux-model".into(),
        name: "Faux Model".into(),
        api: api.into(),
        provider: "faux".into(),
        base_url: "".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost { input: 0.0, output: 0.0, cache_read: 0.0, cache_write: 0.0 },
        context_window: 0,
        max_tokens: 0,
        headers: None,
        compat: None,
    }
}

#[tokio::test]
async fn faux_simple_text() {
    let api = "faux-api-simple-text";
    let provider = Arc::new(FauxProvider::simple_text("Hello from faux!"));
    registry::register(api, provider);

    let model = faux_model(api);
    let stream = registry::stream_model(
        &model,
        Context {
            system_prompt: None,
            messages: vec![],
            tools: None,
        },
        None,
    );
    let events: Vec<_> = stream.collect().await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AssistantMessageEvent::Start { .. }))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AssistantMessageEvent::TextDelta { .. }))
    );

    let last = events.last().unwrap();
    match last {
        AssistantMessageEvent::Done { reason, message } => {
            assert_eq!(*reason, StopReason::Stop);
            assert_eq!(message.stop_reason, StopReason::Stop);
        }
        other => panic!("expected Done, got {:?}", other),
    }

    registry::unregister(api);
}

#[tokio::test]
async fn faux_with_tool_call() {
    let api = "faux-api-tool-call";
    let provider = Arc::new(FauxProvider::new(vec![FauxResponse {
        text_deltas: vec![],
        thinking_deltas: vec![],
        tool_calls: vec![FauxToolCall {
            id: "call_1".into(),
            name: "read_file".into(),
            deltas: vec!["{\"path\":".into(), "\"/x\"}".into()],
            final_arguments: serde_json::json!({"path": "/x"}),
        }],
    }]));
    registry::register(api, provider);

    let model = faux_model(api);
    let stream = registry::stream_model(
        &model,
        Context {
            system_prompt: None,
            messages: vec![],
            tools: None,
        },
        None,
    );
    let events: Vec<_> = stream.collect().await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AssistantMessageEvent::ToolcallStart { .. }))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AssistantMessageEvent::ToolcallDelta { .. }))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AssistantMessageEvent::ToolcallEnd { .. }))
    );
    registry::unregister(api);
}

#[tokio::test]
async fn complete_with_faux() {
    let api = "faux-api-complete";
    let provider = Arc::new(FauxProvider::simple_text("complete test"));
    registry::register(api, provider);

    let model = faux_model(api);
    let stream = registry::stream_model(
        &model,
        Context {
            system_prompt: None,
            messages: vec![],
            tools: None,
        },
        None,
    );
    let result = complete(stream).await.unwrap();
    assert_eq!(result.stop_reason, StopReason::Stop);
    assert!(!result.content.is_empty());
    registry::unregister(api);
}

#[tokio::test]
async fn faux_call_queue_with_tool_use() {
    let provider = Arc::new(FauxProvider::with_call_queue(vec![FauxCall {
        responses: vec![FauxResponse {
            text_deltas: vec![],
            thinking_deltas: vec![],
            tool_calls: vec![FauxToolCall {
                id: "toolu_01".into(),
                name: "search".into(),
                deltas: vec!["{\"q\":".into(), "\"rust\"}".into()],
                final_arguments: serde_json::json!({"q": "rust"}),
            }],
        }],
        stop_reason: StopReason::ToolUse,
    }]));
    registry::register("faux-call-queue", provider);

    let model = Model {
        id: "faux-model".into(),
        name: "Faux".into(),
        api: "faux-call-queue".into(),
        provider: "faux".into(),
        base_url: "".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost { input: 0.0, output: 0.0, cache_read: 0.0, cache_write: 0.0 },
        context_window: 0,
        max_tokens: 0,
        headers: None,
        compat: None,
    };
    let ctx = Context {
        system_prompt: None,
        messages: vec![],
        tools: None,
    };

    let stream = registry::stream_model(&model, ctx, None);
    let events: Vec<_> = stream.collect().await;

    let last = events.last().unwrap();
    match last {
        AssistantMessageEvent::Done { reason, .. } => {
            assert_eq!(*reason, StopReason::ToolUse);
        }
        other => panic!("expected Done with ToolUse, got {:?}", other),
    }

    registry::unregister("faux-call-queue");
}
