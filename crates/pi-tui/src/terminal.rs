use std::io::{stdout, Write};

use crossterm::{
    cursor,
    execute,
    terminal::{self, Clear, ClearType},
};

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

impl ProcessTerminal {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProcessTerminal {
    fn default() -> Self {
        Self::new()
    }
}

impl Terminal for ProcessTerminal {
    fn size(&self) -> TerminalSize {
        let (columns, rows) = terminal::size().unwrap_or((80, 24));
        TerminalSize {
            columns: columns as usize,
            rows: rows as usize,
        }
    }

    fn write(&mut self, data: &str) -> std::io::Result<()> {
        stdout().write_all(data.as_bytes())
    }

    fn move_by(&mut self, rows: i16) -> std::io::Result<()> {
        let mut out = stdout();
        if rows < 0 {
            execute!(out, cursor::MoveUp((-rows) as u16))?;
        } else if rows > 0 {
            execute!(out, cursor::MoveDown(rows as u16))?;
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> std::io::Result<()> {
        execute!(stdout(), cursor::Hide)
    }

    fn show_cursor(&mut self) -> std::io::Result<()> {
        execute!(stdout(), cursor::Show)
    }

    fn clear_line(&mut self) -> std::io::Result<()> {
        execute!(stdout(), Clear(ClearType::CurrentLine))
    }

    fn clear_from_cursor(&mut self) -> std::io::Result<()> {
        execute!(stdout(), Clear(ClearType::FromCursorDown))
    }

    fn clear_screen(&mut self) -> std::io::Result<()> {
        execute!(stdout(), Clear(ClearType::All), cursor::MoveTo(0, 0))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        stdout().flush()
    }
}
