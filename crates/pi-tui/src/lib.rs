pub mod autocomplete;
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
pub mod terminal_image;
pub mod theme;
pub mod tui;
pub mod undo_stack;
pub mod utils;
pub mod virtual_terminal;
pub mod word_navigation;

pub use autocomplete::{
    AutocompleteItem, AutocompleteOptions, AutocompleteProvider, AutocompleteSuggestions,
    CombinedAutocompleteProvider, CompletionEdit, SlashCommand,
};
pub use component::{Component, ComponentId, Container};
pub use components::{
    BackgroundFn, Box, CancellableLoader, Editor, Image, Input, Loader, LoaderIndicatorOptions,
    Markdown, SelectItem, SelectList, SelectorDialog, SelectorDialogOptions, SettingItem,
    SettingsList, SettingsListOptions, SettingsSubmenuDone, Spacer, Text, TruncatedText,
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
    Color, ColorLevel, ERROR, PATH, STATUS_IDLE, STATUS_RUNNING, SYSTEM, Style, TOOL_ERROR,
    TOOL_NAME, USER, color_enabled, color_level, detect_color_level_from_env, paint, paint_with,
    paint_with_level,
};
pub use terminal::{ProcessTerminal, Terminal, TerminalSize};
pub use terminal_image::{
    CellDimensions, ImageCellSize, ImageDimensions, ImageProtocol, ImageRenderOptions,
    RenderedImage, TerminalCapabilities, calculate_image_cell_size, delete_all_kitty_images,
    delete_kitty_image, detect_terminal_capabilities_from_env, encode_iterm2, encode_kitty,
    hyperlink, image_dimensions_from_base64, image_dimensions_from_bytes, is_image_line,
    render_image,
};
pub use theme::{
    EditorTheme, MarkdownTheme, SelectListTheme, SettingsListTheme, ThemeMode, ThemePalette,
    TuiTheme, dark_theme, light_theme,
};
pub use tui::{RenderOutcome, RenderStrategy, RenderSurface, Tui, TuiError};
pub use utils::{
    truncate_to_width, truncate_to_width_with_ellipsis, visible_width, wrap_text_with_ansi,
};
pub use virtual_terminal::{TerminalOp, VirtualTerminal};
