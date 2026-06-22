use async_stream::stream;
use futures::StreamExt;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use super::error::ProviderError;
use crate::stream::EventStream;
use crate::types::{AssistantMessage, AssistantMessageEvent, Model, StopReason, StreamOptions};
use crate::util::http::RetryConfig;

pub fn send_json_stream<F>(
    _client: &reqwest::Client,
    model: &Model,
    opts: Option<&StreamOptions>,
    api_name: &str,
    request: reqwest::RequestBuilder,
    payload: serde_json::Value,
    process_body: F,
) -> EventStream
where
    F: FnOnce(
            Box<dyn futures::Stream<Item = Result<bytes::Bytes, String>> + Send + Unpin>,
            Model,
            Option<CancellationToken>,
        ) -> EventStream
        + Send
        + 'static,
{
    let model = model.clone();
    let model_id = model.id.clone();
    let provider = model.provider.clone();
    let api_name = api_name.to_string();
    let cancel = opts.and_then(|o| o.cancel.clone());
    let retry_cfg = RetryConfig::from_options(opts);
    let hooks = opts.and_then(|o| o.hooks.clone());

    Box::pin(stream! {
        let final_payload = match hooks.as_ref() {
            Some(hooks) => match hooks.apply_payload(&model, payload).await {
                Ok(p) => p,
                Err(e) => {
                    let mut msg = AssistantMessage::empty(&api_name, &model_id);
                    msg.provider = Some(provider);
                    msg.error_message = Some(format!("Payload hook failed: {}", e));
                    msg.stop_reason = StopReason::Error;
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        message: msg,
                    };
                    return;
                }
            },
            None => payload,
        };

        let request = request.json(&final_payload);

        let mut last_error: Option<ProviderError> = None;

        for attempt in 0..=retry_cfg.max_retries {
            if let Some(ref token) = cancel {
                if token.is_cancelled() {
                    let err = ProviderError::cancelled(&api_name, &model_id, &provider);
                    let mut msg = error_event(&api_name, &model_id, &provider, &err);
                    msg.stop_reason = StopReason::Aborted;
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Aborted,
                        message: msg,
                    };
                    return;
                }
            }

            let Some(request) = request.try_clone() else {
                last_error = Some(ProviderError::network(
                    &api_name,
                    &model_id,
                    &provider,
                    "request could not be cloned for retryable send",
                ));
                break;
            };
            let send_future = request.send();
            let response = match retry_cfg.timeout_ms {
                Some(ms) => {
                    match tokio::time::timeout(Duration::from_millis(ms), send_future).await {
                        Ok(Ok(r)) => r,
                        Ok(Err(e)) => {
                            last_error = Some(ProviderError::network(&api_name, &model_id, &provider, e));
                            if !should_retry(&last_error, &retry_cfg, attempt) {
                                break;
                            }
                            continue;
                        }
                        Err(_) => {
                            last_error = Some(ProviderError::timeout(&api_name, &model_id, &provider, ms));
                            if !should_retry(&last_error, &retry_cfg, attempt) {
                                break;
                            }
                            continue;
                        }
                    }
                }
                None => {
                    match send_future.await {
                        Ok(r) => r,
                        Err(e) => {
                            last_error = Some(ProviderError::network(&api_name, &model_id, &provider, e));
                            if !should_retry(&last_error, &retry_cfg, attempt) {
                                break;
                            }
                            continue;
                        }
                    }
                }
            };

            let status = response.status().as_u16();
            let response_headers = headers_to_json(response.headers());

            if let Some(hooks) = hooks.as_ref() {
                let response_info = crate::types::ProviderResponseInfo {
                    status: Some(status),
                    headers: Some(response_headers.clone()),
                };
                if let Err(e) = hooks.emit_response(response_info).await {
                    let mut msg = AssistantMessage::empty(&api_name, &model_id);
                    msg.provider = Some(provider);
                    msg.error_message = Some(format!("Response hook failed: {}", e));
                    msg.stop_reason = StopReason::Error;
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        message: msg,
                    };
                    return;
                }
            }

            if !response.status().is_success() {
                if crate::util::http::is_retryable_status(status) && attempt < retry_cfg.max_retries {
                    let retry_delay = match retry_delay_ms(response.headers(), &retry_cfg) {
                        Ok(ms) => ms,
                        Err(e) => {
                            last_error = Some(ProviderError::retry_after_too_long(
                                &api_name,
                                &model_id,
                                &provider,
                                e,
                            ));
                            break;
                        }
                    };
                    drop(response);
                    if wait_before_retry(retry_delay, cancel.as_ref()).await {
                        continue;
                    }
                    let err = ProviderError::cancelled(&api_name, &model_id, &provider);
                    let mut msg = error_event(&api_name, &model_id, &provider, &err);
                    msg.stop_reason = StopReason::Aborted;
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Aborted,
                        message: msg,
                    };
                    return;
                }

                let body = response.text().await.unwrap_or_default();
                last_error = Some(ProviderError::http_status(
                    &api_name, &model_id, &provider, status, body,
                ));
                break;
            }

            let body_stream: Box<dyn futures::Stream<Item = Result<bytes::Bytes, String>> + Send + Unpin> =
                Box::new(response.bytes_stream().map(|r| r.map_err(|e| e.to_string())));

            let mut event_stream = process_body(body_stream, model.clone(), cancel);
            while let Some(event) = event_stream.next().await {
                yield event;
            }
            return;
        }

        let err = last_error.unwrap_or_else(|| {
            ProviderError::network(&api_name, &model_id, &provider, "max retries exceeded")
        });
        let mut msg = error_event(&api_name, &model_id, &provider, &err);
        if matches!(err.kind, super::error::ProviderErrorKind::Cancelled) {
            msg.stop_reason = StopReason::Aborted;
        }
        yield AssistantMessageEvent::Error {
            reason: msg.stop_reason.clone(),
            message: msg,
        };
    })
}

fn should_retry(error: &Option<ProviderError>, cfg: &RetryConfig, attempt: u32) -> bool {
    if attempt >= cfg.max_retries {
        return false;
    }
    match error {
        Some(e) => match e.kind {
            super::error::ProviderErrorKind::Network => true,
            super::error::ProviderErrorKind::Timeout => true,
            super::error::ProviderErrorKind::HttpStatus => e
                .status
                .map_or(false, |s| crate::util::http::is_retryable_status(s)),
            _ => false,
        },
        None => false,
    }
}

fn headers_to_json(headers: &reqwest::header::HeaderMap) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    for (name, value) in headers {
        if let Ok(value) = value.to_str() {
            object.insert(name.as_str().to_string(), serde_json::json!(value));
        }
    }
    serde_json::Value::Object(object)
}

fn retry_delay_ms(headers: &reqwest::header::HeaderMap, cfg: &RetryConfig) -> Result<u64, String> {
    if let Some(value) = headers
        .get("retry-after-ms")
        .and_then(|value| value.to_str().ok())
    {
        let ms = value
            .trim()
            .parse::<u64>()
            .map_err(|e| format!("Retry-After-MS header is not a valid number: {}", e))?;
        if ms > cfg.max_retry_delay_ms {
            return Err(format!(
                "Retry-After {}ms exceeds max_retry_delay_ms {}ms",
                ms, cfg.max_retry_delay_ms
            ));
        }
        return Ok(ms);
    }

    let retry_after = headers
        .get("retry-after")
        .and_then(|value| value.to_str().ok());
    crate::util::http::parse_retry_after_ms(retry_after, cfg)
}

async fn wait_before_retry(delay_ms: u64, cancel: Option<&CancellationToken>) -> bool {
    if delay_ms == 0 {
        return true;
    }

    match cancel {
        Some(token) => {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(delay_ms)) => true,
                _ = token.cancelled() => false,
            }
        }
        None => {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            true
        }
    }
}

fn error_event(
    api_name: &str,
    model_id: &str,
    provider: &str,
    error: &ProviderError,
) -> AssistantMessage {
    let mut msg = AssistantMessage::empty(api_name, model_id);
    msg.provider = Some(provider.to_string());
    msg.error_message = Some(error.message.clone());
    msg.stop_reason = StopReason::Error;
    msg
}
