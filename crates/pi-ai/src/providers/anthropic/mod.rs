pub mod convert;
pub mod process;
pub mod sse;
pub mod wire;

use async_stream::stream;
use futures::StreamExt;

use crate::registry::ApiProvider;
use crate::stream::EventStream;
use crate::types::{
    AssistantMessage, AssistantMessageEvent, Context, Model, StopReason, StreamOptions,
};
use convert::build_request;

pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: Option<String>,
}

impl AnthropicProvider {
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

impl ApiProvider for AnthropicProvider {
    fn stream(&self, model: &Model, ctx: Context, opts: Option<StreamOptions>) -> EventStream {
        let key = opts
            .as_ref()
            .and_then(|o| o.api_key.clone())
            .or_else(|| self.resolve_key());
        let cancel = opts.as_ref().and_then(|o| o.cancel.clone());

        let Some(api_key) = key else {
            let model_id = model.id.clone();
            return Box::pin(stream! {
                let mut msg = AssistantMessage::empty("anthropic-messages", &model_id);
                msg.error_message = Some("No Anthropic API key found. Set ANTHROPIC_API_KEY or pass apiKey in options.".into());
                msg.stop_reason = StopReason::Error;
                yield AssistantMessageEvent::Error {
                    reason: StopReason::Error,
                    message: msg,
                };
            });
        };

        let req_body = build_request(model, &ctx, &opts);
        let base_url = model.base_url.trim_end_matches('/');
        let url = format!("{}/v1/messages", base_url);

        let mut request = self
            .client
            .post(&url)
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
            .json(&req_body);

        if let Some(opts) = &opts
            && let Some(ref headers) = opts.headers
                && let Some(obj) = headers.as_object() {
                    for (k, v) in obj {
                        if let Some(val) = v.as_str() {
                            request = request.header(k.as_str(), val);
                        }
                    }
                }

        let model = model.clone();
        let model_id = model.id.clone();
        Box::pin(stream! {
            let response = match request.send().await {
                Ok(r) => r,
                Err(e) => {
                    let mut msg = AssistantMessage::empty("anthropic-messages", &model_id);
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
                let mut msg = AssistantMessage::empty("anthropic-messages", &model_id);
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
