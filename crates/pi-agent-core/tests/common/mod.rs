use std::sync::Mutex;
use async_stream::stream;
use pi_ai::registry::ApiProvider;
use pi_ai::types::{AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Model, StopReason, StreamOptions};
use pi_ai::stream::EventStream;

/// A scripted LLM response for one turn.
pub struct ScriptedTurn {
    pub events: Vec<AssistantMessageEvent>,
    pub stop_reason: StopReason,
    pub response_id: String,
    pub model_name: String,
}

/// Test ApiProvider that replays scripted turns from a queue.
pub struct TestProvider {
    pub turns: Mutex<Vec<ScriptedTurn>>,
}

impl TestProvider {
    pub fn new(turns: Vec<ScriptedTurn>) -> Self {
        Self { turns: Mutex::new(turns) }
    }
}

impl ApiProvider for TestProvider {
    fn stream(&self, _model: &Model, _ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        let turn = {
            let mut turns = self.turns.lock().unwrap();
            if turns.is_empty() {
                return Box::pin(stream! {
                    let mut msg = AssistantMessage::empty("test", "test-model");
                    msg.error_message = Some("no more scripted turns".into());
                    msg.stop_reason = StopReason::Error;
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        message: msg,
                    };
                });
            }
            turns.remove(0)
        };

        let content = turn.events.last().map(|e| {
            match e {
                AssistantMessageEvent::Start { partial, .. } => partial.content.clone(),
                AssistantMessageEvent::TextStart { partial, .. } => partial.content.clone(),
                AssistantMessageEvent::TextDelta { partial, .. } => partial.content.clone(),
                AssistantMessageEvent::TextEnd { partial, .. } => partial.content.clone(),
                AssistantMessageEvent::ThinkingStart { partial, .. } => partial.content.clone(),
                AssistantMessageEvent::ThinkingDelta { partial, .. } => partial.content.clone(),
                AssistantMessageEvent::ThinkingEnd { partial, .. } => partial.content.clone(),
                AssistantMessageEvent::ToolcallStart { partial, .. } => partial.content.clone(),
                AssistantMessageEvent::ToolcallDelta { partial, .. } => partial.content.clone(),
                AssistantMessageEvent::ToolcallEnd { partial, .. } => partial.content.clone(),
                AssistantMessageEvent::Done { message, .. } => message.content.clone(),
                AssistantMessageEvent::Error { .. } => vec![],
            }
        }).unwrap_or_default();

        Box::pin(stream! {
            for event in &turn.events {
                yield event.clone();
            }

            let msg = AssistantMessage {
                content,
                api: "test".into(),
                provider: Some("test".into()),
                model: turn.model_name.clone(),
                response_model: None,
                response_id: Some(turn.response_id.clone()),
                usage: Default::default(),
                stop_reason: turn.stop_reason.clone(),
                error_message: None,
                timestamp: 0,
            };
            yield AssistantMessageEvent::Done {
                reason: turn.stop_reason,
                message: msg,
            };
        })
    }
}

/// Helper: create a simple text-response turn.
pub fn text_turn(text: &str) -> ScriptedTurn {
    let text_block = ContentBlock::Text { text: text.into(), text_signature: None };
    let mut msg = AssistantMessage::empty("test", "test-model");
    msg.content.push(text_block.clone());
    let partial = msg.clone();

    ScriptedTurn {
        events: vec![
            AssistantMessageEvent::Start { content_index: None, partial: partial.clone() },
            AssistantMessageEvent::TextStart {
                content_index: 0,
                partial: {
                    let mut p = partial.clone();
                    p.content = vec![text_block.clone()];
                    p
                },
            },
            AssistantMessageEvent::TextDelta { content_index: 0, delta: text.into(), partial: partial.clone() },
            AssistantMessageEvent::TextEnd { content_index: 0, partial: partial.clone() },
        ],
        stop_reason: StopReason::Stop,
        response_id: "resp_1".into(),
        model_name: "test-model".into(),
    }
}

/// Helper: create a tool-use turn.
pub fn tool_use_turn(tool_id: &str, tool_name: &str, arguments: serde_json::Value) -> ScriptedTurn {
    let tool_block = ContentBlock::ToolCall {
        id: tool_id.into(),
        name: tool_name.into(),
        arguments: arguments.clone(),
        thought_signature: None,
    };
    let mut msg = AssistantMessage::empty("test", "test-model");
    msg.content.push(tool_block.clone());
    let partial = msg.clone();

    let json_str = arguments.to_string();

    ScriptedTurn {
        events: vec![
            AssistantMessageEvent::Start { content_index: None, partial: partial.clone() },
            AssistantMessageEvent::ToolcallStart {
                content_index: 0,
                partial: {
                    let mut p = partial.clone();
                    p.content = vec![ContentBlock::ToolCall {
                        id: tool_id.into(),
                        name: tool_name.into(),
                        arguments: serde_json::json!({}),
                        thought_signature: None,
                    }];
                    p
                },
            },
            AssistantMessageEvent::ToolcallDelta {
                content_index: 0,
                delta: json_str,
                partial: partial.clone(),
            },
            AssistantMessageEvent::ToolcallEnd { content_index: 0, partial: partial.clone() },
        ],
        stop_reason: StopReason::ToolUse,
        response_id: "resp_tool".into(),
        model_name: "test-model".into(),
    }
}
