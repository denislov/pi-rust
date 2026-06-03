use async_stream::stream;
use futures::StreamExt;
use std::sync::{Arc, RwLock};

use crate::agent::AgentState;
use crate::convert::convert_to_context;
use crate::types::{AgentEvent, AgentMessage, AgentStream};
use pi_ai::types::{AssistantMessageEvent, ContentBlock, StopReason};

pub fn run_loop(state: Arc<RwLock<AgentState>>) -> AgentStream {
    Box::pin(stream! {
        let cancel = {
            let s = state.read().unwrap();
            s.cancel_token.clone()
        };

        let mut turn: u32 = 0;

        loop {
            if cancel.is_cancelled() {
                yield AgentEvent::AgentError { error: "aborted".into() };
                return;
            }

            turn += 1;

            {
                let max_turns = state.read().unwrap().config.max_turns;
                if turn > max_turns {
                    yield AgentEvent::AgentError {
                        error: format!("max turns ({}) exceeded", max_turns),
                    };
                    return;
                }
            }

            yield AgentEvent::TurnStart { turn };

            let (ctx, model, opts) = {
                let s = state.read().unwrap();
                let ctx = convert_to_context(
                    &s.config.system_prompt,
                    &s.messages,
                    &s.tools,
                );
                let mut opts = s.config.stream_options.clone().unwrap_or_default();
                opts.cancel = Some(cancel.clone());
                (ctx, s.config.model.clone(), opts)
            };

            let mut llm_stream = pi_ai::stream_model(&model, ctx, Some(opts));
            let mut assistant_message: Option<pi_ai::types::AssistantMessage> = None;

            while let Some(event) = llm_stream.next().await {
                let is_terminal = matches!(
                    event,
                    AssistantMessageEvent::Done { .. } | AssistantMessageEvent::Error { .. }
                );
                if let AssistantMessageEvent::Done { message, .. } = &event {
                    assistant_message = Some(message.clone());
                }
                yield AgentEvent::LlmEvent(event);
                if is_terminal {
                    break;
                }
            }

            let assistant = match assistant_message {
                Some(m) => m,
                None => {
                    yield AgentEvent::AgentError {
                        error: "LLM stream ended without Done event".into(),
                    };
                    return;
                }
            };

            {
                let mut s = state.write().unwrap();
                s.messages.push(AgentMessage::Assistant {
                    message_id: assistant.response_id.clone().unwrap_or_default(),
                    message: assistant.clone(),
                });
            }

            match &assistant.stop_reason {
                StopReason::Stop | StopReason::Length => {
                    yield AgentEvent::AgentDone { message: assistant };
                    return;
                }
                StopReason::Error => {
                    yield AgentEvent::AgentError {
                        error: assistant
                            .error_message
                            .clone()
                            .unwrap_or_else(|| "LLM error".into()),
                    };
                    return;
                }
                StopReason::Aborted => {
                    yield AgentEvent::AgentError { error: "aborted".into() };
                    return;
                }
                StopReason::ToolUse => {
                    for block in &assistant.content {
                        let (tool_id, tool_name, tool_args) = match block {
                            ContentBlock::ToolCall { id, name, arguments, .. } => {
                                (id.clone(), name.clone(), arguments.clone())
                            }
                            _ => continue,
                        };

                        let tool = {
                            let s = state.read().unwrap();
                            s.tools.iter().find(|t| t.name == tool_name).cloned()
                        };

                        yield AgentEvent::ToolCallStart {
                            tool_call_id: tool_id.clone(),
                            tool_name: tool_name.clone(),
                        };

                        let result = match &tool {
                            Some(t) => (t.execute)(tool_args).await,
                            None => Err(format!("unknown tool: {}", tool_name)),
                        };

                        yield AgentEvent::ToolCallEnd {
                            tool_call_id: tool_id.clone(),
                            result: result.clone(),
                        };

                        let (content, is_error) = match &result {
                            Ok(blocks) => (blocks.clone(), false),
                            Err(e) => (vec![ContentBlock::Text {
                                text: e.clone(),
                                text_signature: None,
                            }], true),
                        };
                        {
                            let mut s = state.write().unwrap();
                            s.messages.push(AgentMessage::ToolResult {
                                message_id: tool_id.clone(),
                                tool_call_id: tool_id.clone(),
                                tool_name: tool_name.clone(),
                                is_error,
                                content,
                            });
                        }
                    }
                }
            }
        }
    })
}
