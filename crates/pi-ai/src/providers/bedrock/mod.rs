pub mod convert;
pub mod process;
pub mod sigv4;
pub mod wire;

use async_stream::stream;
use futures::StreamExt;
use std::collections::BTreeMap;

use crate::registry::ApiProvider;
use crate::stream::EventStream;
use crate::types::{
    AssistantMessage, AssistantMessageEvent, Context, Model, StopReason, StreamOptions,
};
use convert::build_request;

#[derive(Debug, Clone)]
struct AwsCredentials {
    access_key: String,
    secret_key: String,
    session_token: Option<String>,
}

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

pub fn auth_headers(
    url: &str,
    region: &str,
    body: &[u8],
    opts: &Option<StreamOptions>,
) -> Result<BTreeMap<String, String>, String> {
    if let Some(token) = opts
        .as_ref()
        .and_then(|o| o.bedrock_bearer_token.clone())
        .or_else(|| std::env::var("AWS_BEARER_TOKEN_BEDROCK").ok())
        .filter(|token| !token.trim().is_empty())
    {
        return Ok(BTreeMap::from([(
            "authorization".into(),
            format!("Bearer {}", token),
        )]));
    }

    let credentials = resolve_credentials(None)?;
    let (host, uri, query) = parse_url_for_signing(url)?;
    let (date, amz_date) = current_aws_dates();
    let signed = sigv4::sign(
        sigv4::SignRequest {
            method: "POST",
            uri: &uri,
            query: &query,
            host: &host,
            region,
            service: "bedrock",
            access_key: &credentials.access_key,
            secret_key: &credentials.secret_key,
            session_token: credentials.session_token.as_deref(),
            amz_date: &amz_date,
            date: &date,
            body,
        },
        &[],
    );
    Ok(signed.headers)
}

impl ApiProvider for BedrockProvider {
    fn stream(&self, model: &Model, ctx: Context, opts: Option<StreamOptions>) -> EventStream {
        let region = opts
            .as_ref()
            .and_then(|o| o.bedrock_region.clone())
            .or_else(|| std::env::var("AWS_REGION").ok())
            .or_else(|| std::env::var("AWS_DEFAULT_REGION").ok())
            .or_else(|| region_from_endpoint(&model.base_url))
            .unwrap_or_else(|| "us-east-1".into());

        let req_body = build_request(model, &ctx, &opts);
        let body = match serde_json::to_vec(&req_body) {
            Ok(body) => body,
            Err(error) => {
                let model_id = model.id.clone();
                let provider = model.provider.clone();
                return Box::pin(stream! {
                    let mut msg = AssistantMessage::empty("bedrock-converse-stream", &model_id);
                    msg.provider = Some(provider);
                    msg.error_message = Some(format!("Bedrock request serialization failed: {}", error));
                    msg.stop_reason = StopReason::Error;
                    yield AssistantMessageEvent::Error { reason: StopReason::Error, message: msg };
                });
            }
        };

        let url = format!(
            "{}/model/{}/converse-stream",
            model.base_url.trim_end_matches('/'),
            model.id
        );

        let auth = if opts
            .as_ref()
            .and_then(|o| o.bedrock_bearer_token.as_ref())
            .is_some()
            || std::env::var("AWS_BEARER_TOKEN_BEDROCK")
                .ok()
                .filter(|v| !v.is_empty())
                .is_some()
        {
            auth_headers(&url, &region, &body, &opts)
        } else {
            let credentials = self
                .credentials
                .as_ref()
                .map(|(access_key, secret_key)| AwsCredentials {
                    access_key: access_key.clone(),
                    secret_key: secret_key.clone(),
                    session_token: None,
                })
                .map(Ok)
                .unwrap_or_else(|| resolve_credentials(None));
            match credentials {
                Ok(credentials) => {
                    let (host, uri, query) = match parse_url_for_signing(&url) {
                        Ok(parts) => parts,
                        Err(error) => return error_stream(model, error),
                    };
                    let (date, amz_date) = current_aws_dates();
                    Ok(sigv4::sign(
                        sigv4::SignRequest {
                            method: "POST",
                            uri: &uri,
                            query: &query,
                            host: &host,
                            region: &region,
                            service: "bedrock",
                            access_key: &credentials.access_key,
                            secret_key: &credentials.secret_key,
                            session_token: credentials.session_token.as_deref(),
                            amz_date: &amz_date,
                            date: &date,
                            body: &body,
                        },
                        &[],
                    )
                    .headers)
                }
                Err(error) => Err(error),
            }
        };

        let headers = match auth {
            Ok(headers) => headers,
            Err(error) => return error_stream(model, error),
        };

        let mut request = self
            .client
            .post(&url)
            .header("content-type", "application/json")
            .header("accept", "application/vnd.amazon.eventstream")
            .body(body);
        for (key, value) in headers {
            request = request.header(key, value);
        }

        let cancel = opts.as_ref().and_then(|o| o.cancel.clone());
        let model = model.clone();
        let model_id = model.id.clone();
        Box::pin(stream! {
            let response = match request.send().await {
                Ok(response) => response,
                Err(error) => {
                    let mut msg = AssistantMessage::empty("bedrock-converse-stream", &model_id);
                    msg.provider = Some(model.provider.clone());
                    msg.error_message = Some(format!("HTTP request failed: {}", error));
                    msg.stop_reason = StopReason::Error;
                    yield AssistantMessageEvent::Error { reason: StopReason::Error, message: msg };
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                let mut msg = AssistantMessage::empty("bedrock-converse-stream", &model_id);
                msg.provider = Some(model.provider.clone());
                msg.error_message = Some(format!("HTTP {} : {}", status, body));
                msg.stop_reason = StopReason::Error;
                yield AssistantMessageEvent::Error { reason: StopReason::Error, message: msg };
                return;
            }

            let body_stream = response.bytes_stream().map(|r| r.map_err(|e| e.to_string()));
            let mut event_stream = process::process(Box::pin(body_stream), model, cancel);
            while let Some(event) = event_stream.next().await {
                yield event;
            }
        })
    }
}

fn error_stream(model: &Model, error: String) -> EventStream {
    let model_id = model.id.clone();
    let provider = model.provider.clone();
    Box::pin(stream! {
        let mut msg = AssistantMessage::empty("bedrock-converse-stream", &model_id);
        msg.provider = Some(provider);
        msg.error_message = Some(error);
        msg.stop_reason = StopReason::Error;
        yield AssistantMessageEvent::Error {
            reason: StopReason::Error,
            message: msg,
        };
    })
}

fn resolve_credentials(explicit: Option<(String, String)>) -> Result<AwsCredentials, String> {
    if let Some((access_key, secret_key)) = explicit {
        return Ok(AwsCredentials {
            access_key,
            secret_key,
            session_token: None,
        });
    }
    let access_key = std::env::var("AWS_ACCESS_KEY_ID").ok();
    let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY").ok();
    match (access_key, secret_key) {
        (Some(access_key), Some(secret_key)) if !access_key.is_empty() && !secret_key.is_empty() => {
            Ok(AwsCredentials {
                access_key,
                secret_key,
                session_token: std::env::var("AWS_SESSION_TOKEN").ok(),
            })
        }
        _ => Err("No AWS credentials found. Set AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY, AWS_BEARER_TOKEN_BEDROCK, or pass a Bedrock bearer token.".into()),
    }
}

fn parse_url_for_signing(url: &str) -> Result<(String, String, String), String> {
    let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return Err(format!("Invalid Bedrock URL: {}", url));
    };
    let (host, path_and_query) = match rest.split_once('/') {
        Some((host, path)) => (host, format!("/{}", path)),
        None => (rest, "/".into()),
    };
    let (path, query) = match path_and_query.split_once('?') {
        Some((path, query)) => (path.to_string(), query.to_string()),
        None => (path_and_query, String::new()),
    };
    Ok((host.to_string(), path, query))
}

fn region_from_endpoint(base_url: &str) -> Option<String> {
    let host = base_url
        .strip_prefix("https://")
        .or_else(|| base_url.strip_prefix("http://"))?
        .split('/')
        .next()?;
    let mut parts = host.split('.');
    if parts.next()? != "bedrock-runtime" {
        return None;
    }
    parts.next().map(str::to_string)
}

fn current_aws_dates() -> (String, String) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = now.div_euclid(86_400);
    let seconds = now.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds / 3600;
    let minute = (seconds % 3600) / 60;
    let second = seconds % 60;
    (
        format!("{:04}{:02}{:02}", year, month, day),
        format!(
            "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
            year, month, day, hour, minute, second
        ),
    )
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 }.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096).div_euclid(365);
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2).div_euclid(153);
    let d = doy - (153 * mp + 2).div_euclid(5) + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year, m as u32, d as u32)
}
