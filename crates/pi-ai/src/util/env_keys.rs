/// Resolves an API key from the environment for the given provider.
/// For "anthropic", checks ANTHROPIC_API_KEY plus common aliases.
/// For "deepseek", checks DEEPSEEK_API_KEY plus common aliases.
pub fn env_api_key(provider: &str) -> Option<String> {
    let vars = match provider {
        "anthropic" => &["ANTHROPIC_API_KEY", "CLAUDE_API_KEY", "ANTHROPIC_KEY"][..],
        "deepseek" => &["DEEPSEEK_API_KEY", "DEEPSEEK_KEY"][..],
        _ => &[],
    };
    for var in vars {
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
