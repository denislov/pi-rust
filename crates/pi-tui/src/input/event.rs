use super::KeyEvent;
use crate::terminal::TerminalSize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    Key(KeyEvent),
    Paste(String),
    Raw(String),
    Resize(TerminalSize),
}
