//! Terminal start/stop and capability lifecycle behavior.

use pi_tui::api::terminal::Terminal;
use pi_tui::api::testing::{TerminalOp, VirtualTerminal};

#[test]
fn virtual_terminal_records_lifecycle_operations() {
    let mut terminal = VirtualTerminal::new(80, 24);
    terminal.start().unwrap();
    terminal.set_title("pi").unwrap();
    terminal.set_progress(true).unwrap();
    terminal.set_progress(false).unwrap();
    terminal.stop().unwrap();

    assert_eq!(
        terminal.ops(),
        &[
            TerminalOp::Start,
            TerminalOp::SetTitle("pi".to_string()),
            TerminalOp::SetProgress(true),
            TerminalOp::SetProgress(false),
            TerminalOp::Stop,
        ]
    );
}

#[test]
fn virtual_terminal_reports_kitty_protocol_state() {
    let mut terminal = VirtualTerminal::new(80, 24);
    assert!(!terminal.kitty_protocol_active());
    terminal.set_kitty_protocol_active(true);
    assert!(terminal.kitty_protocol_active());
}
