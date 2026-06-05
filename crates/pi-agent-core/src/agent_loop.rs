use async_stream::stream;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use std::sync::{Arc, RwLock};

use crate::agent::AgentState;
use crate::convert::convert_to_context;
use crate::hooks::{AfterToolCallContext, BeforeToolCallContext};
use crate::queues::drain_queue;
use crate::types::{
    AgentEvent, AgentMessage, AgentStream, AgentToolResult, ThinkingLevel, ToolExecutionMode,
};
use pi_ai::types::{AssistantMessageEvent, ContentBlock, StopReason, ThinkingConfig};

struct PreparedToolCall {
    index: usize,
    tool_id: String,
    tool_name: String,
    tool_args: serde_json::Value,
    tool: Option<crate::types::AgentTool>,
    blocked: Option<AgentToolResult>,
}

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

            // Drain steering queue before building context
            {
                let mut s = state.write().unwrap();
                let mode = s.config.steering_mode;
                let steered = drain_queue(&mut s.steering_queue, mode);
                s.messages.extend(steered);
            }

            let (ctx, model, opts) = {
                let s = state.read().unwrap();
                let ctx = convert_to_context(
                    &s.config.system_prompt,
                    &s.messages,
                    &s.tools,
                    &s.config.resources,
                );
                let mut opts = s.config.stream_options.clone().unwrap_or_default();
                opts.cancel = Some(cancel.clone());
                // Apply thinking level
                if s.config.model.reasoning {
                    match s.config.thinking_level {
                        ThinkingLevel::Off => {
                            opts.thinking = None;
                        }
                        _ => {
                            let budget_tokens = match s.config.thinking_level {
                                ThinkingLevel::Minimal => Some(1024u32),
                                ThinkingLevel::Low => Some(2048u32),
                                ThinkingLevel::Medium => Some(4096u32),
                                ThinkingLevel::High => Some(8192u32),
                                ThinkingLevel::XHigh => Some(16384u32),
                                ThinkingLevel::Off => None,
                            };
                            opts.thinking = Some(ThinkingConfig {
                                enabled: true,
                                budget_tokens,
                                effort: Some(s.config.thinking_level.to_string()),
                            });
                        }
                    }
                } else {
                    opts.thinking = None;
                }
                (ctx, s.config.model.clone(), opts)
            };

            let mut llm_stream = pi_ai::stream_model(&model, ctx, Some(opts));
            let mut assistant_message: Option<pi_ai::types::AssistantMessage> = None;
            let mut stream_error: Option<String> = None;

            while let Some(event) = llm_stream.next().await {
                let is_terminal = matches!(
                    event,
                    AssistantMessageEvent::Done { .. } | AssistantMessageEvent::Error { .. }
                );
                if let AssistantMessageEvent::Done { message, .. } = &event {
                    assistant_message = Some(message.clone());
                }
                if let AssistantMessageEvent::Error { message, .. } = &event {
                    stream_error = Some(
                        message
                            .error_message
                            .clone()
                            .unwrap_or_else(|| "LLM error".into()),
                    );
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
                        error: stream_error
                            .unwrap_or_else(|| "LLM stream ended without Done event".into()),
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
                    // Check follow-up queue and steering queue before finishing
                    let has_more = {
                        let s = state.read().unwrap();
                        !s.follow_up_queue.is_empty() || !s.steering_queue.is_empty()
                    };
                    if has_more {
                        // Drain follow-ups before continuing
                        {
                            let mut s = state.write().unwrap();
                            let mode = s.config.follow_up_mode;
                            let follow_ups = drain_queue(&mut s.follow_up_queue, mode);
                            s.messages.extend(follow_ups);
                        }
                        continue;
                    }
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
                    let tool_calls: Vec<_> = assistant
                        .content
                        .iter()
                        .filter_map(|block| match block {
                            ContentBlock::ToolCall { id, name, arguments, .. } => {
                                Some((id.clone(), name.clone(), arguments.clone()))
                            }
                            _ => None,
                        })
                        .collect();

                    if tool_calls.is_empty() {
                        continue;
                    }

                    let global_mode = {
                        let s = state.read().unwrap();
                        s.config.tool_execution
                    };
                    let has_sequential_override = {
                        let s = state.read().unwrap();
                        tool_calls.iter().any(|(_, name, _)| {
                            s.tools
                                .iter()
                                .find(|t| t.name == *name)
                                .and_then(|t| t.execution_mode)
                                == Some(ToolExecutionMode::Sequential)
                        })
                    };
                    let use_sequential = global_mode == ToolExecutionMode::Sequential || has_sequential_override;

                    if use_sequential {
                        for (tool_id, tool_name, tool_args) in &tool_calls {
                            let tool = {
                                let s = state.read().unwrap();
                                s.tools.iter().find(|t| t.name == *tool_name).cloned()
                            };

                            yield AgentEvent::ToolCallStart {
                                tool_call_id: tool_id.clone(),
                                tool_name: tool_name.clone(),
                            };

                            //--- before hook ---
                            let before_hook = {
                                let s = state.read().unwrap();
                                s.config.hooks.before_tool_call.clone()
                            };
                            let mut blocked = false;
                            if let Some(hook) = &before_hook {
                                let messages = {
                                    let s = state.read().unwrap();
                                    s.messages.clone()
                                };
                                let ctx = BeforeToolCallContext {
                                    assistant_message: assistant.clone(),
                                    tool_call_id: tool_id.clone(),
                                    tool_name: tool_name.clone(),
                                    arguments: tool_args.clone(),
                                    messages,
                                };
                                match hook(ctx).await {
                                    Ok(Some(result)) if result.block => {
                                        let blocked_result = AgentToolResult::error(
                                            result.reason.unwrap_or_else(|| "blocked".into()),
                                        );
                                        yield AgentEvent::ToolCallEnd {
                                            tool_call_id: tool_id.clone(),
                                            tool_name: tool_name.clone(),
                                            result: blocked_result.clone(),
                                        };
                                        {
                                            let mut s = state.write().unwrap();
                                            s.messages.push(AgentMessage::ToolResult {
                                                message_id: tool_id.clone(),
                                                tool_call_id: tool_id.clone(),
                                                tool_name: tool_name.clone(),
                                                is_error: blocked_result.is_error,
                                                content: blocked_result.content.clone(),
                                            });
                                        }
                                        blocked = true;
                                    }
                                    Err(e) => {
                                        let err = AgentToolResult::error(e);
                                        yield AgentEvent::ToolCallEnd {
                                            tool_call_id: tool_id.clone(),
                                            tool_name: tool_name.clone(),
                                            result: err.clone(),
                                        };
                                        {
                                            let mut s = state.write().unwrap();
                                            s.messages.push(AgentMessage::ToolResult {
                                                message_id: tool_id.clone(),
                                                tool_call_id: tool_id.clone(),
                                                tool_name: tool_name.clone(),
                                                is_error: err.is_error,
                                                content: err.content.clone(),
                                            });
                                        }
                                        blocked = true;
                                    }
                                    _ => {}
                                }
                            }
                            if blocked {
                                continue;
                            }

                            //--- execute ---
                            let mut result = match &tool {
                                Some(t) => {
                                    match (t.execute)(tool_args.clone()).await {
                                        Ok(blocks) => AgentToolResult::ok(blocks),
                                        Err(e) => AgentToolResult::error(e),
                                    }
                                }
                                None => AgentToolResult::error(format!("unknown tool: {}", tool_name)),
                            };

                            //--- after hook ---
                            let after_hook = {
                                let s = state.read().unwrap();
                                s.config.hooks.after_tool_call.clone()
                            };
                            if let Some(hook) = &after_hook {
                                let messages = {
                                    let s = state.read().unwrap();
                                    s.messages.clone()
                                };
                                let ctx = AfterToolCallContext {
                                    assistant_message: assistant.clone(),
                                    tool_call_id: tool_id.clone(),
                                    tool_name: tool_name.clone(),
                                    arguments: tool_args.clone(),
                                    result: result.clone(),
                                    messages,
                                };
                                match hook(ctx).await {
                                    Ok(Some(after)) => {
                                        if let Some(content) = after.content {
                                            result.content = content;
                                        }
                                        if let Some(is_error) = after.is_error {
                                            result.is_error = is_error;
                                        }
                                        if let Some(terminate) = after.terminate {
                                            result.terminate = terminate;
                                        }
                                    }
                                    Err(e) => {
                                        result = AgentToolResult::error(e);
                                    }
                                    _ => {}
                                }
                            }

                            yield AgentEvent::ToolCallEnd {
                                tool_call_id: tool_id.clone(),
                                tool_name: tool_name.clone(),
                                result: result.clone(),
                            };

                            {
                                let mut s = state.write().unwrap();
                                s.messages.push(AgentMessage::ToolResult {
                                    message_id: tool_id.clone(),
                                    tool_call_id: tool_id.clone(),
                                    tool_name: tool_name.clone(),
                                    is_error: result.is_error,
                                    content: result.content.clone(),
                                });
                            }
                        }
                    } else {
                        //--- Parallel path ---
                        // 1. Emit ToolCallStart for all calls
                        for (tool_id, tool_name, _) in &tool_calls {
                            yield AgentEvent::ToolCallStart {
                                tool_call_id: tool_id.clone(),
                                tool_name: tool_name.clone(),
                            };
                        }

                        // 2. Prepare all calls (before hooks run sequentially)
                        let mut prepared = Vec::new();
                        for (index, (tool_id, tool_name, tool_args)) in tool_calls.iter().enumerate() {
                            let tool = {
                                let s = state.read().unwrap();
                                s.tools.iter().find(|t| t.name == *tool_name).cloned()
                            };

                            let before_hook = {
                                let s = state.read().unwrap();
                                s.config.hooks.before_tool_call.clone()
                            };
                            let mut blocked = None;
                            if let Some(hook) = &before_hook {
                                let messages = {
                                    let s = state.read().unwrap();
                                    s.messages.clone()
                                };
                                let ctx = BeforeToolCallContext {
                                    assistant_message: assistant.clone(),
                                    tool_call_id: tool_id.clone(),
                                    tool_name: tool_name.clone(),
                                    arguments: tool_args.clone(),
                                    messages,
                                };
                                match hook(ctx).await {
                                    Ok(Some(result)) if result.block => {
                                        blocked = Some(AgentToolResult::error(
                                            result.reason.unwrap_or_else(|| "blocked".into()),
                                        ));
                                    }
                                    Err(e) => {
                                        blocked = Some(AgentToolResult::error(e));
                                    }
                                    _ => {}
                                }
                            }
                            prepared.push(PreparedToolCall {
                                index,
                                tool_id: tool_id.clone(),
                                tool_name: tool_name.clone(),
                                tool_args: tool_args.clone(),
                                tool,
                                blocked,
                            });
                        }

                        // 3. Execute in parallel
                        let after_hook = {
                            let s = state.read().unwrap();
                            s.config.hooks.after_tool_call.clone()
                        };
                        let messages_snapshot = {
                            let s = state.read().unwrap();
                            s.messages.clone()
                        };

                        let assistant_for_parallel = assistant.clone();
                        let mut futures: FuturesUnordered<_> = prepared
                            .into_iter()
                            .map(|p| {
                                let after_hook = after_hook.clone();
                                let messages = messages_snapshot.clone();
                                let asst = assistant_for_parallel.clone();
                                async move {
                                    let blocked_val = p.blocked.clone();
                                    let is_blocked = blocked_val.is_some();
                                    let tool_args = p.tool_args.clone();
                                    let mut result = match blocked_val {
                                        Some(r) => r,
                                        None => match &p.tool {
                                            Some(t) => {
                                                match (t.execute)(tool_args).await {
                                                    Ok(blocks) => AgentToolResult::ok(blocks),
                                                    Err(e) => AgentToolResult::error(e),
                                                }
                                            }
                                            None => AgentToolResult::error(format!("unknown tool: {}", p.tool_name)),
                                        },
                                    };

                                    if !is_blocked {
                                        if let Some(hook) = &after_hook {
                                            let ctx = AfterToolCallContext {
                                                assistant_message: asst.clone(),
                                                tool_call_id: p.tool_id.clone(),
                                                tool_name: p.tool_name.clone(),
                                                arguments: p.tool_args.clone(),
                                                result: result.clone(),
                                                messages: messages.clone(),
                                            };
                                            match hook(ctx).await {
                                                Ok(Some(after)) => {
                                                    if let Some(content) = after.content {
                                                        result.content = content;
                                                    }
                                                    if let Some(is_error) = after.is_error {
                                                        result.is_error = is_error;
                                                    }
                                                    if let Some(terminate) = after.terminate {
                                                        result.terminate = terminate;
                                                    }
                                                }
                                                Err(e) => {
                                                    result = AgentToolResult::error(e);
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    (p.index, result, p.tool_id, p.tool_name)
                                }
                            })
                            .collect();

                        let mut sorted_results: Vec<(usize, AgentToolResult, String, String)> = Vec::new();
                        while let Some(r) = futures.next().await {
                            sorted_results.push(r);
                        }
                        sorted_results.sort_by_key(|(idx, _, _, _)| *idx);

                        let results: Vec<(AgentToolResult, String, String)> = sorted_results
                            .into_iter()
                            .map(|(_, result, id, name)| (result, id, name))
                            .collect();

                        for (result, tool_id, tool_name) in &results {
                            yield AgentEvent::ToolCallEnd {
                                tool_call_id: tool_id.clone(),
                                tool_name: tool_name.clone(),
                                result: result.clone(),
                            };
                        }

                        {
                            let mut s = state.write().unwrap();
                            for (result, tool_id, tool_name) in &results {
                                s.messages.push(AgentMessage::ToolResult {
                                    message_id: tool_id.clone(),
                                    tool_call_id: tool_id.clone(),
                                    tool_name: tool_name.clone(),
                                    is_error: result.is_error,
                                    content: result.content.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }
    })
}
