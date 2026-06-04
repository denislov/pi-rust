pub mod anthropic;
pub mod deepseek;
pub mod faux;
pub mod google;
pub mod openai;

use crate::registry;
use std::sync::Arc;

/// Register all built-in providers in the global registry.
/// Call this once at startup.
pub fn register_builtins() {
    registry::register(
        "anthropic-messages",
        Arc::new(anthropic::AnthropicProvider::new(None)),
    );
    registry::register(
        "deepseek-chat-completions",
        Arc::new(deepseek::DeepSeekProvider::new(None)),
    );
    registry::register(
        "openai-completions",
        Arc::new(openai::completions::OpenAICompletionsProvider::new(None)),
    );
    registry::register(
        "openai-responses",
        Arc::new(openai::responses::OpenAIResponsesProvider::new(None)),
    );
    registry::register(
        "google-generative-ai",
        Arc::new(google::GoogleGenerativeAiProvider::new(None)),
    );
}
