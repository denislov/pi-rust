use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Color {
    #[default]
    Default,
    Red,
    Green,
    Yellow,
    Blue,
    Cyan,
    Magenta,
    White,
}

impl Color {
    fn fg_code(self) -> Option<u8> {
        match self {
            Color::Default => None,
            Color::Red => Some(1),
            Color::Green => Some(2),
            Color::Yellow => Some(3),
            Color::Blue => Some(4),
            Color::Magenta => Some(5),
            Color::Cyan => Some(6),
            Color::White => Some(7),
        }
    }

    fn bg_code(self) -> Option<u8> {
        self.fg_code()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub dim: bool,
    pub reverse: bool,
}

impl Style {
    pub const fn fg(color: Color) -> Self {
        Self {
            fg: color,
            bg: Color::Default,
            bold: false,
            dim: false,
            reverse: false,
        }
    }

    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub const fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    pub const fn reverse(mut self) -> Self {
        self.reverse = true;
        self
    }

    fn has_any(&self) -> bool {
        self.fg != Color::Default
            || self.bg != Color::Default
            || self.bold
            || self.dim
            || self.reverse
    }
}

pub fn paint(text: &str, style: &Style) -> String {
    paint_with(text, style, color_enabled())
}

pub fn paint_with(text: &str, style: &Style, enabled: bool) -> String {
    if !enabled || !style.has_any() {
        return text.to_string();
    }

    let mut params: Vec<String> = Vec::new();
    if style.bold {
        params.push("1".to_string());
    }
    if style.dim {
        params.push("2".to_string());
    }
    if style.reverse {
        params.push("7".to_string());
    }
    if let Some(code) = style.fg.fg_code() {
        params.push(format!("3{code}"));
    }
    if let Some(code) = style.bg.bg_code() {
        params.push(format!("4{code}"));
    }

    format!("\x1b[{}m{}\x1b[0m", params.join(";"), text)
}

static CACHED: OnceLock<bool> = OnceLock::new();

pub fn color_enabled() -> bool {
    *CACHED.get_or_init(|| {
        !(std::env::var_os("NO_COLOR").is_some()
            || std::env::var("TERM").ok().as_deref() == Some("dumb"))
    })
}

pub const USER: Style = Style::fg(Color::Cyan);
pub const TOOL_NAME: Style = Style::fg(Color::Yellow);
pub const ERROR: Style = Style::fg(Color::Red).bold();
pub const TOOL_ERROR: Style = Style::fg(Color::Red);
pub const SYSTEM: Style = Style::fg(Color::Default).dim();
pub const STATUS_IDLE: Style = Style::fg(Color::Default).dim();
pub const STATUS_RUNNING: Style = Style::fg(Color::Yellow);
pub const PATH: Style = Style::fg(Color::Cyan);
