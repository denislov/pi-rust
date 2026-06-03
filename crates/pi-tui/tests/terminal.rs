use pi_tui::{Terminal, TerminalOp, TerminalSize, VirtualTerminal};

#[test]
fn virtual_terminal_records_operations() {
    let mut terminal = VirtualTerminal::new(12, 4);

    terminal.hide_cursor().unwrap();
    terminal.write("hello").unwrap();
    terminal.move_by(-2).unwrap();
    terminal.clear_from_cursor().unwrap();
    terminal.flush().unwrap();

    assert_eq!(
        terminal.size(),
        TerminalSize {
            columns: 12,
            rows: 4
        }
    );
    assert_eq!(
        terminal.ops(),
        &[
            TerminalOp::HideCursor,
            TerminalOp::Write("hello".to_string()),
            TerminalOp::MoveBy(-2),
            TerminalOp::ClearFromCursor,
            TerminalOp::Flush,
        ]
    );
}

#[test]
fn virtual_terminal_can_resize_and_clear_ops() {
    let mut terminal = VirtualTerminal::new(12, 4);
    terminal.write("hello").unwrap();
    terminal.resize(20, 8);
    terminal.clear_ops();

    assert_eq!(
        terminal.size(),
        TerminalSize {
            columns: 20,
            rows: 8
        }
    );
    assert!(terminal.ops().is_empty());
}
