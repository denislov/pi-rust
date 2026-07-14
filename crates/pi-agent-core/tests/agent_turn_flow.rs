mod common;

use common::ProviderGuard;
use futures::StreamExt;
use pi_agent_core::agent_turn_flow::{
    AgentTurnContext, ApplyBeforeProviderRequestHookNode, DecideAfterAssistantNode,
    DecideStopOrToolsNode, ExecuteToolsNode, MaybeCompactRuntimeContextNode,
    MaybePrepareNextTurnNode, PrepareContextNode, PrepareProviderRequestNode, ProviderStreamNode,
    StartTurnNode,
};
use pi_agent_core::api::{AgentLoopTurnUpdate, BeforeProviderRequestResult};
use pi_agent_core::flow::{Action, Flow};
use pi_agent_core::{
    AfterToolCallResult, Agent, AgentEvent, AgentMessage, AgentResources, AgentTool,
    AgentToolOutput, BeforeToolCallResult, CompactionConfig, CompactionSettings, PromptTemplate,
    QueueMode, Skill, ToolExecutionMode,
};
use pi_ai::providers::faux::FauxProvider;
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Message, StopReason,
    StreamOptions,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

const AGENT_TURN_FLOW_SLOW_TOOL_DELAY_MS: u64 = 100;
const AGENT_TURN_FLOW_FAST_TOOL_DELAY_MS: u64 = 10;
const AGENT_TURN_FLOW_FAST_TOOL_ADVANCE: Duration =
    Duration::from_millis(AGENT_TURN_FLOW_FAST_TOOL_DELAY_MS);
const AGENT_TURN_FLOW_REMAINING_TOOL_ADVANCE: Duration =
    Duration::from_millis(AGENT_TURN_FLOW_SLOW_TOOL_DELAY_MS - AGENT_TURN_FLOW_FAST_TOOL_DELAY_MS);

#[test]
fn agent_turn_flow_exposes_real_graph_shape() {
    assert_eq!(
        pi_agent_core::agent_turn_flow::AgentTurnFlow::node_ids(),
        &[
            "start_turn",
            "drain_queued_input",
            "maybe_compact_runtime_context",
            "prepare_provider_request",
            "apply_before_provider_request_hook",
            "provider_stream",
            "decide_after_assistant",
            "maybe_prepare_next_turn",
            "execute_tools",
        ]
    );
}

#[test]
fn parallel_tool_execution_uses_named_ordered_aggregation_helper() {
    let nodes_source = include_str!("../src/agent_turn_flow/nodes.rs");

    assert!(
        nodes_source.contains("async fn collect_parallel_tool_executions("),
        "parallel tool execution should use a named helper for ordered result aggregation"
    );
    assert!(
        nodes_source.contains("executions.sort_by_key(|execution| execution.index)"),
        "parallel tool aggregation should sort final results by assistant tool-call index"
    );
}

#[derive(Clone)]
struct ToolProbe {
    started: UnboundedSender<String>,
    finished: UnboundedSender<String>,
}

fn tool_probe() -> (
    ToolProbe,
    UnboundedReceiver<String>,
    UnboundedReceiver<String>,
) {
    let (started, started_rx) = mpsc::unbounded_channel();
    let (finished, finished_rx) = mpsc::unbounded_channel();
    (ToolProbe { started, finished }, started_rx, finished_rx)
}

async fn recv_tool_signal(rx: &mut UnboundedReceiver<String>) -> String {
    rx.recv().await.expect("tool signal should be sent")
}

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

fn stopped_assistant(response_id: &str, text: &str) -> AssistantMessage {
    let mut message = AssistantMessage::empty("test", "test-model");
    message.response_id = Some(response_id.into());
    message.content.push(ContentBlock::Text {
        text: text.into(),
        text_signature: None,
    });
    message.stop_reason = StopReason::Stop;
    message
}

fn probed_delayed_tool(
    name: &str,
    delay_ms: u64,
    output_text: &str,
    probe: ToolProbe,
) -> AgentTool {
    let name = name.to_string();
    let text = output_text.to_string();
    let tool_name = name.clone();
    AgentTool {
        name,
        description: format!("delayed {}ms", delay_ms),
        parameters: serde_json::json!({"type": "object"}),
        execution_mode: None,
        execute: Arc::new(move |_, _on_update| {
            let text = text.clone();
            let probe = probe.clone();
            let tool_name = tool_name.clone();
            Box::pin(async move {
                let _ = probe.started.send(tool_name.clone());
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                let _ = probe.finished.send(tool_name);
                Ok(AgentToolOutput::new(vec![ContentBlock::Text {
                    text,
                    text_signature: None,
                }]))
            })
        }),
    }
}

fn two_tool_use_turn(
    first_id: &str,
    first_name: &str,
    first_arguments: serde_json::Value,
    second_id: &str,
    second_name: &str,
    second_arguments: serde_json::Value,
) -> common::ScriptedTurn {
    let mut message = AssistantMessage::empty("test", "test-model");
    message.content = vec![
        ContentBlock::ToolCall {
            id: first_id.into(),
            name: first_name.into(),
            arguments: first_arguments,
            thought_signature: None,
        },
        ContentBlock::ToolCall {
            id: second_id.into(),
            name: second_name.into(),
            arguments: second_arguments,
            thought_signature: None,
        },
    ];

    common::ScriptedTurn {
        events: vec![AssistantMessageEvent::Start {
            content_index: None,
            partial: message,
        }],
        stop_reason: StopReason::ToolUse,
        response_id: "resp_two_tools".into(),
        model_name: "test-model".into(),
    }
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
    let mut config = common::agent_config(common::faux_model("test-api"));
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
    let mut config = common::agent_config(common::faux_model("test-api"));
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
async fn start_turn_node_emits_turn_start_and_enforces_max_turns() {
    let mut config = common::agent_config(common::faux_model("start-turn-node"));
    config.max_turns = Some(1);
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "hello"));

    let mut context = AgentTurnContext::from_agent(&agent);
    let mut flow = Flow::new("start_turn").unwrap();
    flow.add_node("start_turn", StartTurnNode).unwrap();

    let first = flow.run(&mut context).await.unwrap();
    assert_eq!(first.last_action.as_str(), "default");
    assert_eq!(context.turn, 1);
    assert!(matches!(
        context.events.as_slice(),
        [AgentEvent::TurnStart { turn }] if *turn == 1
    ));

    let second = flow.run(&mut context).await.unwrap();
    assert_eq!(second.last_action.as_str(), "error");
    assert_eq!(context.turn, 2);
    assert_eq!(context.max_turns_exceeded, Some(1));
    assert!(context.events.iter().any(|event| matches!(
        event,
        AgentEvent::AgentError { error } if error == "max turns (1) exceeded"
    )));
}

#[tokio::test]
async fn provider_request_node_emits_before_provider_request_after_hook_update() {
    let mut config = common::agent_config(common::faux_model("provider-request-hook"));
    config.stream_options = Some(StreamOptions {
        max_tokens: Some(12),
        ..Default::default()
    });
    config.hooks.before_provider_request = Some(Arc::new(|mut ctx| {
        assert_eq!(ctx.stream_options.max_tokens, Some(12));
        Box::pin(async move {
            ctx.context.messages = vec![Message::User {
                content: vec![ContentBlock::Text {
                    text: "hooked context".into(),
                    text_signature: None,
                }],
            }];
            ctx.stream_options.max_tokens = Some(77);
            Ok(Some(BeforeProviderRequestResult {
                context: Some(ctx.context),
                stream_options: Some(ctx.stream_options),
            }))
        })
    }));
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "original context"));

    let mut context = AgentTurnContext::from_agent(&agent);
    let mut flow = Flow::new("prepare_provider_request").unwrap();
    flow.add_node("prepare_provider_request", PrepareProviderRequestNode)
        .unwrap()
        .add_node(
            "apply_before_provider_request_hook",
            ApplyBeforeProviderRequestHookNode,
        )
        .unwrap()
        .edge(
            "prepare_provider_request",
            "apply_before_provider_request_hook",
        )
        .unwrap();

    let outcome = flow.run(&mut context).await.unwrap();

    assert_eq!(
        outcome.last_node.as_str(),
        "apply_before_provider_request_hook"
    );
    let request = context
        .provider_request
        .as_ref()
        .expect("provider request should be stored");
    assert_eq!(request.stream_options.max_tokens, Some(77));
    assert!(
        request.stream_options.cancel.is_some(),
        "hook-updated stream options must retain cancel token"
    );
    assert!(matches!(
        request.context.messages.as_slice(),
        [Message::User { content }]
            if text_content(content) == "hooked context"
    ));
    assert!(context.events.iter().any(|event| matches!(
        event,
        AgentEvent::BeforeProviderRequest { request }
            if request.stream_options.max_tokens == Some(77)
                && request.stream_options.cancel.is_some()
                && matches!(
                    request.context.messages.as_slice(),
                    [Message::User { content }] if text_content(content) == "hooked context"
                )
    )));
}

#[tokio::test]
async fn provider_request_node_applies_override_once_and_preserves_cancel() {
    let mut config = common::agent_config(common::faux_model("provider-request-override"));
    config.stream_options = Some(StreamOptions {
        max_tokens: Some(12),
        ..Default::default()
    });
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "original context"));
    agent.set_provider_request_override(
        Context {
            system_prompt: Some("override system".into()),
            messages: vec![Message::User {
                content: vec![ContentBlock::Text {
                    text: "override context".into(),
                    text_signature: None,
                }],
            }],
            tools: None,
        },
        Some(StreamOptions {
            max_tokens: Some(88),
            ..Default::default()
        }),
    );

    let mut context = AgentTurnContext::from_agent(&agent);
    let mut flow = Flow::new("prepare_provider_request").unwrap();
    flow.add_node("prepare_provider_request", PrepareProviderRequestNode)
        .unwrap();

    flow.run(&mut context).await.unwrap();
    let first = context
        .provider_request
        .clone()
        .expect("override request should be prepared");

    assert_eq!(
        first.context.system_prompt.as_deref(),
        Some("override system")
    );
    assert!(matches!(
        first.context.messages.as_slice(),
        [Message::User { content }] if text_content(content) == "override context"
    ));
    assert_eq!(first.stream_options.max_tokens, Some(88));
    assert!(
        first.stream_options.cancel.is_some(),
        "override stream options must retain cancel token"
    );

    flow.run(&mut context).await.unwrap();
    let second = context
        .provider_request
        .as_ref()
        .expect("second request should be prepared");

    assert_ne!(
        second.context.system_prompt.as_deref(),
        Some("override system")
    );
    assert!(matches!(
        second.context.messages.as_slice(),
        [Message::User { content }] if text_content(content) == "original context"
    ));
    assert_eq!(second.stream_options.max_tokens, Some(12));
    assert!(second.stream_options.cancel.is_some());
}

#[tokio::test]
async fn maybe_prepare_next_turn_drains_follow_up_before_done() {
    let prepare_calls = Arc::new(AtomicUsize::new(0));
    let prepare_calls_for_hook = prepare_calls.clone();
    let mut config = common::agent_config(common::faux_model("maybe-prepare-next-turn"));
    config.follow_up_mode = QueueMode::OneAtATime;
    config.hooks.prepare_next_turn = Some(Arc::new(move |ctx| {
        prepare_calls_for_hook.fetch_add(1, Ordering::SeqCst);
        assert_eq!(ctx.turn, 1);
        Box::pin(async move {
            Ok(Some(AgentLoopTurnUpdate {
                messages: Some(vec![AgentMessage::UserText {
                    message_id: "prepared".into(),
                    text: "prepared context".into(),
                }]),
                ..Default::default()
            }))
        })
    }));
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "hello"));
    agent.follow_up("follow up one");

    let mut context = AgentTurnContext::from_agent(&agent);
    context.turn = 1;
    context.assistant_message = Some(stopped_assistant("resp_1", "first answer"));

    let mut flow = Flow::new("decide_after_assistant").unwrap();
    flow.add_node("decide_after_assistant", DecideAfterAssistantNode)
        .unwrap()
        .add_node("maybe_prepare_next_turn", MaybePrepareNextTurnNode)
        .unwrap()
        .edge_on(
            "decide_after_assistant",
            Action::new("continue").unwrap(),
            "maybe_prepare_next_turn",
        )
        .unwrap();

    let outcome = flow.run(&mut context).await.unwrap();

    assert_eq!(outcome.last_action.as_str(), "continue");
    assert_eq!(prepare_calls.load(Ordering::SeqCst), 1);
    assert!(context.has_more_queued_input);
    assert!(!context.should_finish);
    assert!(context.follow_up_queue.is_empty());
    assert!(context.messages.iter().any(|message| {
        matches!(message, AgentMessage::UserText { text, .. } if text == "prepared context")
    }));
    assert!(context.messages.iter().any(|message| {
        matches!(message, AgentMessage::UserText { text, .. } if text == "follow up one")
    }));
    assert!(
        !context
            .events
            .iter()
            .any(|event| matches!(event, AgentEvent::AgentDone { .. }))
    );
}

#[tokio::test]
async fn agent_run_uses_configured_provider_streamer_without_global_registry() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_streamer = calls.clone();
    let mut config = common::agent_config(common::faux_model("scoped-provider-streamer-only"));
    config.provider_streamer = Some(Arc::new(move |model, _context, _opts| {
        assert_eq!(model.api, "scoped-provider-streamer-only");
        calls_for_streamer.fetch_add(1, Ordering::SeqCst);
        let model_id = model.id.clone();
        Box::pin(async_stream::stream! {
            let mut message = AssistantMessage::empty("scoped", &model_id);
            message.content.push(ContentBlock::Text {
                text: "streamed through scoped provider".into(),
                text_signature: None,
            });
            message.stop_reason = StopReason::Stop;
            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message,
            };
        })
    }));

    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "hello"));

    let mut stream = agent.run().expect("agent should run with a user message");
    let mut done_text = None;
    while let Some(event) = stream.next().await {
        if let AgentEvent::AgentDone { message } = event {
            done_text = Some(text_content(&message.content));
            break;
        }
    }

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        done_text.as_deref(),
        Some("streamed through scoped provider")
    );
}

#[tokio::test]
async fn runtime_compaction_node_uses_configured_provider_streamer_without_global_registry() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_streamer = calls.clone();
    let mut config = common::agent_config(common::faux_model_with_window(
        "runtime-compaction-scoped-provider-streamer-only",
        100,
    ));
    config.compaction = Some(CompactionConfig {
        settings: CompactionSettings {
            enabled: true,
            reserve_tokens: 0,
            keep_recent_tokens: 8,
        },
        custom_instructions: None,
    });
    config.provider_streamer = Some(Arc::new(move |model, _context, _opts| {
        assert_eq!(
            model.api,
            "runtime-compaction-scoped-provider-streamer-only"
        );
        calls_for_streamer.fetch_add(1, Ordering::SeqCst);
        let model_id = model.id.clone();
        Box::pin(async_stream::stream! {
            let mut message = AssistantMessage::empty("scoped", &model_id);
            message.content.push(ContentBlock::Text {
                text: "summary through scoped compaction streamer".into(),
                text_signature: None,
            });
            message.stop_reason = StopReason::Stop;
            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message,
            };
        })
    }));

    let agent = Agent::new(config);
    agent.add_message(user_msg("old_1", &"old context ".repeat(40)));
    agent.add_message(user_msg("old_2", &"more old context ".repeat(40)));

    let mut context = AgentTurnContext::from_agent(&agent);
    let mut flow = Flow::new("maybe_compact_runtime_context").unwrap();
    flow.add_node(
        "maybe_compact_runtime_context",
        MaybeCompactRuntimeContextNode,
    )
    .unwrap();

    let outcome = flow.run(&mut context).await.unwrap();

    assert_eq!(outcome.last_node.as_str(), "maybe_compact_runtime_context");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        context.runtime_compaction.summary.as_deref(),
        Some("summary through scoped compaction streamer")
    );
}

#[tokio::test]
async fn runtime_compaction_node_summarizes_and_updates_context_messages() {
    let api = "agent-turn-flow-runtime-compaction";
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("summary of old context", StopReason::Stop),
        ])),
    );
    let mut config = _provider_guard.agent_config(common::faux_model_with_window(api, 100));
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
}

#[tokio::test]
async fn execute_tools_node_runs_sequential_tool_and_appends_result_message() {
    let api = "agent-turn-flow-execute-tool";
    let mut config = common::agent_config(common::faux_model(api));
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(common::TestProvider::new(vec![common::tool_use_turn(
            "call_1",
            "echo",
            serde_json::json!({"text": "hello"}),
        )])),
    );
    _provider_guard.install(&mut config);
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
}

#[tokio::test]
async fn execute_tools_node_honors_before_hook_block() {
    let api = "agent-turn-flow-execute-before-hook";
    let calls = Arc::new(AtomicUsize::new(0));
    let mut config = common::agent_config(common::faux_model(api));
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(common::TestProvider::new(vec![common::tool_use_turn(
            "call_1",
            "echo",
            serde_json::json!({}),
        )])),
    );
    _provider_guard.install(&mut config);
    config.tool_execution = ToolExecutionMode::Sequential;
    config.hooks.before_tool_call = Some(Arc::new(|ctx| {
        assert_eq!(ctx.tool_name, "echo");
        Box::pin(async move {
            Ok(Some(BeforeToolCallResult {
                block: true,
                reason: Some("blocked by flow hook".into()),
            }))
        })
    }));
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "use the tool"));
    let calls_for_tool = calls.clone();
    agent.add_tool(AgentTool {
        name: "echo".into(),
        description: "echo input".into(),
        parameters: serde_json::json!({"type": "object"}),
        execution_mode: None,
        execute: Arc::new(move |_, _on_update| {
            calls_for_tool.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                Ok(AgentToolOutput::new(vec![ContentBlock::Text {
                    text: "executed".into(),
                    text_signature: None,
                }]))
            })
        }),
    });

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
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(context.tool_results.len(), 1);
    assert!(context.tool_results[0].is_error);
    assert_eq!(
        text_content(&context.tool_results[0].content),
        "blocked by flow hook"
    );
    assert!(context.messages.iter().any(|message| matches!(
        message,
        AgentMessage::ToolResult { tool_call_id, is_error: true, content, .. }
            if tool_call_id == "call_1" && text_content(content) == "blocked by flow hook"
    )));
}

#[tokio::test]
async fn execute_tools_node_honors_after_hook_result_update() {
    let api = "agent-turn-flow-execute-after-hook";
    let mut config = common::agent_config(common::faux_model(api));
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(common::TestProvider::new(vec![common::tool_use_turn(
            "call_1",
            "echo",
            serde_json::json!({}),
        )])),
    );
    _provider_guard.install(&mut config);
    config.tool_execution = ToolExecutionMode::Sequential;
    config.hooks.after_tool_call = Some(Arc::new(|ctx| {
        assert_eq!(ctx.tool_name, "echo");
        assert!(!ctx.result.is_error);
        assert_eq!(text_content(&ctx.result.content), "original");
        Box::pin(async move {
            Ok(Some(AfterToolCallResult {
                content: Some(vec![ContentBlock::Text {
                    text: "rewritten by flow hook".into(),
                    text_signature: None,
                }]),
                is_error: Some(true),
                terminate: Some(false),
            }))
        })
    }));
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "use the tool"));
    agent.add_tool(AgentTool::new_text(
        "echo",
        "echo input",
        serde_json::json!({"type": "object"}),
        |_| async { Ok("original".to_string()) },
    ));

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
        "rewritten by flow hook"
    );
    assert!(context.events.iter().any(|event| matches!(
        event,
        AgentEvent::ToolCallEnd { result, .. }
            if result.is_error && text_content(&result.content) == "rewritten by flow hook"
    )));
    assert!(context.messages.iter().any(|message| matches!(
        message,
        AgentMessage::ToolResult { is_error: true, content, .. }
            if text_content(content) == "rewritten by flow hook"
    )));
}

#[tokio::test]
async fn execute_tools_node_emits_tool_update_events_before_end() {
    let api = "agent-turn-flow-execute-tool-update";
    let mut config = common::agent_config(common::faux_model(api));
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(common::TestProvider::new(vec![common::tool_use_turn(
            "call_1",
            "echo",
            serde_json::json!({}),
        )])),
    );
    _provider_guard.install(&mut config);
    config.tool_execution = ToolExecutionMode::Sequential;
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "use the tool"));
    agent.add_tool(AgentTool {
        name: "echo".into(),
        description: "echo input".into(),
        parameters: serde_json::json!({"type": "object"}),
        execution_mode: None,
        execute: Arc::new(|_, on_update| {
            Box::pin(async move {
                if let Some(on_update) = on_update {
                    on_update(AgentToolOutput::new(vec![ContentBlock::Text {
                        text: "progress".into(),
                        text_signature: None,
                    }]));
                }
                Ok(AgentToolOutput::new(vec![ContentBlock::Text {
                    text: "final".into(),
                    text_signature: None,
                }]))
            })
        }),
    });

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

    flow.run(&mut context).await.unwrap();

    let tool_events: Vec<_> = context
        .events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolCallUpdate { update, .. } => {
                Some(("update", text_content(&update.content)))
            }
            AgentEvent::ToolCallEnd { result, .. } => Some(("end", text_content(&result.content))),
            _ => None,
        })
        .collect();
    assert_eq!(
        tool_events,
        vec![
            ("update", "progress".to_string()),
            ("end", "final".to_string())
        ]
    );
}

#[tokio::test]
async fn execute_tools_node_finishes_when_all_tool_results_terminate() {
    let api = "agent-turn-flow-execute-terminate";
    let should_stop_calls = Arc::new(AtomicUsize::new(0));
    let should_stop_calls_for_hook = should_stop_calls.clone();
    let prepare_calls = Arc::new(AtomicUsize::new(0));
    let prepare_calls_for_hook = prepare_calls.clone();
    let mut config = common::agent_config(common::faux_model(api));
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(common::TestProvider::new(vec![common::tool_use_turn(
            "call_1",
            "echo",
            serde_json::json!({}),
        )])),
    );
    _provider_guard.install(&mut config);
    config.tool_execution = ToolExecutionMode::Sequential;
    config.hooks.after_tool_call = Some(Arc::new(|_| {
        Box::pin(async move {
            Ok(Some(AfterToolCallResult {
                content: None,
                is_error: None,
                terminate: Some(true),
            }))
        })
    }));
    config.hooks.should_stop_after_turn = Some(Arc::new(move |_| {
        should_stop_calls_for_hook.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { Ok(false) })
    }));
    config.hooks.prepare_next_turn = Some(Arc::new(move |_| {
        prepare_calls_for_hook.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { Ok(None) })
    }));
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "use the tool"));
    agent.add_tool(AgentTool::new_text(
        "echo",
        "echo input",
        serde_json::json!({"type": "object"}),
        |_| async { Ok("ok".to_string()) },
    ));

    let mut context = AgentTurnContext::from_agent(&agent);
    let mut flow = Flow::new("prepare_context").unwrap();
    flow.add_node("prepare_context", PrepareContextNode)
        .unwrap()
        .add_node("provider_stream", ProviderStreamNode)
        .unwrap()
        .add_node("decide_after_assistant", DecideAfterAssistantNode)
        .unwrap()
        .add_node("execute_tools", ExecuteToolsNode)
        .unwrap()
        .add_node("maybe_prepare_next_turn", MaybePrepareNextTurnNode)
        .unwrap()
        .edge("prepare_context", "provider_stream")
        .unwrap()
        .edge("provider_stream", "decide_after_assistant")
        .unwrap()
        .edge_on(
            "decide_after_assistant",
            Action::new("tools").unwrap(),
            "execute_tools",
        )
        .unwrap()
        .edge_on(
            "execute_tools",
            Action::new("continue").unwrap(),
            "maybe_prepare_next_turn",
        )
        .unwrap();

    let outcome = flow.run(&mut context).await.unwrap();

    assert_eq!(outcome.last_action.as_str(), "done");
    assert_eq!(outcome.last_node.as_str(), "maybe_prepare_next_turn");
    assert_eq!(should_stop_calls.load(Ordering::SeqCst), 1);
    assert_eq!(prepare_calls.load(Ordering::SeqCst), 0);
    assert!(context.tool_results.iter().all(|result| result.terminate));
    assert!(
        context
            .events
            .iter()
            .any(|event| matches!(event, AgentEvent::AgentDone { .. }))
    );
}

#[tokio::test(start_paused = true)]
async fn execute_tools_node_runs_parallel_tools_and_appends_results_in_assistant_order() {
    let api = "agent-turn-flow-execute-parallel-tools";
    let mut config = common::agent_config(common::faux_model(api));
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(common::TestProvider::new(vec![two_tool_use_turn(
            "call_slow",
            "slow",
            serde_json::json!({}),
            "call_fast",
            "fast",
            serde_json::json!({}),
        )])),
    );
    _provider_guard.install(&mut config);
    config.tool_execution = ToolExecutionMode::Parallel;
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "use both tools"));
    let (probe, mut started, mut finished) = tool_probe();
    agent.add_tool(probed_delayed_tool(
        "slow",
        AGENT_TURN_FLOW_SLOW_TOOL_DELAY_MS,
        "slow_result",
        probe.clone(),
    ));
    agent.add_tool(probed_delayed_tool(
        "fast",
        AGENT_TURN_FLOW_FAST_TOOL_DELAY_MS,
        "fast_result",
        probe,
    ));

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

    let flow_task = tokio::spawn(async move {
        let outcome = flow.run(&mut context).await.unwrap();
        (outcome, context)
    });
    let mut started_tools = vec![
        recv_tool_signal(&mut started).await,
        recv_tool_signal(&mut started).await,
    ];
    started_tools.sort();
    assert_eq!(started_tools, vec!["fast", "slow"]);

    tokio::time::advance(AGENT_TURN_FLOW_FAST_TOOL_ADVANCE).await;
    assert_eq!(recv_tool_signal(&mut finished).await, "fast");
    tokio::time::advance(AGENT_TURN_FLOW_REMAINING_TOOL_ADVANCE).await;
    assert_eq!(recv_tool_signal(&mut finished).await, "slow");

    let (outcome, context) = flow_task.await.unwrap();

    assert_eq!(outcome.last_action.as_str(), "continue");
    let end_events: Vec<_> = context
        .events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolCallEnd { tool_name, .. } => Some(tool_name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(end_events, vec!["fast", "slow"]);

    let result_messages: Vec<_> = context
        .messages
        .iter()
        .filter_map(|message| match message {
            AgentMessage::ToolResult {
                tool_name, content, ..
            } => Some((tool_name.as_str(), text_content(content))),
            _ => None,
        })
        .collect();
    assert_eq!(
        result_messages,
        vec![
            ("slow", "slow_result".to_string()),
            ("fast", "fast_result".to_string())
        ]
    );
}

#[tokio::test]
async fn execute_tools_node_records_unknown_tool_as_error_result() {
    let api = "agent-turn-flow-execute-unknown-tool";
    let mut config = common::agent_config(common::faux_model(api));
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(common::TestProvider::new(vec![common::tool_use_turn(
            "call_1",
            "missing_tool",
            serde_json::json!({}),
        )])),
    );
    _provider_guard.install(&mut config);
    config.tool_execution = ToolExecutionMode::Sequential;
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "use the tool"));

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
}

#[tokio::test]
async fn provider_stream_node_records_llm_events_and_final_assistant_message() {
    let api = "agent-turn-flow-provider-stream";
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("final answer", StopReason::Stop),
        ])),
    );
    let config = _provider_guard.agent_config(common::faux_model(api));
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "hello"));

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
}

#[tokio::test]
async fn provider_stream_node_maps_missing_done_to_agent_error_event() {
    let api = "agent-turn-flow-provider-error";
    let _provider_guard = ProviderGuard::register(api, Arc::new(common::TestProvider::new(vec![])));
    let config = _provider_guard.agent_config(common::faux_model(api));
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "hello"));

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
}

#[tokio::test]
async fn decide_node_finishes_text_response_and_appends_assistant_message() {
    let api = "agent-turn-flow-decide-done";
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("final answer", StopReason::Stop),
        ])),
    );
    let config = _provider_guard.agent_config(common::faux_model(api));
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "hello"));

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
}

#[tokio::test]
async fn decide_node_extracts_tool_calls_and_returns_tools_action() {
    let api = "agent-turn-flow-decide-tools";
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(common::TestProvider::new(vec![common::tool_use_turn(
            "call_1",
            "echo",
            serde_json::json!({"text": "hello"}),
        )])),
    );
    let config = _provider_guard.agent_config(common::faux_model(api));
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "use the tool"));

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
}

#[tokio::test]
async fn decide_after_assistant_tool_use_without_calls_routes_to_tools() {
    let should_stop_calls = Arc::new(AtomicUsize::new(0));
    let should_stop_calls_for_hook = should_stop_calls.clone();
    let prepare_calls = Arc::new(AtomicUsize::new(0));
    let prepare_calls_for_hook = prepare_calls.clone();
    let mut config =
        common::agent_config(common::faux_model("agent-turn-flow-decide-empty-tool-use"));
    config.hooks.should_stop_after_turn = Some(Arc::new(move |_| {
        should_stop_calls_for_hook.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { Ok(false) })
    }));
    config.hooks.prepare_next_turn = Some(Arc::new(move |_| {
        prepare_calls_for_hook.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { Ok(None) })
    }));
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "use a tool"));

    let mut assistant = AssistantMessage::empty("test", "test-model");
    assistant.response_id = Some("resp_empty_tool_use".into());
    assistant.stop_reason = StopReason::ToolUse;

    let mut context = AgentTurnContext::from_agent(&agent);
    context.assistant_message = Some(assistant);

    let mut flow = Flow::new("decide_after_assistant").unwrap();
    flow.add_node("decide_after_assistant", DecideAfterAssistantNode)
        .unwrap()
        .add_node("execute_tools", ExecuteToolsNode)
        .unwrap()
        .add_node("maybe_prepare_next_turn", MaybePrepareNextTurnNode)
        .unwrap()
        .add_node(
            "drain_queued_input",
            pi_agent_core::agent_turn_flow::DrainQueuedInputNode,
        )
        .unwrap()
        .edge_on(
            "decide_after_assistant",
            Action::new("tools").unwrap(),
            "execute_tools",
        )
        .unwrap()
        .edge_on(
            "execute_tools",
            Action::new("continue").unwrap(),
            "maybe_prepare_next_turn",
        )
        .unwrap()
        .edge_on(
            "execute_tools",
            Action::new("continue_provider").unwrap(),
            "drain_queued_input",
        )
        .unwrap();

    let outcome = flow.run(&mut context).await.unwrap();

    assert_eq!(outcome.last_node.as_str(), "drain_queued_input");
    assert!(context.pending_tool_calls.is_empty());
    assert_eq!(should_stop_calls.load(Ordering::SeqCst), 0);
    assert_eq!(prepare_calls.load(Ordering::SeqCst), 0);
}
