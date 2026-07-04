mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::stream;
use pi_ai::providers::faux::{FauxProvider, FauxResponse};
use pi_ai::registry::{self, ApiProvider};
use pi_ai::stream::EventStream;
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Message, Model, StopReason,
    StreamOptions,
};
use pi_coding_agent::interactive::test_harness::{
    run_scripted_idle_interactive, run_scripted_idle_interactive_with_delays,
    run_scripted_idle_interactive_with_size, run_scripted_interactive,
    run_scripted_interactive_with_provider_driver,
    run_scripted_interactive_with_session_dir_size_and_waits,
};
use pi_tui::TerminalOp;
use support::EnvGuard;
use tokio::sync::Notify;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn text_response(text: &str) -> FauxResponse {
    FauxResponse {
        text_deltas: vec![text.to_string()],
        thinking_deltas: vec![],
        tool_calls: vec![],
    }
}

fn six_line_markdown() -> &'static str {
    "- one\n- two\n- three\n- four\n- five\n- six"
}

struct PausingTwoTurnProvider {
    contexts: Arc<Mutex<Vec<Context>>>,
    first_started: Arc<Notify>,
}

impl ApiProvider for PausingTwoTurnProvider {
    fn stream(&self, model: &Model, ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        let call_index = {
            let mut contexts = self.contexts.lock().unwrap();
            contexts.push(ctx);
            contexts.len()
        };
        let first_started = Arc::clone(&self.first_started);
        let model_id = model.id.clone();
        Box::pin(async_stream::stream! {
            if call_index == 1 {
                first_started.notify_waiters();
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let text = if call_index == 1 { "first" } else { "second" };
            let mut message = AssistantMessage::empty("interactive-steer", &model_id);
            message.provider = Some("interactive-steer".into());
            message.content.push(ContentBlock::Text {
                text: text.into(),
                text_signature: None,
            });
            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message,
            };
        })
    }
}

struct DelegationConfirmationProvider {
    calls: Arc<Mutex<usize>>,
    parent_ready: Arc<Notify>,
    child_started: Arc<Notify>,
}

impl ApiProvider for DelegationConfirmationProvider {
    fn stream(&self, model: &Model, _ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        let call_index = {
            let mut calls = self.calls.lock().unwrap();
            *calls += 1;
            *calls
        };
        let parent_ready = Arc::clone(&self.parent_ready);
        let child_started = Arc::clone(&self.child_started);
        let model_id = model.id.clone();
        Box::pin(async_stream::stream! {
            let mut message = AssistantMessage::empty("interactive-delegation", &model_id);
            message.provider = Some("interactive-delegation".into());
            match call_index {
                1 => {
                    message.content.push(ContentBlock::ToolCall {
                        id: "tool_delegate_agent".to_string(),
                        name: "delegate_agent".to_string(),
                        arguments: serde_json::json!({
                            "agent_id": "coder",
                            "task": "implement parser"
                        }),
                        thought_signature: None,
                    });
                    message.stop_reason = StopReason::ToolUse;
                    yield AssistantMessageEvent::Done {
                        reason: StopReason::ToolUse,
                        message,
                    };
                }
                2 => {
                    parent_ready.notify_waiters();
                    message.content.push(ContentBlock::Text {
                        text: "parent ready".to_string(),
                        text_signature: None,
                    });
                    message.stop_reason = StopReason::Stop;
                    yield AssistantMessageEvent::Done {
                        reason: StopReason::Stop,
                        message,
                    };
                }
                3 => {
                    child_started.notify_waiters();
                    message.content.push(ContentBlock::Text {
                        text: "child result".to_string(),
                        text_signature: None,
                    });
                    message.stop_reason = StopReason::Stop;
                    yield AssistantMessageEvent::Done {
                        reason: StopReason::Stop,
                        message,
                    };
                }
                _ => {
                    message.content.push(ContentBlock::Text {
                        text: "unexpected extra call".to_string(),
                        text_signature: None,
                    });
                    message.stop_reason = StopReason::Stop;
                    yield AssistantMessageEvent::Done {
                        reason: StopReason::Stop,
                        message,
                    };
                }
            }
        })
    }
}

#[tokio::test]
async fn scripted_interactive_submit_while_running_sends_steer_control() {
    let contexts = Arc::new(Mutex::new(Vec::new()));
    let first_started = Arc::new(Notify::new());
    let provider = Arc::new(PausingTwoTurnProvider {
        contexts: Arc::clone(&contexts),
        first_started: Arc::clone(&first_started),
    });

    let output = tokio::time::timeout(
        Duration::from_secs(1),
        run_scripted_interactive_with_provider_driver(provider, move |tx| async move {
            tx.send("first prompt\r".to_string()).unwrap();
            first_started.notified().await;
            tx.send("steer now\r".to_string()).unwrap();
            drop(tx);
        }),
    )
    .await
    .expect("interactive run should finish")
    .unwrap();

    let contexts = contexts.lock().unwrap();
    assert_eq!(contexts.len(), 2, "{}", output.rendered);
    assert!(
        contexts[1].messages.iter().any(|message| matches!(
            message,
            Message::User { content }
                if content.iter().any(|block| matches!(
                    block,
                    ContentBlock::Text { text, .. } if text == "steer now"
                ))
        )),
        "{:#?}",
        contexts[1].messages
    );
}

#[tokio::test]
async fn scripted_interactive_shift_enter_while_running_sends_follow_up_control() {
    let contexts = Arc::new(Mutex::new(Vec::new()));
    let first_started = Arc::new(Notify::new());
    let provider = Arc::new(PausingTwoTurnProvider {
        contexts: Arc::clone(&contexts),
        first_started: Arc::clone(&first_started),
    });

    let output = tokio::time::timeout(
        Duration::from_secs(1),
        run_scripted_interactive_with_provider_driver(provider, move |tx| async move {
            tx.send("first prompt\r".to_string()).unwrap();
            first_started.notified().await;
            tx.send("follow up now\x1b[13;2u".to_string()).unwrap();
            drop(tx);
        }),
    )
    .await
    .expect("interactive run should finish")
    .unwrap();

    let contexts = contexts.lock().unwrap();
    assert_eq!(contexts.len(), 2, "{}", output.rendered);
    assert!(
        contexts[1].messages.iter().any(|message| matches!(
            message,
            Message::User { content }
                if content.iter().any(|block| matches!(
                    block,
                    ContentBlock::Text { text, .. } if text == "follow up now"
                ))
        )),
        "{:#?}",
        contexts[1].messages
    );
}

#[tokio::test]
async fn scripted_interactive_prompt_renders_assistant_text() {
    let provider = FauxProvider::new(vec![text_response("hello from tui")]);
    let output = run_scripted_interactive(provider, "say hi\r\x03")
        .await
        .unwrap();
    assert!(output.contains("say hi"));
    assert!(output.contains("hello from tui"));
    assert!(output.contains("status: idle"));
    assert!(output.terminal_restored);
}

#[tokio::test]
async fn scripted_interactive_self_healing_edit_uses_model_repair_policy() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("src")).unwrap();
    std::fs::write(temp.path().join("src/app.txt"), "one\ntwo\nthree\n").unwrap();
    let provider = FauxProvider::simple_text(r#"{"edits":[{"oldText":"deux","newText":"dos"}]}"#);

    let output = run_scripted_interactive_with_session_dir_size_and_waits(
        provider,
        temp.path(),
        vec![
            (
                "/self-healing-edit src/app.txt two => deux --model-repair --check grep -q dos src/app.txt\r",
                "self_healing_edit.completed",
            ),
            ("\x03", ""),
        ],
        80,
        24,
    )
    .await
    .expect("scripted interactive self-healing edit should succeed");

    assert_eq!(
        std::fs::read_to_string(temp.path().join("src/app.txt")).unwrap(),
        "one\ndos\nthree\n"
    );
    assert!(output.contains("Successfully replaced"), "{output:?}");
    assert_eq!(output.exit_code, 0);
}

#[tokio::test]
async fn scripted_interactive_agent_invocation_renders_selected_profile_reply() {
    let _guard = ENV_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("agents")).unwrap();
    std::fs::write(
        dir.path().join("agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
"#,
    )
    .unwrap();
    let env = EnvGuard::new(&["PI_RUST_DIR"]);
    env.set_pi_rust_dir(dir.path());

    let provider = FauxProvider::new(vec![text_response("agent reply")]);
    let output = run_scripted_interactive(provider, "/agent:coder do work\r").await;

    let output = output.unwrap();
    assert!(output.contains("/agent:coder do work"), "{output:?}");
    assert!(output.contains("agent reply"), "{output:?}");
    assert!(output.contains("status: idle"), "{output:?}");
    assert!(
        !output.contains("requires AgentInvocationFlow"),
        "{output:?}"
    );
}

#[tokio::test]
async fn scripted_interactive_agent_team_renders_member_replies() {
    let _guard = ENV_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("agents")).unwrap();
    std::fs::create_dir_all(dir.path().join("teams")).unwrap();
    std::fs::write(
        dir.path().join("agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("agents/reviewer.toml"),
        r#"
schema_version = 1
id = "reviewer"
display_name = "Reviewer"
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("teams/implementation.toml"),
        r#"
schema_version = 1
id = "implementation"
display_name = "Implementation Team"
supervisor = "deterministic"
strategy = "plan_execute_review"
members = ["coder", "reviewer"]
"#,
    )
    .unwrap();
    let env = EnvGuard::new(&["PI_RUST_DIR"]);
    env.set_pi_rust_dir(dir.path());

    let provider = FauxProvider::new(vec![
        text_response("coder reply"),
        text_response("reviewer reply"),
    ]);
    let output = run_scripted_interactive(provider, "/team:implementation do work\r").await;

    let output = output.unwrap();
    assert!(
        output.contains("/team:implementation do work"),
        "{output:?}"
    );
    assert!(output.contains("coder reply"), "{output:?}");
    assert!(output.contains("reviewer reply"), "{output:?}");
    assert!(output.contains("status: idle"), "{output:?}");
    assert!(!output.contains("requires AgentTeamFlow"), "{output:?}");
}

#[tokio::test]
async fn scripted_interactive_approves_pending_delegation_confirmation() {
    let _guard = ENV_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("agents")).unwrap();
    std::fs::write(
        dir.path().join("agents/delegating-planner.toml"),
        r#"
schema_version = 1
id = "delegating-planner"
display_name = "Delegating Planner"

[delegation]
allow_delegate_agent = true
max_depth = 1
max_parallel_children = 1
require_confirmation = "always"
allowed_agents = ["coder"]
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
system_prompt = "Coder child instructions."
"#,
    )
    .unwrap();
    let env = EnvGuard::new(&["PI_RUST_DIR"]);
    env.set_pi_rust_dir(dir.path());

    let parent_ready = Arc::new(Notify::new());
    let child_started = Arc::new(Notify::new());
    let calls = Arc::new(Mutex::new(0));
    let provider = Arc::new(DelegationConfirmationProvider {
        calls: Arc::clone(&calls),
        parent_ready: Arc::clone(&parent_ready),
        child_started: Arc::clone(&child_started),
    });
    let output = tokio::time::timeout(
        Duration::from_secs(2),
        run_scripted_interactive_with_provider_driver(provider, move |tx| async move {
            tx.send("/agent\r\x1b[B\rdelegating\rplan feature\r".to_string())
                .unwrap();
            parent_ready.notified().await;
            tokio::time::sleep(Duration::from_millis(100)).await;
            tx.send("/delegations\r".to_string()).unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;
            tx.send("\r".to_string()).unwrap();
            child_started.notified().await;
            tokio::time::sleep(Duration::from_millis(100)).await;
            drop(tx);
        }),
    )
    .await
    .expect("interactive delegation approval should finish");

    let output = output.unwrap();
    assert_eq!(*calls.lock().unwrap(), 3, "{output:?}");
    assert!(
        output.contains("Delegation confirmation required"),
        "{output:?}"
    );
    assert!(output.contains("/delegation approve op_"), "{output:?}");
    assert!(output.contains("Approving delegation"), "{output:?}");
    assert!(output.contains("child result"), "{output:?}");
    assert!(output.contains("status: idle"), "{output:?}");
}

#[tokio::test]
async fn scripted_interactive_prompt_leaves_terminal_progress_off_by_default() {
    let provider = FauxProvider::new(vec![text_response("progress done")]);
    let output = run_scripted_interactive(provider, "show progress\r")
        .await
        .unwrap();

    let progress_ops = output
        .ops
        .iter()
        .filter_map(|op| match op {
            TerminalOp::SetProgress(active) => Some(*active),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(progress_ops.is_empty(), "{:?}", output.ops);
}

#[tokio::test]
async fn scripted_interactive_clone_after_rust_native_prompt_creates_session() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![text_response("assistant reply")]);

    let output = run_scripted_interactive_with_session_dir_size_and_waits(
        provider,
        dir.path(),
        vec![
            ("hello\r", "assistant reply"),
            ("/clone\r", "session.cloned"),
        ],
        80,
        24,
    )
    .await
    .unwrap();

    let files = collect_jsonl_files(dir.path());
    assert_eq!(files.len(), 2, "{files:?}");
    assert_eq!(rust_session_dirs(dir.path()).len(), 2);
    assert!(
        output.contains("Cloned to new session"),
        "{}",
        output.rendered
    );
    assert!(!output.contains("not implemented"), "{}", output.rendered);
    assert!(
        files.iter().any(|path| std::fs::read_to_string(path)
            .unwrap()
            .contains("session.cloned")),
        "{files:?}"
    );
}

#[tokio::test]
async fn scripted_interactive_fork_after_rust_native_prompt_creates_session() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![text_response("assistant reply")]);

    let output = run_scripted_interactive_with_session_dir_size_and_waits(
        provider,
        dir.path(),
        vec![
            ("hello\r", "assistant reply"),
            ("/fork\r", "session.forked"),
        ],
        80,
        24,
    )
    .await
    .unwrap();

    let files = collect_jsonl_files(dir.path());
    assert_eq!(files.len(), 2, "{files:?}");
    assert_eq!(rust_session_dirs(dir.path()).len(), 2);
    assert!(
        output.contains("Forked to new session"),
        "{}",
        output.rendered
    );
    assert!(!output.contains("not implemented"), "{}", output.rendered);
    assert!(
        files.iter().any(|path| std::fs::read_to_string(path)
            .unwrap()
            .contains("session.forked")),
        "{files:?}"
    );
}

#[tokio::test]
async fn scripted_interactive_compact_after_rust_native_prompt_records_compaction() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FauxProvider::with_call_queue(vec![
        FauxProvider::text_call("assistant reply", StopReason::Stop),
        FauxProvider::text_call("summary from compact", StopReason::Stop),
    ]);

    let output = run_scripted_interactive_with_session_dir_size_and_waits(
        provider,
        dir.path(),
        vec![
            ("hello\r", "assistant reply"),
            ("/compact keep decisions\r", "session.compaction.completed"),
        ],
        80,
        24,
    )
    .await
    .unwrap();

    let files = collect_jsonl_files(dir.path());
    assert_eq!(files.len(), 1, "{files:?}");
    let session_text = std::fs::read_to_string(&files[0]).unwrap();
    assert!(
        !session_text.contains("\"type\":\"compaction\""),
        "{session_text}"
    );
    assert!(
        session_text.contains("session.compaction.completed"),
        "{session_text}"
    );
    assert!(
        output.contains("summary from compact"),
        "{}",
        output.rendered
    );
    assert!(!output.contains("not implemented"), "{}", output.rendered);
}

#[tokio::test]
async fn scripted_interactive_renders_assistant_markdown() {
    let provider = FauxProvider::new(vec![text_response(
        "# Title\n\nA paragraph with **bold** text and `code`.\n\n- one",
    )]);
    let output = run_scripted_interactive(provider, "format\r")
        .await
        .unwrap();
    let frame = output.rendered_lines.join("\n");

    assert!(frame.contains("Title"), "{frame}");
    assert!(frame.contains("A paragraph with "), "{frame}");
    assert!(frame.contains("bold"), "{frame}");
    assert!(frame.contains("code"), "{frame}");
    assert!(frame.contains("- one"), "{frame}");
    assert!(!frame.contains("# Title"), "{frame}");
    assert!(!frame.contains("**bold**"), "{frame}");
    assert!(!frame.contains("`code`"), "{frame}");
}

#[tokio::test]
async fn scripted_interactive_keeps_cursor_after_first_typed_character() {
    let output = run_scripted_idle_interactive("a").await.unwrap();

    assert_eq!(output.cursor_col, 3);
    assert!(
        output
            .ops
            .iter()
            .any(|op| *op == TerminalOp::MoveToColumn(3))
    );
    assert!(!output.ops.contains(&TerminalOp::ClearScreen));
}

#[tokio::test]
async fn scripted_interactive_positions_cursor_after_wide_unicode() {
    let cjk = run_scripted_idle_interactive("好").await.unwrap();
    assert_eq!(cjk.cursor_col, 4);
    assert!(!cjk.ops.contains(&TerminalOp::ClearScreen));

    let emoji = run_scripted_idle_interactive("🎉").await.unwrap();
    assert_eq!(emoji.cursor_col, 4);
    assert!(!emoji.ops.contains(&TerminalOp::ClearScreen));
}

#[tokio::test]
async fn scripted_interactive_backspace_returns_cursor_to_prompt_start() {
    let output = run_scripted_idle_interactive("a\x7f").await.unwrap();

    assert_eq!(output.cursor_col, 2);
    assert!(!output.ops.contains(&TerminalOp::ClearScreen));
}

#[tokio::test]
async fn scripted_interactive_left_arrow_moves_cursor_within_prompt() {
    let output = run_scripted_idle_interactive("ab\x1b[D").await.unwrap();

    assert_eq!(output.cursor_col, 3);
    assert!(
        output
            .ops
            .iter()
            .any(|op| *op == TerminalOp::MoveToColumn(3))
    );
}

#[tokio::test]
async fn scripted_interactive_coalesces_fast_typed_input_renders() {
    let input = "abcdefghijklmnopqrst";
    let output = run_scripted_idle_interactive(input).await.unwrap();

    assert_eq!(output.cursor_col, 2 + input.len());
    assert!(
        sync_render_count(&output.ops) <= 2,
        "expected first frame plus one coalesced edit render, got {}",
        sync_render_count(&output.ops)
    );
}

#[tokio::test]
async fn scripted_interactive_coalesces_fast_assistant_delta_renders() {
    let deltas = vec!["x".to_string(); 40];
    let provider = FauxProvider::new(vec![FauxResponse {
        text_deltas: deltas,
        thinking_deltas: vec![],
        tool_calls: vec![],
    }]);

    let output = run_scripted_interactive(provider, "say hi\r")
        .await
        .unwrap();

    assert!(output.contains(&"x".repeat(40)));
    assert!(
        sync_render_count(&output.ops) <= 4,
        "expected assistant deltas to be batched, got {} sync renders",
        sync_render_count(&output.ops)
    );
}

#[tokio::test]
async fn scripted_interactive_noop_key_release_does_not_render() {
    let output = run_scripted_idle_interactive("\x1b[97;3:3u").await.unwrap();

    let prompt_cursor_moves = output
        .ops
        .iter()
        .filter(|op| **op == TerminalOp::MoveToColumn(2))
        .count();
    assert_eq!(prompt_cursor_moves, 1);
}

#[tokio::test]
async fn scripted_interactive_slash_suggestions_render_after_slash() {
    let output = run_scripted_idle_interactive("/").await.unwrap();
    let frame = output.rendered_lines.join("\n");

    assert!(frame.contains("> /"), "{frame}");
    assert!(frame.contains("/help"), "{frame}");
    assert!(frame.contains("Show help"), "{frame}");
    assert!(frame.contains("/settings"), "{frame}");
    assert!(frame.contains("Open settings menu"), "{frame}");
    assert!(frame.contains("(1/30)"), "{frame}");
}

#[tokio::test]
async fn scripted_interactive_slash_suggestion_tab_accepts_filtered_command() {
    let output = run_scripted_idle_interactive("/mo\t").await.unwrap();
    let frame = output.rendered_lines.join("\n");

    assert!(frame.contains("> /model"), "{frame}");
    assert!(!frame.contains("Select model"), "{frame}");
    assert!(!frame.contains("(1/"), "{frame}");
}

fn sync_render_count(ops: &[TerminalOp]) -> usize {
    ops.iter()
        .filter(|op| matches!(op, TerminalOp::Write(data) if data.contains("\x1b[?2026h")))
        .count()
}

fn collect_jsonl_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    collect_jsonl_files_recursive(root, &mut files);
    files.sort();
    files
}

fn rust_session_dirs(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    collect_rust_session_dirs(root, &mut dirs);
    dirs.sort();
    dirs
}

fn collect_rust_session_dirs(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.join("session.json").is_file() && path.join("events.jsonl").is_file() {
                out.push(path);
            } else {
                collect_rust_session_dirs(&path, out);
            }
        }
    }
}

fn collect_jsonl_files_recursive(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_files_recursive(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
            out.push(path);
        }
    }
}

struct RecordingModelProvider {
    model_ids: Arc<Mutex<Vec<String>>>,
    api_keys: Arc<Mutex<Vec<Option<String>>>>,
}

impl ApiProvider for RecordingModelProvider {
    fn stream(&self, model: &Model, _ctx: Context, opts: Option<StreamOptions>) -> EventStream {
        self.model_ids.lock().unwrap().push(model.id.clone());
        self.api_keys
            .lock()
            .unwrap()
            .push(opts.and_then(|opts| opts.api_key));
        let mut message = AssistantMessage::empty("recording", &model.id);
        message.content.push(ContentBlock::Text {
            text: "model ok".to_string(),
            text_signature: None,
        });
        message.stop_reason = StopReason::Stop;
        Box::pin(stream::iter(vec![AssistantMessageEvent::Done {
            reason: StopReason::Stop,
            message,
        }]))
    }
}

#[tokio::test]
async fn scripted_interactive_initial_render_uses_content_height() {
    let output = run_scripted_idle_interactive_with_size("", 80, 24)
        .await
        .unwrap();
    let frame = output.rendered_lines.join("\n");

    assert!(
        output.rendered_lines.len() <= 9,
        "initial render should not pad to the full terminal height: {output:?}"
    );
    assert!(
        frame.contains(env!("CARGO_PKG_VERSION")),
        "welcome line should include the binary version: {frame}"
    );
    assert!(
        frame.contains("submit"),
        "welcome line should mention submit: {frame}"
    );
    assert!(
        frame.contains("/help"),
        "welcome line should mention /help: {frame}"
    );
    assert!(frame.contains("─"), "input border should render: {frame}");
}

#[tokio::test]
async fn scripted_interactive_keeps_full_transcript_in_terminal_output() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![text_response(six_line_markdown())]);
    let output = run_scripted_interactive_with_session_dir_size_and_waits(
        provider,
        temp.path(),
        vec![("prompt\r", "six"), ("typed", "six")],
        40,
        6,
    )
    .await
    .unwrap();
    let frame = output.rendered_lines.join("\n");

    assert!(frame.contains("one"), "{frame}");
    assert!(frame.contains("six"));
    assert!(frame.contains("> typed"));
    assert!(frame.contains("status: idle"));
    assert!(
        output.rendered_lines.len() > 6,
        "transcript should grow beyond terminal height instead of acting as a fixed viewport: {output:?}"
    );
}

#[tokio::test]
async fn scripted_interactive_page_up_does_not_window_terminal_transcript() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![text_response(six_line_markdown())]);
    let output = run_scripted_interactive_with_session_dir_size_and_waits(
        provider,
        temp.path(),
        vec![("prompt\r", "six"), ("\x1b[5~", "six")],
        40,
        6,
    )
    .await
    .unwrap();
    let frame = output.rendered_lines.join("\n");

    assert!(frame.contains("one"));
    assert!(frame.contains("six"));
    assert!(frame.contains("> "));
    assert!(frame.contains("status: idle"));

    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![text_response(six_line_markdown())]);
    let output = run_scripted_interactive_with_session_dir_size_and_waits(
        provider,
        temp.path(),
        vec![("prompt\r", "six"), ("\x1b[5~\x1b[6~", "six")],
        40,
        6,
    )
    .await
    .unwrap();
    let frame = output.rendered_lines.join("\n");

    assert!(frame.contains("one"));
    assert!(frame.contains("six"));
    assert!(frame.contains("> "));
    assert!(frame.contains("status: idle"));
}

#[tokio::test]
async fn scripted_interactive_new_output_remains_in_terminal_transcript_after_page_up() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::with_call_queue(vec![
        FauxProvider::text_call(six_line_markdown(), StopReason::Stop),
        FauxProvider::text_call("brand new bottom", StopReason::Stop),
    ]);
    let output = run_scripted_interactive_with_session_dir_size_and_waits(
        provider,
        temp.path(),
        vec![
            ("first\r", "six"),
            ("\x1b[5~", "six"),
            ("second\r", "brand new bottom"),
        ],
        40,
        6,
    )
    .await
    .unwrap();
    let frame = output.rendered_lines.join("\n");

    assert!(frame.contains("one"), "{frame}");
    assert!(frame.contains("brand new bottom"), "{frame}");
    assert!(!frame.contains("new output below"), "{frame}");
    assert!(frame.contains("> "), "{frame}");
    assert!(frame.contains("status: idle"), "{frame}");
}

#[tokio::test]
async fn scripted_interactive_shows_welcome_line_on_empty_transcript() {
    let output = run_scripted_idle_interactive("").await.unwrap();
    let frame = output.rendered_lines.join("\n");
    assert!(frame.contains("pi-rust"), "welcome line missing: {frame}");
    assert!(
        frame.contains(env!("CARGO_PKG_VERSION")),
        "welcome line should mention version: {frame}"
    );
    assert!(
        frame.contains("submit"),
        "welcome line should mention submit: {frame}"
    );
}

#[tokio::test]
async fn scripted_interactive_footer_shows_usage_after_a_turn() {
    let provider = FauxProvider::new(vec![text_response("ok")]);
    let output = run_scripted_interactive(provider, "hi\r\x03")
        .await
        .unwrap();
    let frame = output.rendered_lines.join("\n");
    assert!(
        frame.contains("status: idle"),
        "footer must keep status: idle: {frame}"
    );
    assert!(
        frame.contains("↑") && frame.contains("↓"),
        "footer should show usage stats after a turn: {frame}"
    );
}

#[tokio::test]
async fn scripted_interactive_quit_exits_when_idle() {
    let output = run_scripted_idle_interactive("/quit\r").await.unwrap();
    assert_eq!(output.exit_code, 0, "exit code should be 0 for /quit");
}

#[tokio::test]
async fn scripted_interactive_help_does_not_crash() {
    let output = run_scripted_idle_interactive("/help\r/quit\r")
        .await
        .unwrap();
    assert_eq!(
        output.exit_code, 0,
        "exit code should be 0 after /help then /quit"
    );
}

#[tokio::test]
async fn scripted_interactive_help_lists_registry_commands() {
    let output = run_scripted_idle_interactive("/help\r").await.unwrap();
    assert!(output.contains("/model"), "{output:?}");
    assert!(output.contains("/reload"), "{output:?}");
    assert_eq!(output.exit_code, 0);
}

#[tokio::test]
async fn scripted_interactive_model_command_switches_footer_model() {
    let output = run_scripted_idle_interactive("/model claude-haiku-4-5\r")
        .await
        .unwrap();
    assert!(output.contains("Model set: claude-haiku-4-5"), "{output:?}");
    assert!(
        output.contains("claude-haiku-4-5 • thinking off"),
        "{output:?}"
    );
    assert_eq!(output.exit_code, 0);
}

#[tokio::test]
async fn scripted_interactive_model_command_changes_next_prompt_model() {
    let _guard = ENV_LOCK.lock().await;
    let target_model = pi_ai::lookup_model("claude-haiku-4-5").expect("known model");
    let previous_provider = registry::lookup(&target_model.api);
    let model_ids = Arc::new(Mutex::new(Vec::new()));
    registry::register(
        &target_model.api,
        Arc::new(RecordingModelProvider {
            model_ids: Arc::clone(&model_ids),
            api_keys: Arc::new(Mutex::new(Vec::new())),
        }),
    );

    let output = run_scripted_idle_interactive("/model claude-haiku-4-5\rhi\r").await;

    match previous_provider {
        Some(provider) => registry::register(&target_model.api, provider),
        None => registry::unregister(&target_model.api),
    }

    output.unwrap();
    assert_eq!(
        model_ids.lock().unwrap().as_slice(),
        &["claude-haiku-4-5".to_string()]
    );
}

#[tokio::test]
async fn scripted_interactive_model_selector_confirms_filtered_model() {
    let _guard = ENV_LOCK.lock().await;
    let default_model = pi_ai::lookup_model("claude-sonnet-4-5").expect("known default model");
    let previous_provider = registry::lookup(&default_model.api);
    registry::register(
        &default_model.api,
        Arc::new(RecordingModelProvider {
            model_ids: Arc::new(Mutex::new(Vec::new())),
            api_keys: Arc::new(Mutex::new(Vec::new())),
        }),
    );
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("auth.toml"),
        "[anthropic]\ntype = \"api_key\"\nkey = \"anthropic-auth\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            dir.path().join("auth.toml"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
    let env = EnvGuard::new(&["PI_RUST_DIR"]);
    env.set_pi_rust_dir(dir.path());

    let output = run_scripted_idle_interactive("/model\rclaude-haiku-4-5\r").await;

    match previous_provider {
        Some(provider) => registry::register(&default_model.api, provider),
        None => registry::unregister(&default_model.api),
    }

    let output = output.unwrap();
    assert!(output.contains("Model set: claude-haiku-4-5"), "{output:?}");
    assert!(
        output.contains("claude-haiku-4-5 • thinking off"),
        "{output:?}"
    );
    assert!(!output.contains("not implemented"), "{output:?}");
}

#[tokio::test]
async fn scripted_interactive_model_selector_lists_configured_provider_models() {
    let _guard = ENV_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("auth.toml"),
        "[openai]\ntype = \"api_key\"\nkey = \"openai-auth\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            dir.path().join("auth.toml"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
    let env = EnvGuard::new(&[
        "PI_RUST_DIR",
        "ANTHROPIC_API_KEY",
        "CLAUDE_API_KEY",
        "ANTHROPIC_KEY",
    ]);
    env.set_pi_rust_dir(dir.path());
    env.remove("ANTHROPIC_API_KEY");
    env.remove("CLAUDE_API_KEY");
    env.remove("ANTHROPIC_KEY");

    let output = run_scripted_idle_interactive("/model\r").await;

    let output = output.unwrap();
    let frame = output.rendered_lines.join("\n");
    assert!(frame.contains("Select model"), "{frame}");
    assert!(frame.contains("gpt-5"), "{frame}");
    assert!(
        !frame.contains("claude-haiku-4-5"),
        "model selector should exclude providers without a configured key: {frame}"
    );
}

#[tokio::test]
async fn scripted_interactive_model_command_refreshes_api_key_for_new_provider() {
    let _guard = ENV_LOCK.lock().await;
    let target_model = pi_ai::lookup_model("gpt-5").expect("known model");
    let previous_provider = registry::lookup(&target_model.api);
    let model_ids = Arc::new(Mutex::new(Vec::new()));
    let api_keys = Arc::new(Mutex::new(Vec::new()));
    registry::register(
        &target_model.api,
        Arc::new(RecordingModelProvider {
            model_ids: Arc::clone(&model_ids),
            api_keys: Arc::clone(&api_keys),
        }),
    );

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("auth.toml"),
        "[anthropic]\ntype = \"api_key\"\nkey = \"anthropic-auth\"\n\n[openai]\ntype = \"api_key\"\nkey = \"openai-auth\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            dir.path().join("auth.toml"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
    let env = EnvGuard::new(&["PI_RUST_DIR", "OPENAI_API_KEY", "ANTHROPIC_API_KEY"]);
    env.set_pi_rust_dir(dir.path());
    env.remove("OPENAI_API_KEY");
    env.remove("ANTHROPIC_API_KEY");

    let output = run_scripted_idle_interactive("/model gpt-5\rhi\r").await;

    match previous_provider {
        Some(provider) => registry::register(&target_model.api, provider),
        None => registry::unregister(&target_model.api),
    }

    output.unwrap();
    assert_eq!(model_ids.lock().unwrap().as_slice(), &["gpt-5".to_string()]);
    assert_eq!(
        api_keys.lock().unwrap().as_slice(),
        &[Some("openai-auth".to_string())]
    );
}

#[tokio::test]
async fn scripted_interactive_model_selector_cancel_keeps_current_model() {
    let output = run_scripted_idle_interactive("/model\r\x1b").await.unwrap();
    assert!(!output.contains("Model set:"), "{output:?}");
    assert!(output.contains("Model selection canceled"), "{output:?}");
    assert!(
        !output.contains("no-model"),
        "footer should show the kept model: {output:?}"
    );
    assert!(!output.contains("not implemented"), "{output:?}");
    assert_eq!(output.exit_code, 0);
}

#[tokio::test]
async fn scripted_interactive_known_pending_command_is_not_sent_to_provider() {
    let output = run_scripted_idle_interactive("/scoped-models\r")
        .await
        .unwrap();
    assert!(output.contains("/scoped-models"), "{output:?}");
    assert!(output.contains("not implemented"), "{output:?}");
    assert_eq!(output.exit_code, 0);
}

#[tokio::test]
async fn scripted_interactive_settings_command_enters_settings_menu() {
    let output = run_scripted_idle_interactive("/settings\r").await.unwrap();
    let frame = output.rendered_lines.join("\n");

    assert!(frame.contains("Settings"), "{frame}");
    assert!(frame.contains("Theme"), "{frame}");
    assert!(frame.contains("Auto compact"), "{frame}");
    assert!(frame.contains("Enter/Space to change"), "{frame}");
    assert!(frame.contains("─"), "{frame}");
    assert!(!frame.contains("not implemented"), "{frame}");
    let editor_row = output
        .rendered_lines
        .iter()
        .position(|line| line.contains("> "))
        .expect("editor row should render");
    let settings_row = output
        .rendered_lines
        .iter()
        .position(|line| line.contains("Settings"))
        .expect("settings panel should render");
    let footer_row = output
        .rendered_lines
        .iter()
        .position(|line| line.contains("status: idle"))
        .expect("footer should render");
    assert!(
        editor_row < settings_row && settings_row < footer_row,
        "settings should render below the input box and above the footer: {:?}",
        output.rendered_lines
    );
}

#[tokio::test]
async fn scripted_interactive_settings_escape_closes_menu_after_idle_timeout() {
    use std::time::Duration;

    // The decisive test: send /settings, then a lone ESC, then a /help command.
    // - With the idle-flush fix: ESC fires after ~10ms, menu closes, /help runs
    //   and renders "Show help".
    // - Without the fix: ESC stays buffered, the next bytes get parsed as Alt+/,
    //   which the menu input handler ignores while selecting_settings is true,
    //   and /help never executes -- even though stdin closure later flushes ESC.
    let output = run_scripted_idle_interactive_with_delays(
        vec![
            ("/settings\r", Duration::from_millis(20)),
            ("\x1b", Duration::from_millis(40)),
            ("/help\r", Duration::from_millis(20)),
        ],
        80,
        24,
    )
    .await
    .unwrap();
    let frame = output.rendered_lines.join("\n");
    assert!(
        !frame.contains("Theme:"),
        "settings panel should be closed after Esc;\nframe:\n{frame}"
    );
    assert!(
        frame.contains("show this help"),
        "/help should run after Esc closes the settings menu;\nframe:\n{frame}"
    );
    assert_eq!(output.exit_code, 0);
}

#[tokio::test]
async fn scripted_interactive_unknown_command_is_not_sent_to_provider() {
    let output = run_scripted_idle_interactive("/definitely-unknown\r")
        .await
        .unwrap();
    assert!(
        output.contains("unknown command: /definitely-unknown"),
        "{output:?}"
    );
    assert_eq!(output.exit_code, 0);
}

#[tokio::test]
async fn scripted_interactive_name_updates_footer_session_label() {
    let output = run_scripted_idle_interactive("/name Project Phoenix\r")
        .await
        .unwrap();
    assert!(
        output.contains("Session name set: Project Phoenix"),
        "{output:?}"
    );
    assert!(output.contains("• Project Phoenix"), "{output:?}");
    assert_eq!(output.exit_code, 0);
}
