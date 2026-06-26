mod key;
mod keybindings;
mod stdin_buffer;

pub use key::{
    Key, KeyEvent, KeyEventKind, KeyModifiers, is_key_release, is_kitty_protocol_active,
    matches_key, parse_key, set_kitty_protocol_active,
};
pub use keybindings::{
    KeybindingConflict, KeybindingDefinition, KeybindingsConfig, KeybindingsManager,
    TUI_KEYBINDINGS,
};
pub use stdin_buffer::StdinBuffer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    Key(KeyEvent),
    Paste(String),
    Raw(String),
    Resize(crate::TerminalSize),
}
