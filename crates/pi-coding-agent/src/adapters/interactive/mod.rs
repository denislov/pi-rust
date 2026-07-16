pub mod app;
mod clipboard;
mod commands;
mod delegation_confirmation_menu;
pub mod event_bridge;
mod git_branch;
mod input;
pub mod key_hints;
pub(crate) mod keybindings;
mod r#loop;
mod model_selector;
mod profile_menu;
mod prompt_task;
mod render;
mod root;
mod session_actions;
mod session_selector;
mod slash;
pub mod transcript;
pub(super) mod tree_selector;

pub use app::run_interactive_mode;
#[cfg(test)]
pub use app::test_harness;
#[cfg(test)]
pub use event_bridge::CodingEventBridge;
pub use event_bridge::UiEvent;
pub use transcript::{Transcript, TranscriptItem};
