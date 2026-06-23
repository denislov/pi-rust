use async_stream::stream;
use futures::channel::mpsc;
use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt};
use std::sync::{Arc, RwLock};

use crate::agent::AgentState;
use crate::compaction::estimate::estimate_tokens;
use crate::compaction::prepare::prepare_compaction;
use crate::compaction::summarize::summarize;
use crate::hooks::{
    AfterToolCallContext, BeforeProviderRequestContext, BeforeToolCallContext,
    PrepareNextTurnContext, ShouldStopAfterTurnContext,
};
use crate::loop_runtime::context::prepare_provider_request;
use crate::loop_runtime::tools::{
    ToolCallExecution, append_tool_result_messages, extract_tool_calls, should_use_sequential_tools,
};
use crate::queues::drain_queue;
use crate::types::{
    AgentEvent, AgentMessage, AgentStream, AgentToolOutput, AgentToolResult,
    ProviderRequestSnapshot, ToolUpdateCallback,
};
use pi_ai::types::{AssistantMessage, AssistantMessageEvent, StopReason};

struct PreparedToolCall {
    index: usize,
    tool_id: String,
    tool_name: String,
    tool_args: serde_json::Value,
    tool: Option<crate::types::AgentTool>,
    blocked: Option<AgentToolResult>,
}

fn message_id(message: &AgentMessage) -> &str {
    match message {
        AgentMessage::UserText { message_id, .. }
        | AgentMessage::Assistant { message_id, .. }
        | AgentMessage::ToolResult { message_id, .. }
        | AgentMessage::SystemPrompt { message_id, .. }
        | AgentMessage::CompactionSummary { message_id, .. }
        | AgentMessage::BashExecution { message_id, .. }
        | AgentMessage::Custom { message_id, .. }
        | AgentMessage::BranchSummary { message_id, .. } => message_id,
    }
}

async fn compact_before_provider_request(
    state: &Arc<RwLock<AgentState>>,
) -> Result<Option<(String, String, u32)>, String> {
    let (config, messages, model, cancel) = {
        let s = state.read().unwrap();
        (
            s.config.compaction.clone(),
            s.messages.clone(),
            s.config.model.clone(),
            s.cancel_token.clone(),
        )
    };

    let Some(config) = config else {
        return Ok(None);
    };
    if !config.settings.enabled {
        return Ok(None);
    }

    let tokens_before = estimate_tokens(&messages);
    let (to_summarize, keep) = prepare_compaction(&messages, &config.settings);
    if to_summarize.is_empty() {
        return Ok(None);
    }

    let summary = summarize(
        &model,
        &to_summarize,
        config.custom_instructions.as_deref(),
        Some(cancel),
    )
    .await
    .map_err(|err| err.to_string())?;

    let first_kept_message_id = keep.first().map(message_id).unwrap_or("none").to_string();

    {
        let mut s = state.write().unwrap();
        let mut compacted = Vec::with_capacity(1 + keep.len());
        compacted.push(AgentMessage::CompactionSummary {
            message_id: format!("compaction_{}", tokens_before),
            summary: summary.clone(),
            tokens_before,
        });
        compacted.extend(keep);
        s.messages = compacted;
    }

    Ok(Some((summary, first_kept_message_id, tokens_before)))
}

async fn should_stop_after_turn(
    state: &Arc<RwLock<AgentState>>,
    assistant: &AssistantMessage,
) -> Result<bool, String> {
    let hook = {
        let s = state.read().unwrap();
        s.config.hooks.should_stop_after_turn.clone()
    };
    let Some(hook) = hook else {
        return Ok(false);
    };
    let messages = {
        let s = state.read().unwrap();
        s.messages.clone()
    };
    hook(ShouldStopAfterTurnContext {
        messages,
        assistant_message: assistant.clone(),
    })
    .await
}

async fn prepare_next_turn(state: &Arc<RwLock<AgentState>>, turn: u32) -> Result<(), String> {
    let hook = {
        let s = state.read().unwrap();
        s.config.hooks.prepare_next_turn.clone()
    };
    let Some(hook) = hook else {
        return Ok(());
    };
    let messages = {
        let s = state.read().unwrap();
        s.messages.clone()
    };
    let Some(update) = hook(PrepareNextTurnContext { messages, turn }).await? else {
        return Ok(());
    };

    let mut s = state.write().unwrap();
    if let Some(messages) = update.messages {
        s.messages = messages;
    }
    if let Some(model) = update.model {
        s.config.model = model;
    }
    if let Some(thinking_level) = update.thinking_level {
        s.config.thinking_level = thinking_level;
    }
    if let Some(stream_options) = update.stream_options {
        s.config.stream_options = Some(stream_options);
    }
    Ok(())
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
                if let Some(max_turns) = max_turns
                    && turn > max_turns
                {
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

            match compact_before_provider_request(&state).await {
                Ok(Some((summary, first_kept_message_id, tokens_before))) => {
                    yield AgentEvent::SessionCompacted {
                        summary,
                        first_kept_message_id,
                        tokens_before,
                        details: None,
                    };
                }
                Ok(None) => {}
                Err(error) => {
                    yield AgentEvent::AgentError { error };
                    return;
                }
            }

            let transform_hook = {
                let s = state.read().unwrap();
                s.config.hooks.transform_context.clone()
            };
            let transformed_messages = if let Some(hook) = transform_hook {
                let original = {
                    let s = state.read().unwrap();
                    s.messages.clone()
                };
                match hook(original).await {
                    Ok(messages) => Some(messages),
                    Err(error) => {
                        yield AgentEvent::AgentError { error };
                        return;
                    }
                }
            } else {
                None
            };

            let convert_hook = {
                let s = state.read().unwrap();
                s.config.hooks.convert_to_llm.clone()
            };
            let llm_messages_override = if let Some(hook) = convert_hook {
                let (msgs, resources) = {
                    let s = state.read().unwrap();
                    let msgs = transformed_messages
                        .clone()
                        .unwrap_or_else(|| s.messages.clone());
                    (msgs, s.config.resources.clone())
                };
                match hook(msgs, resources).await {
                    Ok(llm_messages) => Some(llm_messages),
                    Err(error) => {
                        yield AgentEvent::AgentError { error };
                        return;
                    }
                }
            } else {
                None
            };

            let mut request = match prepare_provider_request(
                &state,
                cancel.clone(),
                transformed_messages,
                llm_messages_override,
            ) {
                Ok(request) => request,
                Err(error) => {
                    yield AgentEvent::AgentError { error };
                    return;
                }
            };

            let provider_hook = {
                let s = state.read().unwrap();
                s.config.hooks.before_provider_request.clone()
            };
            if let Some(hook) = provider_hook {
                let snapshot = ProviderRequestSnapshot {
                    model: request.model.clone(),
                    context: request.context.clone(),
                    stream_options: request.stream_options.clone(),
                };
                match hook(BeforeProviderRequestContext::from(snapshot)).await {
                    Ok(Some(update)) => {
                        if let Some(updated_context) = update.context {
                            request.context = updated_context;
                        }
                        if let Some(updated_options) = update.stream_options {
                            request.stream_options = updated_options;
                        }
                        request.stream_options.cancel = Some(cancel.clone());
                    }
                    Ok(None) => {}
                    Err(error) => {
                        yield AgentEvent::AgentError { error };
                        return;
                    }
                }
            }

            yield AgentEvent::BeforeProviderRequest {
                request: ProviderRequestSnapshot {
                    model: request.model.clone(),
                    context: request.context.clone(),
                    stream_options: request.stream_options.clone(),
                },
            };

            let mut llm_stream =
                pi_ai::stream_model(&request.model, request.context, Some(request.stream_options));
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
                    match should_stop_after_turn(&state, &assistant).await {
                        Ok(true) => {
                            yield AgentEvent::AgentDone { message: assistant };
                            return;
                        }
                        Ok(false) => {}
                        Err(error) => {
                            yield AgentEvent::AgentError { error };
                            return;
                        }
                    }

                    if let Err(error) = prepare_next_turn(&state, turn).await {
                        yield AgentEvent::AgentError { error };
                        return;
                    }

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
                    let tool_calls = extract_tool_calls(&assistant);

                    if tool_calls.is_empty() {
                        continue;
                    }

                    let global_mode = {
                        let s = state.read().unwrap();
                        s.config.tool_execution
                    };
                    let use_sequential = {
                        let s = state.read().unwrap();
                        should_use_sequential_tools(global_mode, &tool_calls, &s.tools)
                    };
                    let mut batch_results: Vec<AgentToolResult> = Vec::new();

                    if use_sequential {
                        for call in &tool_calls {
                            let tool_id = &call.tool_call_id;
                            let tool_name = &call.tool_name;
                            let tool_args = &call.arguments;
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
                                            append_tool_result_messages(
                                                &mut s.messages,
                                                &[ToolCallExecution {
                                                    index: call.index,
                                                    tool_call_id: tool_id.clone(),
                                                    tool_name: tool_name.clone(),
                                                    result: blocked_result.clone(),
                                                }],
                                            );
                                        }
                                        batch_results.push(blocked_result);
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
                                            append_tool_result_messages(
                                                &mut s.messages,
                                                &[ToolCallExecution {
                                                    index: call.index,
                                                    tool_call_id: tool_id.clone(),
                                                    tool_name: tool_name.clone(),
                                                    result: err.clone(),
                                                }],
                                            );
                                        }
                                        batch_results.push(err);
                                        blocked = true;
                                    }
                                    _ => {}
                                }
                            }
                            if blocked {
                                continue;
                            }

                            //--- execute ---
                            let (update_tx, mut update_rx) = mpsc::unbounded::<AgentToolOutput>();
                            let update_callback: ToolUpdateCallback = Arc::new(move |update| {
                                let _ = update_tx.unbounded_send(update);
                            });
                            let mut execute_future = Box::pin({
                                let tool = tool.clone();
                                let tool_args = tool_args.clone();
                                let tool_name = tool_name.clone();
                                async move {
                                    match tool {
                                        Some(t) => match (t.execute)(
                                            tool_args,
                                            Some(update_callback),
                                        )
                                        .await
                                        {
                                            Ok(output) => AgentToolResult::from_output(output),
                                            Err(e) => AgentToolResult::error(e),
                                        },
                                        None => AgentToolResult::error(format!(
                                            "unknown tool: {}",
                                            tool_name
                                        )),
                                    }
                                }
                            })
                            .fuse();
                            let mut update_open = true;
                            let mut result = loop {
                                if !update_open {
                                    break execute_future.await;
                                }
                                futures::select! {
                                    maybe_update = update_rx.next().fuse() => {
                                        if let Some(update) = maybe_update {
                                            yield AgentEvent::ToolCallUpdate {
                                                tool_call_id: tool_id.clone(),
                                                tool_name: tool_name.clone(),
                                                update,
                                            };
                                        } else {
                                            update_open = false;
                                        }
                                    }
                                    completed = &mut execute_future => {
                                        break completed;
                                    }
                                }
                            };
                            while let Some(Some(update)) = update_rx.next().now_or_never() {
                                yield AgentEvent::ToolCallUpdate {
                                    tool_call_id: tool_id.clone(),
                                    tool_name: tool_name.clone(),
                                    update,
                                };
                            }

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
                                append_tool_result_messages(
                                    &mut s.messages,
                                    &[ToolCallExecution {
                                        index: call.index,
                                        tool_call_id: tool_id.clone(),
                                        tool_name: tool_name.clone(),
                                        result: result.clone(),
                                    }],
                                );
                            }
                            batch_results.push(result);
                        }
                    } else {
                        //--- Parallel path ---
                        // 1. Emit ToolCallStart for all calls
                        for call in &tool_calls {
                            yield AgentEvent::ToolCallStart {
                                tool_call_id: call.tool_call_id.clone(),
                                tool_name: call.tool_name.clone(),
                            };
                        }

                        // 2. Prepare all calls (before hooks run sequentially)
                        let mut prepared = Vec::new();
                        for call in &tool_calls {
                            let tool_id = &call.tool_call_id;
                            let tool_name = &call.tool_name;
                            let tool_args = &call.arguments;
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
                                index: call.index,
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
                                                match (t.execute)(tool_args, None).await {
                                                    Ok(output) => AgentToolResult::from_output(output),
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
                                    ToolCallExecution {
                                        index: p.index,
                                        tool_call_id: p.tool_id,
                                        tool_name: p.tool_name,
                                        result,
                                    }
                                }
                            })
                            .collect();

                        let mut sorted_results: Vec<ToolCallExecution> = Vec::new();
                        while let Some(execution) = futures.next().await {
                            yield AgentEvent::ToolCallEnd {
                                tool_call_id: execution.tool_call_id.clone(),
                                tool_name: execution.tool_name.clone(),
                                result: execution.result.clone(),
                            };
                            sorted_results.push(execution);
                        }
                        sorted_results.sort_by_key(|execution| execution.index);

                        {
                            let mut s = state.write().unwrap();
                            append_tool_result_messages(&mut s.messages, &sorted_results);
                        }
                        batch_results.extend(
                            sorted_results
                                .into_iter()
                                .map(|execution| execution.result),
                        );
                    }

                    match should_stop_after_turn(&state, &assistant).await {
                        Ok(true) => {
                            yield AgentEvent::AgentDone { message: assistant };
                            return;
                        }
                        Ok(false) => {}
                        Err(error) => {
                            yield AgentEvent::AgentError { error };
                            return;
                        }
                    }

                    if !batch_results.is_empty()
                        && batch_results.iter().all(|result| result.terminate)
                    {
                        yield AgentEvent::AgentDone { message: assistant };
                        return;
                    }

                    if let Err(error) = prepare_next_turn(&state, turn).await {
                        yield AgentEvent::AgentError { error };
                        return;
                    }
                }
            }
        }
    })
}
