use std::sync::Arc;
use futures::StreamExt;
use pi_ai::types::*;
use pi_ai::registry;
use pi_ai::providers::faux::{FauxProvider, FauxResponse, FauxToolCall};
use pi_ai::stream::complete;

fn faux_model() -> Model {
    Model {
        id: "faux-model".into(), name: "Faux Model".into(),
        api: "faux-api".into(), provider: "faux".into(),
        base_url: "".into(), reasoning: false,
        input: 0.0, output: 0.0, cache_read: None, cache_write: None,
        context_window: 0, max_tokens: None, headers: None,
    }
}

#[tokio::test]
async fn faux_simple_text() {
    let provider = Arc::new(FauxProvider::simple_text("Hello from faux!"));
    registry::register("faux-api", provider);

    let model = faux_model();
    let stream = registry::stream_model(
        &model,
        Context { system_prompt: None, messages: vec![], tools: None },
        None,
    );
    let events: Vec<_> = stream.collect().await;
    assert!(events.iter().any(|e| matches!(e, AssistantMessageEvent::Start { .. })));
    assert!(events.iter().any(|e| matches!(e, AssistantMessageEvent::TextDelta { .. })));

    let last = events.last().unwrap();
    match last {
        AssistantMessageEvent::Done { reason, message } => {
            assert_eq!(*reason, StopReason::Stop);
            assert_eq!(message.stop_reason, StopReason::Stop);
        }
        other => panic!("expected Done, got {:?}", other),
    }

    registry::unregister("faux-api");
}

#[tokio::test]
async fn faux_with_tool_call() {
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
    registry::register("faux-api", provider);

    let model = faux_model();
    let stream = registry::stream_model(
        &model,
        Context { system_prompt: None, messages: vec![], tools: None },
        None,
    );
    let events: Vec<_> = stream.collect().await;
    assert!(events.iter().any(|e| matches!(e, AssistantMessageEvent::ToolcallStart { .. })));
    assert!(events.iter().any(|e| matches!(e, AssistantMessageEvent::ToolcallDelta { .. })));
    assert!(events.iter().any(|e| matches!(e, AssistantMessageEvent::ToolcallEnd { .. })));
    registry::unregister("faux-api");
}

#[tokio::test]
async fn complete_with_faux() {
    let provider = Arc::new(FauxProvider::simple_text("complete test"));
    registry::register("faux-api", provider);

    let model = faux_model();
    let stream = registry::stream_model(
        &model,
        Context { system_prompt: None, messages: vec![], tools: None },
        None,
    );
    let result = complete(stream).await.unwrap();
    assert_eq!(result.stop_reason, StopReason::Stop);
    assert!(!result.content.is_empty());
    registry::unregister("faux-api");
}
