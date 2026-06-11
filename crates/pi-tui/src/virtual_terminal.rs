use std::time::Duration;

use crate::{Terminal, TerminalSize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalOp {
    Start,
    Stop,
    DrainInput { max_ms: u64, idle_ms: u64 },
    SetTitle(String),
    SetProgress(bool),
    SetKittyProtocolActive(bool),
    Write(String),
    MoveBy(i16),
    HideCursor,
    ShowCursor,
    ClearLine,
    ClearFromCursor,
    ClearScreen,
    Flush,
}

pub struct VirtualTerminal {
    size: TerminalSize,
    ops: Vec<TerminalOp>,
    kitty_protocol_active: bool,
}

impl VirtualTerminal {
    pub fn new(columns: usize, rows: usize) -> Self {
        Self {
            size: TerminalSize { columns, rows },
            ops: Vec::new(),
            kitty_protocol_active: false,
        }
    }

    pub fn resize(&mut self, columns: usize, rows: usize) {
        self.size = TerminalSize { columns, rows };
    }

    pub fn ops(&self) -> &[TerminalOp] {
        &self.ops
    }

    pub fn clear_ops(&mut self) {
        self.ops.clear();
    }

    pub fn written_output(&self) -> String {
        self.ops
            .iter()
            .filter_map(|op| match op {
                TerminalOp::Write(data) => Some(data.as_str()),
                _ => None,
            })
            .collect()
    }

    pub fn set_kitty_protocol_active(&mut self, active: bool) {
        self.kitty_protocol_active = active;
        self.ops.push(TerminalOp::SetKittyProtocolActive(active));
    }
}

impl Terminal for VirtualTerminal {
    fn size(&self) -> TerminalSize {
        self.size
    }

    fn write(&mut self, data: &str) -> std::io::Result<()> {
        self.ops.push(TerminalOp::Write(data.to_string()));
        Ok(())
    }

    fn move_by(&mut self, rows: i16) -> std::io::Result<()> {
        self.ops.push(TerminalOp::MoveBy(rows));
        Ok(())
    }

    fn hide_cursor(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::HideCursor);
        Ok(())
    }

    fn show_cursor(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::ShowCursor);
        Ok(())
    }

    fn clear_line(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::ClearLine);
        Ok(())
    }

    fn clear_from_cursor(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::ClearFromCursor);
        Ok(())
    }

    fn clear_screen(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::ClearScreen);
        Ok(())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::Flush);
        Ok(())
    }

    fn start(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::Start);
        Ok(())
    }

    fn stop(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::Stop);
        Ok(())
    }

    fn drain_input(&mut self, max: Duration, idle: Duration) -> std::io::Result<()> {
        self.ops.push(TerminalOp::DrainInput {
            max_ms: max.as_millis() as u64,
            idle_ms: idle.as_millis() as u64,
        });
        Ok(())
    }

    fn set_title(&mut self, title: &str) -> std::io::Result<()> {
        self.ops.push(TerminalOp::SetTitle(title.to_string()));
        Ok(())
    }

    fn set_progress(&mut self, active: bool) -> std::io::Result<()> {
        self.ops.push(TerminalOp::SetProgress(active));
        Ok(())
    }

    fn kitty_protocol_active(&self) -> bool {
        self.kitty_protocol_active
    }
}
