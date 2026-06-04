use bytes::Bytes;
use futures::stream;
use pi_ai::types::{AssistantMessageEvent, ContentBlock, Model, ModelCost, ModelInput, StopReason};

fn test_model() -> Model {
    Model {
        id: "claude-sonnet-4-5".into(),
        name: "Claude Sonnet 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.30,
            cache_write: 3.75,
        },
        context_window: 200000,
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
async fn text_only_stream() {
    let body = stream::iter(
        fixture_bytes("tests/fixtures/anthropic-text.sse")
            .into_iter()
            .map(Ok::<_, String>),
    );
    let model = test_model();
    let event_stream = pi_ai::providers::anthropic::process::process(body, model, None);
    use futures::StreamExt;

    let events: Vec<_> = event_stream.collect().await;
    assert!(
        events.len() >= 3,
        "expected at least 3 events, got {}",
        events.len()
    );

    assert!(matches!(events[0], AssistantMessageEvent::Start { .. }));

    let has_text_start = events
        .iter()
        .any(|e| matches!(e, AssistantMessageEvent::TextStart { .. }));
    let has_text_delta = events
        .iter()
        .any(|e| matches!(e, AssistantMessageEvent::TextDelta { .. }));
    let has_text_end = events
        .iter()
        .any(|e| matches!(e, AssistantMessageEvent::TextEnd { .. }));
    assert!(has_text_start);
    assert!(has_text_delta);
    assert!(has_text_end);

    let last = events.last().unwrap();
    match last {
        AssistantMessageEvent::Done { reason, message } => {
            assert_eq!(*reason, StopReason::Stop);
            assert!(!message.content.is_empty());
            if let ContentBlock::Text { text, .. } = &message.content[0] {
                assert!(text.contains("Hello"));
            } else {
                panic!("expected text content block");
            }
        }
        _ => panic!("expected Done event, got {:?}", last),
    }
}

#[tokio::test]
async fn thinking_and_tool_use_stream() {
    let body = stream::iter(
        fixture_bytes("tests/fixtures/anthropic-thinking-tooluse.sse")
            .into_iter()
            .map(Ok::<_, String>),
    );
    let model = test_model();
    let event_stream = pi_ai::providers::anthropic::process::process(body, model, None);
    use futures::StreamExt;

    let events: Vec<_> = event_stream.collect().await;

    let has_thinking = events
        .iter()
        .any(|e| matches!(e, AssistantMessageEvent::ThinkingStart { .. }));
    let has_toolcall = events
        .iter()
        .any(|e| matches!(e, AssistantMessageEvent::ToolcallStart { .. }));
    let has_text = events
        .iter()
        .any(|e| matches!(e, AssistantMessageEvent::TextStart { .. }));
    assert!(has_thinking, "should have thinking events");
    assert!(has_toolcall, "should have tool use events");
    assert!(has_text, "should have text events");

    match events.last().unwrap() {
        AssistantMessageEvent::Done { reason, message } => {
            assert_eq!(*reason, StopReason::ToolUse);
            let has_complete_tool = message.content.iter().any(|b| {
                matches!(b, ContentBlock::ToolCall { arguments, .. } if arguments.as_object().map_or(false, |o| o.contains_key("city")))
            });
            assert!(has_complete_tool, "tool call should have parsed arguments");
        }
        _ => panic!("expected Done event"),
    }
}
