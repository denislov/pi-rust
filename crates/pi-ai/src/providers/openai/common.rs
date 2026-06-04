#[derive(Debug, Clone, Default)]
pub struct CompatFlags {
    pub supports_developer_role: bool,
    pub supports_usage_in_streaming: bool,
    pub supports_strict_mode: bool,
    pub max_tokens_field: String,
}

pub fn resolve_completions_compat(model: &crate::types::Model) -> CompatFlags {
    let mut flags = CompatFlags {
        supports_developer_role: false,
        supports_usage_in_streaming: true,
        supports_strict_mode: false,
        max_tokens_field: "max_completion_tokens".to_string(),
    };

    if let Some(ref compat) = model.compat {
        if let Some(v) = compat.get("supportsDeveloperRole") {
            flags.supports_developer_role = v.as_bool().unwrap_or(false);
        }
        if let Some(v) = compat.get("supportsUsageInStreaming") {
            flags.supports_usage_in_streaming = v.as_bool().unwrap_or(true);
        }
        if let Some(v) = compat.get("supportsStrictMode") {
            flags.supports_strict_mode = v.as_bool().unwrap_or(false);
        }
        if let Some(v) = compat.get("maxTokensField") {
            flags.max_tokens_field = v.as_str().unwrap_or("max_completion_tokens").to_string();
        }
    }

    flags
}
