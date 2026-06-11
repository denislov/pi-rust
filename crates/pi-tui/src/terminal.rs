use std::io::{Write, stdout};
use std::time::Duration;

use crossterm::{
    cursor, execute,
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

    fn start(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn stop(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn drain_input(&mut self, _max: Duration, _idle: Duration) -> std::io::Result<()> {
        Ok(())
    }

    fn set_title(&mut self, _title: &str) -> std::io::Result<()> {
        Ok(())
    }

    fn set_progress(&mut self, _active: bool) -> std::io::Result<()> {
        Ok(())
    }

    fn kitty_protocol_active(&self) -> bool {
        false
    }
}

pub struct ProcessTerminal {
    raw_mode_enabled_by_us: bool,
    kitty_protocol_active: bool,
}

impl ProcessTerminal {
    pub fn new() -> Self {
        Self {
            raw_mode_enabled_by_us: false,
            kitty_protocol_active: false,
        }
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

    fn start(&mut self) -> std::io::Result<()> {
        let already_raw = terminal::is_raw_mode_enabled().unwrap_or(false);
        if !already_raw {
            terminal::enable_raw_mode()?;
            self.raw_mode_enabled_by_us = true;
        }
        self.write("\x1b[?2004h")?;
        self.write("\x1b[>7u\x1b[?u\x1b[c")?;
        self.hide_cursor()?;
        self.flush()
    }

    fn stop(&mut self) -> std::io::Result<()> {
        self.write("\x1b[?2004l")?;
        self.write("\x1b[<u")?;
        self.show_cursor()?;
        self.flush()?;
        if self.raw_mode_enabled_by_us {
            terminal::disable_raw_mode()?;
            self.raw_mode_enabled_by_us = false;
        }
        self.kitty_protocol_active = false;
        Ok(())
    }

    fn set_title(&mut self, title: &str) -> std::io::Result<()> {
        self.write(&format!("\x1b]0;{title}\x07"))
    }

    fn set_progress(&mut self, active: bool) -> std::io::Result<()> {
        if active {
            self.write("\x1b]9;4;3\x07")
        } else {
            self.write("\x1b]9;4;0;\x07")
        }
    }

    fn kitty_protocol_active(&self) -> bool {
        self.kitty_protocol_active
    }
}
