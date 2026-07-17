pub mod auth;
pub mod convert;
pub mod sigv4;
pub mod stream;
pub mod wire;

use async_stream::stream;
use futures::StreamExt;
use std::time::{Duration, Instant};

use crate::model::Model;
use crate::protocol::stream::EventStream;
use crate::protocol::{
    AssistantMessage, AssistantMessageEvent, Context, StopReason, StreamOptions,
};
use crate::registry::ApiProvider;
use crate::transport::http::{send_json_stream_with_request_factory, validate_options};
pub use auth::auth_headers;
use convert::build_request;

pub struct BedrockProvider {
    client: reqwest::Client,
    credentials: Option<(String, String)>,
}

impl BedrockProvider {
    pub fn new(credentials: Option<(String, String)>) -> Self {
        Self {
            client: reqwest::Client::new(),
            credentials,
        }
    }
}

impl ApiProvider for BedrockProvider {
    fn stream(&self, model: &Model, ctx: Context, opts: Option<StreamOptions>) -> EventStream {
        if let Err(error) = validate_options("bedrock-converse-stream", opts.as_ref()) {
            return error_stream(model, StopReason::Error, error);
        }
        let uses_bearer = opts
            .as_ref()
            .and_then(|options| options.bedrock_bearer_token.as_ref())
            .is_some();
        let explicit_credentials = if uses_bearer {
            Ok(None)
        } else if let Some((access_key, secret_key)) = self.credentials.as_ref() {
            Ok(Some(auth::AwsCredentials {
                access_key: access_key.clone(),
                secret_key: secret_key.clone(),
                session_token: None,
            }))
        } else {
            auth::resolve_explicit_credentials_from_options(&opts)
        };
        let explicit_credentials = match explicit_credentials {
            Ok(credentials) => credentials,
            Err(error) => return error_stream(model, StopReason::Error, error),
        };
        if uses_bearer || explicit_credentials.is_some() {
            return stream_with_credentials(
                self.client.clone(),
                model,
                ctx,
                opts,
                explicit_credentials,
                uses_bearer,
            );
        }

        let client = self.client.clone();
        let model = model.clone();
        let profile = opts
            .as_ref()
            .and_then(|options| options.bedrock_profile.clone());
        let cancel = opts.as_ref().and_then(|options| options.cancel.clone());
        let timeout_ms = opts.as_ref().and_then(|options| options.timeout_ms);
        Box::pin(stream! {
            if timeout_ms == Some(0) {
                yield terminal_error(&model, StopReason::Error, "Bedrock invocation timed out after 0 ms".into());
                return;
            }
            let started = Instant::now();
            let resolution = auth::resolve_credentials_from_chain(profile.as_deref());
            tokio::pin!(resolution);
            let credentials = match (cancel.as_ref(), timeout_ms) {
                (Some(cancel), Some(timeout_ms)) => tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {
                        yield terminal_error(&model, StopReason::Aborted, "Bedrock credential resolution was cancelled".into());
                        return;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(timeout_ms)) => {
                        yield terminal_error(&model, StopReason::Error, format!("Bedrock invocation timed out after {timeout_ms} ms"));
                        return;
                    }
                    result = &mut resolution => result,
                },
                (Some(cancel), None) => tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {
                        yield terminal_error(&model, StopReason::Aborted, "Bedrock credential resolution was cancelled".into());
                        return;
                    }
                    result = &mut resolution => result,
                },
                (None, Some(timeout_ms)) => tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(timeout_ms)) => {
                        yield terminal_error(&model, StopReason::Error, format!("Bedrock invocation timed out after {timeout_ms} ms"));
                        return;
                    }
                    result = &mut resolution => result,
                },
                (None, None) => resolution.await,
            };
            let credentials = match credentials {
                Ok(credentials) => credentials,
                Err(error) => {
                    yield terminal_error(&model, StopReason::Error, error);
                    return;
                }
            };
            let mut remaining_options = opts;
            if let Some(timeout_ms) = timeout_ms {
                let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                let remaining_ms = timeout_ms.saturating_sub(elapsed_ms);
                if remaining_ms == 0 {
                    yield terminal_error(&model, StopReason::Error, format!("Bedrock invocation timed out after {timeout_ms} ms"));
                    return;
                }
                remaining_options
                    .get_or_insert_with(StreamOptions::default)
                    .timeout_ms = Some(remaining_ms);
            }
            let mut events = stream_with_credentials(
                client,
                &model,
                ctx,
                remaining_options,
                Some(credentials),
                false,
            );
            while let Some(event) = events.next().await {
                yield event;
            }
        })
    }
}

fn stream_with_credentials(
    client: reqwest::Client,
    model: &Model,
    ctx: Context,
    opts: Option<StreamOptions>,
    credentials: Option<auth::AwsCredentials>,
    uses_bearer: bool,
) -> EventStream {
    let region = opts
        .as_ref()
        .and_then(|options| options.bedrock_region.clone())
        .or_else(|| auth::region_from_endpoint(&model.base_url))
        .unwrap_or_else(|| "us-east-1".into());
    let request_body = build_request(model, &ctx, &opts);
    let payload = match serde_json::to_value(request_body) {
        Ok(payload) => payload,
        Err(error) => {
            return error_stream(
                model,
                StopReason::Error,
                format!("Bedrock request serialization failed: {error}"),
            );
        }
    };
    let url = format!(
        "{}/model/{}/converse-stream",
        model.base_url.trim_end_matches('/'),
        model.id
    );

    let request_url = url.clone();
    let request_region = region.clone();
    let request_options = opts.clone();
    let model_headers = model.headers.clone();
    send_json_stream_with_request_factory(
        model,
        opts.as_ref(),
        "bedrock-converse-stream",
        payload,
        move |payload| {
            let body = serde_json::to_vec(payload)
                .map_err(|error| format!("Bedrock request serialization failed: {error}"))?;
            let auth_headers = if uses_bearer {
                auth::auth_headers(&request_url, &request_region, &body, &request_options)?
            } else {
                let credentials = credentials
                    .as_ref()
                    .ok_or_else(|| "Bedrock credentials are unavailable".to_string())?;
                let (host, uri, query) = auth::parse_url_for_signing(&request_url)?;
                sigv4::sign(
                    sigv4::SignRequest {
                        method: "POST",
                        uri: &uri,
                        query: &query,
                        host: &host,
                        region: &request_region,
                        service: "bedrock",
                        access_key: &credentials.access_key,
                        secret_key: &credentials.secret_key,
                        session_token: credentials.session_token.as_deref(),
                        time: std::time::SystemTime::now(),
                        body: &body,
                    },
                    &[],
                )?
                .headers
            };

            let mut request = client
                .post(&request_url)
                .header("content-type", "application/json")
                .header("accept", "application/vnd.amazon.eventstream")
                .body(body);
            for (key, value) in safe_custom_headers(
                model_headers.as_ref(),
                request_options
                    .as_ref()
                    .and_then(|options| options.headers.as_ref()),
            )? {
                request = request.header(key, value);
            }
            for (key, value) in auth_headers {
                request = request.header(key, value);
            }
            Ok(request)
        },
        |body, model, cancel| stream::process(body, model, cancel),
    )
}

fn safe_custom_headers(
    model_headers: Option<&serde_json::Value>,
    option_headers: Option<&serde_json::Value>,
) -> Result<Vec<(String, String)>, String> {
    const SIGNATURE_HEADERS: &[&str] = &[
        "authorization",
        "host",
        "x-amz-content-sha256",
        "x-amz-date",
        "x-amz-security-token",
    ];
    let mut headers = std::collections::BTreeMap::new();
    for source in [model_headers, option_headers].into_iter().flatten() {
        let object = source
            .as_object()
            .ok_or_else(|| "Bedrock custom headers must be a JSON object".to_string())?;
        for (key, value) in object {
            if SIGNATURE_HEADERS
                .iter()
                .any(|reserved| key.eq_ignore_ascii_case(reserved))
            {
                return Err(format!(
                    "Bedrock custom header `{key}` cannot override a signature header"
                ));
            }
            let value = value
                .as_str()
                .ok_or_else(|| format!("Bedrock custom header `{key}` must be a string"))?;
            headers.insert(key.clone(), value.to_string());
        }
    }
    Ok(headers.into_iter().collect())
}

fn error_stream(model: &Model, reason: StopReason, error: String) -> EventStream {
    let event = terminal_error(model, reason, error);
    Box::pin(stream! {
        yield event;
    })
}

fn terminal_error(model: &Model, reason: StopReason, error: String) -> AssistantMessageEvent {
    let model_id = model.id.clone();
    let provider = model.provider.clone();
    let mut message = AssistantMessage::empty("bedrock-converse-stream", &model_id);
    message.provider = Some(provider);
    message.error_message = Some(error);
    message.stop_reason = reason.clone();
    AssistantMessageEvent::Error { reason, message }
}
