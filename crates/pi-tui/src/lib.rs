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
pub mod terminal_colors;
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
    BackgroundFn, Box, CancellableLoader, DefaultTextStyle, Editor, Image, Input, Loader,
    LoaderIndicatorOptions, Markdown, SelectItem, SelectList, SelectorDialog,
    SelectorDialogOptions, SettingItem, SettingsList, SettingsListOptions, SettingsSubmenuDone,
    Spacer, Text, TruncatedText,
};
pub use cursor::{CURSOR_MARKER, CursorPosition, extract_cursor_marker};
pub use fuzzy::{FuzzyMatch, fuzzy_filter_indices, fuzzy_match};
pub use input::{
    InputEvent, Key, KeyEvent, KeyEventKind, KeyModifiers, KeybindingConflict,
    KeybindingDefinition, KeybindingsConfig, KeybindingsManager, StdinBuffer, TUI_KEYBINDINGS,
    is_key_release, is_kitty_protocol_active, matches_key, parse_key, set_kitty_protocol_active,
};
pub use overlay::{
    OverlayAnchor, OverlayHandle, OverlayMargin, OverlayOptions, OverlayVisibleFn, SizeValue,
};
pub use runtime::RenderScheduler;
pub use style::{
    Color, ColorLevel, ERROR, PATH, STATUS_IDLE, STATUS_RUNNING, SYSTEM, Style, TOOL_ERROR,
    TOOL_NAME, USER, color_enabled, color_level, detect_color_level_from_env, paint, paint_with,
    paint_with_level,
};
pub use terminal::{
    NegotiationResult, ProcessTerminal, Terminal, TerminalSize, is_apple_terminal_session,
    normalize_apple_terminal_input,
};
pub use terminal_colors::{
    RgbColor, TerminalColorScheme, is_color_scheme_report, is_osc11_background_color_response,
    parse_color_scheme_report, parse_osc11_background_color, query_background_color,
    query_color_scheme, set_color_scheme_notifications,
};
pub use terminal_image::{
    CellDimensions, ImageCellSize, ImageDimensions, ImageProtocol, ImageRenderOptions,
    RenderedImage, TerminalCapabilities, calculate_image_cell_size, delete_all_kitty_images,
    delete_kitty_image, detect_terminal_capabilities_from_env, encode_iterm2, encode_kitty,
    hyperlink, image_dimensions_from_base64, image_dimensions_from_bytes, is_image_line,
    render_image,
};
pub use theme::{
    EditorTheme, ImageTheme, MarkdownTheme, SelectListTheme, SettingsListTheme, ThemeMode,
    ThemePalette, TuiTheme, dark_theme, light_theme,
};
pub use tui::{InputListenerResult, RenderOutcome, RenderStrategy, RenderSurface, Tui, TuiError};
pub use utils::{
    truncate_to_width, truncate_to_width_with_ellipsis, visible_width, wrap_text_with_ansi,
};
pub use virtual_terminal::{TerminalOp, VirtualTerminal};

/// Stable generic terminal UI facade.
///
/// Product-specific coding-agent actions, sessions, model state, tree state,
/// tools, and plugin dispatch belong in `pi-coding-agent` adapters.
pub mod api {
    pub use crate::autocomplete::{
        AutocompleteItem, AutocompleteOptions, AutocompleteProvider, AutocompleteSuggestions,
        CombinedAutocompleteProvider, CompletionEdit, SlashCommand,
    };
    pub use crate::component::{Component, ComponentId, Container};
    pub use crate::components::{
        Box, CancellableLoader, Editor, Image, Input, Loader, Markdown, SelectItem, SelectList,
        SelectorDialog, SelectorDialogOptions, SettingItem, SettingsList, SettingsListOptions,
        Spacer, Text, TruncatedText,
    };
    pub use crate::input::{
        InputEvent, Key, KeyEvent, KeyEventKind, KeyModifiers, KeybindingConflict,
        KeybindingDefinition, KeybindingsConfig, KeybindingsManager, StdinBuffer,
    };
    pub use crate::overlay::{OverlayAnchor, OverlayHandle, OverlayOptions};
    pub use crate::runtime::RenderScheduler;
    pub use crate::terminal::{ProcessTerminal, Terminal, TerminalSize};
    pub use crate::theme::{ThemeMode, ThemePalette, TuiTheme, dark_theme, light_theme};
    pub use crate::tui::{
        InputListenerResult, RenderOutcome, RenderStrategy, RenderSurface, Tui, TuiError,
    };
    pub use crate::virtual_terminal::{TerminalOp, VirtualTerminal};
}
