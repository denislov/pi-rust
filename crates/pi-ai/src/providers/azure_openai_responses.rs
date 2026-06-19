use async_stream::stream;
use futures::StreamExt;
use std::collections::BTreeMap;

use crate::providers::openai::responses::{convert, process, wire};
use crate::registry::ApiProvider;
use crate::stream::EventStream;
use crate::types::{
    AssistantMessage, AssistantMessageEvent, Context, Model, StopReason, StreamOptions,
};
use crate::util::env_keys::env_api_key;
use crate::util::http::RetryConfig;

const DEFAULT_AZURE_API_VERSION: &str = "v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AzureRequestTarget {
    pub url: String,
    pub deployment_name: String,
}

pub struct AzureOpenAIResponsesProvider {
    client: reqwest::Client,
    api_key: Option<String>,
}

impl AzureOpenAIResponsesProvider {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
        }
    }

    fn resolve_key(&self) -> Option<String> {
        self.api_key
            .clone()
            .or_else(|| env_api_key("azure-openai-responses"))
    }
}

pub fn build_request(
    model: &Model,
    ctx: &Context,
    opts: &Option<StreamOptions>,
) -> wire::ResponseCreateRequest {
    let mut request = convert::build_request(model, ctx, opts);
    request.model = resolve_deployment_name(model, opts);
    request
}

pub fn resolve_target(
    model: &Model,
    opts: &Option<StreamOptions>,
) -> Result<AzureRequestTarget, String> {
    let api_version = opts
        .as_ref()
        .and_then(|o| o.azure_api_version.clone())
        .or_else(|| std::env::var("AZURE_OPENAI_API_VERSION").ok())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_AZURE_API_VERSION.into());

    let base_url = opts
        .as_ref()
        .and_then(|o| o.azure_base_url.clone())
        .or_else(|| std::env::var("AZURE_OPENAI_BASE_URL").ok())
        .or_else(|| {
            opts.as_ref()
                .and_then(|o| o.azure_resource_name.clone())
                .or_else(|| std::env::var("AZURE_OPENAI_RESOURCE_NAME").ok())
                .map(|resource| format!("https://{}.openai.azure.com/openai/v1", resource))
        })
        .or_else(|| {
            if model.base_url.trim().is_empty() {
                None
            } else {
                Some(model.base_url.clone())
            }
        })
        .ok_or_else(|| {
            "Azure OpenAI base URL is required. Set AZURE_OPENAI_BASE_URL or AZURE_OPENAI_RESOURCE_NAME, or pass azureBaseUrl, azureResourceName, or model.baseUrl.".to_string()
        })?;

    let base_url = normalize_azure_base_url(&base_url)?;
    Ok(AzureRequestTarget {
        url: format!("{}/responses?api-version={}", base_url, api_version),
        deployment_name: resolve_deployment_name(model, opts),
    })
}

fn resolve_deployment_name(model: &Model, opts: &Option<StreamOptions>) -> String {
    opts.as_ref()
        .and_then(|o| o.azure_deployment_name.clone())
        .or_else(|| {
            let map = std::env::var("AZURE_OPENAI_DEPLOYMENT_NAME_MAP").ok()?;
            parse_deployment_name_map(&map).remove(&model.id)
        })
        .unwrap_or_else(|| model.id.clone())
}

fn parse_deployment_name_map(value: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for entry in value.split(',') {
        let Some((model_id, deployment)) = entry.trim().split_once('=') else {
            continue;
        };
        let model_id = model_id.trim();
        let deployment = deployment.trim();
        if !model_id.is_empty() && !deployment.is_empty() {
            map.insert(model_id.to_string(), deployment.to_string());
        }
    }
    map
}

fn normalize_azure_base_url(base_url: &str) -> Result<String, String> {
    let trimmed = base_url.trim().trim_end_matches('/');
    let Some((scheme, rest)) = trimmed.split_once("://") else {
        return Err(format!("Invalid Azure OpenAI base URL: {}", base_url));
    };
    if scheme != "https" && scheme != "http" {
        return Err(format!("Invalid Azure OpenAI base URL: {}", base_url));
    }

    let (host, path) = match rest.split_once('/') {
        Some((host, path)) => (host, format!("/{}", path.trim_end_matches('/'))),
        None => (rest, String::new()),
    };
    if host.is_empty() {
        return Err(format!("Invalid Azure OpenAI base URL: {}", base_url));
    }

    let is_azure_host =
        host.ends_with(".openai.azure.com") || host.ends_with(".cognitiveservices.azure.com");
    let path = if is_azure_host && (path.is_empty() || path == "/" || path == "/openai") {
        "/openai/v1".to_string()
    } else {
        path
    };

    Ok(format!("{}://{}{}", scheme, host, path)
        .trim_end_matches('/')
        .to_string())
}

fn build_headers(model: &Model, opts: &Option<StreamOptions>) -> BTreeMap<String, String> {
    let mut headers = BTreeMap::new();
    append_json_headers(&mut headers, model.headers.as_ref());
    if let Some(opts) = opts {
        append_json_headers(&mut headers, opts.headers.as_ref());
    }
    headers
}

fn append_json_headers(headers: &mut BTreeMap<String, String>, value: Option<&serde_json::Value>) {
    let Some(obj) = value.and_then(|v| v.as_object()) else {
        return;
    };
    for (key, value) in obj {
        if let Some(value) = value.as_str() {
            headers.insert(key.clone(), value.to_string());
        }
    }
}

impl ApiProvider for AzureOpenAIResponsesProvider {
    fn stream(&self, model: &Model, ctx: Context, opts: Option<StreamOptions>) -> EventStream {
        let key = opts
            .as_ref()
            .and_then(|o| o.api_key.clone())
            .or_else(|| self.resolve_key());
        let cancel = opts.as_ref().and_then(|o| o.cancel.clone());

        let Some(api_key) = key else {
            let model_id = model.id.clone();
            return Box::pin(stream! {
                let mut msg = AssistantMessage::empty("azure-openai-responses", &model_id);
                msg.provider = Some("azure-openai-responses".into());
                msg.error_message = Some("No Azure OpenAI API key found. Set AZURE_OPENAI_API_KEY or pass apiKey in options.".into());
                msg.stop_reason = StopReason::Error;
                yield AssistantMessageEvent::Error {
                    reason: StopReason::Error,
                    message: msg,
                };
            });
        };

        let target = match resolve_target(model, &opts) {
            Ok(target) => target,
            Err(error) => {
                let model_id = model.id.clone();
                return Box::pin(stream! {
                    let mut msg = AssistantMessage::empty("azure-openai-responses", &model_id);
                    msg.provider = Some("azure-openai-responses".into());
                    msg.error_message = Some(error);
                    msg.stop_reason = StopReason::Error;
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        message: msg,
                    };
                });
            }
        };

        let req_body = build_request(model, &ctx, &opts);
        let mut request = self
            .client
            .post(&target.url)
            .header("api-key", api_key)
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
            .json(&req_body);

        for (key, value) in build_headers(model, &opts) {
            request = request.header(key, value);
        }

        let model = model.clone();
        let model_id = model.id.clone();
        let retry_cfg = RetryConfig::from_options(opts.as_ref());
        Box::pin(stream! {
            let send_future = request.send();
            let response = match retry_cfg.timeout_ms {
                Some(ms) => match tokio::time::timeout(std::time::Duration::from_millis(ms), send_future).await {
                    Ok(Ok(r)) => r,
                    Ok(Err(e)) => {
                        let mut msg = AssistantMessage::empty("azure-openai-responses", &model_id);
                        msg.provider = Some(model.provider.clone());
                        msg.error_message = Some(format!("HTTP request failed: {}", e));
                        msg.stop_reason = StopReason::Error;
                        yield AssistantMessageEvent::Error { reason: StopReason::Error, message: msg };
                        return;
                    }
                    Err(_) => {
                        let mut msg = AssistantMessage::empty("azure-openai-responses", &model_id);
                        msg.provider = Some(model.provider.clone());
                        msg.error_message = Some(format!("Request timed out after {}ms", ms));
                        msg.stop_reason = StopReason::Error;
                        yield AssistantMessageEvent::Error { reason: StopReason::Error, message: msg };
                        return;
                    }
                },
                None => match send_future.await {
                    Ok(r) => r,
                    Err(e) => {
                        let mut msg = AssistantMessage::empty("azure-openai-responses", &model_id);
                        msg.provider = Some(model.provider.clone());
                        msg.error_message = Some(format!("HTTP request failed: {}", e));
                        msg.stop_reason = StopReason::Error;
                        yield AssistantMessageEvent::Error { reason: StopReason::Error, message: msg };
                        return;
                    }
                },
            };

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                let mut msg = AssistantMessage::empty("azure-openai-responses", &model_id);
                msg.provider = Some(model.provider.clone());
                msg.error_message = Some(format!("HTTP {} : {}", status, body));
                msg.stop_reason = StopReason::Error;
                yield AssistantMessageEvent::Error {
                    reason: StopReason::Error,
                    message: msg,
                };
                return;
            }

            let body_stream = response
                .bytes_stream()
                .map(|r| r.map_err(|e| e.to_string()));

            let mut event_stream = process::process(body_stream, model, cancel);
            while let Some(event) = event_stream.next().await {
                yield event;
            }
        })
    }
}
