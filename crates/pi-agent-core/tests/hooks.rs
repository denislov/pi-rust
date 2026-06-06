mod common;
use common::{TestProvider, faux_model, tool_use_turn};
use futures::StreamExt;
use pi_agent_core::{
    AfterToolCallResult, Agent, AgentConfig, AgentEvent, AgentMessage, AgentTool,
    BeforeToolCallResult, QueueMode,
};
use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::{ContentBlock, StopReason};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn simple_text_tool(name: &str, text: &str) -> AgentTool {
    let text = text.to_string();
    let name = name.to_string();
    AgentTool {
        name,
        description: "simple".into(),
        parameters: serde_json::json!({"type": "object"}),
        execution_mode: None,
        execute: Arc::new(move |_| {
            let text = text.clone();
            Box::pin(async move {
                Ok(vec![ContentBlock::Text {
                    text,
                    text_signature: None,
                }])
            })
        }),
    }
}

fn script_tool_then_stop(api: &str, tool_name: &str, args: serde_json::Value) {
    let json_str = args.to_string();
    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::single_call(
                vec![pi_ai::providers::faux::FauxResponse {
                    text_deltas: vec![],
                    thinking_deltas: vec![],
                    tool_calls: vec![pi_ai::providers::faux::FauxToolCall {
                        id: "tool_1".into(),
                        name: tool_name.into(),
                        deltas: vec![json_str],
                        final_arguments: args,
                    }],
                }],
                StopReason::ToolUse,
            ),
            FauxProvider::text_call("I'm done.", StopReason::Stop),
        ])),
    );
}

#[tokio::test]
async fn before_hook_blocks_tool_execution() {
    let api = "hooks-before";
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_real = calls.clone();
    let mut config = AgentConfig::new(faux_model(api));
    config.hooks.before_tool_call = Some(Arc::new(move |ctx| {
        assert_eq!(ctx.tool_name, "echo");
        Box::pin(async move {
            Ok(Some(BeforeToolCallResult {
                block: true,
                reason: Some("blocked by test".into()),
            }))
        })
    }));

    let agent = Agent::new(config);
    let calls_for_tool = calls_real.clone();
    agent.add_tool(AgentTool {
        name: "echo".into(),
        description: "echo".into(),
        parameters: serde_json::json!({"type": "object"}),
        execution_mode: None,
        execute: Arc::new(move |_| {
            calls_for_tool.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                Ok(vec![ContentBlock::Text {
                    text: "executed".into(),
                    text_signature: None,
                }])
            })
        }),
    });

    script_tool_then_stop(api, "echo", serde_json::json!({}));
    let mut stream = agent.prompt("run");
    let mut saw_blocked_result = false;
    while let Some(event) = stream.next().await {
        if let pi_agent_core::AgentEvent::ToolCallEnd { result, .. } = event {
            saw_blocked_result = result.is_error
                && result
                    .content
                    .iter()
                    .any(|block| matches!(block, ContentBlock::Text { text, .. } if text == "blocked by test"));
        }
    }

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(saw_blocked_result);

    registry::unregister(api);
}

#[tokio::test]
async fn after_hook_replaces_tool_result() {
    let api = "hooks-after";
    let mut config = AgentConfig::new(faux_model(api));
    config.hooks.after_tool_call = Some(Arc::new(|ctx| {
        assert_eq!(ctx.tool_name, "echo");
        assert!(!ctx.result.is_error);
        Box::pin(async move {
            Ok(Some(AfterToolCallResult {
                content: Some(vec![ContentBlock::Text {
                    text: "rewritten".into(),
                    text_signature: None,
                }]),
                is_error: Some(true),
                terminate: Some(false),
            }))
        })
    }));

    let agent = Agent::new(config);
    agent.add_tool(simple_text_tool("echo", "original"));
    script_tool_then_stop(api, "echo", serde_json::json!({}));

    let mut stream = agent.prompt("run");
    while stream.next().await.is_some() {}

    let messages = agent.messages();
    assert!(messages.iter().any(|msg| matches!(
        msg,
        pi_agent_core::AgentMessage::ToolResult { is_error: true, content, .. }
            if content.iter().any(|block| matches!(block, ContentBlock::Text { text, .. } if text == "rewritten"))
    )));

    registry::unregister(api);
}

#[tokio::test]
async fn after_hook_terminate_stops_loop_after_tool_results() {
    let api = "hooks-after-terminate";
    let mut config = AgentConfig::new(faux_model(api));
    config.hooks.after_tool_call = Some(Arc::new(|_| {
        Box::pin(async move {
            Ok(Some(AfterToolCallResult {
                content: None,
                is_error: None,
                terminate: Some(true),
            }))
        })
    }));

    let agent = Agent::new(config);
    agent.add_tool(simple_text_tool("echo", "ok"));
    registry::register(
        api,
        Arc::new(TestProvider::new(vec![tool_use_turn(
            "tool_1",
            "echo",
            serde_json::json!({}),
        )])),
    );

    let events: Vec<_> = agent.prompt("run").collect().await;

    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::AgentDone { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEvent::AgentError { .. }))
    );

    registry::unregister(api);
}

#[tokio::test]
async fn should_stop_after_turn_runs_before_follow_up_queue() {
    let api = "hooks-should-stop";
    let calls = Arc::new(AtomicUsize::new(0));
    let hook_calls = calls.clone();
    let mut config = AgentConfig::new(faux_model(api));
    config.follow_up_mode = QueueMode::All;
    config.hooks.should_stop_after_turn = Some(Arc::new(move |_| {
        hook_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { Ok(true) })
    }));

    let agent = Agent::new(config);
    agent.follow_up("should not run");
    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("first.", StopReason::Stop),
            FauxProvider::text_call("second.", StopReason::Stop),
        ])),
    );

    let events: Vec<_> = agent.prompt("run").collect().await;

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::AgentDone { .. }))
    );
    let assistant_count = agent
        .messages()
        .iter()
        .filter(|message| matches!(message, AgentMessage::Assistant { .. }))
        .count();
    assert_eq!(assistant_count, 1);

    registry::unregister(api);
}

#[tokio::test]
async fn prepare_next_turn_can_replace_messages_before_follow_up_turn() {
    let api = "hooks-prepare-next-turn";
    let calls = Arc::new(AtomicUsize::new(0));
    let hook_calls = calls.clone();
    let mut config = AgentConfig::new(faux_model(api));
    config.follow_up_mode = QueueMode::All;
    config.hooks.prepare_next_turn = Some(Arc::new(move |ctx| {
        hook_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            if ctx.turn == 1 {
                Ok(Some(pi_agent_core::hooks::AgentLoopTurnUpdate {
                    messages: Some(vec![AgentMessage::UserText {
                        message_id: "prepared".into(),
                        text: "prepared context".into(),
                    }]),
                    ..Default::default()
                }))
            } else {
                Ok(None)
            }
        })
    }));

    let agent = Agent::new(config);
    agent.follow_up("follow-up");
    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("first.", StopReason::Stop),
            FauxProvider::text_call("second.", StopReason::Stop),
        ])),
    );

    let events: Vec<_> = agent.prompt("run").collect().await;

    assert!(calls.load(Ordering::SeqCst) >= 1);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::AgentDone { .. }))
    );
    assert!(agent.messages().iter().any(|message| {
        matches!(message, AgentMessage::UserText { text, .. } if text == "prepared context")
    }));

    registry::unregister(api);
}
