use crate::{Terminal, TerminalSize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalOp {
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
}

impl VirtualTerminal {
    pub fn new(columns: usize, rows: usize) -> Self {
        Self {
            size: TerminalSize { columns, rows },
            ops: Vec::new(),
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
}
