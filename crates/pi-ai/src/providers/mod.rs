pub mod anthropic;
pub mod azure_openai_responses;
pub mod bedrock;
pub mod cloudflare;
pub mod deepseek;
pub mod faux;
pub mod github_copilot_headers;
pub mod google;
pub mod images;
pub mod mistral;
pub mod openai;
pub mod openai_codex_responses;

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
        "openai-codex-responses",
        Arc::new(openai_codex_responses::OpenAICodexResponsesProvider::new(
            None,
        )),
    );
    registry::register(
        "azure-openai-responses",
        Arc::new(azure_openai_responses::AzureOpenAIResponsesProvider::new(
            None,
        )),
    );
    registry::register(
        "bedrock-converse-stream",
        Arc::new(bedrock::BedrockProvider::new(None)),
    );
    registry::register(
        "google-generative-ai",
        Arc::new(google::GoogleGenerativeAiProvider::new(None)),
    );
    registry::register(
        "mistral-conversations",
        Arc::new(mistral::MistralProvider::new(None)),
    );
}
