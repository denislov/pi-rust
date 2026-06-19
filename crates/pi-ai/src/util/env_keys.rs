fn provider_env_vars(provider: &str) -> &'static [&'static str] {
    match provider {
        "anthropic" => &["ANTHROPIC_API_KEY", "CLAUDE_API_KEY", "ANTHROPIC_KEY"],
        "openai" => &["OPENAI_API_KEY"],
        "azure-openai-responses" => &["AZURE_OPENAI_API_KEY"],
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
        "minimax" => &["MINIMAX_API_KEY"],
        "minimax-cn" => &["MINIMAX_CN_API_KEY"],
        "xiaomi" => &["XIAOMI_API_KEY"],
        "xiaomi-token-plan-cn" => &["XIAOMI_TOKEN_PLAN_CN_API_KEY"],
        "xiaomi-token-plan-ams" => &["XIAOMI_TOKEN_PLAN_AMS_API_KEY"],
        "xiaomi-token-plan-sgp" => &["XIAOMI_TOKEN_PLAN_SGP_API_KEY"],
        "github-copilot" => &["COPILOT_GITHUB_TOKEN"],
        "openai-codex" => &["OPENAI_CODEX_API_KEY", "OPENAI_API_KEY"],
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
    if self_auth_present(provider) {
        return Some("<authenticated>".to_string());
    }
    None
}

/// Providers that authenticate via an external credential chain rather than a
/// single API-key env var. Returns true when credentials appear to be present;
/// real signing/ADC is implemented in M8.
fn self_auth_present(provider: &str) -> bool {
    match provider {
        "amazon-bedrock" => [
            "AWS_PROFILE",
            "AWS_ACCESS_KEY_ID",
            "AWS_BEARER_TOKEN_BEDROCK",
        ]
        .iter()
        .any(|v| std::env::var_os(v).is_some_and(|s| !s.is_empty())),
        "google-vertex" => {
            std::env::var_os("GOOGLE_APPLICATION_CREDENTIALS").is_some_and(|s| !s.is_empty())
        }
        _ => false,
    }
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

    #[test]
    fn returns_minimax_key() {
        unsafe {
            std::env::set_var("MINIMAX_API_KEY", "mm-test");
        }
        assert_eq!(env_api_key("minimax"), Some("mm-test".into()));
        unsafe {
            std::env::remove_var("MINIMAX_API_KEY");
        }
    }

    #[test]
    fn returns_copilot_token() {
        unsafe {
            std::env::set_var("COPILOT_GITHUB_TOKEN", "ghp-test");
        }
        assert_eq!(env_api_key("github-copilot"), Some("ghp-test".into()));
        unsafe {
            std::env::remove_var("COPILOT_GITHUB_TOKEN");
        }
    }

    #[test]
    fn bedrock_returns_sentinel_when_aws_profile_set() {
        unsafe {
            std::env::set_var("AWS_PROFILE", "default");
        }
        assert_eq!(
            env_api_key("amazon-bedrock"),
            Some("<authenticated>".into())
        );
        unsafe {
            std::env::remove_var("AWS_PROFILE");
        }
    }
}
