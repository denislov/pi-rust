pub mod auth;
pub mod convert;
pub mod datetime;
pub mod sigv4;
pub mod stream;
pub mod wire;

use async_stream::stream;
use futures::StreamExt;

use crate::protocol::{
    AssistantMessage, AssistantMessageEvent, Context, StopReason, StreamOptions,
};

use crate::model::Model;
use crate::protocol::stream::EventStream;
use crate::registry::ApiProvider;
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
        let region = opts
            .as_ref()
            .and_then(|o| o.bedrock_region.clone())
            .or_else(|| auth::region_from_endpoint(&model.base_url))
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
        {
            auth::auth_headers(&url, &region, &body, &opts)
        } else {
            let credentials = self
                .credentials
                .as_ref()
                .map(|(access_key, secret_key)| auth::AwsCredentials {
                    access_key: access_key.clone(),
                    secret_key: secret_key.clone(),
                    session_token: None,
                })
                .map(Ok)
                .unwrap_or_else(|| auth::resolve_credentials_from_options(&opts));
            match credentials {
                Ok(credentials) => {
                    let (host, uri, query) = match auth::parse_url_for_signing(&url) {
                        Ok(parts) => parts,
                        Err(error) => return error_stream(model, error),
                    };
                    let (date, amz_date) = datetime::current_aws_dates();
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
            let mut event_stream = stream::process(Box::pin(body_stream), model, cancel);
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
