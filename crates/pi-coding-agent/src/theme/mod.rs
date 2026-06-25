//! Theme system ported from TypeScript `packages/coding-agent/src/modes/interactive/theme/theme.ts`.
//!
//! Implements the 51-token color model, variable resolution, JSON loading,
//! runtime color resolution, and terminal background detection. Built-in
//! themes (`dark.json`, `light.json`) and `theme-schema.json` are embedded
//! alongside this module. ANSI escape generation lives in the `pi-tui`
//! `Style`/`paint` layer.

mod builtin;
mod color_value;
mod detection;
mod json;
mod resolve;
mod runtime;
mod tokens;

pub use builtin::{DARK_JSON, LIGHT_JSON, SCHEMA_JSON, builtin_dark, builtin_light};
pub use color_value::ColorValue;
pub use detection::{
    DetectionConfidence, DetectionSource, Rgb, TerminalBackgroundDetection, TerminalTheme,
    detect_terminal_background, get_default_theme, get_theme_for_rgb_color,
    parse_osc11_background_color,
};
pub use json::{ExportSection, ThemeJson};
pub use resolve::{ResolveError, ResolvedColor, resolve};
pub use runtime::ResolvedTheme;
pub use tokens::{REQUIRED_TOKEN_KEYS, ThemeBg, ThemeColor};
