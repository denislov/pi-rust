use pi_ai::transport::headers::merge_headers;
use pi_ai::transport::retry::{RetryConfig, is_retryable_status, parse_retry_after_ms};

#[test]
fn headers_merge_priority() {
    let model = serde_json::json!({"x-custom": "model-value"});
    let opts = serde_json::json!({"x-custom": "option-value"});
    let headers = merge_headers(Some(&model), Some(&opts), []);
    assert_eq!(
        headers.get("x-custom").map(String::as_str),
        Some("option-value")
    );
}

#[test]
fn headers_generated_preserved() {
    let opts = serde_json::json!({"x-extra": "extra"});
    let headers = merge_headers(
        None,
        Some(&opts),
        [("authorization".into(), "Bearer sk-test".into())],
    );
    assert_eq!(
        headers.get("authorization").map(String::as_str),
        Some("Bearer sk-test")
    );
    assert_eq!(headers.get("x-extra").map(String::as_str), Some("extra"));
}

#[test]
fn retryable_status_408() {
    assert!(is_retryable_status(408));
}

#[test]
fn retryable_status_429() {
    assert!(is_retryable_status(429));
}

#[test]
fn retryable_status_500() {
    assert!(is_retryable_status(500));
}

#[test]
fn non_retryable_status_400() {
    assert!(!is_retryable_status(400));
}

#[test]
fn non_retryable_status_200() {
    assert!(!is_retryable_status(200));
}

#[test]
fn parse_retry_after_seconds() {
    let cfg = RetryConfig {
        max_retries: 2,
        timeout_ms: None,
        max_retry_delay_ms: 10_000,
    };
    let ms = parse_retry_after_ms(Some("5"), &cfg).unwrap();
    assert_eq!(ms, 5000);
}

#[test]
fn parse_retry_after_exceeds_max() {
    let cfg = RetryConfig {
        max_retries: 2,
        timeout_ms: None,
        max_retry_delay_ms: 1_000,
    };
    assert!(parse_retry_after_ms(Some("5"), &cfg).is_err());
}

#[test]
fn parse_retry_after_none() {
    let cfg = RetryConfig {
        max_retries: 2,
        timeout_ms: None,
        max_retry_delay_ms: 10_000,
    };
    assert_eq!(parse_retry_after_ms(None, &cfg).unwrap(), 0);
}

#[test]
fn retry_config_defaults() {
    let cfg = RetryConfig::from_options(None);
    assert_eq!(cfg.max_retries, 2);
    assert_eq!(cfg.max_retry_delay_ms, 10_000);
    assert_eq!(cfg.timeout_ms, None);
}

#[test]
fn retry_config_from_options() {
    let opts = pi_ai::StreamOptions {
        max_retries: Some(1),
        max_retry_delay_ms: Some(5_000),
        timeout_ms: Some(30_000),
        ..Default::default()
    };
    let cfg = RetryConfig::from_options(Some(&opts));
    assert_eq!(cfg.max_retries, 1);
    assert_eq!(cfg.max_retry_delay_ms, 5_000);
    assert_eq!(cfg.timeout_ms, Some(30_000));
}
