use crate::types::hooks::ProviderStreamHooks;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreamOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(rename = "maxTokens", skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(rename = "apiKey", skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(rename = "cacheRetention", skip_serializing_if = "Option::is_none")]
    pub cache_retention: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
    #[serde(rename = "toolChoice", skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(rename = "azureApiVersion", skip_serializing_if = "Option::is_none")]
    pub azure_api_version: Option<String>,
    #[serde(rename = "azureResourceName", skip_serializing_if = "Option::is_none")]
    pub azure_resource_name: Option<String>,
    #[serde(rename = "azureBaseUrl", skip_serializing_if = "Option::is_none")]
    pub azure_base_url: Option<String>,
    #[serde(
        rename = "azureDeploymentName",
        skip_serializing_if = "Option::is_none"
    )]
    pub azure_deployment_name: Option<String>,
    #[serde(rename = "bedrockRegion", skip_serializing_if = "Option::is_none")]
    pub bedrock_region: Option<String>,
    #[serde(rename = "bedrockProfile", skip_serializing_if = "Option::is_none")]
    pub bedrock_profile: Option<String>,
    #[serde(rename = "bedrockBearerToken", skip_serializing_if = "Option::is_none")]
    pub bedrock_bearer_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Value>,
    #[serde(skip)]
    pub cancel: Option<tokio_util::sync::CancellationToken>,
    #[serde(rename = "timeoutMs", skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(rename = "maxRetries", skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
    #[serde(rename = "maxRetryDelayMs", skip_serializing_if = "Option::is_none")]
    pub max_retry_delay_ms: Option<u64>,
    #[serde(skip)]
    pub hooks: Option<ProviderStreamHooks>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThinkingConfig {
    pub enabled: bool,
    #[serde(rename = "budgetTokens", skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_options_serializes_retry_fields() {
        let opts = StreamOptions {
            timeout_ms: Some(30000),
            max_retries: Some(3),
            max_retry_delay_ms: Some(5000),
            ..Default::default()
        };
        let json = serde_json::to_string(&opts).unwrap();
        assert!(json.contains(r#""timeoutMs":30000"#));
        assert!(json.contains(r#""maxRetries":3"#));
        assert!(json.contains(r#""maxRetryDelayMs":5000"#));
    }

    #[test]
    fn stream_options_skips_none_retry_fields() {
        let opts = StreamOptions::default();
        let json = serde_json::to_string(&opts).unwrap();
        assert!(!json.contains("timeoutMs"));
        assert!(!json.contains("maxRetries"));
        assert!(!json.contains("maxRetryDelayMs"));
    }
}
