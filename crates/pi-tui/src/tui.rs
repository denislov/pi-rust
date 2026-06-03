use crate::Terminal;

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
}

impl<T: Terminal> Tui<T> {
    pub fn new(terminal: T) -> Self {
        Self { terminal }
    }

    pub fn terminal(&self) -> &T {
        &self.terminal
    }
}
