pub mod convert;
pub mod stream;
pub mod wire;

use async_stream::stream;

use crate::protocol::{
    AssistantMessage, AssistantMessageEvent, Context, StopReason, StreamOptions,
};

use crate::model::Model;
use crate::protocol::stream::EventStream;
use crate::registry::ApiProvider;
use crate::transport::http::send_json_stream;
use convert::build_request;

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
        let payload = match serde_json::to_value(&request_body) {
            Ok(payload) => payload,
            Err(error) => return serialization_error(model, error),
        };
        let base_url = model.base_url.trim_end_matches('/');
        let url = format!("{}/chat/completions", base_url);
        let mut request = self
            .client
            .post(&url)
            .bearer_auth(api_key)
            .header("content-type", "application/json")
            .header("accept", "application/json");

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

        send_json_stream(
            &self.client,
            model,
            opts.as_ref(),
            "deepseek-chat-completions",
            request,
            payload,
            |body, model, cancel| stream::process(body, model, cancel),
        )
    }
}

fn serialization_error(model: &Model, error: serde_json::Error) -> EventStream {
    let model_id = model.id.clone();
    let provider = model.provider.clone();
    Box::pin(stream! {
        let mut message = AssistantMessage::empty("deepseek-chat-completions", &model_id);
        message.provider = Some(provider);
        message.error_message = Some(format!("DeepSeek request serialization failed: {error}"));
        message.stop_reason = StopReason::Error;
        yield AssistantMessageEvent::Error {
            reason: StopReason::Error,
            message,
        };
    })
}
