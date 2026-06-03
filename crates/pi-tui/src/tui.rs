use crate::{Component, Terminal, visible_width};

const SYNC_START: &str = "\x1b[?2026h";
const SYNC_END: &str = "\x1b[?2026l";
const LINE_RESET: &str = "\x1b[0m\x1b]8;;\x07";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderStrategy {
    FullRedraw,
    Differential { first_changed_line: usize },
    NoChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOutcome {
    pub strategy: RenderStrategy,
    pub line_count: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("terminal I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("line {line_index} is {width} columns wide, exceeding max width {max_width}: {line:?}")]
    LineTooWide {
        line_index: usize,
        width: usize,
        max_width: usize,
        line: String,
    },
}

pub struct Tui<T: Terminal> {
    terminal: T,
    children: Vec<Box<dyn Component>>,
    previous_lines: Vec<String>,
    previous_width: usize,
    previous_height: usize,
    cursor_row: usize,
    clear_on_shrink: bool,
    full_redraws: usize,
}

impl<T: Terminal> Tui<T> {
    pub fn new(terminal: T) -> Self {
        Self {
            terminal,
            children: Vec::new(),
            previous_lines: Vec::new(),
            previous_width: 0,
            previous_height: 0,
            cursor_row: 0,
            clear_on_shrink: true,
            full_redraws: 0,
        }
    }

    pub fn terminal(&self) -> &T {
        &self.terminal
    }

    pub fn terminal_mut(&mut self) -> &mut T {
        &mut self.terminal
    }

    pub fn add_child(&mut self, child: Box<dyn Component>) {
        self.children.push(child);
    }

    pub fn clear_children(&mut self) {
        self.children.clear();
    }

    pub fn full_redraws(&self) -> usize {
        self.full_redraws
    }

    pub fn set_clear_on_shrink(&mut self, enabled: bool) {
        self.clear_on_shrink = enabled;
    }

    pub fn render_once(&mut self) -> Result<RenderOutcome, TuiError> {
        let size = self.terminal.size();
        let width = size.columns;
        let height = size.rows;
        let lines = self.render_lines(width);
        validate_lines(&lines, width)?;

        let strategy = self.choose_strategy(&lines, width, height);
        match strategy {
            RenderStrategy::NoChange => {}
            RenderStrategy::FullRedraw => self.render_full(&lines)?,
            RenderStrategy::Differential { first_changed_line } => {
                self.render_differential(&lines, first_changed_line)?
            }
        }

        self.previous_lines = lines.clone();
        self.previous_width = width;
        self.previous_height = height;
        self.cursor_row = lines.len().saturating_sub(1);

        Ok(RenderOutcome {
            strategy,
            line_count: lines.len(),
        })
    }

    fn render_lines(&mut self, width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        for child in &mut self.children {
            lines.extend(child.render(width));
        }
        lines
    }

    fn choose_strategy(&self, lines: &[String], width: usize, height: usize) -> RenderStrategy {
        if self.previous_width == 0 || self.previous_height == 0 {
            return RenderStrategy::FullRedraw;
        }
        if self.previous_width != width || self.previous_height != height {
            return RenderStrategy::FullRedraw;
        }
        if self.clear_on_shrink && lines.len() < self.previous_lines.len() {
            return RenderStrategy::FullRedraw;
        }
        first_changed_line(&self.previous_lines, lines)
            .map(|first_changed_line| RenderStrategy::Differential { first_changed_line })
            .unwrap_or(RenderStrategy::NoChange)
    }

    fn render_full(&mut self, lines: &[String]) -> Result<(), TuiError> {
        self.full_redraws += 1;
        self.terminal.write(SYNC_START)?;
        self.terminal.hide_cursor()?;
        self.terminal.clear_screen()?;
        self.write_lines(lines)?;
        self.terminal.write(SYNC_END)?;
        self.terminal.flush()?;
        Ok(())
    }

    fn render_differential(
        &mut self,
        lines: &[String],
        first_changed_line: usize,
    ) -> Result<(), TuiError> {
        self.terminal.write(SYNC_START)?;
        let target = first_changed_line as i16;
        let current = self.cursor_row as i16;
        self.terminal.move_by(target - current)?;
        self.terminal.clear_from_cursor()?;
        self.write_lines(&lines[first_changed_line..])?;
        self.terminal.write(SYNC_END)?;
        self.terminal.flush()?;
        Ok(())
    }

    fn write_lines(&mut self, lines: &[String]) -> Result<(), TuiError> {
        for (index, line) in lines.iter().enumerate() {
            self.terminal.write(line)?;
            self.terminal.write(LINE_RESET)?;
            if index + 1 < lines.len() {
                self.terminal.write("\n")?;
            }
        }
        Ok(())
    }
}

fn validate_lines(lines: &[String], max_width: usize) -> Result<(), TuiError> {
    for (line_index, line) in lines.iter().enumerate() {
        let width = visible_width(line);
        if width > max_width {
            return Err(TuiError::LineTooWide {
                line_index,
                width,
                max_width,
                line: line.clone(),
            });
        }
    }
    Ok(())
}

fn first_changed_line(previous: &[String], next: &[String]) -> Option<usize> {
    let shared = previous.len().min(next.len());
    for index in 0..shared {
        if previous[index] != next[index] {
            return Some(index);
        }
    }
    if previous.len() != next.len() {
        Some(shared)
    } else {
        None
    }
}
