use crate::common;

use common::ProviderGuard;
use futures::StreamExt;
use pi_agent_core::api::agent::ProviderStreamer;
use pi_agent_core::api::compaction::{
    BranchSummaryOptions, collect_entries_for_branch_summary,
    generate_branch_summary_with_provider_streamer, prepare_branch_entries,
};
use pi_agent_core::api::execution::{
    ExecutionOutput, FileSystem, ShellCaptureOptions, TruncationLimit, execute_shell_with_capture,
    truncate_head, truncate_tail,
};
use pi_agent_core::api::testing::{
    InMemoryExecutionEnv, ProxyAssistantMessageEvent, ProxyMessageState, ProxyStreamOptions,
    build_proxy_request_body, process_proxy_event, stream_proxy_with_transport,
};
use pi_agent_core::api::transcript::{SessionEntry, StoredAgentMessage, StoredUsage};
use pi_ai::api::conversation::{AssistantMessage, ContentBlock, Context, StopReason};
use pi_ai::api::model::{Model, ModelCost, ModelInput};
use pi_ai::api::stream::{AssistantMessageEvent, StreamOptions};
use pi_ai::api::testing::{FauxCall, FauxProvider, FauxResponse};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn timestamp() -> String {
    "2026-06-20T00:00:00.000Z".into()
}

fn user_entry(id: &str, parent: Option<&str>, text: &str) -> SessionEntry {
    SessionEntry::message(
        id.into(),
        parent.map(str::to_string),
        timestamp(),
        StoredAgentMessage::User {
            content: vec![ContentBlock::Text {
                text: text.into(),
                text_signature: None,
            }],
            timestamp: 1,
        },
    )
}

fn assistant_entry(id: &str, parent: Option<&str>, text: &str) -> SessionEntry {
    SessionEntry::message(
        id.into(),
        parent.map(str::to_string),
        timestamp(),
        StoredAgentMessage::Assistant {
            content: vec![ContentBlock::Text {
                text: text.into(),
                text_signature: None,
            }],
            api: "faux".into(),
            provider: "faux".into(),
            model: "faux".into(),
            response_model: None,
            response_id: None,
            usage: StoredUsage::default(),
            stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: 1,
        },
    )
}

fn faux_model(api: &str) -> Model {
    Model {
        id: "m9-faux-model".into(),
        name: "M9 Faux".into(),
        api: api.into(),
        provider: "faux".into(),
        base_url: "http://localhost".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 128_000,
        max_tokens: 4096,
        headers: None,
        compat: None,
    }
}

#[test]
fn branch_summary_collects_abandoned_branch_and_prepares_messages() {
    let entries = vec![
        user_entry("root", None, "start"),
        assistant_entry("old-a", Some("root"), "worked on src/lib.rs"),
        user_entry(
            "old-b",
            Some("old-a"),
            "read README.md then edit src/lib.rs",
        ),
        user_entry("target", Some("root"), "alternate path"),
        SessionEntry::branch_summary(
            "summary-1".into(),
            Some("target".into()),
            timestamp(),
            "previous branch summary".into(),
            "old-b".into(),
            Some(serde_json::json!({
                "readFiles": ["README.md"],
                "modifiedFiles": ["src/lib.rs"]
            })),
            false,
        ),
    ];

    let collected = collect_entries_for_branch_summary(&entries, Some("old-b"), "target").unwrap();
    assert_eq!(collected.common_ancestor_id.as_deref(), Some("root"));
    assert_eq!(
        collected
            .entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["old-a", "old-b"]
    );

    let prepared = prepare_branch_entries(&entries, 0);
    assert!(prepared.messages.iter().any(|message| matches!(
        message,
        pi_agent_core::api::agent::AgentMessage::BranchSummary { .. }
    )));
    assert!(prepared.file_ops.read.contains("README.md"));
    assert!(prepared.file_ops.modified.contains("src/lib.rs"));
    assert!(prepared.total_tokens > 0);
}

#[tokio::test]
async fn branch_summary_generation_uses_faux_provider_and_adds_preamble() {
    let api = "m9-branch-summary-faux";
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![FauxCall {
            responses: vec![FauxResponse {
                text_deltas: vec!["## Goal\nFinish M9".into()],
                thinking_deltas: vec![],
                tool_calls: vec![],
            }],
            stop_reason: StopReason::Stop,
        }])),
    );

    let result = generate_branch_summary_with_provider_streamer(
        &[user_entry(
            "u1",
            None,
            "read crates/pi-agent-core/src/lib.rs",
        )],
        BranchSummaryOptions {
            model: faux_model(api),
            api_key: "test-key".into(),
            headers: Some(serde_json::json!({"x-test": "yes"})),
            custom_instructions: Some("focus on Rust parity".into()),
            replace_instructions: false,
            reserve_tokens: 16_384,
        },
        Some(_provider_guard.provider_streamer()),
    )
    .await
    .unwrap();

    assert!(
        result
            .summary
            .starts_with("The user explored a different conversation branch")
    );
    assert!(result.summary.contains("## Goal"));
}

#[tokio::test]
async fn branch_summary_generation_uses_provider_streamer_without_global_registry() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_streamer = calls.clone();
    let provider_streamer: ProviderStreamer = Arc::new(move |model, _context, opts| {
        assert_eq!(model.api, "m9-branch-summary-scoped-streamer-only");
        assert_eq!(
            opts.and_then(|options| options.api_key).as_deref(),
            Some("scoped-key")
        );
        calls_for_streamer.fetch_add(1, Ordering::SeqCst);
        let model_id = model.id.clone();
        Box::pin(async_stream::stream! {
            let mut message = AssistantMessage::empty("scoped", &model_id);
            message.content.push(ContentBlock::Text {
                text: "## Goal\nScoped branch summary".into(),
                text_signature: None,
            });
            message.stop_reason = StopReason::Stop;
            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message,
            };
        })
    });

    let result = generate_branch_summary_with_provider_streamer(
        &[user_entry(
            "u1",
            None,
            "read crates/pi-agent-core/src/lib.rs",
        )],
        BranchSummaryOptions {
            model: faux_model("m9-branch-summary-scoped-streamer-only"),
            api_key: "scoped-key".into(),
            headers: None,
            custom_instructions: None,
            replace_instructions: false,
            reserve_tokens: 16_384,
        },
        Some(provider_streamer),
    )
    .await
    .unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(result.summary.contains("Scoped branch summary"));
}

#[test]
fn truncation_head_tail_and_proxy_event_processing_are_ts_compatible() {
    let head = truncate_head(
        "one\ntwo\nthree\nfour",
        TruncationLimit {
            max_lines: 2,
            max_bytes: 1024,
        },
    );
    assert_eq!(head.content, "one\ntwo");
    assert!(head.truncated);
    assert_eq!(head.truncated_by.as_deref(), Some("lines"));

    let tail = truncate_tail(
        "one\ntwo\nthree\nfour",
        TruncationLimit {
            max_lines: 2,
            max_bytes: 1024,
        },
    );
    assert_eq!(tail.content, "three\nfour");
    assert!(tail.truncated);

    let mut partial = AssistantMessage::empty("proxy", "model");
    let event = process_proxy_event(
        ProxyAssistantMessageEvent::TextStart { content_index: 0 },
        &mut partial,
    )
    .unwrap();
    assert!(matches!(event, AssistantMessageEvent::TextStart { .. }));
    process_proxy_event(
        ProxyAssistantMessageEvent::TextDelta {
            content_index: 0,
            delta: "hel".into(),
        },
        &mut partial,
    )
    .unwrap();
    process_proxy_event(
        ProxyAssistantMessageEvent::TextDelta {
            content_index: 0,
            delta: "lo".into(),
        },
        &mut partial,
    )
    .unwrap();
    let event = process_proxy_event(
        ProxyAssistantMessageEvent::Done {
            reason: StopReason::Stop,
            usage: Default::default(),
        },
        &mut partial,
    )
    .unwrap();
    match event {
        AssistantMessageEvent::Done { message, .. } => {
            assert_eq!(
                message.content,
                vec![ContentBlock::Text {
                    text: "hello".into(),
                    text_signature: None,
                }]
            );
        }
        _ => panic!("expected done"),
    }
}

#[test]
fn proxy_message_state_accumulates_split_tool_call_json() {
    let model = faux_model("proxy-api");
    let mut state = ProxyMessageState::new(&model);
    state
        .process(ProxyAssistantMessageEvent::ToolcallStart {
            content_index: 0,
            id: "call-1".into(),
            tool_name: "read".into(),
        })
        .unwrap();
    state
        .process(ProxyAssistantMessageEvent::ToolcallDelta {
            content_index: 0,
            delta: "{\"path\":\"".into(),
        })
        .unwrap();
    state
        .process(ProxyAssistantMessageEvent::ToolcallDelta {
            content_index: 0,
            delta: r#"README.md"}"#.into(),
        })
        .unwrap();

    match &state.partial.content[0] {
        ContentBlock::ToolCall { arguments, .. } => {
            assert_eq!(arguments["path"], "README.md");
        }
        _ => panic!("expected tool call"),
    }
}

#[tokio::test]
async fn shell_capture_truncates_tail_and_persists_full_output() {
    let env = InMemoryExecutionEnv::new("/workspace");
    let full = (0..2500)
        .map(|idx| format!("line-{idx}"))
        .collect::<Vec<_>>()
        .join("\n");
    env.set_command(
        "big-output",
        ExecutionOutput {
            stdout: full.clone(),
            stderr: String::new(),
            exit_code: 0,
        },
    );

    let result = execute_shell_with_capture(
        &env,
        "big-output",
        ShellCaptureOptions {
            max_lines: 3,
            max_bytes: 1024,
        },
    )
    .await
    .unwrap();

    assert_eq!(result.exit_code, Some(0));
    assert!(!result.cancelled);
    assert!(result.truncated);
    assert!(result.output.contains("line-2499"));
    assert!(!result.output.contains("line-0"));
    let full_output_path = result.full_output_path.expect("full output path");
    assert_eq!(env.read_text_file(&full_output_path).await.unwrap(), full);
}

#[tokio::test]
async fn proxy_stream_uses_transport_and_reconstructs_events() {
    let model = faux_model("proxy-api");
    let context = Context {
        system_prompt: Some("system".into()),
        messages: vec![],
        tools: None,
    };
    let options = ProxyStreamOptions {
        proxy_url: "https://proxy.invalid".into(),
        auth_token: "proxy-token".into(),
        stream_options: StreamOptions {
            temperature: Some(0.3),
            session_id: Some("session-1".into()),
            headers: Some(serde_json::json!({"x-extra": "yes"})),
            ..Default::default()
        },
    };

    let body = build_proxy_request_body(&model, &context, &options).unwrap();
    assert_eq!(body["model"]["id"], "m9-faux-model");
    assert_eq!(body["options"]["temperature"], 0.3);
    assert!(body["options"].get("apiKey").is_none());

    let mut stream = stream_proxy_with_transport(model, context, options, |_request| {
        Box::pin(async {
            Ok(vec![
                ProxyAssistantMessageEvent::TextStart { content_index: 0 },
                ProxyAssistantMessageEvent::TextDelta {
                    content_index: 0,
                    delta: "proxied".into(),
                },
                ProxyAssistantMessageEvent::Done {
                    reason: StopReason::Stop,
                    usage: Default::default(),
                },
            ])
        })
    });

    let mut done = None;
    while let Some(event) = stream.next().await {
        if let AssistantMessageEvent::Done { message, .. } = event {
            done = Some(message);
            break;
        }
    }
    let done = done.expect("done event");
    assert_eq!(
        done.content,
        vec![ContentBlock::Text {
            text: "proxied".into(),
            text_signature: None,
        }]
    );
}
