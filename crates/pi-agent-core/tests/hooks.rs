mod common;
use common::faux_model;
use futures::StreamExt;
use pi_agent_core::{
    AfterToolCallResult, Agent, AgentConfig, AgentTool, AgentToolResult, BeforeToolCallResult,
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
