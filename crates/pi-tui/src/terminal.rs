#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub columns: usize,
    pub rows: usize,
}

pub trait Terminal {
    fn size(&self) -> TerminalSize;
    fn write(&mut self, data: &str) -> std::io::Result<()>;
    fn move_by(&mut self, rows: i16) -> std::io::Result<()>;
    fn hide_cursor(&mut self) -> std::io::Result<()>;
    fn show_cursor(&mut self) -> std::io::Result<()>;
    fn clear_line(&mut self) -> std::io::Result<()>;
    fn clear_from_cursor(&mut self) -> std::io::Result<()>;
    fn clear_screen(&mut self) -> std::io::Result<()>;
    fn flush(&mut self) -> std::io::Result<()>;
}

pub struct ProcessTerminal;
