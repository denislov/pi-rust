use pi_ai::providers::faux::{FauxProvider, FauxResponse};
use pi_ai::types::StopReason;
use pi_coding_agent::interactive::test_harness::{
    run_scripted_idle_interactive, run_scripted_interactive,
    run_scripted_interactive_with_session_dir_size_and_waits,
};
use pi_tui::TerminalOp;

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
async fn scripted_interactive_renders_assistant_markdown() {
    let provider = FauxProvider::new(vec![text_response(
        "# Title\n\nA paragraph with **bold** text and `code`.\n\n- one",
    )]);
    let output = run_scripted_interactive(provider, "format\r")
        .await
        .unwrap();
    let frame = output.rendered_lines.join("\n");

    assert!(frame.contains("Title"), "{frame}");
    assert!(frame.contains("A paragraph with bold text and"), "{frame}");
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

fn sync_render_count(ops: &[TerminalOp]) -> usize {
    ops.iter()
        .filter(|op| matches!(op, TerminalOp::Write(data) if data.contains("\x1b[?2026h")))
        .count()
}

#[tokio::test]
async fn scripted_interactive_keeps_prompt_anchored_below_transcript_viewport() {
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

    assert!(!frame.contains("one"));
    assert!(frame.contains("six"));
    assert!(frame.contains("> typed"));
    assert!(frame.contains("status: idle"));
    assert_eq!(
        output
            .rendered_lines
            .iter()
            .position(|line| line.contains("> typed")),
        Some(4)
    );
}

#[tokio::test]
async fn scripted_interactive_page_up_locks_transcript_until_page_down_returns_bottom() {
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
    assert!(!frame.contains("six"));
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

    assert!(!frame.contains("one"));
    assert!(frame.contains("six"));
    assert!(frame.contains("> "));
    assert!(frame.contains("status: idle"));
}

#[tokio::test]
async fn scripted_interactive_new_output_does_not_unlock_scrolled_transcript() {
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
    assert!(!frame.contains("brand new bottom"), "{frame}");
    assert!(frame.contains("new output below"), "{frame}");
    assert!(frame.contains("> "), "{frame}");
    assert!(frame.contains("status: idle"), "{frame}");
}

#[tokio::test]
async fn scripted_interactive_shows_welcome_line_on_empty_transcript() {
    let output = run_scripted_idle_interactive("").await.unwrap();
    let frame = output.rendered_lines.join("\n");
    assert!(frame.contains("pi · "), "welcome line missing: {frame}");
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
