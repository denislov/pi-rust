use std::sync::Mutex;
use async_stream::stream;
use crate::registry::ApiProvider;
use crate::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Model,
    StopReason, StreamOptions, Usage,
};
use crate::stream::EventStream;

pub struct FauxProvider {
    pub responses: Mutex<Vec<FauxResponse>>,
}

#[derive(Clone)]
pub struct FauxResponse {
    pub text_deltas: Vec<String>,
    pub thinking_deltas: Vec<String>,
    pub tool_calls: Vec<FauxToolCall>,
}

#[derive(Clone)]
pub struct FauxToolCall {
    pub id: String,
    pub name: String,
    pub deltas: Vec<String>,
    pub final_arguments: serde_json::Value,
}

impl FauxProvider {
    pub fn new(responses: Vec<FauxResponse>) -> Self {
        Self { responses: Mutex::new(responses) }
    }

    pub fn simple_text(text: &str) -> Self {
        Self::new(vec![FauxResponse {
            text_deltas: vec![text.to_string()],
            thinking_deltas: vec![],
            tool_calls: vec![],
        }])
    }
}

impl ApiProvider for FauxProvider {
    fn stream(&self, model: &Model, _ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        let responses = self.responses.lock().unwrap().clone();
        let model_id = model.id.clone();
        Box::pin(stream! {
            let mut partial = AssistantMessage::empty("faux", &model_id);
            partial.provider = Some("faux".into());
            partial.timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            yield AssistantMessageEvent::Start { partial: partial.clone() };

            for resp in &responses {
                if !resp.text_deltas.is_empty() {
                    let mut p = partial.clone();
                    p.content.push(ContentBlock::Text {
                        text: resp.text_deltas.join(""),
                        text_signature: None,
                    });
                    yield AssistantMessageEvent::TextStart { partial: p };
                    for delta in &resp.text_deltas {
                        if let Some(ContentBlock::Text { text, .. }) = partial.content.last_mut() {
                            text.push_str(delta);
                        }
                        yield AssistantMessageEvent::TextDelta {
                            delta: delta.clone(),
                            partial: partial.clone(),
                        };
                    }
                    yield AssistantMessageEvent::TextEnd { partial: partial.clone() };
                }

                if !resp.thinking_deltas.is_empty() {
                    let mut p = partial.clone();
                    p.content.push(ContentBlock::Thinking {
                        thinking: resp.thinking_deltas.join(""),
                        thinking_signature: None,
                        redacted: None,
                    });
                    yield AssistantMessageEvent::ThinkingStart { partial: p };
                    for delta in &resp.thinking_deltas {
                        yield AssistantMessageEvent::ThinkingDelta {
                            delta: delta.clone(),
                            partial: partial.clone(),
                        };
                    }
                    yield AssistantMessageEvent::ThinkingEnd { partial: partial.clone() };
                }

                for tc in &resp.tool_calls {
                    let mut p = partial.clone();
                    p.content.push(ContentBlock::ToolCall {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        arguments: tc.final_arguments.clone(),
                        thought_signature: None,
                    });
                    yield AssistantMessageEvent::ToolcallStart { partial: p };
                    let mut accumulated = String::new();
                    for delta in &tc.deltas {
                        accumulated.push_str(delta);
                        if let Some(ContentBlock::ToolCall { arguments, .. }) = partial.content.last_mut() {
                            *arguments = serde_json::json!(accumulated);
                        }
                        yield AssistantMessageEvent::ToolcallDelta {
                            delta: delta.to_string(),
                            partial: partial.clone(),
                        };
                    }
                    yield AssistantMessageEvent::ToolcallEnd { partial: partial.clone() };
                }
            }

            partial.usage = Usage {
                input: 10, output: 20, total_tokens: 30,
                ..Default::default()
            };
            partial.stop_reason = StopReason::Stop;

            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message: partial,
            };
        })
    }
}
