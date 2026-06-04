use pi_ai::util::http::{RetryConfig, is_retryable_status, parse_retry_after_ms};

fn default_cfg() -> RetryConfig {
    RetryConfig {
        max_retries: 2,
        timeout_ms: None,
        max_retry_delay_ms: 10_000,
    }
}

#[test]
fn retryable_408() {
    assert!(is_retryable_status(408));
}

#[test]
fn retryable_409() {
    assert!(is_retryable_status(409));
}

#[test]
fn retryable_429() {
    assert!(is_retryable_status(429));
}

#[test]
fn retryable_500() {
    assert!(is_retryable_status(500));
}

#[test]
fn retryable_503() {
    assert!(is_retryable_status(503));
}

#[test]
fn non_retryable_200() {
    assert!(!is_retryable_status(200));
}

#[test]
fn non_retryable_400() {
    assert!(!is_retryable_status(400));
}

#[test]
fn non_retryable_404() {
    assert!(!is_retryable_status(404));
}

#[test]
fn parse_retry_after_seconds() {
    let ms = parse_retry_after_ms(Some("5"), &default_cfg()).unwrap();
    assert_eq!(ms, 5000);
}

#[test]
fn parse_retry_after_none_returns_zero() {
    let ms = parse_retry_after_ms(None, &default_cfg()).unwrap();
    assert_eq!(ms, 0);
}

#[test]
fn parse_retry_after_exceeds_max_delay() {
    let cfg = RetryConfig {
        max_retry_delay_ms: 1000,
        ..default_cfg()
    };
    let result = parse_retry_after_ms(Some("5"), &cfg);
    assert!(result.is_err());
}

#[test]
fn parse_retry_after_within_max_delay() {
    let cfg = RetryConfig {
        max_retry_delay_ms: 10000,
        ..default_cfg()
    };
    let result = parse_retry_after_ms(Some("5"), &cfg);
    assert_eq!(result.unwrap(), 5000);
}

#[test]
fn parse_retry_after_invalid_header() {
    let result = parse_retry_after_ms(Some("not-a-number"), &default_cfg());
    assert!(result.is_err());
}
