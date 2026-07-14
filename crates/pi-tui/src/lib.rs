#[doc(hidden)]
pub mod autocomplete;
#[doc(hidden)]
pub mod component;
#[doc(hidden)]
pub mod components;
#[doc(hidden)]
pub mod cursor;
#[doc(hidden)]
pub mod fuzzy;
#[doc(hidden)]
pub mod input;
#[doc(hidden)]
pub mod kill_ring;
#[doc(hidden)]
pub mod overlay;
#[doc(hidden)]
pub mod runtime;
#[doc(hidden)]
pub mod style;
#[doc(hidden)]
pub mod terminal;
#[doc(hidden)]
pub mod terminal_colors;
#[doc(hidden)]
pub mod terminal_image;
#[doc(hidden)]
pub mod theme;
#[doc(hidden)]
pub mod tui;
#[doc(hidden)]
pub mod undo_stack;
#[doc(hidden)]
pub mod utils;
#[doc(hidden)]
pub mod virtual_terminal;
#[doc(hidden)]
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
    GENERIC_TUI_KEYBINDINGS, InputEvent, Key, KeyEvent, KeyEventKind, KeyModifiers,
    KeybindingConflict, KeybindingDefinition, KeybindingsConfig, KeybindingsManager, StdinBuffer,
    TUI_KEYBINDINGS, is_key_release, is_kitty_protocol_active, matches_key, parse_key,
    set_kitty_protocol_active,
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
        BackgroundFn, Box, CancellableLoader, DefaultTextStyle, Editor, Image, Input, Loader,
        LoaderIndicatorOptions, Markdown, SelectItem, SelectList, SelectorDialog,
        SelectorDialogOptions, SettingItem, SettingsList, SettingsListOptions, SettingsSubmenuDone,
        Spacer, Text, TruncatedText,
    };
    pub use crate::cursor::{CURSOR_MARKER, CursorPosition, extract_cursor_marker};
    pub use crate::fuzzy::{FuzzyMatch, fuzzy_filter_indices, fuzzy_match};
    pub use crate::input::{
        GENERIC_TUI_KEYBINDINGS, InputEvent, Key, KeyEvent, KeyEventKind, KeyModifiers,
        KeybindingConflict, KeybindingDefinition, KeybindingsConfig, KeybindingsManager,
        StdinBuffer, is_key_release, matches_key, parse_key,
    };
    pub use crate::kill_ring::KillRing;
    pub use crate::overlay::{
        OverlayAnchor, OverlayHandle, OverlayMargin, OverlayOptions, OverlayVisibleFn, SizeValue,
    };
    pub use crate::runtime::RenderScheduler;
    pub use crate::style::{
        Color, ColorLevel, Style, detect_color_level_from_env, paint, paint_with, paint_with_level,
    };
    pub use crate::terminal::{
        NegotiationResult, ProcessTerminal, Terminal, TerminalSize, is_apple_terminal_session,
        normalize_apple_terminal_input,
    };
    pub use crate::terminal_colors::{
        RgbColor, TerminalColorScheme, is_color_scheme_report, is_osc11_background_color_response,
        parse_color_scheme_report, parse_osc11_background_color, query_background_color,
        query_color_scheme, set_color_scheme_notifications,
    };
    pub use crate::terminal_image::{
        CellDimensions, ImageCellSize, ImageDimensions, ImageProtocol, ImageRenderOptions,
        RenderedImage, TerminalCapabilities, calculate_image_cell_size, delete_all_kitty_images,
        delete_kitty_image, detect_terminal_capabilities_from_env, encode_iterm2, encode_kitty,
        hyperlink, image_dimensions_from_base64, image_dimensions_from_bytes, is_image_line,
        render_image,
    };
    pub use crate::theme::{
        EditorTheme, ImageTheme, MarkdownTheme, SelectListTheme, SettingsListTheme, ThemeMode,
        ThemePalette, TuiTheme, dark_theme, light_theme,
    };
    pub use crate::tui::{
        InputListenerResult, RenderOutcome, RenderStrategy, RenderSurface, Tui, TuiError,
    };
    pub use crate::undo_stack::UndoStack;
    pub use crate::utils::{
        truncate_to_width, truncate_to_width_with_ellipsis, visible_width, wrap_text_with_ansi,
    };
    pub use crate::virtual_terminal::{TerminalOp, VirtualTerminal};
    pub use crate::word_navigation::{find_word_backward, find_word_forward};
}
