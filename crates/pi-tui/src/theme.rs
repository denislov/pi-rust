use crate::{Color, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePalette {
    pub accent: Color,
    pub muted: Color,
    pub text: Color,
    pub background: Color,
    pub error: Color,
    pub success: Color,
    pub warning: Color,
    pub path: Color,
}

impl ThemePalette {
    pub const fn dark() -> Self {
        Self {
            accent: Color::Cyan,
            muted: Color::Ansi256(244),
            text: Color::White,
            background: Color::Default,
            error: Color::Red,
            success: Color::Green,
            warning: Color::Yellow,
            path: Color::Cyan,
        }
    }

    pub const fn light() -> Self {
        Self {
            accent: Color::Blue,
            muted: Color::Ansi256(242),
            text: Color::Default,
            background: Color::White,
            error: Color::Red,
            success: Color::Green,
            warning: Color::Yellow,
            path: Color::Blue,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkdownTheme {
    pub heading: Style,
    pub link: Style,
    pub link_url: Style,
    pub code: Style,
    pub code_block: Style,
    pub code_block_border: Style,
    pub quote: Style,
    pub quote_border: Style,
    pub hr: Style,
    pub list_bullet: Style,
    pub bold: Style,
}

impl Default for MarkdownTheme {
    fn default() -> Self {
        Self {
            heading: Style::fg(Color::Default).bold(),
            link: Style::fg(Color::Cyan),
            link_url: Style::fg(Color::Default).dim(),
            code: Style::default().reverse(),
            code_block: Style::fg(Color::Default).dim(),
            code_block_border: Style::fg(Color::Default).dim(),
            quote: Style::fg(Color::Default).dim(),
            quote_border: Style::fg(Color::Default).dim(),
            hr: Style::fg(Color::Default).dim(),
            list_bullet: Style::fg(Color::Default),
            bold: Style::fg(Color::Default).bold(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectListTheme {
    pub selected_prefix: Style,
    pub selected_text: Style,
    pub description: Style,
    pub scroll_info: Style,
    pub no_match: Style,
}

impl Default for SelectListTheme {
    fn default() -> Self {
        Self {
            selected_prefix: Style::fg(Color::Default),
            selected_text: Style::fg(Color::Default),
            description: Style::fg(Color::Default).dim(),
            scroll_info: Style::fg(Color::Default).dim(),
            no_match: Style::fg(Color::Default).dim(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsListTheme {
    pub label: Style,
    pub selected_label: Style,
    pub value: Style,
    pub selected_value: Style,
    pub description: Style,
    pub cursor: Style,
    pub hint: Style,
}

impl Default for SettingsListTheme {
    fn default() -> Self {
        Self {
            label: Style::fg(Color::Default),
            selected_label: Style::fg(Color::Default).bold(),
            value: Style::fg(Color::Default).dim(),
            selected_value: Style::fg(Color::Default).bold(),
            description: Style::fg(Color::Default).dim(),
            cursor: Style::fg(Color::Default).bold(),
            hint: Style::fg(Color::Default).dim(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorTheme {
    pub border: Style,
    pub placeholder: Style,
    pub select_list: SelectListTheme,
}

impl Default for EditorTheme {
    fn default() -> Self {
        Self {
            border: Style::fg(Color::Default).dim(),
            placeholder: Style::fg(Color::Default).dim(),
            select_list: SelectListTheme::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiTheme {
    pub name: String,
    pub mode: ThemeMode,
    pub palette: ThemePalette,
    pub markdown: MarkdownTheme,
    pub select_list: SelectListTheme,
    pub settings_list: SettingsListTheme,
    pub editor: EditorTheme,
}

impl TuiTheme {
    pub fn dark() -> Self {
        Self::from_palette("dark", ThemeMode::Dark, ThemePalette::dark())
    }

    pub fn light() -> Self {
        Self::from_palette("light", ThemeMode::Light, ThemePalette::light())
    }

    pub fn custom(name: impl Into<String>, palette: ThemePalette) -> Self {
        Self::from_palette(name, ThemeMode::Custom, palette)
    }

    fn from_palette(name: impl Into<String>, mode: ThemeMode, palette: ThemePalette) -> Self {
        let select_list = SelectListTheme {
            selected_prefix: Style::fg(palette.accent).bold(),
            selected_text: Style::fg(palette.accent).bold(),
            description: Style::fg(palette.muted).dim(),
            scroll_info: Style::fg(palette.muted).dim(),
            no_match: Style::fg(palette.muted).dim(),
        };
        let markdown = MarkdownTheme {
            heading: Style::fg(palette.accent).bold(),
            link: Style::fg(palette.accent),
            link_url: Style::fg(palette.muted).dim(),
            code: Style::fg(palette.warning),
            code_block: Style::fg(palette.muted).dim(),
            code_block_border: Style::fg(palette.muted).dim(),
            quote: Style::fg(palette.muted).dim(),
            quote_border: Style::fg(palette.muted).dim(),
            hr: Style::fg(palette.muted).dim(),
            list_bullet: Style::fg(palette.accent),
            bold: Style::fg(palette.text).bold(),
        };
        let settings_list = SettingsListTheme {
            label: Style::fg(palette.text),
            selected_label: Style::fg(palette.accent).bold(),
            value: Style::fg(palette.muted),
            selected_value: Style::fg(palette.accent).bold(),
            description: Style::fg(palette.muted).dim(),
            cursor: Style::fg(palette.accent).bold(),
            hint: Style::fg(palette.muted).dim(),
        };
        let editor = EditorTheme {
            border: Style::fg(palette.muted).dim(),
            placeholder: Style::fg(palette.muted).dim(),
            select_list,
        };

        Self {
            name: name.into(),
            mode,
            palette,
            markdown,
            select_list,
            settings_list,
            editor,
        }
    }
}

pub fn dark_theme() -> TuiTheme {
    TuiTheme::dark()
}

pub fn light_theme() -> TuiTheme {
    TuiTheme::light()
}
