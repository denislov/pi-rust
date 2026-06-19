pub mod component;
pub mod components;
pub mod cursor;
pub mod fuzzy;
pub mod input;
pub mod kill_ring;
pub mod overlay;
pub mod runtime;
pub mod style;
pub mod terminal;
pub mod tui;
pub mod undo_stack;
pub mod utils;
pub mod virtual_terminal;
pub mod word_navigation;

pub use component::{Component, ComponentId, Container};
pub use components::{
    BackgroundFn, Box, CancellableLoader, Editor, Input, Loader, LoaderIndicatorOptions, Markdown,
    SelectItem, SelectList, SettingItem, SettingsList, SettingsListOptions, Spacer, Text,
    TruncatedText,
};
pub use cursor::{CURSOR_MARKER, CursorPosition, extract_cursor_marker};
pub use fuzzy::{FuzzyMatch, fuzzy_filter_indices, fuzzy_match};
pub use input::{
    InputEvent, Key, KeyEvent, KeyEventKind, KeyModifiers, KeybindingConflict,
    KeybindingDefinition, KeybindingsConfig, KeybindingsManager, StdinBuffer, TUI_KEYBINDINGS,
    is_key_release, matches_key, parse_key,
};
pub use overlay::{OverlayAnchor, OverlayHandle, OverlayMargin, OverlayOptions, SizeValue};
pub use runtime::RenderScheduler;
pub use style::{
    Color, ERROR, PATH, STATUS_IDLE, STATUS_RUNNING, SYSTEM, Style, TOOL_ERROR, TOOL_NAME, USER,
    color_enabled, paint, paint_with,
};
pub use terminal::{ProcessTerminal, Terminal, TerminalSize};
pub use tui::{RenderOutcome, RenderStrategy, RenderSurface, Tui, TuiError};
pub use utils::{truncate_to_width, visible_width};
pub use virtual_terminal::{TerminalOp, VirtualTerminal};
