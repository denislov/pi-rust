use async_stream::stream;
use futures::StreamExt;
use std::future::Future;
use std::time::Duration;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use super::error::ProviderError;
use crate::model::Model;
use crate::protocol::stream::EventStream;
use crate::protocol::{AssistantMessage, AssistantMessageEvent, StopReason, StreamOptions};
use crate::transport::retry::RetryConfig;

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
    send_json_stream_with_request_factory(
        model,
        opts,
        api_name,
        payload,
        move |payload| {
            request
                .try_clone()
                .map(|request| request.json(payload))
                .ok_or_else(|| "request could not be cloned for retryable send".to_string())
        },
        process_body,
    )
}

pub fn send_json_stream_with_request_factory<FRequest, FBody>(
    model: &Model,
    opts: Option<&StreamOptions>,
    api_name: &str,
    payload: serde_json::Value,
    mut build_request: FRequest,
    process_body: FBody,
) -> EventStream
where
    FRequest: FnMut(&serde_json::Value) -> Result<reqwest::RequestBuilder, String> + Send + 'static,
    FBody: FnOnce(
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
    let deadline = retry_cfg.timeout_ms.map(|timeout_ms| InvocationDeadline {
        at: Instant::now() + Duration::from_millis(timeout_ms),
        timeout_ms,
    });
    let option_error = validate_options(&api_name, opts).err();

    Box::pin(stream! {
        if let Some(error) = option_error {
            let error = ProviderError::unsupported_option(
                &api_name,
                &model_id,
                &provider,
                error,
            );
            yield AssistantMessageEvent::Error {
                reason: StopReason::Error,
                message: error_event(&api_name, &model_id, &provider, &error),
            };
            return;
        }
        if retry_cfg.timeout_ms == Some(0) {
            yield wait_error_event(
                &api_name,
                &model_id,
                &provider,
                WaitError::Timeout { timeout_ms: 0 },
            );
            return;
        }
        let final_payload = match hooks.as_ref() {
            Some(hooks) => match wait_for(
                hooks.apply_payload(&model, payload),
                cancel.as_ref(),
                deadline,
            ).await {
                Ok(Ok(payload)) => payload,
                Ok(Err(error)) => {
                    let mut msg = AssistantMessage::empty(&api_name, &model_id);
                    msg.provider = Some(provider.clone());
                    msg.error_message = Some(format!("Payload hook failed: {}", error));
                    msg.stop_reason = StopReason::Error;
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        message: msg,
                    };
                    return;
                }
                Err(wait_error) => {
                    yield wait_error_event(
                        &api_name,
                        &model_id,
                        &provider,
                        wait_error,
                    );
                    return;
                }
            },
            None => payload,
        };

        let mut last_error: Option<ProviderError> = None;

        for attempt in 0..=retry_cfg.max_retries {
            let request = match build_request(&final_payload) {
                Ok(request) => request,
                Err(error) => {
                    last_error = Some(ProviderError::network(
                    &api_name,
                    &model_id,
                    &provider,
                        error,
                    ));
                    break;
                }
            };
            let response = match wait_for(request.send(), cancel.as_ref(), deadline).await {
                Ok(Ok(response)) => response,
                Ok(Err(error)) => {
                    last_error = Some(ProviderError::network(
                        &api_name,
                        &model_id,
                        &provider,
                        error,
                    ));
                    if !should_retry(&last_error, &retry_cfg, attempt) {
                        break;
                    }
                    continue;
                }
                Err(WaitError::Timeout { timeout_ms }) => {
                    last_error = Some(ProviderError::timeout(
                        &api_name,
                        &model_id,
                        &provider,
                        timeout_ms,
                    ));
                    if !should_retry(&last_error, &retry_cfg, attempt) {
                        break;
                    }
                    continue;
                }
                Err(WaitError::Cancelled) => {
                    yield wait_error_event(
                        &api_name,
                        &model_id,
                        &provider,
                        WaitError::Cancelled,
                    );
                    return;
                }
            };

            let status = response.status().as_u16();
            let response_headers = headers_to_json(response.headers());

            if let Some(hooks) = hooks.as_ref() {
                let response_info = crate::protocol::ProviderResponseInfo {
                    status: Some(status),
                    headers: Some(response_headers.clone()),
                };
                match wait_for(
                    hooks.emit_response(response_info),
                    cancel.as_ref(),
                    deadline,
                ).await {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        let mut msg = AssistantMessage::empty(&api_name, &model_id);
                        msg.provider = Some(provider.clone());
                        msg.error_message = Some(format!("Response hook failed: {}", error));
                        msg.stop_reason = StopReason::Error;
                        yield AssistantMessageEvent::Error {
                            reason: StopReason::Error,
                            message: msg,
                        };
                        return;
                    }
                    Err(wait_error) => {
                        yield wait_error_event(
                            &api_name,
                            &model_id,
                            &provider,
                            wait_error,
                        );
                        return;
                    }
                }
            }

            if !response.status().is_success() {
                if crate::transport::retry::is_retryable_status(status) && attempt < retry_cfg.max_retries {
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
                    match wait_for(
                        tokio::time::sleep(Duration::from_millis(retry_delay)),
                        cancel.as_ref(),
                        deadline,
                    ).await {
                        Ok(()) => continue,
                        Err(wait_error) => {
                            yield wait_error_event(
                                &api_name,
                                &model_id,
                                &provider,
                                wait_error,
                            );
                            return;
                        }
                    }
                }

                let body = match wait_for(response.text(), cancel.as_ref(), deadline).await {
                    Ok(Ok(body)) => body,
                    Ok(Err(error)) => format!("failed to read error response body: {error}"),
                    Err(wait_error) => {
                        yield wait_error_event(
                            &api_name,
                            &model_id,
                            &provider,
                            wait_error,
                        );
                        return;
                    }
                };
                last_error = Some(ProviderError::http_status(
                    &api_name, &model_id, &provider, status, body,
                ));
                break;
            }

            let body_stream: Box<dyn futures::Stream<Item = Result<bytes::Bytes, String>> + Send + Unpin> =
                Box::new(response.bytes_stream().map(|r| r.map_err(|e| e.to_string())));

            let mut event_stream = process_body(body_stream, model.clone(), cancel.clone());
            loop {
                match wait_for(event_stream.next(), cancel.as_ref(), deadline).await {
                    Ok(Some(event)) => yield event,
                    Ok(None) => break,
                    Err(wait_error) => {
                        yield wait_error_event(
                            &api_name,
                            &model_id,
                            &provider,
                            wait_error,
                        );
                        return;
                    }
                }
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

pub(crate) fn validate_options(api: &str, opts: Option<&StreamOptions>) -> Result<(), String> {
    let Some(opts) = opts else {
        return Ok(());
    };

    if let Some(headers) = &opts.headers {
        let object = headers
            .as_object()
            .ok_or_else(|| "headers must be a JSON object".to_string())?;
        if let Some((name, _)) = object.iter().find(|(_, value)| !value.is_string()) {
            return Err(format!("header `{name}` must have a string value"));
        }
    }

    if let Some(transport) = opts.transport.as_deref() {
        let supports_sse = matches!(
            api,
            "anthropic-messages"
                | "openai-completions"
                | "openai-responses"
                | "azure-openai-responses"
                | "google-generative-ai"
                | "mistral-conversations"
                | "openai-codex-responses"
        );
        if transport != "sse" || !supports_sse {
            return Err(format!(
                "transport `{transport}` is unsupported by API `{api}`"
            ));
        }
    }

    let azure_fields_present = opts.azure_api_version.is_some()
        || opts.azure_resource_name.is_some()
        || opts.azure_base_url.is_some()
        || opts.azure_deployment_name.is_some();
    if azure_fields_present && api != "azure-openai-responses" {
        return Err(format!("Azure options are unsupported by API `{api}`"));
    }

    if opts.session_id.is_some()
        && !matches!(
            api,
            "openai-responses"
                | "azure-openai-responses"
                | "mistral-conversations"
                | "openai-codex-responses"
        )
    {
        return Err(format!("session_id is unsupported by API `{api}`"));
    }

    if opts.tool_choice.is_some() && api == "deepseek-chat-completions" {
        return Err("tool_choice is unsupported by API `deepseek-chat-completions`".into());
    }
    if let Some(tool_choice) = &opts.tool_choice
        && api == "openai-codex-responses"
        && !tool_choice.is_string()
    {
        return Err("Codex tool_choice must be a string".into());
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct InvocationDeadline {
    at: Instant,
    timeout_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WaitError {
    Cancelled,
    Timeout { timeout_ms: u64 },
}

async fn wait_for<F: Future>(
    future: F,
    cancel: Option<&CancellationToken>,
    deadline: Option<InvocationDeadline>,
) -> Result<F::Output, WaitError> {
    match (cancel, deadline) {
        (Some(cancel), Some(deadline)) => tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(WaitError::Cancelled),
            _ = tokio::time::sleep_until(deadline.at) => {
                Err(WaitError::Timeout { timeout_ms: deadline.timeout_ms })
            }
            output = future => Ok(output),
        },
        (Some(cancel), None) => tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(WaitError::Cancelled),
            output = future => Ok(output),
        },
        (None, Some(deadline)) => tokio::select! {
            biased;
            _ = tokio::time::sleep_until(deadline.at) => {
                Err(WaitError::Timeout { timeout_ms: deadline.timeout_ms })
            }
            output = future => Ok(output),
        },
        (None, None) => Ok(future.await),
    }
}

fn wait_error_event(
    api_name: &str,
    model_id: &str,
    provider: &str,
    error: WaitError,
) -> AssistantMessageEvent {
    let provider_error = match error {
        WaitError::Cancelled => ProviderError::cancelled(api_name, model_id, provider),
        WaitError::Timeout { timeout_ms } => {
            ProviderError::timeout(api_name, model_id, provider, timeout_ms)
        }
    };
    let mut message = error_event(api_name, model_id, provider, &provider_error);
    let reason = match error {
        WaitError::Cancelled => StopReason::Aborted,
        WaitError::Timeout { .. } => StopReason::Error,
    };
    message.stop_reason = reason.clone();
    AssistantMessageEvent::Error { reason, message }
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
                .is_some_and(crate::transport::retry::is_retryable_status),
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
    crate::transport::retry::parse_retry_after_ms(retry_after, cfg)
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
