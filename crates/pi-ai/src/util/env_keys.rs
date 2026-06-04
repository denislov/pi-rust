fn provider_env_vars(provider: &str) -> &'static [&'static str] {
    match provider {
        "anthropic" => &["ANTHROPIC_API_KEY", "CLAUDE_API_KEY", "ANTHROPIC_KEY"],
        "openai" => &["OPENAI_API_KEY"],
        "deepseek" => &["DEEPSEEK_API_KEY", "DEEPSEEK_KEY"],
        "google" => &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
        "groq" => &["GROQ_API_KEY"],
        "cerebras" => &["CEREBRAS_API_KEY"],
        "xai" => &["XAI_API_KEY"],
        "openrouter" => &["OPENROUTER_API_KEY"],
        "vercel-ai-gateway" => &["AI_GATEWAY_API_KEY"],
        "zai" => &["ZAI_API_KEY"],
        "mistral" => &["MISTRAL_API_KEY"],
        "moonshotai" | "moonshotai-cn" => &["MOONSHOT_API_KEY"],
        "huggingface" => &["HF_TOKEN"],
        "fireworks" => &["FIREWORKS_API_KEY"],
        "together" => &["TOGETHER_API_KEY"],
        "opencode" | "opencode-go" => &["OPENCODE_API_KEY"],
        "kimi-coding" => &["KIMI_API_KEY"],
        "cloudflare-workers-ai" | "cloudflare-ai-gateway" => &["CLOUDFLARE_API_KEY"],
        _ => &[],
    }
}

pub fn env_api_key(provider: &str) -> Option<String> {
    for var in provider_env_vars(provider) {
        if let Ok(val) = std::env::var(var) {
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_none_when_not_set() {
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("CLAUDE_API_KEY");
        }
        assert_eq!(env_api_key("anthropic"), None);
    }

    #[test]
    fn returns_anthropic_key() {
        unsafe {
            std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-test");
        }
        assert_eq!(env_api_key("anthropic"), Some("sk-ant-test".into()));
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
    }

    #[test]
    fn returns_none_for_unknown_provider() {
        assert_eq!(env_api_key("nonexistent"), None);
    }

    #[test]
    fn returns_deepseek_key() {
        unsafe {
            std::env::set_var("DEEPSEEK_API_KEY", "sk-deepseek-test");
        }
        assert_eq!(env_api_key("deepseek"), Some("sk-deepseek-test".into()));
        unsafe {
            std::env::remove_var("DEEPSEEK_API_KEY");
        }
    }
}
