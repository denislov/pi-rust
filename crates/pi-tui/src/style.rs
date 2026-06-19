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
    Ansi256(u8),
    Rgb(u8, u8, u8),
}

impl Color {
    fn ansi16_code(self) -> Option<u8> {
        match self {
            Color::Default => None,
            Color::Red => Some(1),
            Color::Green => Some(2),
            Color::Yellow => Some(3),
            Color::Blue => Some(4),
            Color::Magenta => Some(5),
            Color::Cyan => Some(6),
            Color::White => Some(7),
            Color::Ansi256(_) | Color::Rgb(_, _, _) => None,
        }
    }

    fn sgr_params(self, foreground: bool) -> Vec<String> {
        match self {
            Color::Default => Vec::new(),
            Color::Ansi256(index) => {
                vec![
                    if foreground { "38" } else { "48" }.to_string(),
                    "5".to_string(),
                    index.to_string(),
                ]
            }
            Color::Rgb(red, green, blue) => {
                vec![
                    if foreground { "38" } else { "48" }.to_string(),
                    "2".to_string(),
                    red.to_string(),
                    green.to_string(),
                    blue.to_string(),
                ]
            }
            color => color
                .ansi16_code()
                .map(|code| vec![format!("{}{code}", if foreground { "3" } else { "4" })])
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum ColorLevel {
    #[default]
    None,
    Ansi16,
    Ansi256,
    TrueColor,
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
    paint_with_level(
        text,
        style,
        if enabled {
            ColorLevel::TrueColor
        } else {
            ColorLevel::None
        },
    )
}

pub fn paint_with_level(text: &str, style: &Style, level: ColorLevel) -> String {
    if level == ColorLevel::None || !style.has_any() {
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
    params.extend(style.fg.sgr_params(true));
    params.extend(style.bg.sgr_params(false));

    format!("\x1b[{}m{}\x1b[0m", params.join(";"), text)
}

static CACHED_COLOR_LEVEL: OnceLock<ColorLevel> = OnceLock::new();

pub fn color_enabled() -> bool {
    color_level() != ColorLevel::None
}

pub fn color_level() -> ColorLevel {
    *CACHED_COLOR_LEVEL.get_or_init(|| detect_color_level_from_env(std::env::vars()))
}

pub fn detect_color_level_from_env<I, K, V>(env: I) -> ColorLevel
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    let mut no_color = false;
    let mut term = String::new();
    let mut color_term = String::new();
    let mut term_program = String::new();
    let mut terminal_emulator = String::new();
    let mut kitty = false;
    let mut ghostty = false;
    let mut wezterm = false;
    let mut iterm = false;
    let mut windows_terminal = false;

    for (key, value) in env {
        let key = key.as_ref();
        let value = value.as_ref();
        match key {
            "NO_COLOR" => no_color = true,
            "TERM" => term = value.to_lowercase(),
            "COLORTERM" => color_term = value.to_lowercase(),
            "TERM_PROGRAM" => term_program = value.to_lowercase(),
            "TERMINAL_EMULATOR" => terminal_emulator = value.to_lowercase(),
            "KITTY_WINDOW_ID" => kitty = true,
            "GHOSTTY_RESOURCES_DIR" => ghostty = true,
            "WEZTERM_PANE" => wezterm = true,
            "ITERM_SESSION_ID" => iterm = true,
            "WT_SESSION" => windows_terminal = true,
            _ => {}
        }
    }

    if no_color || term == "dumb" {
        return ColorLevel::None;
    }

    if matches!(color_term.as_str(), "truecolor" | "24bit")
        || kitty
        || ghostty
        || wezterm
        || iterm
        || windows_terminal
        || matches!(
            term_program.as_str(),
            "kitty" | "ghostty" | "wezterm" | "iterm.app" | "vscode" | "alacritty"
        )
        || terminal_emulator == "jetbrains-jediterm"
        || term.contains("ghostty")
    {
        return ColorLevel::TrueColor;
    }

    if term.contains("256color") {
        return ColorLevel::Ansi256;
    }

    if term.is_empty() {
        ColorLevel::None
    } else {
        ColorLevel::Ansi16
    }
}

pub const USER: Style = Style::fg(Color::Cyan);
pub const TOOL_NAME: Style = Style::fg(Color::Yellow);
pub const ERROR: Style = Style::fg(Color::Red).bold();
pub const TOOL_ERROR: Style = Style::fg(Color::Red);
pub const SYSTEM: Style = Style::fg(Color::Default).dim();
pub const STATUS_IDLE: Style = Style::fg(Color::Default).dim();
pub const STATUS_RUNNING: Style = Style::fg(Color::Yellow);
pub const PATH: Style = Style::fg(Color::Cyan);
