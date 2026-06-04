#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub timeout_ms: Option<u64>,
    pub max_retry_delay_ms: u64,
}

impl RetryConfig {
    pub fn from_options(opts: Option<&crate::types::StreamOptions>) -> Self {
        let default_max_retries = 2;
        let default_max_retry_delay_ms = 10_000;
        match opts {
            Some(o) => Self {
                max_retries: o.max_retries.unwrap_or(default_max_retries),
                timeout_ms: o.timeout_ms,
                max_retry_delay_ms: o.max_retry_delay_ms.unwrap_or(default_max_retry_delay_ms),
            },
            None => Self {
                max_retries: default_max_retries,
                timeout_ms: None,
                max_retry_delay_ms: default_max_retry_delay_ms,
            },
        }
    }
}

pub fn is_retryable_status(status: u16) -> bool {
    matches!(status, 408 | 409 | 429 | 500..=599)
}

pub fn parse_retry_after_ms(header: Option<&str>, cfg: &RetryConfig) -> Result<u64, String> {
    let seconds: f64 = match header {
        Some(h) => h
            .trim()
            .parse::<f64>()
            .map_err(|e| format!("Retry-After header is not a valid number: {}", e))?,
        None => return Ok(0),
    };
    let ms = (seconds * 1000.0) as u64;
    if ms > cfg.max_retry_delay_ms {
        return Err(format!(
            "Retry-After {}ms exceeds max_retry_delay_ms {}ms",
            ms, cfg.max_retry_delay_ms
        ));
    }
    Ok(ms)
}
