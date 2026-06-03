pub mod faux;
pub mod anthropic;

use std::sync::Arc;
use crate::registry;

/// Register all built-in providers in the global registry.
/// Call this once at startup.
pub fn register_builtins() {
    registry::register(
        "anthropic-messages",
        Arc::new(anthropic::AnthropicProvider::new(None)),
    );
}
