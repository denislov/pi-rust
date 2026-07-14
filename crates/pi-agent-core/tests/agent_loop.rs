mod common;
use common::{ProviderGuard, ScriptedTurn, TestProvider, text_turn, tool_use_turn};
use futures::StreamExt;
use pi_agent_core::{
    Agent, AgentConfig, AgentEvent, AgentMessage, AgentTool, AgentToolOutput, ToolExecutionMode,
};
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Message, Model, ModelCost,
    ModelInput, StopReason,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

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
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
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
        max_turns: Some(5),
        stream_options: None,
        ..common::agent_config(test_model(api_key))
    }
}

fn done_text_message(response_id: &str, text: &str) -> AssistantMessage {
    let mut message = AssistantMessage::empty("test", "test-model");
    message.response_id = Some(response_id.into());
    message.content.push(ContentBlock::Text {
        text: text.into(),
        text_signature: None,
    });
    message.stop_reason = StopReason::Stop;
    message
}

fn tool_use_message(response_id: &str, tool_id: &str, tool_name: &str) -> AssistantMessage {
    let mut message = AssistantMessage::empty("test", "test-model");
    message.response_id = Some(response_id.into());
    message.content.push(ContentBlock::ToolCall {
        id: tool_id.into(),
        name: tool_name.into(),
        arguments: serde_json::json!({}),
        thought_signature: None,
    });
    message.stop_reason = StopReason::ToolUse;
    message
}

fn context_contains_user_text(context: &Context, expected: &str) -> bool {
    context.messages.iter().any(|message| {
        matches!(
            message,
            Message::User { content }
                if content.iter().any(|block| {
                    matches!(block, ContentBlock::Text { text, .. } if text == expected)
                })
        )
    })
}

#[tokio::test]
async fn single_turn_text_response() {
    let api_key = "test-api-1";
    let provider = Arc::new(TestProvider::new(vec![text_turn("Hello, world!")]));
    let _provider_guard = ProviderGuard::register(api_key, provider);

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
}

#[tokio::test]
async fn llm_events_stream_before_provider_done() {
    let (release_tx, release_rx) = oneshot::channel::<()>();
    let release_rx = Arc::new(Mutex::new(Some(release_rx)));
    let mut config = test_config("live-stream-provider");
    config.provider_streamer = Some(Arc::new(move |_model, _context, _opts| {
        let release_rx = release_rx.clone();
        Box::pin(async_stream::stream! {
            let mut partial = AssistantMessage::empty("test", "test-model");
            partial.content.push(ContentBlock::Text {
                text: "partial".into(),
                text_signature: None,
            });
            yield AssistantMessageEvent::TextDelta {
                content_index: 0,
                delta: "partial".into(),
                partial: partial.clone(),
            };
            let release_rx = {
                release_rx
                    .lock()
                    .unwrap()
                    .take()
                    .expect("release receiver should be available")
            };
            let _ = release_rx.await;
            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message: done_text_message("resp_live_stream", "done"),
            };
        })
    }));

    let agent = Agent::new(config);
    let mut stream = agent.prompt("hi");

    tokio::time::timeout(Duration::from_millis(200), async {
        while let Some(event) = stream.next().await {
            if matches!(
                event,
                AgentEvent::LlmEvent(AssistantMessageEvent::TextDelta { delta, .. })
                    if delta == "partial"
            ) {
                return;
            }
        }
        panic!("stream ended before partial LLM event");
    })
    .await
    .expect("partial LLM event should arrive before provider completes");

    release_tx.send(()).unwrap();
    while stream.next().await.is_some() {}
}

#[tokio::test]
async fn follow_up_queued_during_provider_turn_is_not_lost_and_continues() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_streamer = calls.clone();
    let (started_tx, mut started_rx) = mpsc::unbounded_channel::<()>();
    let (release_tx, release_rx) = oneshot::channel::<()>();
    let release_rx = Arc::new(Mutex::new(Some(release_rx)));
    let mut config = test_config("live-follow-up-provider");
    config.provider_streamer = Some(Arc::new(move |_model, context, _opts| {
        let call = calls_for_streamer.fetch_add(1, Ordering::SeqCst) + 1;
        let started_tx = started_tx.clone();
        let release_rx = release_rx.clone();
        Box::pin(async_stream::stream! {
            if call == 1 {
                let _ = started_tx.send(());
                let release_rx = {
                    release_rx
                        .lock()
                        .unwrap()
                        .take()
                        .expect("release receiver should be available")
                };
                let _ = release_rx.await;
                yield AssistantMessageEvent::Done {
                    reason: StopReason::Stop,
                    message: done_text_message("resp_first", "first"),
                };
            } else {
                assert!(
                    context_contains_user_text(&context, "queued during provider"),
                    "follow-up queued while provider awaited should reach the next provider call"
                );
                yield AssistantMessageEvent::Done {
                    reason: StopReason::Stop,
                    message: done_text_message("resp_second", "second"),
                };
            }
        })
    }));

    let agent = Agent::new(config);
    let collect_task = {
        let stream = agent.prompt("first");
        tokio::spawn(async move { stream.collect::<Vec<_>>().await })
    };

    started_rx
        .recv()
        .await
        .expect("first provider call should start");
    agent.follow_up("queued during provider");
    release_tx.send(()).unwrap();

    let events = collect_task.await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(events.iter().any(|event| {
        matches!(
            event,
            AgentEvent::AgentDone { message }
                if message.content.iter().any(|block| {
                    matches!(block, ContentBlock::Text { text, .. } if text == "second")
                })
        )
    }));
    assert!(agent.messages().iter().any(|message| {
        matches!(message, AgentMessage::UserText { text, .. } if text == "queued during provider")
    }));
}

#[tokio::test]
async fn steer_queued_during_tool_turn_is_not_lost_before_next_provider_call() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_streamer = calls.clone();
    let saw_steer = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let saw_steer_for_streamer = saw_steer.clone();
    let mut config = test_config("live-steer-tool");
    config.provider_streamer = Some(Arc::new(move |_model, context, _opts| {
        let call = calls_for_streamer.fetch_add(1, Ordering::SeqCst) + 1;
        let saw_steer_for_streamer = saw_steer_for_streamer.clone();
        Box::pin(async_stream::stream! {
            if call == 1 {
                yield AssistantMessageEvent::Done {
                    reason: StopReason::ToolUse,
                    message: tool_use_message("resp_tool", "tool_1", "blocking"),
                };
            } else {
                if context_contains_user_text(&context, "steered during tool") {
                    saw_steer_for_streamer.store(true, Ordering::SeqCst);
                }
                yield AssistantMessageEvent::Done {
                    reason: StopReason::Stop,
                    message: done_text_message("resp_done", "done"),
                };
            }
        })
    }));

    let agent = Agent::new(config);
    let (tool_started_tx, mut tool_started_rx) = mpsc::unbounded_channel::<()>();
    let (tool_release_tx, tool_release_rx) = oneshot::channel::<()>();
    let tool_release_rx = Arc::new(Mutex::new(Some(tool_release_rx)));
    agent.add_tool(AgentTool {
        name: "blocking".into(),
        description: "blocks until released".into(),
        parameters: serde_json::json!({"type": "object"}),
        execution_mode: None,
        execute: Arc::new(move |_, _on_update| {
            let tool_started_tx = tool_started_tx.clone();
            let tool_release_rx = tool_release_rx.clone();
            Box::pin(async move {
                let _ = tool_started_tx.send(());
                let release_rx = {
                    tool_release_rx
                        .lock()
                        .unwrap()
                        .take()
                        .expect("tool release receiver should be available")
                };
                let _ = release_rx.await;
                Ok(AgentToolOutput::new(vec![ContentBlock::Text {
                    text: "tool done".into(),
                    text_signature: None,
                }]))
            })
        }),
    });

    let collect_task = {
        let stream = agent.prompt("use tool");
        tokio::spawn(async move { stream.collect::<Vec<_>>().await })
    };

    tool_started_rx.recv().await.expect("tool should start");
    agent.steer("steered during tool");
    tool_release_tx.send(()).unwrap();
    let _events = collect_task.await.unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(
        saw_steer.load(Ordering::SeqCst),
        "steer queued while tool awaited should reach the next provider call"
    );
}

#[tokio::test]
async fn provider_override_set_during_inflight_turn_survives_current_writeback() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_streamer = calls.clone();
    let (started_tx, mut started_rx) = mpsc::unbounded_channel::<()>();
    let (release_tx, release_rx) = oneshot::channel::<()>();
    let release_rx = Arc::new(Mutex::new(Some(release_rx)));
    let observed_system_prompts = Arc::new(Mutex::new(Vec::new()));
    let observed_for_streamer = observed_system_prompts.clone();
    let mut config = test_config("override-race-provider");
    config.system_prompt = None;
    config.provider_streamer = Some(Arc::new(move |_model, context, _opts| {
        let call = calls_for_streamer.fetch_add(1, Ordering::SeqCst) + 1;
        observed_for_streamer
            .lock()
            .unwrap()
            .push(context.system_prompt.clone());
        let started_tx = started_tx.clone();
        let release_rx = release_rx.clone();
        Box::pin(async_stream::stream! {
            if call == 1 {
                let _ = started_tx.send(());
                let release_rx = {
                    release_rx
                        .lock()
                        .unwrap()
                        .take()
                        .expect("release receiver should be available")
                };
                let _ = release_rx.await;
            }
            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message: done_text_message(&format!("resp_{call}"), "done"),
            };
        })
    }));

    let agent = Agent::new(config);
    agent.add_message(AgentMessage::UserText {
        message_id: "user_0".into(),
        text: "first".into(),
    });
    agent.set_provider_request_override(
        Context {
            system_prompt: Some("initial override".into()),
            messages: vec![],
            tools: None,
        },
        None,
    );

    let collect_task = {
        let stream = agent.run().expect("agent should run");
        tokio::spawn(async move { stream.collect::<Vec<_>>().await })
    };
    started_rx
        .recv()
        .await
        .expect("first provider call should start");
    agent.set_provider_request_override(
        Context {
            system_prompt: Some("new override".into()),
            messages: vec![],
            tools: None,
        },
        None,
    );
    release_tx.send(()).unwrap();
    let _ = collect_task.await.unwrap();

    let second_events: Vec<_> = agent.prompt("second").collect().await;
    assert!(
        second_events
            .iter()
            .any(|event| matches!(event, AgentEvent::AgentDone { .. }))
    );

    let observed = observed_system_prompts.lock().unwrap().clone();
    assert_eq!(observed.first(), Some(&Some("initial override".into())));
    assert_eq!(observed.get(1), Some(&Some("new override".into())));
}

#[tokio::test]
async fn tool_use_turn_executes_tool() {
    let api_key = "test-api-2";
    let provider = Arc::new(TestProvider::new(vec![
        tool_use_turn("tool_1", "echo", serde_json::json!({"text": "hi"})),
        text_turn("Tool executed successfully."),
    ]));
    let _provider_guard = ProviderGuard::register(api_key, provider);

    let agent = Agent::new(test_config(api_key));

    let tool = AgentTool {
        name: "echo".into(),
        description: "echoes input".into(),
        parameters: serde_json::json!({"type": "object", "properties": {"text": {"type": "string"}}}),
        execution_mode: None,
        execute: Arc::new(|args, _on_update| {
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("no text");
            let result = vec![ContentBlock::Text {
                text: format!("echo: {}", text),
                text_signature: None,
            }];
            Box::pin(async move { Ok(AgentToolOutput::new(result)) })
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
}

#[tokio::test(start_paused = true)]
async fn tool_update_events_stream_before_tool_end() {
    let api_key = "test-api-tool-updates";
    let provider = Arc::new(TestProvider::new(vec![
        tool_use_turn("tool_1", "streaming", serde_json::json!({})),
        text_turn("done"),
    ]));
    let _provider_guard = ProviderGuard::register(api_key, provider);

    let mut config = test_config(api_key);
    config.tool_execution = ToolExecutionMode::Sequential;
    let agent = Agent::new(config);
    agent.add_tool(AgentTool {
        name: "streaming".into(),
        description: "streams updates".into(),
        parameters: serde_json::json!({"type": "object"}),
        execution_mode: None,
        execute: Arc::new(|_, on_update| {
            Box::pin(async move {
                if let Some(on_update) = on_update {
                    on_update(AgentToolOutput::new(vec![ContentBlock::Text {
                        text: "partial".into(),
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

    let events: Vec<_> = agent.prompt("go").collect().await;
    let update_index = events
        .iter()
        .position(|event| {
            matches!(
                event,
                AgentEvent::ToolCallUpdate {
                    tool_call_id,
                    update,
                    ..
                } if tool_call_id == "tool_1"
                    && matches!(
                        update.content.first(),
                        Some(ContentBlock::Text { text, .. }) if text == "partial"
                    )
            )
        })
        .expect("expected tool update event");
    let end_index = events
        .iter()
        .position(|event| matches!(event, AgentEvent::ToolCallEnd { tool_call_id, .. } if tool_call_id == "tool_1"))
        .expect("expected tool end event");

    assert!(update_index < end_index);
}

#[tokio::test]
async fn unknown_tool_yields_error_content_and_continues() {
    let api_key = "test-api-3";
    let provider = Arc::new(TestProvider::new(vec![
        tool_use_turn("tool_1", "nonexistent", serde_json::json!({})),
        text_turn("I tried but the tool was not found."),
    ]));
    let _provider_guard = ProviderGuard::register(api_key, provider);

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
    assert!(tool_end.is_error);
    assert!(
        tool_end
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::Text { text, .. } if text.contains("unknown tool")))
    );

    let has_done = events
        .iter()
        .any(|e| matches!(e, AgentEvent::AgentDone { .. }));
    assert!(has_done);
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
    let _provider_guard = ProviderGuard::register(api_key, provider);

    let mut config = test_config(api_key);
    config.max_turns = Some(2);

    let agent = Agent::new(config);
    let tool = AgentTool {
        name: "echo".into(),
        description: "echo".into(),
        parameters: serde_json::json!({"type": "object"}),
        execution_mode: None,
        execute: Arc::new(|_, _on_update| {
            Box::pin(async {
                Ok(AgentToolOutput::new(vec![ContentBlock::Text {
                    text: "ok".into(),
                    text_signature: None,
                }]))
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
}

#[tokio::test]
async fn unlimited_max_turns_runs_to_natural_completion() {
    // Parity check with TS `pi/packages/agent`: when `max_turns` is `None`,
    // the loop must keep running until the model stops producing tool calls
    // (or another stop condition fires), with no hard turn ceiling.
    let api_key = "test-api-no-cap";
    let mut turns = Vec::new();
    for _ in 0..40 {
        turns.push(tool_use_turn(
            "tool_1",
            "echo",
            serde_json::json!({"text": "x"}),
        ));
    }
    turns.push(text_turn("Done after many tool calls."));
    let provider = Arc::new(TestProvider::new(turns));
    let _provider_guard = ProviderGuard::register(api_key, provider);

    let mut config = test_config(api_key);
    config.max_turns = None;

    let agent = Agent::new(config);
    let tool = AgentTool {
        name: "echo".into(),
        description: "echo".into(),
        parameters: serde_json::json!({"type": "object"}),
        execution_mode: None,
        execute: Arc::new(|_, _on_update| {
            Box::pin(async {
                Ok(AgentToolOutput::new(vec![ContentBlock::Text {
                    text: "ok".into(),
                    text_signature: None,
                }]))
            })
        }),
    };
    agent.add_tool(tool);

    let stream = agent.prompt("go");
    let events: Vec<_> = stream.collect().await;

    let has_done = events
        .iter()
        .any(|e| matches!(e, AgentEvent::AgentDone { .. }));
    assert!(
        has_done,
        "AgentDone should be emitted; events without a turn cap should not be aborted"
    );
    let has_max_turns_error = events
        .iter()
        .any(|e| matches!(e, AgentEvent::AgentError { error } if error.contains("max turns")));
    assert!(
        !has_max_turns_error,
        "max_turns: None must not yield a max-turns error"
    );
}

#[tokio::test]
async fn abort_mid_turn_yields_error() {
    let api_key = "test-api-5";
    let provider = Arc::new(TestProvider::new(vec![text_turn("Hello")]));
    let _provider_guard = ProviderGuard::register(api_key, provider);

    let agent = Agent::new(test_config(api_key));

    let stream = agent.prompt("hi");
    agent.abort();

    let events: Vec<_> = stream.collect().await;
    let has_abort_error = events
        .iter()
        .any(|e| matches!(e, AgentEvent::AgentError { error } if error.contains("aborted")));
    assert!(has_abort_error, "should have aborted error");
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
    let _provider_guard = ProviderGuard::register(api_key, provider);

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
}

#[tokio::test]
async fn run_returns_error_when_no_messages() {
    let api_key = "test-run-empty";
    let _provider_guard = ProviderGuard::register(api_key, Arc::new(TestProvider::new(vec![])));
    let agent = Agent::new(test_config(api_key));
    let result = agent.run();
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(err.contains("no messages"), "got: {}", err);
}

#[tokio::test]
async fn run_returns_error_when_last_message_is_assistant() {
    let api_key = "test-run-assistant-tail";
    let _provider_guard = ProviderGuard::register(api_key, Arc::new(TestProvider::new(vec![])));
    let agent = Agent::new(test_config(api_key));
    agent.add_message(AgentMessage::UserText {
        message_id: "u".into(),
        text: "hi".into(),
    });
    agent.add_message(AgentMessage::Assistant {
        message_id: "a".into(),
        message: AssistantMessage::empty("test", "test-model"),
    });
    let result = agent.run();
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(err.contains("assistant"), "got: {}", err);
}

#[tokio::test]
async fn run_succeeds_when_last_message_is_user() {
    let api_key = "test-run-user-tail";
    let _provider_guard =
        ProviderGuard::register(api_key, Arc::new(TestProvider::new(vec![text_turn("ok")])));
    let agent = Agent::new(test_config(api_key));
    agent.add_message(AgentMessage::UserText {
        message_id: "u".into(),
        text: "hi".into(),
    });
    let stream = agent.run();
    assert!(stream.is_ok());
    let mut s = stream.unwrap();
    while s.next().await.is_some() {}
}
