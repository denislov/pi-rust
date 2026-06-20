use pi_tui::{
    CURSOR_MARKER, Component, RenderStrategy, TerminalOp, Text, Tui, TuiError, VirtualTerminal,
};

struct RawComponent {
    lines: Vec<String>,
}

impl RawComponent {
    fn new(lines: &[&str]) -> Self {
        Self {
            lines: lines.iter().map(|line| line.to_string()).collect(),
        }
    }
}

impl Component for RawComponent {
    fn render(&mut self, _width: usize) -> Vec<String> {
        self.lines.clone()
    }
}

fn assert_no_global_clear(terminal: &VirtualTerminal) {
    assert!(!terminal.ops().contains(&TerminalOp::ClearScreen));
    assert!(!terminal.ops().contains(&TerminalOp::ClearFromCursor));
    let written = terminal.written_output();
    assert!(!written.contains("\x1b[2J"));
    assert!(!written.contains("\x1b[3J"));
    assert!(!written.contains("\x1b[H"));
}

#[test]
fn first_render_appends_inline_without_clearing_or_homing() {
    let terminal = VirtualTerminal::new(20, 5);
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(Text::new("hello")));

    let outcome = tui.render_once().unwrap();

    assert_eq!(outcome.strategy, RenderStrategy::FullRedraw);
    assert_eq!(outcome.line_count, 1);
    assert_eq!(tui.full_redraws(), 1);
    assert!(tui.terminal().ops().contains(&TerminalOp::HideCursor));
    assert_no_global_clear(tui.terminal());
    assert!(tui.terminal().written_output().contains("\x1b[?2026h"));
    assert!(
        tui.terminal()
            .written_output()
            .contains("hello\x1b[0m\x1b]8;;\x07")
    );
    assert!(tui.terminal().written_output().contains("\x1b[?2026l"));
}

#[test]
fn full_render_writes_lines_with_carriage_return_newline() {
    let terminal = VirtualTerminal::new(20, 5);
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(RawComponent::new(&["one", "two"])));

    tui.render_once().unwrap();

    assert!(
        tui.terminal()
            .written_output()
            .contains("one\x1b[0m\x1b]8;;\x07\r\ntwo")
    );
}

#[test]
fn line_too_wide_errors_before_writing() {
    let terminal = VirtualTerminal::new(4, 5);
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(RawComponent::new(&["too wide"])));

    let err = tui.render_once().unwrap_err();

    match err {
        TuiError::LineTooWide {
            line_index,
            width,
            max_width,
            ..
        } => {
            assert_eq!(line_index, 0);
            assert_eq!(width, 8);
            assert_eq!(max_width, 4);
        }
        other => panic!("expected LineTooWide, got {other:?}"),
    }
    assert!(tui.terminal().ops().is_empty());
}

#[test]
fn differential_render_returns_to_column_zero_before_clearing() {
    let terminal = VirtualTerminal::new(20, 5);
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(RawComponent::new(&["header", "working"])));
    tui.render_once().unwrap();
    tui.terminal_mut().clear_ops();

    tui.clear_children();
    tui.add_child(Box::new(RawComponent::new(&["header", "done"])));
    tui.render_once().unwrap();

    let ops = tui.terminal().ops();
    let move_to_column = ops
        .iter()
        .position(|op| *op == TerminalOp::MoveToColumn(0))
        .expect("expected differential render to return to column zero");
    let clear_line = ops
        .iter()
        .position(|op| *op == TerminalOp::ClearLine)
        .expect("expected differential render to clear the changed owned line");
    assert!(move_to_column < clear_line);
    assert_no_global_clear(tui.terminal());
}

#[test]
fn differential_render_moves_from_actual_hardware_cursor_row() {
    let terminal = VirtualTerminal::new(20, 5);
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(RawComponent {
        lines: vec![format!("a{CURSOR_MARKER}bc"), "old".to_string()],
    }));
    tui.render_once().unwrap();
    tui.terminal_mut().clear_ops();

    tui.clear_children();
    tui.add_child(Box::new(RawComponent {
        lines: vec![format!("a{CURSOR_MARKER}bc"), "new".to_string()],
    }));
    tui.render_once().unwrap();

    assert!(tui.terminal().ops().contains(&TerminalOp::MoveBy(1)));
}

#[test]
fn unchanged_render_still_repositions_hardware_cursor() {
    let terminal = VirtualTerminal::new(20, 5);
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(RawComponent {
        lines: vec![format!("a{CURSOR_MARKER}bc")],
    }));
    tui.render_once().unwrap();
    tui.terminal_mut().clear_ops();

    tui.clear_children();
    tui.add_child(Box::new(RawComponent {
        lines: vec![format!("ab{CURSOR_MARKER}c")],
    }));
    let outcome = tui.render_once().unwrap();

    assert_eq!(outcome.strategy, RenderStrategy::NoChange);
    let ops = tui.terminal().ops();
    let move_to_column = ops
        .iter()
        .position(|op| *op == TerminalOp::MoveToColumn(2))
        .expect("expected cursor movement to move to marker column");
    ops[move_to_column + 1..]
        .iter()
        .position(|op| *op == TerminalOp::Flush)
        .expect("expected marker-only cursor movement to flush");
}

#[test]
fn second_render_updates_from_first_changed_line_without_full_clear() {
    let terminal = VirtualTerminal::new(20, 5);
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(RawComponent::new(&[
        "header", "working", "footer",
    ])));
    tui.render_once().unwrap();
    tui.terminal_mut().clear_ops();

    tui.clear_children();
    tui.add_child(Box::new(RawComponent::new(&["header", "done", "footer"])));
    let outcome = tui.render_once().unwrap();

    assert_eq!(
        outcome.strategy,
        RenderStrategy::Differential {
            first_changed_line: 1
        }
    );
    assert_no_global_clear(tui.terminal());
    assert!(tui.terminal().ops().contains(&TerminalOp::MoveBy(-1)));
    assert!(tui.terminal().ops().contains(&TerminalOp::ClearLine));
    assert!(tui.terminal().written_output().contains("done"));
}

#[test]
fn differential_render_writes_growth_beyond_terminal_height() {
    let terminal = VirtualTerminal::new(20, 4);
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(RawComponent::new(&["welcome", "> ", "footer"])));
    tui.render_once().unwrap();
    tui.terminal_mut().clear_ops();

    tui.clear_children();
    tui.add_child(Box::new(RawComponent::new(&[
        "welcome", "one", "two", "three", "four", "five", "six", "> ", "footer",
    ])));
    let outcome = tui.render_once().unwrap();

    assert_eq!(
        outcome.strategy,
        RenderStrategy::Differential {
            first_changed_line: 1
        }
    );
    assert_no_global_clear(tui.terminal());
    let written = tui.terminal().written_output();
    assert!(written.contains("one"), "{written:?}");
    assert!(written.contains("six"), "{written:?}");
    assert!(written.contains("footer"), "{written:?}");
}

#[test]
fn width_change_triggers_scoped_redraw_without_global_clear() {
    let terminal = VirtualTerminal::new(20, 5);
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(Text::new("hello")));
    tui.render_once().unwrap();
    tui.terminal_mut().clear_ops();
    tui.terminal_mut().resize(30, 5);

    let outcome = tui.render_once().unwrap();

    assert_eq!(outcome.strategy, RenderStrategy::FullRedraw);
    assert_no_global_clear(tui.terminal());
    assert!(tui.terminal().ops().contains(&TerminalOp::ClearLine));
    assert!(tui.terminal().written_output().contains("hello"));
}

#[test]
fn shrink_with_clear_on_shrink_clears_only_owned_rows() {
    let terminal = VirtualTerminal::new(20, 5);
    let mut tui = Tui::new(terminal);
    tui.set_clear_on_shrink(true);
    tui.add_child(Box::new(RawComponent::new(&["one", "two", "three"])));
    tui.render_once().unwrap();
    tui.terminal_mut().clear_ops();

    tui.clear_children();
    tui.add_child(Box::new(RawComponent::new(&["one"])));
    let outcome = tui.render_once().unwrap();

    assert_eq!(outcome.strategy, RenderStrategy::FullRedraw);
    assert_no_global_clear(tui.terminal());
    let cleared_rows = tui
        .terminal()
        .ops()
        .iter()
        .filter(|op| **op == TerminalOp::ClearLine)
        .count();
    assert_eq!(cleared_rows, 3);
    assert!(tui.terminal().written_output().contains("one"));
    assert!(!tui.terminal().written_output().contains("two"));
}

#[test]
fn unchanged_render_reports_no_change() {
    let terminal = VirtualTerminal::new(20, 5);
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(Text::new("hello")));
    tui.render_once().unwrap();
    tui.terminal_mut().clear_ops();

    let outcome = tui.render_once().unwrap();

    assert_eq!(outcome.strategy, RenderStrategy::NoChange);
    assert!(tui.terminal().ops().is_empty());
}
