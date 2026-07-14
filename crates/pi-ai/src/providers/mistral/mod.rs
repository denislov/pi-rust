pub mod convert;
pub mod process;
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

pub struct MistralProvider {
    client: reqwest::Client,
    api_key: Option<String>,
}

impl MistralProvider {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
        }
    }

    fn resolve_key(&self) -> Option<String> {
        self.api_key.clone()
    }
}

pub fn build_headers(model: &Model, opts: &Option<StreamOptions>) -> BTreeMap<String, String> {
    let mut headers = BTreeMap::new();
    append_json_headers(&mut headers, model.headers.as_ref());
    if let Some(opts) = opts {
        append_json_headers(&mut headers, opts.headers.as_ref());
        if let Some(session_id) = &opts.session_id {
            headers
                .entry("x-affinity".into())
                .or_insert_with(|| session_id.clone());
        }
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

impl ApiProvider for MistralProvider {
    fn stream(&self, model: &Model, ctx: Context, opts: Option<StreamOptions>) -> EventStream {
        let key = opts
            .as_ref()
            .and_then(|o| o.api_key.clone())
            .or_else(|| self.resolve_key());
        let cancel = opts.as_ref().and_then(|o| o.cancel.clone());

        let Some(api_key) = key else {
            let model_id = model.id.clone();
            return Box::pin(stream! {
                let mut msg = AssistantMessage::empty("mistral-conversations", &model_id);
                msg.provider = Some("mistral".into());
                msg.error_message = Some("No Mistral API key found. Set MISTRAL_API_KEY or pass apiKey in options.".into());
                msg.stop_reason = StopReason::Error;
                yield AssistantMessageEvent::Error {
                    reason: StopReason::Error,
                    message: msg,
                };
            });
        };

        let req_body = build_request(model, &ctx, &opts);
        let base_url = model.base_url.trim_end_matches('/');
        let url = if base_url.ends_with("/v1") {
            format!("{}/chat/completions", base_url)
        } else {
            format!("{}/v1/chat/completions", base_url)
        };

        let mut request = self
            .client
            .post(&url)
            .bearer_auth(api_key)
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
            .json(&req_body);

        for (key, value) in build_headers(model, &opts) {
            request = request.header(key, value);
        }

        let model = model.clone();
        let model_id = model.id.clone();
        Box::pin(stream! {
            let response = match request.send().await {
                Ok(r) => r,
                Err(e) => {
                    let mut msg = AssistantMessage::empty("mistral-conversations", &model_id);
                    msg.provider = Some(model.provider.clone());
                    msg.error_message = Some(format!("HTTP request failed: {}", e));
                    msg.stop_reason = StopReason::Error;
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        message: msg,
                    };
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                let mut msg = AssistantMessage::empty("mistral-conversations", &model_id);
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
