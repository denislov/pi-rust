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

            let send_future = request.try_clone().expect("reqwest request must be cloneable").send();
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

            if let Some(hooks) = hooks.as_ref() {
                let response_info = crate::types::ProviderResponseInfo {
                    status: Some(status),
                    headers: Some(serde_json::json!({})),
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
                let body = response.text().await.unwrap_or_default();
                last_error = Some(ProviderError::http_status(
                    &api_name, &model_id, &provider, status, body,
                ));
                if !should_retry(&last_error, &retry_cfg, attempt) {
                    break;
                }
                continue;
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
