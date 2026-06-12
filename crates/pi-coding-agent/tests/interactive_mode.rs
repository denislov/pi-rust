use pi_ai::providers::faux::{FauxProvider, FauxResponse};
use pi_coding_agent::interactive::test_harness::{
    run_scripted_idle_interactive, run_scripted_interactive,
};
use pi_tui::TerminalOp;

fn text_response(text: &str) -> FauxResponse {
    FauxResponse {
        text_deltas: vec![text.to_string()],
        thinking_deltas: vec![],
        tool_calls: vec![],
    }
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
