#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderErrorKind {
    MissingCredentials,
    Network,
    Timeout,
    Cancelled,
    HttpStatus,
    RetryAfterTooLong,
    HookFailed,
    StreamParse,
}

#[derive(Debug, Clone)]
pub struct ProviderError {
    pub kind: ProviderErrorKind,
    pub api: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub status: Option<u16>,
    pub message: String,
    pub body: Option<String>,
    pub retry_after_ms: Option<u64>,
}

impl ProviderError {
    pub fn missing_credentials(api: &str, model_id: &str, provider: &str) -> Self {
        Self {
            kind: ProviderErrorKind::MissingCredentials,
            api: api.to_string(),
            provider: Some(provider.to_string()),
            model: Some(model_id.to_string()),
            status: None,
            message: format!(
                "No API key found for provider {}. Set the appropriate env var or pass apiKey in options.",
                model_id
            ),
            body: None,
            retry_after_ms: None,
        }
    }

    pub fn network(
        api: &str,
        model_id: &str,
        provider: &str,
        error: impl std::fmt::Display,
    ) -> Self {
        Self {
            kind: ProviderErrorKind::Network,
            api: api.to_string(),
            provider: Some(provider.to_string()),
            model: Some(model_id.to_string()),
            status: None,
            message: format!("HTTP request failed: {}", error),
            body: None,
            retry_after_ms: None,
        }
    }

    pub fn timeout(api: &str, model_id: &str, provider: &str, ms: u64) -> Self {
        Self {
            kind: ProviderErrorKind::Timeout,
            api: api.to_string(),
            provider: Some(provider.to_string()),
            model: Some(model_id.to_string()),
            status: None,
            message: format!("Request timed out after {}ms", ms),
            body: None,
            retry_after_ms: None,
        }
    }

    pub fn http_status(
        api: &str,
        model_id: &str,
        provider: &str,
        status: u16,
        body: String,
    ) -> Self {
        Self {
            kind: ProviderErrorKind::HttpStatus,
            api: api.to_string(),
            provider: Some(provider.to_string()),
            model: Some(model_id.to_string()),
            status: Some(status),
            message: format!("HTTP {} : {}", status, body),
            body: Some(body),
            retry_after_ms: None,
        }
    }

    pub fn cancelled(api: &str, model_id: &str, provider: &str) -> Self {
        Self {
            kind: ProviderErrorKind::Cancelled,
            api: api.to_string(),
            provider: Some(provider.to_string()),
            model: Some(model_id.to_string()),
            status: None,
            message: "cancelled".to_string(),
            body: None,
            retry_after_ms: None,
        }
    }

    pub fn retry_after_too_long(
        api: &str,
        model_id: &str,
        provider: &str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind: ProviderErrorKind::RetryAfterTooLong,
            api: api.to_string(),
            provider: Some(provider.to_string()),
            model: Some(model_id.to_string()),
            status: None,
            message: message.into(),
            body: None,
            retry_after_ms: None,
        }
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
