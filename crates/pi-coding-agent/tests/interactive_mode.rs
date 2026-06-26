use std::sync::{Arc, Mutex};

use futures::stream;
use pi_ai::providers::faux::{FauxProvider, FauxResponse};
use pi_ai::registry::{self, ApiProvider};
use pi_ai::stream::EventStream;
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Model, StopReason,
    StreamOptions,
};
use pi_coding_agent::interactive::test_harness::{
    run_scripted_idle_interactive, run_scripted_idle_interactive_with_delays,
    run_scripted_idle_interactive_with_size, run_scripted_interactive,
    run_scripted_interactive_with_session_dir_size_and_waits,
};
use pi_tui::TerminalOp;

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
async fn scripted_interactive_clone_after_prompt_forks_current_session() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![text_response("assistant reply")]);

    let output = run_scripted_interactive_with_session_dir_size_and_waits(
        provider,
        dir.path(),
        vec![
            ("hello\r", "assistant reply"),
            ("/clone\r", "parentSession"),
        ],
        80,
        24,
    )
    .await
    .unwrap();

    let files = collect_jsonl_files(dir.path());
    assert_eq!(files.len(), 2, "{files:?}");
    assert!(
        output.contains("Cloned to new session"),
        "{}",
        output.rendered
    );
    assert!(
        files.iter().any(|path| std::fs::read_to_string(path)
            .map(|text| text.contains("parentSession"))
            .unwrap_or(false)),
        "{files:?}"
    );
}

#[tokio::test]
async fn scripted_interactive_fork_after_prompt_creates_parent_session() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![text_response("assistant reply")]);

    let output = run_scripted_interactive_with_session_dir_size_and_waits(
        provider,
        dir.path(),
        vec![("hello\r", "assistant reply"), ("/fork\r", "parentSession")],
        80,
        24,
    )
    .await
    .unwrap();

    let files = collect_jsonl_files(dir.path());
    assert_eq!(files.len(), 2, "{files:?}");
    assert!(
        output.contains("Forked to new session"),
        "{}",
        output.rendered
    );
    assert!(
        files.iter().any(|path| std::fs::read_to_string(path)
            .map(|text| text.contains("parentSession"))
            .unwrap_or(false)),
        "{files:?}"
    );
}

#[tokio::test]
async fn scripted_interactive_compact_after_prompt_appends_compaction_entry() {
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
            ("/compact keep decisions\r", "summary from compact"),
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
        session_text.contains("\"type\":\"compaction\""),
        "{session_text}"
    );
    assert!(
        session_text.contains("summary from compact"),
        "{session_text}"
    );
    assert!(
        session_text.contains("\"firstKeptEntryId\""),
        "{session_text}"
    );
    assert!(
        output.contains("summary from compact"),
        "{}",
        output.rendered
    );
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
    assert!(frame.contains("(1/22)"), "{frame}");
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
    let prior_pi_rust_dir = std::env::var_os("PI_RUST_DIR");
    unsafe {
        std::env::set_var("PI_RUST_DIR", dir.path());
    }

    let output = run_scripted_idle_interactive("/model\rclaude-haiku-4-5\r").await;

    unsafe {
        match prior_pi_rust_dir {
            Some(value) => std::env::set_var("PI_RUST_DIR", value),
            None => std::env::remove_var("PI_RUST_DIR"),
        }
    }
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
    let prior_pi_rust_dir = std::env::var_os("PI_RUST_DIR");
    let prior_anthropic_key = std::env::var_os("ANTHROPIC_API_KEY");
    let prior_claude_key = std::env::var_os("CLAUDE_API_KEY");
    let prior_anthropic_key_alt = std::env::var_os("ANTHROPIC_KEY");
    unsafe {
        std::env::set_var("PI_RUST_DIR", dir.path());
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("CLAUDE_API_KEY");
        std::env::remove_var("ANTHROPIC_KEY");
    }

    let output = run_scripted_idle_interactive("/model\r").await;

    unsafe {
        match prior_pi_rust_dir {
            Some(value) => std::env::set_var("PI_RUST_DIR", value),
            None => std::env::remove_var("PI_RUST_DIR"),
        }
        match prior_anthropic_key {
            Some(value) => std::env::set_var("ANTHROPIC_API_KEY", value),
            None => std::env::remove_var("ANTHROPIC_API_KEY"),
        }
        match prior_claude_key {
            Some(value) => std::env::set_var("CLAUDE_API_KEY", value),
            None => std::env::remove_var("CLAUDE_API_KEY"),
        }
        match prior_anthropic_key_alt {
            Some(value) => std::env::set_var("ANTHROPIC_KEY", value),
            None => std::env::remove_var("ANTHROPIC_KEY"),
        }
    }

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
    let prior_pi_rust_dir = std::env::var_os("PI_RUST_DIR");
    let prior_openai_key = std::env::var_os("OPENAI_API_KEY");
    let prior_anthropic_key = std::env::var_os("ANTHROPIC_API_KEY");
    unsafe {
        std::env::set_var("PI_RUST_DIR", dir.path());
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    let output = run_scripted_idle_interactive("/model gpt-5\rhi\r").await;

    unsafe {
        match prior_pi_rust_dir {
            Some(value) => std::env::set_var("PI_RUST_DIR", value),
            None => std::env::remove_var("PI_RUST_DIR"),
        }
        match prior_openai_key {
            Some(value) => std::env::set_var("OPENAI_API_KEY", value),
            None => std::env::remove_var("OPENAI_API_KEY"),
        }
        match prior_anthropic_key {
            Some(value) => std::env::set_var("ANTHROPIC_API_KEY", value),
            None => std::env::remove_var("ANTHROPIC_API_KEY"),
        }
    }
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
