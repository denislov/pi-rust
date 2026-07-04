pub mod app;
mod clipboard;
mod commands;
mod delegation_confirmation_menu;
pub mod event_bridge;
mod git_branch;
mod input;
pub mod key_hints;
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
#[cfg(any(test, feature = "test-harness", debug_assertions))]
pub use app::test_harness;
pub use event_bridge::{CodingEventBridge, UiEvent};
pub use key_hints::{app_key_hint, format_key_text, key_hint};
pub use transcript::{Transcript, TranscriptItem};
