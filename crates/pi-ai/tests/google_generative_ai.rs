use bytes::Bytes;
use futures::stream;
use pi_ai::providers::google;
use pi_ai::types::{AssistantMessageEvent, ContentBlock, Model, ModelCost, ModelInput, StopReason};

fn test_model() -> Model {
    Model {
        id: "gemini-2.5-flash".into(),
        name: "Gemini 2.5 Flash".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 8192,
        headers: None,
        compat: None,
    }
}

fn fixture_bytes(path: &str) -> Vec<Bytes> {
    let content = std::fs::read_to_string(path).unwrap();
    vec![Bytes::from(content)]
}

#[tokio::test]
async fn google_fixture_maps_thinking_tool_and_done() {
    let body = stream::iter(
        fixture_bytes("tests/fixtures/google-text-tool.sse")
            .into_iter()
            .map(Ok::<_, String>),
    );
    let model = test_model();
    let event_stream = google::process::process(body, model, None);
    use futures::StreamExt;

    let events: Vec<_> = event_stream.collect().await;

    assert!(!events.is_empty(), "should have events");
    assert!(
        matches!(events[0], AssistantMessageEvent::Start { .. }),
        "first event should be Start"
    );

    let has_thinking = events
        .iter()
        .any(|e| matches!(e, AssistantMessageEvent::ThinkingStart { .. }));
    let has_toolcall = events
        .iter()
        .any(|e| matches!(e, AssistantMessageEvent::ToolcallStart { .. }));
    assert!(has_thinking, "should have thinking events");
    assert!(has_toolcall, "should have tool call events");

    let last = events.last().unwrap();
    match last {
        AssistantMessageEvent::Done { reason, message } => {
            assert_eq!(*reason, StopReason::Stop);
            assert!(message.usage.total_tokens > 0);
            assert!(
                message.content.iter().any(|b| {
                    matches!(b, ContentBlock::ToolCall { name, .. } if name == "read")
                })
            );
        }
        _ => panic!("expected Done event, got {:?}", last),
    }
}

#[test]
fn builtins_register_google_api() {
    pi_ai::registry::unregister("google-generative-ai");
    pi_ai::providers::register_builtins();
    assert!(pi_ai::registry::lookup("google-generative-ai").is_some());
}

#[tokio::test]
async fn complete_smoke_test() {
    let body = stream::iter(
        fixture_bytes("tests/fixtures/google-text-tool.sse")
            .into_iter()
            .map(Ok::<_, String>),
    );
    let model = test_model();
    let event_stream = google::process::process(body, model, None);
    let result = pi_ai::stream::complete(event_stream).await;
    assert!(result.is_ok(), "complete() should return Ok");
    let msg = result.unwrap();
    assert!(!msg.content.is_empty());
    assert!(
        msg.content
            .iter()
            .any(|b| { matches!(b, ContentBlock::ToolCall { name, .. } if name == "read") })
    );
}
