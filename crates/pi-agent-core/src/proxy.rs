use futures::StreamExt;
use pi_ai::stream::EventStream;
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Model, StopReason,
    StreamOptions, Usage,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

#[derive(Debug, Clone)]
pub struct ProxyStreamOptions {
    pub proxy_url: String,
    pub auth_token: String,
    pub stream_options: StreamOptions,
}

#[derive(Debug, Clone)]
pub struct ProxyRequest {
    pub url: String,
    pub auth_token: String,
    pub body: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ProxyAssistantMessageEvent {
    #[serde(rename = "start")]
    Start,
    #[serde(rename = "text_start")]
    TextStart {
        #[serde(rename = "contentIndex")]
        content_index: u32,
    },
    #[serde(rename = "text_delta")]
    TextDelta {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        delta: String,
    },
    #[serde(rename = "text_end")]
    TextEnd {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        #[serde(rename = "contentSignature", skip_serializing_if = "Option::is_none")]
        content_signature: Option<String>,
    },
    #[serde(rename = "thinking_start")]
    ThinkingStart {
        #[serde(rename = "contentIndex")]
        content_index: u32,
    },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        delta: String,
    },
    #[serde(rename = "thinking_end")]
    ThinkingEnd {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        #[serde(rename = "contentSignature", skip_serializing_if = "Option::is_none")]
        content_signature: Option<String>,
    },
    #[serde(rename = "toolcall_start")]
    ToolcallStart {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
    },
    #[serde(rename = "toolcall_delta")]
    ToolcallDelta {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        delta: String,
    },
    #[serde(rename = "toolcall_end")]
    ToolcallEnd {
        #[serde(rename = "contentIndex")]
        content_index: u32,
    },
    #[serde(rename = "done")]
    Done { reason: StopReason, usage: Usage },
    #[serde(rename = "error")]
    Error {
        reason: StopReason,
        #[serde(rename = "errorMessage", skip_serializing_if = "Option::is_none")]
        error_message: Option<String>,
        usage: Usage,
    },
}

pub type ProxyTransportFuture =
    Pin<Box<dyn Future<Output = Result<Vec<ProxyAssistantMessageEvent>, String>> + Send>>;

#[derive(Debug, Clone)]
pub struct ProxyMessageState {
    pub partial: AssistantMessage,
    tool_json: BTreeMap<u32, String>,
}

impl ProxyMessageState {
    pub fn new(model: &Model) -> Self {
        let mut partial = AssistantMessage::empty("proxy", &model.id);
        partial.provider = Some(model.provider.clone());
        partial.model = model.id.clone();
        partial.api = model.api.clone();
        Self {
            partial,
            tool_json: BTreeMap::new(),
        }
    }

    pub fn from_partial(partial: AssistantMessage) -> Self {
        Self {
            partial,
            tool_json: BTreeMap::new(),
        }
    }

    pub fn process(
        &mut self,
        proxy_event: ProxyAssistantMessageEvent,
    ) -> Result<AssistantMessageEvent, String> {
        process_proxy_event_with_tool_state(proxy_event, &mut self.partial, &mut self.tool_json)
    }
}

pub fn build_proxy_request_body(
    model: &Model,
    context: &Context,
    options: &ProxyStreamOptions,
) -> Result<serde_json::Value, serde_json::Error> {
    let mut stream_options = serde_json::to_value(&options.stream_options)?;
    if let serde_json::Value::Object(ref mut map) = stream_options {
        map.remove("apiKey");
        map.remove("cancel");
        map.retain(|_, value| !value.is_null());
    }
    Ok(serde_json::json!({
        "model": model,
        "context": context,
        "options": stream_options,
    }))
}

pub fn stream_proxy(model: Model, context: Context, options: ProxyStreamOptions) -> EventStream {
    stream_proxy_with_transport(model, context, options, |request| {
        Box::pin(async move {
            let client = reqwest::Client::new();
            let response = client
                .post(format!("{}/api/stream", request.url.trim_end_matches('/')))
                .bearer_auth(request.auth_token)
                .json(&request.body)
                .send()
                .await
                .map_err(|error| error.to_string())?;
            if !response.status().is_success() {
                return Err(format!(
                    "Proxy error: {} {}",
                    response.status().as_u16(),
                    response.status().canonical_reason().unwrap_or_default()
                ));
            }

            let mut events = Vec::new();
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|error| error.to_string())?;
                buffer.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(newline) = buffer.find('\n') {
                    let line = buffer[..newline].trim_end().to_string();
                    buffer = buffer[newline + 1..].to_string();
                    if let Some(data) = line.strip_prefix("data: ") {
                        let trimmed = data.trim();
                        if !trimmed.is_empty() {
                            let event = serde_json::from_str::<ProxyAssistantMessageEvent>(trimmed)
                                .map_err(|error| error.to_string())?;
                            events.push(event);
                        }
                    }
                }
            }
            Ok(events)
        })
    })
}

pub fn stream_proxy_with_transport<F>(
    model: Model,
    context: Context,
    options: ProxyStreamOptions,
    transport: F,
) -> EventStream
where
    F: FnOnce(ProxyRequest) -> ProxyTransportFuture + Send + 'static,
{
    Box::pin(async_stream::stream! {
        let mut state = ProxyMessageState::new(&model);

        let body = match build_proxy_request_body(&model, &context, &options) {
            Ok(body) => body,
            Err(error) => {
                state.partial.stop_reason = StopReason::Error;
                state.partial.error_message = Some(error.to_string());
                yield AssistantMessageEvent::Error {
                    reason: StopReason::Error,
                    message: state.partial,
                };
                return;
            }
        };

        let request = ProxyRequest {
            url: options.proxy_url,
            auth_token: options.auth_token,
            body,
        };

        match transport(request).await {
            Ok(events) => {
                for event in events {
                    match state.process(event) {
                        Ok(event) => yield event,
                        Err(error) => {
                            state.partial.stop_reason = StopReason::Error;
                            state.partial.error_message = Some(error);
                            yield AssistantMessageEvent::Error {
                                reason: StopReason::Error,
                                message: state.partial.clone(),
                            };
                            return;
                        }
                    }
                }
            }
            Err(error) => {
                state.partial.stop_reason = StopReason::Error;
                state.partial.error_message = Some(error);
                yield AssistantMessageEvent::Error {
                    reason: StopReason::Error,
                    message: state.partial,
                };
            }
        }
    })
}

pub fn process_proxy_event(
    proxy_event: ProxyAssistantMessageEvent,
    partial: &mut AssistantMessage,
) -> Result<AssistantMessageEvent, String> {
    let mut state = ProxyMessageState::from_partial(partial.clone());
    let event = state.process(proxy_event)?;
    *partial = state.partial;
    Ok(event)
}

fn process_proxy_event_with_tool_state(
    proxy_event: ProxyAssistantMessageEvent,
    partial: &mut AssistantMessage,
    tool_json: &mut BTreeMap<u32, String>,
) -> Result<AssistantMessageEvent, String> {
    match proxy_event {
        ProxyAssistantMessageEvent::Start => Ok(AssistantMessageEvent::Start {
            content_index: None,
            partial: partial.clone(),
        }),
        ProxyAssistantMessageEvent::TextStart { content_index } => {
            ensure_content_len(&mut partial.content, content_index as usize);
            partial.content[content_index as usize] = ContentBlock::Text {
                text: String::new(),
                text_signature: None,
            };
            Ok(AssistantMessageEvent::TextStart {
                content_index,
                partial: partial.clone(),
            })
        }
        ProxyAssistantMessageEvent::TextDelta {
            content_index,
            delta,
        } => match partial.content.get_mut(content_index as usize) {
            Some(ContentBlock::Text { text, .. }) => {
                text.push_str(&delta);
                Ok(AssistantMessageEvent::TextDelta {
                    content_index,
                    delta,
                    partial: partial.clone(),
                })
            }
            _ => Err("Received text_delta for non-text content".into()),
        },
        ProxyAssistantMessageEvent::TextEnd {
            content_index,
            content_signature,
        } => match partial.content.get_mut(content_index as usize) {
            Some(ContentBlock::Text { text_signature, .. }) => {
                *text_signature = content_signature;
                Ok(AssistantMessageEvent::TextEnd {
                    content_index,
                    partial: partial.clone(),
                })
            }
            _ => Err("Received text_end for non-text content".into()),
        },
        ProxyAssistantMessageEvent::ThinkingStart { content_index } => {
            ensure_content_len(&mut partial.content, content_index as usize);
            partial.content[content_index as usize] = ContentBlock::Thinking {
                thinking: String::new(),
                thinking_signature: None,
                redacted: None,
            };
            Ok(AssistantMessageEvent::ThinkingStart {
                content_index,
                partial: partial.clone(),
            })
        }
        ProxyAssistantMessageEvent::ThinkingDelta {
            content_index,
            delta,
        } => match partial.content.get_mut(content_index as usize) {
            Some(ContentBlock::Thinking { thinking, .. }) => {
                thinking.push_str(&delta);
                Ok(AssistantMessageEvent::ThinkingDelta {
                    content_index,
                    delta,
                    partial: partial.clone(),
                })
            }
            _ => Err("Received thinking_delta for non-thinking content".into()),
        },
        ProxyAssistantMessageEvent::ThinkingEnd {
            content_index,
            content_signature,
        } => match partial.content.get_mut(content_index as usize) {
            Some(ContentBlock::Thinking {
                thinking_signature, ..
            }) => {
                *thinking_signature = content_signature;
                Ok(AssistantMessageEvent::ThinkingEnd {
                    content_index,
                    partial: partial.clone(),
                })
            }
            _ => Err("Received thinking_end for non-thinking content".into()),
        },
        ProxyAssistantMessageEvent::ToolcallStart {
            content_index,
            id,
            tool_name,
        } => {
            ensure_content_len(&mut partial.content, content_index as usize);
            partial.content[content_index as usize] = ContentBlock::ToolCall {
                id,
                name: tool_name,
                arguments: serde_json::json!({}),
                thought_signature: None,
            };
            tool_json.insert(content_index, String::new());
            Ok(AssistantMessageEvent::ToolcallStart {
                content_index,
                partial: partial.clone(),
            })
        }
        ProxyAssistantMessageEvent::ToolcallDelta {
            content_index,
            delta,
        } => match partial.content.get_mut(content_index as usize) {
            Some(ContentBlock::ToolCall { arguments, .. }) => {
                let accumulated = tool_json.entry(content_index).or_default();
                accumulated.push_str(&delta);
                *arguments = pi_ai::util::json_repair::parse_streaming_json(accumulated);
                Ok(AssistantMessageEvent::ToolcallDelta {
                    content_index,
                    delta,
                    partial: partial.clone(),
                })
            }
            _ => Err("Received toolcall_delta for non-toolCall content".into()),
        },
        ProxyAssistantMessageEvent::ToolcallEnd { content_index } => {
            match partial.content.get(content_index as usize) {
                Some(ContentBlock::ToolCall { .. }) => {
                    tool_json.remove(&content_index);
                    Ok(AssistantMessageEvent::ToolcallEnd {
                        content_index,
                        partial: partial.clone(),
                    })
                }
                _ => Err("Received toolcall_end for non-toolCall content".into()),
            }
        }
        ProxyAssistantMessageEvent::Done { reason, usage } => {
            partial.stop_reason = reason.clone();
            partial.usage = usage;
            Ok(AssistantMessageEvent::Done {
                reason,
                message: partial.clone(),
            })
        }
        ProxyAssistantMessageEvent::Error {
            reason,
            error_message,
            usage,
        } => {
            partial.stop_reason = reason.clone();
            partial.error_message = error_message;
            partial.usage = usage;
            Ok(AssistantMessageEvent::Error {
                reason,
                message: partial.clone(),
            })
        }
    }
}

fn ensure_content_len(content: &mut Vec<ContentBlock>, index: usize) {
    while content.len() <= index {
        content.push(ContentBlock::Text {
            text: String::new(),
            text_signature: None,
        });
    }
}
