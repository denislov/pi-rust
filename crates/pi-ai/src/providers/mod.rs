pub mod anthropic;
pub mod faux;

use crate::registry;
use std::sync::Arc;

/// Register all built-in providers in the global registry.
/// Call this once at startup.
pub fn register_builtins() {
    registry::register(
        "anthropic-messages",
        Arc::new(anthropic::AnthropicProvider::new(None)),
    );
}
