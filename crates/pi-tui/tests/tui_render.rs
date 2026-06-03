use pi_tui::{
    Component, RenderStrategy, TerminalOp, Text, Tui, TuiError, VirtualTerminal,
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

#[test]
fn first_render_uses_synchronized_full_redraw() {
    let terminal = VirtualTerminal::new(20, 5);
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(Text::new("hello")));

    let outcome = tui.render_once().unwrap();

    assert_eq!(outcome.strategy, RenderStrategy::FullRedraw);
    assert_eq!(outcome.line_count, 1);
    assert_eq!(tui.full_redraws(), 1);
    assert!(tui.terminal().ops().contains(&TerminalOp::HideCursor));
    assert!(tui.terminal().ops().contains(&TerminalOp::ClearScreen));
    assert!(tui.terminal().written_output().contains("\x1b[?2026h"));
    assert!(tui.terminal().written_output().contains("hello\x1b[0m\x1b]8;;\x07"));
    assert!(tui.terminal().written_output().contains("\x1b[?2026l"));
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
