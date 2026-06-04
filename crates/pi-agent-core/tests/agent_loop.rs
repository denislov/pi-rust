mod common;
use common::{ScriptedTurn, TestProvider, text_turn, tool_use_turn};
use futures::StreamExt;
use pi_agent_core::{Agent, AgentConfig, AgentEvent, AgentMessage, AgentTool};
use pi_ai::registry;
use pi_ai::types::{AssistantMessage, AssistantMessageEvent, ContentBlock, Model, ModelCost, ModelInput, StopReason};
use std::sync::Arc;

fn test_model(api_key: &str) -> Model {
    Model {
        id: "test-model".into(),
        name: "Test Model".into(),
        api: api_key.into(),
        provider: "test".into(),
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

fn test_config(api_key: &str) -> AgentConfig {
    AgentConfig {
        model: test_model(api_key),
        system_prompt: Some("Be helpful.".into()),
        max_turns: 5,
        stream_options: None,
    }
}

#[tokio::test]
async fn single_turn_text_response() {
    let api_key = "test-api-1";
    let provider = Arc::new(TestProvider::new(vec![text_turn("Hello, world!")]));
    registry::register(api_key, provider);

    let agent = Agent::new(test_config(api_key));

    let stream = agent.prompt("hi");
    let events: Vec<_> = stream.collect().await;

    let has_done = events
        .iter()
        .any(|e| matches!(e, AgentEvent::AgentDone { .. }));
    assert!(has_done, "should have AgentDone event");

    let has_text = events.iter().any(|e| {
        matches!(
            e,
            AgentEvent::LlmEvent(AssistantMessageEvent::TextDelta { .. })
        )
    });
    assert!(has_text, "should have text delta event");

    let msgs = agent.messages();
    assert_eq!(msgs.len(), 2); // UserText + Assistant
    assert!(matches!(&msgs[0], AgentMessage::UserText { .. }));
    assert!(matches!(&msgs[1], AgentMessage::Assistant { .. }));

    registry::unregister(api_key);
}

#[tokio::test]
async fn tool_use_turn_executes_tool() {
    let api_key = "test-api-2";
    let provider = Arc::new(TestProvider::new(vec![
        tool_use_turn("tool_1", "echo", serde_json::json!({"text": "hi"})),
        text_turn("Tool executed successfully."),
    ]));
    registry::register(api_key, provider);

    let agent = Agent::new(test_config(api_key));

    let tool = AgentTool {
        name: "echo".into(),
        description: "echoes input".into(),
        parameters: serde_json::json!({"type": "object", "properties": {"text": {"type": "string"}}}),
        execute: Arc::new(|args| {
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("no text");
            let result = vec![ContentBlock::Text {
                text: format!("echo: {}", text),
                text_signature: None,
            }];
            Box::pin(async move { Ok(result) })
        }),
    };
    agent.add_tool(tool);

    let stream = agent.prompt("echo hi");
    let events: Vec<_> = stream.collect().await;

    let has_tool_start = events
        .iter()
        .any(|e| matches!(e, AgentEvent::ToolCallStart { .. }));
    let has_tool_end = events
        .iter()
        .any(|e| matches!(e, AgentEvent::ToolCallEnd { .. }));
    assert!(has_tool_start, "should have ToolCallStart");
    assert!(has_tool_end, "should have ToolCallEnd");

    let has_done = events
        .iter()
        .any(|e| matches!(e, AgentEvent::AgentDone { .. }));
    assert!(has_done, "should have AgentDone");

    let msgs = agent.messages();
    assert_eq!(msgs.len(), 4); // UserText, Assistant(tool_use), ToolResult, Assistant(text)
    assert!(matches!(&msgs[2], AgentMessage::ToolResult { .. }));

    registry::unregister(api_key);
}

#[tokio::test]
async fn unknown_tool_yields_error_content_and_continues() {
    let api_key = "test-api-3";
    let provider = Arc::new(TestProvider::new(vec![
        tool_use_turn("tool_1", "nonexistent", serde_json::json!({})),
        text_turn("I tried but the tool was not found."),
    ]));
    registry::register(api_key, provider);

    let agent = Agent::new(test_config(api_key));

    let stream = agent.prompt("use nonexistent tool");
    let events: Vec<_> = stream.collect().await;

    let tool_end = events
        .iter()
        .find_map(|e| match e {
            AgentEvent::ToolCallEnd { result, .. } => Some(result.clone()),
            _ => None,
        })
        .unwrap();
    assert!(tool_end.is_err());
    assert!(tool_end.unwrap_err().contains("unknown tool"));

    let has_done = events
        .iter()
        .any(|e| matches!(e, AgentEvent::AgentDone { .. }));
    assert!(has_done);

    registry::unregister(api_key);
}

#[tokio::test]
async fn max_turns_exceeded_yields_error() {
    let api_key = "test-api-4";
    let mut turns = Vec::new();
    for _ in 0..10 {
        turns.push(tool_use_turn(
            "tool_1",
            "echo",
            serde_json::json!({"text": "x"}),
        ));
    }
    let provider = Arc::new(TestProvider::new(turns));
    registry::register(api_key, provider);

    let mut config = test_config(api_key);
    config.max_turns = 2;

    let agent = Agent::new(config);
    let tool = AgentTool {
        name: "echo".into(),
        description: "echo".into(),
        parameters: serde_json::json!({"type": "object"}),
        execute: Arc::new(|_| {
            Box::pin(async {
                Ok(vec![ContentBlock::Text {
                    text: "ok".into(),
                    text_signature: None,
                }])
            })
        }),
    };
    agent.add_tool(tool);

    let stream = agent.prompt("go");
    let events: Vec<_> = stream.collect().await;

    let has_error = events
        .iter()
        .any(|e| matches!(e, AgentEvent::AgentError { error } if error.contains("max turns")));
    assert!(has_error, "should have max turns error");

    registry::unregister(api_key);
}

#[tokio::test]
async fn abort_mid_turn_yields_error() {
    let api_key = "test-api-5";
    let provider = Arc::new(TestProvider::new(vec![text_turn("Hello")]));
    registry::register(api_key, provider);

    let agent = Agent::new(test_config(api_key));

    let stream = agent.prompt("hi");
    agent.abort();

    let events: Vec<_> = stream.collect().await;
    let has_abort_error = events
        .iter()
        .any(|e| matches!(e, AgentEvent::AgentError { error } if error.contains("aborted")));
    assert!(has_abort_error, "should have aborted error");

    registry::unregister(api_key);
}

#[tokio::test]
async fn provider_error_event_preserves_error_message() {
    let api_key = "test-api-provider-error";
    let mut message = AssistantMessage::empty("test", "test-model");
    message.error_message = Some("provider failed".into());
    message.stop_reason = StopReason::Error;
    let provider = Arc::new(TestProvider::new(vec![ScriptedTurn {
        events: vec![AssistantMessageEvent::Error {
            reason: StopReason::Error,
            message,
        }],
        stop_reason: StopReason::Error,
        response_id: "resp_error".into(),
        model_name: "test-model".into(),
    }]));
    registry::register(api_key, provider);

    let agent = Agent::new(test_config(api_key));

    let stream = agent.prompt("hi");
    let events: Vec<_> = stream.collect().await;
    let has_provider_error = events.iter().any(|event| {
        matches!(
            event,
            AgentEvent::AgentError { error } if error.contains("provider failed")
        )
    });
    assert!(has_provider_error, "should preserve provider error");

    registry::unregister(api_key);
}
