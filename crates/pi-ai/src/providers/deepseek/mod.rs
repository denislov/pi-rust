pub mod convert;
pub mod process;
pub mod wire;

use async_stream::stream;

use crate::registry::ApiProvider;
use crate::stream::EventStream;
use crate::types::{
    AssistantMessage, AssistantMessageEvent, Context, Model, StopReason, StreamOptions,
};
use convert::build_request;
use process::response_to_events;
use wire::ChatCompletionResponse;

pub struct DeepSeekProvider {
    client: reqwest::Client,
    api_key: Option<String>,
}

impl DeepSeekProvider {
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

impl ApiProvider for DeepSeekProvider {
    fn stream(&self, model: &Model, ctx: Context, opts: Option<StreamOptions>) -> EventStream {
        let key = opts
            .as_ref()
            .and_then(|opts| opts.api_key.clone())
            .or_else(|| self.resolve_key());
        let cancel = opts.as_ref().and_then(|opts| opts.cancel.clone());

        let Some(api_key) = key else {
            let model_id = model.id.clone();
            return Box::pin(stream! {
                let mut msg = AssistantMessage::empty("deepseek-chat-completions", &model_id);
                msg.provider = Some("deepseek".into());
                msg.error_message = Some("No DeepSeek API key found. Set DEEPSEEK_API_KEY or pass apiKey in options.".into());
                msg.stop_reason = StopReason::Error;
                yield AssistantMessageEvent::Error {
                    reason: StopReason::Error,
                    message: msg,
                };
            });
        };

        let request_body = build_request(model, &ctx, &opts);
        let base_url = model.base_url.trim_end_matches('/');
        let url = format!("{}/chat/completions", base_url);
        let mut request = self
            .client
            .post(&url)
            .bearer_auth(api_key)
            .header("content-type", "application/json")
            .header("accept", "application/json")
            .json(&request_body);

        if let Some(opts) = &opts
            && let Some(headers) = &opts.headers
            && let Some(obj) = headers.as_object()
        {
            for (key, value) in obj {
                if let Some(value) = value.as_str() {
                    request = request.header(key.as_str(), value);
                }
            }
        }

        let model = model.clone();
        let model_id = model.id.clone();
        Box::pin(stream! {
            if let Some(token) = &cancel
                && token.is_cancelled() {
                    let mut msg = AssistantMessage::empty("deepseek-chat-completions", &model_id);
                    msg.provider = Some("deepseek".into());
                    msg.error_message = Some("cancelled".into());
                    msg.stop_reason = StopReason::Aborted;
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Aborted,
                        message: msg,
                    };
                    return;
                }

            let response = match request.send().await {
                Ok(response) => response,
                Err(error) => {
                    let mut msg = AssistantMessage::empty("deepseek-chat-completions", &model_id);
                    msg.provider = Some("deepseek".into());
                    msg.error_message = Some(format!("HTTP request failed: {}", error));
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
                let mut msg = AssistantMessage::empty("deepseek-chat-completions", &model_id);
                msg.provider = Some("deepseek".into());
                msg.error_message = Some(format!("HTTP {} : {}", status, body));
                msg.stop_reason = StopReason::Error;
                yield AssistantMessageEvent::Error {
                    reason: StopReason::Error,
                    message: msg,
                };
                return;
            }

            let parsed = match response.json::<ChatCompletionResponse>().await {
                Ok(parsed) => parsed,
                Err(error) => {
                    let mut msg = AssistantMessage::empty("deepseek-chat-completions", &model_id);
                    msg.provider = Some("deepseek".into());
                    msg.error_message = Some(format!("DeepSeek response parse error: {}", error));
                    msg.stop_reason = StopReason::Error;
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        message: msg,
                    };
                    return;
                }
            };

            for event in response_to_events(parsed, &model) {
                yield event;
            }
        })
    }
}
