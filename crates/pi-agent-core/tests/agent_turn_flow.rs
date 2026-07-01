mod common;

use pi_agent_core::agent_turn_flow::{
    AgentTurnContext, DecideStopOrToolsNode, ExecuteToolsNode, MaybeCompactRuntimeContextNode,
    PrepareContextNode, ProviderStreamNode,
};
use pi_agent_core::flow::{Action, Flow};
use pi_agent_core::{
    Agent, AgentConfig, AgentEvent, AgentMessage, AgentResources, AgentTool, CompactionConfig,
    CompactionSettings, PromptTemplate, Skill, ToolExecutionMode,
};
use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::{AssistantMessageEvent, ContentBlock, StopReason, StreamOptions};
use std::sync::Arc;

fn user_msg(id: &str, text: &str) -> AgentMessage {
    AgentMessage::UserText {
        message_id: id.into(),
        text: text.into(),
    }
}

fn text_content(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

#[test]
fn agent_turn_context_snapshots_agent_state_without_draining_queues() {
    let resources = AgentResources {
        skills: vec![Skill {
            name: "rust".into(),
            description: "Rust guidance".into(),
            location: "/skills/rust/SKILL.md".into(),
            content: "Use Rust idioms.".into(),
            disable_model_invocation: false,
        }],
        prompt_templates: vec![PromptTemplate {
            name: "review".into(),
            description: "Review code".into(),
            content: "Review $1".into(),
            location: "/prompts/review.md".into(),
        }],
    };
    let mut config = AgentConfig::new(common::faux_model("test-api"));
    config.system_prompt = Some("system rules".into());
    config.max_turns = Some(3);
    config.resources = resources;

    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "hello"));
    agent.add_tool(AgentTool::new_text(
        "echo",
        "echo input",
        serde_json::json!({"type": "object"}),
        |_| async { Ok("ok".to_string()) },
    ));
    agent.steer("steer this turn");
    agent.follow_up("follow up next");

    let context = AgentTurnContext::from_agent(&agent);

    assert_eq!(
        context.config.system_prompt.as_deref(),
        Some("system rules")
    );
    assert_eq!(context.config.max_turns, Some(3));
    assert_eq!(context.messages.len(), 1);
    assert!(matches!(
        &context.messages[0],
        AgentMessage::UserText { text, .. } if text == "hello"
    ));
    assert_eq!(context.tools.len(), 1);
    assert_eq!(context.tools[0].name, "echo");
    assert_eq!(context.resources.skills.len(), 1);
    assert_eq!(context.resources.skills[0].name, "rust");
    assert_eq!(context.resources.prompt_templates.len(), 1);
    assert_eq!(context.resources.prompt_templates[0].name, "review");
    assert_eq!(context.steering_queue.len(), 1);
    assert_eq!(context.follow_up_queue.len(), 1);
    assert_eq!(context.turn, 0);
    assert!(context.provider_request.is_none());
    assert!(context.assistant_message.is_none());
    assert!(context.pending_tool_calls.is_empty());
    assert!(context.tool_results.is_empty());
    assert!(context.events.is_empty());
    assert!(!context.cancel_token.is_cancelled());

    let drained = agent.drain_steering_queue();
    assert_eq!(drained.len(), 1);
}

#[tokio::test]
async fn prepare_context_node_builds_provider_request_from_context_snapshot() {
    let mut config = AgentConfig::new(common::faux_model("test-api"));
    config.system_prompt = Some("system rules".into());
    config.stream_options = Some(StreamOptions {
        temperature: Some(0.2),
        max_tokens: Some(123),
        ..Default::default()
    });

    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "hello"));
    agent.add_tool(AgentTool::new_text(
        "echo",
        "echo input",
        serde_json::json!({"type": "object"}),
        |_| async { Ok("ok".to_string()) },
    ));

    let (expected_context, expected_options) = agent.provider_request_snapshot();
    let expected_options = expected_options.expect("stream options should be configured");

    let mut context = AgentTurnContext::from_agent(&agent);
    let mut flow = Flow::new("prepare_context").unwrap();
    flow.add_node("prepare_context", PrepareContextNode)
        .unwrap();

    let outcome = flow.run(&mut context).await.unwrap();

    assert_eq!(outcome.last_node.as_str(), "prepare_context");
    let request = context
        .provider_request
        .as_ref()
        .expect("node should attach provider request");
    assert_eq!(request.model.id, "faux-model");
    assert_eq!(request.context, expected_context);
    assert_eq!(
        request.stream_options.temperature,
        expected_options.temperature
    );
    assert_eq!(
        request.stream_options.max_tokens,
        expected_options.max_tokens
    );
    assert!(request.stream_options.cancel.is_some());
}

#[tokio::test]
async fn runtime_compaction_node_summarizes_and_updates_context_messages() {
    let api = "agent-turn-flow-runtime-compaction";
    let mut config = AgentConfig::new(common::faux_model_with_window(api, 100));
    config.compaction = Some(CompactionConfig {
        settings: CompactionSettings {
            enabled: true,
            reserve_tokens: 0,
            keep_recent_tokens: 8,
        },
        custom_instructions: None,
    });

    let agent = Agent::new(config);
    agent.add_message(user_msg("old_1", &"old context ".repeat(40)));
    agent.add_message(user_msg("old_2", &"more old context ".repeat(40)));

    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("summary of old context", StopReason::Stop),
        ])),
    );

    let mut context = AgentTurnContext::from_agent(&agent);
    let mut flow = Flow::new("maybe_compact_runtime_context").unwrap();
    flow.add_node(
        "maybe_compact_runtime_context",
        MaybeCompactRuntimeContextNode,
    )
    .unwrap();

    let outcome = flow.run(&mut context).await.unwrap();

    assert_eq!(outcome.last_node.as_str(), "maybe_compact_runtime_context");
    assert!(matches!(
        context.messages.first(),
        Some(AgentMessage::CompactionSummary { summary, .. })
            if summary == "summary of old context"
    ));
    assert!(context.messages.iter().any(|message| matches!(
        message,
        AgentMessage::UserText { message_id, .. } if message_id == "old_2"
    )));
    assert_eq!(
        context.runtime_compaction.summary.as_deref(),
        Some("summary of old context")
    );
    assert_eq!(
        context.runtime_compaction.first_kept_message_id.as_deref(),
        Some("old_2")
    );
    assert!(context.runtime_compaction.tokens_before.unwrap() > 0);
    assert!(context.events.iter().any(|event| matches!(
        event,
        AgentEvent::SessionCompacted { summary, first_kept_message_id, .. }
            if summary == "summary of old context" && first_kept_message_id == "old_2"
    )));

    registry::unregister(api);
}

#[tokio::test]
async fn execute_tools_node_runs_sequential_tool_and_appends_result_message() {
    let api = "agent-turn-flow-execute-tool";
    let mut config = AgentConfig::new(common::faux_model(api));
    config.tool_execution = ToolExecutionMode::Sequential;
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "use the tool"));
    agent.add_tool(AgentTool::new_text(
        "echo",
        "echo input",
        serde_json::json!({"type": "object"}),
        |args| async move {
            Ok(args
                .get("text")
                .and_then(|value| value.as_str())
                .unwrap_or("missing")
                .to_string())
        },
    ));

    registry::register(
        api,
        Arc::new(common::TestProvider::new(vec![common::tool_use_turn(
            "call_1",
            "echo",
            serde_json::json!({"text": "hello"}),
        )])),
    );

    let mut context = AgentTurnContext::from_agent(&agent);
    let mut flow = Flow::new("prepare_context").unwrap();
    flow.add_node("prepare_context", PrepareContextNode)
        .unwrap()
        .add_node("provider_stream", ProviderStreamNode)
        .unwrap()
        .add_node("decide_stop_or_tools", DecideStopOrToolsNode)
        .unwrap()
        .add_node("execute_tools", ExecuteToolsNode)
        .unwrap()
        .edge("prepare_context", "provider_stream")
        .unwrap()
        .edge("provider_stream", "decide_stop_or_tools")
        .unwrap()
        .edge_on(
            "decide_stop_or_tools",
            Action::new("tools").unwrap(),
            "execute_tools",
        )
        .unwrap();

    let outcome = flow.run(&mut context).await.unwrap();

    assert_eq!(outcome.last_node.as_str(), "execute_tools");
    assert_eq!(outcome.last_action.as_str(), "continue");
    assert!(context.pending_tool_calls.is_empty());
    assert_eq!(context.tool_results.len(), 1);
    assert_eq!(text_content(&context.tool_results[0].content), "hello");
    assert!(context.events.iter().any(|event| matches!(
        event,
        AgentEvent::ToolCallStart { tool_call_id, tool_name, arguments }
            if tool_call_id == "call_1" && tool_name == "echo" && arguments == &serde_json::json!({"text": "hello"})
    )));
    assert!(context.events.iter().any(|event| matches!(
        event,
        AgentEvent::ToolCallEnd { tool_call_id, tool_name, result }
            if tool_call_id == "call_1" && tool_name == "echo" && !result.is_error && text_content(&result.content) == "hello"
    )));
    assert!(context.messages.iter().any(|message| matches!(
        message,
        AgentMessage::ToolResult { tool_call_id, tool_name, is_error: false, content, .. }
            if tool_call_id == "call_1" && tool_name == "echo" && text_content(content) == "hello"
    )));

    registry::unregister(api);
}

#[tokio::test]
async fn execute_tools_node_records_unknown_tool_as_error_result() {
    let api = "agent-turn-flow-execute-unknown-tool";
    let mut config = AgentConfig::new(common::faux_model(api));
    config.tool_execution = ToolExecutionMode::Sequential;
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "use the tool"));

    registry::register(
        api,
        Arc::new(common::TestProvider::new(vec![common::tool_use_turn(
            "call_1",
            "missing_tool",
            serde_json::json!({}),
        )])),
    );

    let mut context = AgentTurnContext::from_agent(&agent);
    let mut flow = Flow::new("prepare_context").unwrap();
    flow.add_node("prepare_context", PrepareContextNode)
        .unwrap()
        .add_node("provider_stream", ProviderStreamNode)
        .unwrap()
        .add_node("decide_stop_or_tools", DecideStopOrToolsNode)
        .unwrap()
        .add_node("execute_tools", ExecuteToolsNode)
        .unwrap()
        .edge("prepare_context", "provider_stream")
        .unwrap()
        .edge("provider_stream", "decide_stop_or_tools")
        .unwrap()
        .edge_on(
            "decide_stop_or_tools",
            Action::new("tools").unwrap(),
            "execute_tools",
        )
        .unwrap();

    let outcome = flow.run(&mut context).await.unwrap();

    assert_eq!(outcome.last_action.as_str(), "continue");
    assert_eq!(context.tool_results.len(), 1);
    assert!(context.tool_results[0].is_error);
    assert_eq!(
        text_content(&context.tool_results[0].content),
        "unknown tool: missing_tool"
    );
    assert!(context.events.iter().any(|event| matches!(
        event,
        AgentEvent::ToolCallEnd { tool_call_id, tool_name, result }
            if tool_call_id == "call_1"
                && tool_name == "missing_tool"
                && result.is_error
                && text_content(&result.content) == "unknown tool: missing_tool"
    )));
    assert!(context.messages.iter().any(|message| matches!(
        message,
        AgentMessage::ToolResult { tool_call_id, tool_name, is_error: true, content, .. }
            if tool_call_id == "call_1"
                && tool_name == "missing_tool"
                && text_content(content) == "unknown tool: missing_tool"
    )));

    registry::unregister(api);
}

#[tokio::test]
async fn provider_stream_node_records_llm_events_and_final_assistant_message() {
    let api = "agent-turn-flow-provider-stream";
    let config = AgentConfig::new(common::faux_model(api));
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "hello"));

    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("final answer", StopReason::Stop),
        ])),
    );

    let mut context = AgentTurnContext::from_agent(&agent);
    let mut flow = Flow::new("prepare_context").unwrap();
    flow.add_node("prepare_context", PrepareContextNode)
        .unwrap()
        .add_node("provider_stream", ProviderStreamNode)
        .unwrap()
        .edge("prepare_context", "provider_stream")
        .unwrap();

    let outcome = flow.run(&mut context).await.unwrap();

    assert_eq!(outcome.last_node.as_str(), "provider_stream");
    assert!(context.events.iter().any(|event| matches!(
        event,
        AgentEvent::LlmEvent(AssistantMessageEvent::TextDelta { delta, .. }) if delta == "final answer"
    )));
    let assistant = context
        .assistant_message
        .as_ref()
        .expect("provider stream should store final assistant message");
    assert!(assistant.content.iter().any(|block| matches!(
        block,
        ContentBlock::Text { text, .. } if text == "final answer"
    )));

    registry::unregister(api);
}

#[tokio::test]
async fn provider_stream_node_maps_missing_done_to_agent_error_event() {
    let api = "agent-turn-flow-provider-error";
    let config = AgentConfig::new(common::faux_model(api));
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "hello"));

    registry::register(api, Arc::new(common::TestProvider::new(vec![])));

    let mut context = AgentTurnContext::from_agent(&agent);
    let mut flow = Flow::new("prepare_context").unwrap();
    flow.add_node("prepare_context", PrepareContextNode)
        .unwrap()
        .add_node("provider_stream", ProviderStreamNode)
        .unwrap()
        .edge("prepare_context", "provider_stream")
        .unwrap();

    let outcome = flow.run(&mut context).await.unwrap();

    assert_eq!(outcome.last_node.as_str(), "provider_stream");
    assert!(context.assistant_message.is_none());
    assert!(context.events.iter().any(|event| matches!(
        event,
        AgentEvent::LlmEvent(AssistantMessageEvent::Error { .. })
    )));
    assert!(context.events.iter().any(|event| matches!(
        event,
        AgentEvent::AgentError { error } if error == "no more scripted turns"
    )));

    registry::unregister(api);
}

#[tokio::test]
async fn decide_node_finishes_text_response_and_appends_assistant_message() {
    let api = "agent-turn-flow-decide-done";
    let config = AgentConfig::new(common::faux_model(api));
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "hello"));

    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("final answer", StopReason::Stop),
        ])),
    );

    let mut context = AgentTurnContext::from_agent(&agent);
    let mut flow = Flow::new("prepare_context").unwrap();
    flow.add_node("prepare_context", PrepareContextNode)
        .unwrap()
        .add_node("provider_stream", ProviderStreamNode)
        .unwrap()
        .add_node("decide_stop_or_tools", DecideStopOrToolsNode)
        .unwrap()
        .edge("prepare_context", "provider_stream")
        .unwrap()
        .edge("provider_stream", "decide_stop_or_tools")
        .unwrap();

    let outcome = flow.run(&mut context).await.unwrap();

    assert_eq!(outcome.last_node.as_str(), "decide_stop_or_tools");
    assert_eq!(outcome.last_action.as_str(), "done");
    assert!(context.events.iter().any(|event| matches!(
        event,
        AgentEvent::AgentDone { message }
            if message.content.iter().any(|block| matches!(
                block,
                ContentBlock::Text { text, .. } if text == "final answer"
            ))
    )));
    assert!(context.messages.iter().any(|message| matches!(
        message,
        AgentMessage::Assistant { message, .. }
            if message.content.iter().any(|block| matches!(
                block,
                ContentBlock::Text { text, .. } if text == "final answer"
            ))
    )));

    registry::unregister(api);
}

#[tokio::test]
async fn decide_node_extracts_tool_calls_and_returns_tools_action() {
    let api = "agent-turn-flow-decide-tools";
    let config = AgentConfig::new(common::faux_model(api));
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "use the tool"));

    registry::register(
        api,
        Arc::new(common::TestProvider::new(vec![common::tool_use_turn(
            "call_1",
            "echo",
            serde_json::json!({"text": "hello"}),
        )])),
    );

    let mut context = AgentTurnContext::from_agent(&agent);
    let mut flow = Flow::new("prepare_context").unwrap();
    flow.add_node("prepare_context", PrepareContextNode)
        .unwrap()
        .add_node("provider_stream", ProviderStreamNode)
        .unwrap()
        .add_node("decide_stop_or_tools", DecideStopOrToolsNode)
        .unwrap()
        .edge("prepare_context", "provider_stream")
        .unwrap()
        .edge("provider_stream", "decide_stop_or_tools")
        .unwrap();

    let outcome = flow.run(&mut context).await.unwrap();

    assert_eq!(outcome.last_node.as_str(), "decide_stop_or_tools");
    assert_eq!(outcome.last_action.as_str(), "tools");
    assert_eq!(context.pending_tool_calls.len(), 1);
    assert_eq!(context.pending_tool_calls[0].index, 0);
    assert_eq!(context.pending_tool_calls[0].id, "call_1");
    assert_eq!(context.pending_tool_calls[0].name, "echo");
    assert_eq!(
        context.pending_tool_calls[0].arguments,
        serde_json::json!({"text": "hello"})
    );
    assert!(
        !context
            .events
            .iter()
            .any(|event| matches!(event, AgentEvent::AgentDone { .. }))
    );
    assert!(context.messages.iter().any(|message| matches!(
        message,
        AgentMessage::Assistant { message, .. }
            if message.content.iter().any(|block| matches!(
                block,
                ContentBlock::ToolCall { id, name, .. } if id == "call_1" && name == "echo"
            ))
    )));

    registry::unregister(api);
}
