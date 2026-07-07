use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::ai_runtime::stream_model_with_global_runtime;
use crate::compaction::estimate::estimate_context_tokens;
use crate::compaction::prepare::{prepare_compaction, should_compact};
use crate::compaction::summarize::summarize_with_provider_streamer;
use crate::convert::{assemble_context, convert_to_context, default_convert_to_llm};
use crate::flow::{Action, FlowNode};
use crate::hooks::{
    AfterToolCallContext, AfterToolCallHook, BeforeProviderRequestContext, BeforeToolCallContext,
    PrepareNextTurnContext, ShouldStopAfterTurnContext,
};
use crate::loop_runtime::context::stream_options_for_turn;
use crate::loop_runtime::tools::{
    ToolCallExecution, ToolCallRequest, append_tool_result_messages, extract_tool_calls,
    should_use_sequential_tools,
};
use crate::queues::drain_queue;
use crate::types::{
    AgentEvent, AgentMessage, AgentTool, AgentToolOutput, AgentToolResult, ProviderRequestSnapshot,
    ToolUpdateCallback,
};
use futures::channel::mpsc;
use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt};
use pi_ai::types::{AssistantMessage, AssistantMessageEvent, StopReason, Usage};

use super::context::{AgentTurnContext, PendingToolCall, RuntimeCompactionState};

const ACTION_DEFAULT: &str = "default";
const ACTION_CONTINUE: &str = "continue";
const ACTION_CONTINUE_PROVIDER: &str = "continue_provider";
const ACTION_TOOLS: &str = "tools";
const ACTION_DONE: &str = "done";
const ACTION_ERROR: &str = "error";
const ACTION_ABORTED: &str = "aborted";

pub struct StartTurnNode;

impl FlowNode<AgentTurnContext> for StartTurnNode {
    fn name(&self) -> &str {
        "start_turn"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move { start_turn(ctx) })
    }
}

pub struct DrainQueuedInputNode;

impl FlowNode<AgentTurnContext> for DrainQueuedInputNode {
    fn name(&self) -> &str {
        "drain_queued_input"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            drain_queued_input(ctx);
            default_action()
        })
    }
}

pub struct PrepareContextNode;

impl FlowNode<AgentTurnContext> for PrepareContextNode {
    fn name(&self) -> &str {
        "prepare_context"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move { prepare_provider_request(ctx).await })
    }
}

pub struct PrepareProviderRequestNode;

impl FlowNode<AgentTurnContext> for PrepareProviderRequestNode {
    fn name(&self) -> &str {
        "prepare_provider_request"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move { prepare_provider_request(ctx).await })
    }
}

pub struct ApplyBeforeProviderRequestHookNode;

impl FlowNode<AgentTurnContext> for ApplyBeforeProviderRequestHookNode {
    fn name(&self) -> &str {
        "apply_before_provider_request_hook"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move { apply_before_provider_request_hook(ctx).await })
    }
}

pub struct MaybeCompactRuntimeContextNode;

impl FlowNode<AgentTurnContext> for MaybeCompactRuntimeContextNode {
    fn name(&self) -> &str {
        "maybe_compact_runtime_context"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            maybe_compact_runtime_context(ctx).await?;
            default_action()
        })
    }
}

pub struct ProviderStreamNode;

impl FlowNode<AgentTurnContext> for ProviderStreamNode {
    fn name(&self) -> &str {
        "provider_stream"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move { stream_provider(ctx).await })
    }
}

pub struct DecideStopOrToolsNode;

impl FlowNode<AgentTurnContext> for DecideStopOrToolsNode {
    fn name(&self) -> &str {
        "decide_stop_or_tools"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move { decide_stop_or_tools(ctx) })
    }
}

pub struct DecideAfterAssistantNode;

impl FlowNode<AgentTurnContext> for DecideAfterAssistantNode {
    fn name(&self) -> &str {
        "decide_after_assistant"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move { decide_after_assistant(ctx) })
    }
}

pub struct MaybePrepareNextTurnNode;

impl FlowNode<AgentTurnContext> for MaybePrepareNextTurnNode {
    fn name(&self) -> &str {
        "maybe_prepare_next_turn"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move { maybe_prepare_next_turn(ctx).await })
    }
}

pub struct ExecuteToolsNode;

impl FlowNode<AgentTurnContext> for ExecuteToolsNode {
    fn name(&self) -> &str {
        "execute_tools"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move { execute_tools(ctx).await })
    }
}

pub fn start_turn(ctx: &mut AgentTurnContext) -> Result<Action, String> {
    if ctx.cancel_token.is_cancelled() {
        ctx.should_finish = true;
        ctx.emit(AgentEvent::AgentError {
            error: "aborted".into(),
        });
        return action(ACTION_ABORTED);
    }

    ctx.turn += 1;
    if let Some(max_turns) = ctx.config.max_turns
        && ctx.turn > max_turns
    {
        ctx.max_turns_exceeded = Some(max_turns);
        ctx.should_finish = true;
        ctx.emit(AgentEvent::AgentError {
            error: format!("max turns ({}) exceeded", max_turns),
        });
        return action(ACTION_ERROR);
    }

    ctx.emit(AgentEvent::TurnStart { turn: ctx.turn });
    default_action()
}

pub fn drain_queued_input(ctx: &mut AgentTurnContext) {
    ctx.sync_live_queues();
    let steered = drain_queue(&mut ctx.steering_queue, ctx.config.steering_mode);
    ctx.messages.extend(steered);
    ctx.has_more_queued_input = !ctx.steering_queue.is_empty() || !ctx.follow_up_queue.is_empty();
}

pub fn prepare_context(ctx: &mut AgentTurnContext) -> Result<(), String> {
    let context = convert_to_context(
        &ctx.config.system_prompt,
        &ctx.messages,
        &ctx.tools,
        &ctx.resources,
    );
    let mut stream_options = stream_options_for_turn(
        &ctx.config.model,
        ctx.config.stream_options.clone().unwrap_or_default(),
        ctx.config.thinking_level,
    );
    stream_options.cancel = Some(ctx.cancel_token.clone());

    let mut request = ProviderRequestSnapshot {
        model: ctx.config.model.clone(),
        context,
        stream_options,
    };

    if let Some(override_request) = ctx.take_provider_request_override() {
        request.context = override_request.context;
        if let Some(override_options) = override_request.stream_options {
            request.stream_options = override_options;
        }
        request.stream_options.cancel = Some(ctx.cancel_token.clone());
    }

    ctx.provider_request = Some(request);
    Ok(())
}

pub async fn prepare_provider_request(ctx: &mut AgentTurnContext) -> Result<Action, String> {
    let transformed_messages = if let Some(hook) = ctx.config.hooks.transform_context.clone() {
        match hook(ctx.messages.clone()).await {
            Ok(messages) => Some(messages),
            Err(error) => {
                ctx.emit(AgentEvent::AgentError {
                    error: error.clone(),
                });
                return action(ACTION_ERROR);
            }
        }
    } else {
        None
    };

    let llm_messages_override = if let Some(hook) = ctx.config.hooks.convert_to_llm.clone() {
        let messages = transformed_messages
            .clone()
            .unwrap_or_else(|| ctx.messages.clone());
        match hook(messages, ctx.resources.clone()).await {
            Ok(llm_messages) => Some(llm_messages),
            Err(error) => {
                ctx.emit(AgentEvent::AgentError {
                    error: error.clone(),
                });
                return action(ACTION_ERROR);
            }
        }
    } else {
        None
    };

    let messages_for_context = transformed_messages.as_ref().unwrap_or(&ctx.messages);
    let context = if let Some(llm_messages) = llm_messages_override {
        assemble_context(
            &ctx.config.system_prompt,
            messages_for_context,
            llm_messages,
            &ctx.tools,
            &ctx.resources,
        )
    } else if transformed_messages.is_some() {
        let llm_messages = default_convert_to_llm(messages_for_context, &ctx.resources);
        assemble_context(
            &ctx.config.system_prompt,
            messages_for_context,
            llm_messages,
            &ctx.tools,
            &ctx.resources,
        )
    } else {
        convert_to_context(
            &ctx.config.system_prompt,
            &ctx.messages,
            &ctx.tools,
            &ctx.resources,
        )
    };

    let mut stream_options = stream_options_for_turn(
        &ctx.config.model,
        ctx.config.stream_options.clone().unwrap_or_default(),
        ctx.config.thinking_level,
    );
    stream_options.cancel = Some(ctx.cancel_token.clone());

    let mut request = ProviderRequestSnapshot {
        model: ctx.config.model.clone(),
        context,
        stream_options,
    };

    if let Some(override_request) = ctx.take_provider_request_override() {
        request.context = override_request.context;
        if let Some(override_options) = override_request.stream_options {
            request.stream_options = override_options;
        }
        request.stream_options.cancel = Some(ctx.cancel_token.clone());
    }

    ctx.provider_request = Some(request);

    default_action()
}

pub async fn apply_before_provider_request_hook(
    ctx: &mut AgentTurnContext,
) -> Result<Action, String> {
    let mut request = match ctx.provider_request.clone() {
        Some(request) => request,
        None => {
            let error = "provider request is not prepared".to_string();
            ctx.emit(AgentEvent::AgentError {
                error: error.clone(),
            });
            return action(ACTION_ERROR);
        }
    };

    if let Some(hook) = ctx.config.hooks.before_provider_request.clone() {
        match hook(BeforeProviderRequestContext::from(request.clone())).await {
            Ok(Some(update)) => {
                if let Some(updated_context) = update.context {
                    request.context = updated_context;
                }
                if let Some(updated_options) = update.stream_options {
                    request.stream_options = updated_options;
                }
                request.stream_options.cancel = Some(ctx.cancel_token.clone());
            }
            Ok(None) => {}
            Err(error) => {
                ctx.emit(AgentEvent::AgentError {
                    error: error.clone(),
                });
                return action(ACTION_ERROR);
            }
        }
    }

    ctx.provider_request = Some(request.clone());
    ctx.emit(AgentEvent::BeforeProviderRequest { request });
    default_action()
}

pub async fn maybe_compact_runtime_context(ctx: &mut AgentTurnContext) -> Result<(), String> {
    let Some(config) = ctx.config.compaction.clone() else {
        return Ok(());
    };

    let usage_estimate = estimate_context_tokens(&ctx.messages);
    let tokens_before = usage_estimate.tokens;
    if !should_compact(
        tokens_before,
        ctx.config.model.context_window,
        &config.settings,
    ) {
        return Ok(());
    }

    let (mut to_summarize, mut keep) = prepare_compaction(&ctx.messages, &config.settings);
    if to_summarize.is_empty() {
        (to_summarize, keep) =
            split_for_compaction_after_usage_anchor(&ctx.messages, usage_estimate.last_usage_index);
    }
    if to_summarize.is_empty() {
        return Ok(());
    }

    let summary = summarize_with_provider_streamer(
        &ctx.config.model,
        &to_summarize,
        config.custom_instructions.as_deref(),
        ctx.config.stream_options.clone(),
        Some(ctx.cancel_token.clone()),
        ctx.config.provider_streamer.clone(),
    )
    .await
    .map_err(|err| err.to_string())?;

    let first_kept_message_id = keep.first().map(message_id).unwrap_or("none").to_string();
    for message in &mut keep {
        clear_assistant_usage(message);
    }

    let mut compacted = Vec::with_capacity(1 + keep.len());
    compacted.push(AgentMessage::CompactionSummary {
        message_id: format!("compaction_{}", tokens_before),
        summary: summary.clone(),
        tokens_before,
    });
    compacted.extend(keep);
    ctx.messages = compacted;

    ctx.runtime_compaction = RuntimeCompactionState {
        summary: Some(summary.clone()),
        first_kept_message_id: Some(first_kept_message_id.clone()),
        tokens_before: Some(tokens_before),
    };
    ctx.emit(AgentEvent::SessionCompacted {
        summary,
        first_kept_message_id,
        tokens_before,
        details: None,
    });

    Ok(())
}

pub async fn stream_provider(ctx: &mut AgentTurnContext) -> Result<Action, String> {
    let request = ctx
        .provider_request
        .clone()
        .ok_or_else(|| "provider request is not prepared".to_string())?;
    let mut llm_stream = match ctx.config.provider_streamer.clone() {
        Some(provider_streamer) => provider_streamer(
            &request.model,
            request.context,
            Some(request.stream_options),
        ),
        None => stream_model_with_global_runtime(
            &request.model,
            request.context,
            Some(request.stream_options),
        ),
    };
    let mut assistant_message = None;
    let mut stream_error = None;

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
        ctx.emit(AgentEvent::LlmEvent(event));
        if is_terminal {
            break;
        }
    }

    if let Some(message) = assistant_message {
        ctx.assistant_message = Some(message);
        return default_action();
    }

    let error = stream_error.unwrap_or_else(|| "LLM stream ended without Done event".into());
    ctx.emit(AgentEvent::AgentError { error });
    Action::new("error").map_err(|err| err.to_string())
}

pub fn decide_stop_or_tools(ctx: &mut AgentTurnContext) -> Result<Action, String> {
    let assistant = ctx
        .assistant_message
        .clone()
        .ok_or_else(|| "assistant message is not available".to_string())?;

    ctx.messages.push(AgentMessage::Assistant {
        message_id: assistant.response_id.clone().unwrap_or_default(),
        message: assistant.clone(),
    });

    match assistant.stop_reason {
        StopReason::Stop | StopReason::Length => {
            ctx.events
                .push(AgentEvent::AgentDone { message: assistant });
            Action::new("done").map_err(|err| err.to_string())
        }
        StopReason::Error => {
            let error = assistant
                .error_message
                .clone()
                .unwrap_or_else(|| "LLM error".into());
            ctx.emit(AgentEvent::AgentError { error });
            Action::new("error").map_err(|err| err.to_string())
        }
        StopReason::Aborted => {
            ctx.emit(AgentEvent::AgentError {
                error: "aborted".into(),
            });
            Action::new("aborted").map_err(|err| err.to_string())
        }
        StopReason::ToolUse => {
            let tool_calls = extract_tool_calls(&assistant);
            ctx.pending_tool_calls = tool_calls
                .into_iter()
                .map(|call| PendingToolCall {
                    index: call.index,
                    id: call.tool_call_id,
                    name: call.tool_name,
                    arguments: call.arguments,
                })
                .collect();
            if ctx.pending_tool_calls.is_empty() {
                Action::new("continue").map_err(|err| err.to_string())
            } else {
                Action::new("tools").map_err(|err| err.to_string())
            }
        }
    }
}

pub fn decide_after_assistant(ctx: &mut AgentTurnContext) -> Result<Action, String> {
    let assistant = ctx
        .assistant_message
        .clone()
        .ok_or_else(|| "assistant message is not available".to_string())?;

    ctx.messages.push(AgentMessage::Assistant {
        message_id: assistant.response_id.clone().unwrap_or_default(),
        message: assistant.clone(),
    });

    match assistant.stop_reason {
        StopReason::Stop | StopReason::Length => action(ACTION_CONTINUE),
        StopReason::Error => {
            let error = assistant
                .error_message
                .clone()
                .unwrap_or_else(|| "LLM error".into());
            ctx.should_finish = true;
            ctx.emit(AgentEvent::AgentError { error });
            action(ACTION_ERROR)
        }
        StopReason::Aborted => {
            ctx.should_finish = true;
            ctx.emit(AgentEvent::AgentError {
                error: "aborted".into(),
            });
            action(ACTION_ABORTED)
        }
        StopReason::ToolUse => {
            let tool_calls = extract_tool_calls(&assistant);
            ctx.pending_tool_calls = tool_calls
                .into_iter()
                .map(|call| PendingToolCall {
                    index: call.index,
                    id: call.tool_call_id,
                    name: call.tool_name,
                    arguments: call.arguments,
                })
                .collect();
            action(ACTION_TOOLS)
        }
    }
}

pub async fn maybe_prepare_next_turn(ctx: &mut AgentTurnContext) -> Result<Action, String> {
    let assistant = ctx
        .assistant_message
        .clone()
        .ok_or_else(|| "assistant message is not available".to_string())?;
    ctx.sync_live_queues();

    match assistant.stop_reason {
        StopReason::Stop | StopReason::Length => {
            let Some(should_stop) = should_stop_after_turn(ctx, &assistant).await? else {
                return action(ACTION_ERROR);
            };
            if should_stop {
                ctx.should_finish = true;
                ctx.has_more_queued_input = false;
                ctx.emit(AgentEvent::AgentDone { message: assistant });
                return action(ACTION_DONE);
            }

            if let Some(action) = prepare_next_turn_or_error(ctx).await? {
                return Ok(action);
            }

            let has_more = !ctx.follow_up_queue.is_empty() || !ctx.steering_queue.is_empty();
            ctx.has_more_queued_input = has_more;
            if has_more {
                let follow_ups = drain_queue(&mut ctx.follow_up_queue, ctx.config.follow_up_mode);
                ctx.messages.extend(follow_ups);
                ctx.should_finish = false;
                action(ACTION_CONTINUE)
            } else {
                ctx.should_finish = true;
                ctx.emit(AgentEvent::AgentDone { message: assistant });
                action(ACTION_DONE)
            }
        }
        StopReason::ToolUse => {
            let Some(should_stop) = should_stop_after_turn(ctx, &assistant).await? else {
                return action(ACTION_ERROR);
            };
            if should_stop {
                ctx.should_finish = true;
                ctx.has_more_queued_input = false;
                ctx.emit(AgentEvent::AgentDone { message: assistant });
                return action(ACTION_DONE);
            }

            if ctx.tool_results_all_terminate {
                ctx.should_finish = true;
                ctx.has_more_queued_input = false;
                ctx.emit(AgentEvent::AgentDone { message: assistant });
                return action(ACTION_DONE);
            }

            if let Some(action) = prepare_next_turn_or_error(ctx).await? {
                return Ok(action);
            }

            ctx.should_finish = false;
            ctx.has_more_queued_input =
                !ctx.follow_up_queue.is_empty() || !ctx.steering_queue.is_empty();
            action(ACTION_CONTINUE)
        }
        StopReason::Error => {
            ctx.should_finish = true;
            action(ACTION_ERROR)
        }
        StopReason::Aborted => {
            ctx.should_finish = true;
            action(ACTION_ABORTED)
        }
    }
}

pub async fn execute_tools(ctx: &mut AgentTurnContext) -> Result<Action, String> {
    ctx.tool_results_all_terminate = false;
    let pending = std::mem::take(&mut ctx.pending_tool_calls);
    if pending.is_empty() {
        return action(ACTION_CONTINUE_PROVIDER);
    }

    let requests: Vec<_> = pending
        .iter()
        .map(|call| ToolCallRequest {
            index: call.index,
            tool_call_id: call.id.clone(),
            tool_name: call.name.clone(),
            arguments: call.arguments.clone(),
        })
        .collect();
    let use_sequential =
        should_use_sequential_tools(ctx.config.tool_execution, &requests, &ctx.tools);

    let executions = if use_sequential {
        let mut executions = Vec::with_capacity(pending.len());
        for call in pending {
            ctx.emit(AgentEvent::ToolCallStart {
                tool_call_id: call.id.clone(),
                tool_name: call.name.clone(),
                arguments: call.arguments.clone(),
            });

            let result = match before_tool_result(ctx, &call).await {
                Some(result) => result,
                None => {
                    let tool = find_tool(&ctx.tools, &call.name);
                    let result = execute_tool_with_updates(ctx, &call, tool).await;
                    after_tool_result(ctx, &call, result).await
                }
            };

            ctx.emit(AgentEvent::ToolCallEnd {
                tool_call_id: call.id.clone(),
                tool_name: call.name.clone(),
                result: result.clone(),
            });
            executions.push(ToolCallExecution {
                index: call.index,
                tool_call_id: call.id,
                tool_name: call.name,
                result,
            });
        }
        executions
    } else {
        for call in &pending {
            ctx.emit(AgentEvent::ToolCallStart {
                tool_call_id: call.id.clone(),
                tool_name: call.name.clone(),
                arguments: call.arguments.clone(),
            });
        }

        let after_hook = ctx.config.hooks.after_tool_call.clone();
        let assistant_message = ctx.assistant_message.clone();
        let messages = ctx.messages.clone();
        let mut prepared = Vec::with_capacity(pending.len());
        for call in pending {
            let blocked = before_tool_result(ctx, &call).await;
            let tool = find_tool(&ctx.tools, &call.name);
            prepared.push((call, tool, blocked));
        }

        collect_parallel_tool_executions(ctx, prepared, after_hook, assistant_message, messages)
            .await
    };

    let all_terminate = !executions.is_empty()
        && executions
            .iter()
            .all(|execution| execution.result.terminate);
    ctx.tool_results
        .extend(executions.iter().map(|execution| execution.result.clone()));
    append_tool_result_messages(&mut ctx.messages, &executions);

    ctx.tool_results_all_terminate = all_terminate;

    Action::new("continue").map_err(|err| err.to_string())
}

async fn collect_parallel_tool_executions(
    ctx: &mut AgentTurnContext,
    prepared: Vec<(PendingToolCall, Option<AgentTool>, Option<AgentToolResult>)>,
    after_hook: Option<AfterToolCallHook>,
    assistant_message: Option<AssistantMessage>,
    messages: Vec<AgentMessage>,
) -> Vec<ToolCallExecution> {
    let mut futures: FuturesUnordered<_> = prepared
        .into_iter()
        .map(|(call, tool, blocked)| {
            let after_hook = after_hook.clone();
            let assistant_message = assistant_message.clone();
            let messages = messages.clone();
            async move {
                let result = match blocked {
                    Some(result) => result,
                    None => {
                        let result =
                            execute_tool(tool, call.name.clone(), call.arguments.clone()).await;
                        apply_after_tool_hook(
                            after_hook,
                            assistant_message,
                            messages,
                            &call,
                            result,
                        )
                        .await
                    }
                };
                ToolCallExecution {
                    index: call.index,
                    tool_call_id: call.id,
                    tool_name: call.name,
                    result,
                }
            }
        })
        .collect();

    let mut executions = Vec::new();
    while let Some(execution) = futures.next().await {
        ctx.emit(AgentEvent::ToolCallEnd {
            tool_call_id: execution.tool_call_id.clone(),
            tool_name: execution.tool_name.clone(),
            result: execution.result.clone(),
        });
        executions.push(execution);
    }
    executions.sort_by_key(|execution| execution.index);
    executions
}

async fn before_tool_result(
    ctx: &AgentTurnContext,
    call: &PendingToolCall,
) -> Option<AgentToolResult> {
    let hook = ctx.config.hooks.before_tool_call.clone()?;
    let assistant_message = ctx.assistant_message.clone()?;
    let hook_context = BeforeToolCallContext {
        assistant_message,
        tool_call_id: call.id.clone(),
        tool_name: call.name.clone(),
        arguments: call.arguments.clone(),
        messages: ctx.messages.clone(),
    };

    match hook(hook_context).await {
        Ok(Some(result)) if result.block => Some(AgentToolResult::error(
            result.reason.unwrap_or_else(|| "blocked".into()),
        )),
        Err(error) => Some(AgentToolResult::error(error)),
        _ => None,
    }
}

async fn after_tool_result(
    ctx: &AgentTurnContext,
    call: &PendingToolCall,
    result: AgentToolResult,
) -> AgentToolResult {
    apply_after_tool_hook(
        ctx.config.hooks.after_tool_call.clone(),
        ctx.assistant_message.clone(),
        ctx.messages.clone(),
        call,
        result,
    )
    .await
}

async fn apply_after_tool_hook(
    hook: Option<AfterToolCallHook>,
    assistant_message: Option<AssistantMessage>,
    messages: Vec<AgentMessage>,
    call: &PendingToolCall,
    mut result: AgentToolResult,
) -> AgentToolResult {
    let Some(hook) = hook else {
        return result;
    };
    let Some(assistant_message) = assistant_message else {
        return result;
    };
    let hook_context = AfterToolCallContext {
        assistant_message,
        tool_call_id: call.id.clone(),
        tool_name: call.name.clone(),
        arguments: call.arguments.clone(),
        result: result.clone(),
        messages,
    };

    match hook(hook_context).await {
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
            result
        }
        Err(error) => AgentToolResult::error(error),
        _ => result,
    }
}

fn find_tool(tools: &[AgentTool], name: &str) -> Option<AgentTool> {
    tools.iter().find(|tool| tool.name == name).cloned()
}

async fn execute_tool_with_updates(
    ctx: &mut AgentTurnContext,
    call: &PendingToolCall,
    tool: Option<AgentTool>,
) -> AgentToolResult {
    let (update_tx, mut update_rx) = mpsc::unbounded::<AgentToolOutput>();
    let update_callback: ToolUpdateCallback = Arc::new(move |update| {
        let _ = update_tx.unbounded_send(update);
    });
    let mut execute_future = Box::pin({
        let arguments = call.arguments.clone();
        let tool_name = call.name.clone();
        async move {
            match tool {
                Some(tool) => match (tool.execute)(arguments, Some(update_callback)).await {
                    Ok(output) => AgentToolResult::from_output(output),
                    Err(error) => AgentToolResult::error(error),
                },
                None => AgentToolResult::error(format!("unknown tool: {}", tool_name)),
            }
        }
    })
    .fuse();
    let mut update_open = true;
    let result = loop {
        if !update_open {
            break execute_future.await;
        }
        futures::select! {
            maybe_update = update_rx.next().fuse() => {
                if let Some(update) = maybe_update {
                    ctx.emit(AgentEvent::ToolCallUpdate {
                        tool_call_id: call.id.clone(),
                        tool_name: call.name.clone(),
                        update,
                    });
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
        ctx.emit(AgentEvent::ToolCallUpdate {
            tool_call_id: call.id.clone(),
            tool_name: call.name.clone(),
            update,
        });
    }
    result
}

async fn execute_tool(
    tool: Option<AgentTool>,
    tool_name: String,
    arguments: serde_json::Value,
) -> AgentToolResult {
    match tool {
        Some(tool) => match (tool.execute)(arguments, None).await {
            Ok(output) => AgentToolResult::from_output(output),
            Err(error) => AgentToolResult::error(error),
        },
        None => AgentToolResult::error(format!("unknown tool: {}", tool_name)),
    }
}

fn default_action() -> Result<Action, String> {
    action(ACTION_DEFAULT)
}

fn action(value: &str) -> Result<Action, String> {
    Action::new(value).map_err(|err| err.to_string())
}

async fn should_stop_after_turn(
    ctx: &mut AgentTurnContext,
    assistant: &AssistantMessage,
) -> Result<Option<bool>, String> {
    let Some(hook) = ctx.config.hooks.should_stop_after_turn.clone() else {
        return Ok(Some(false));
    };

    match hook(ShouldStopAfterTurnContext {
        messages: ctx.messages.clone(),
        assistant_message: assistant.clone(),
    })
    .await
    {
        Ok(should_stop) => Ok(Some(should_stop)),
        Err(error) => {
            ctx.should_finish = true;
            ctx.emit(AgentEvent::AgentError {
                error: error.clone(),
            });
            Ok(None)
        }
    }
}

async fn prepare_next_turn_or_error(ctx: &mut AgentTurnContext) -> Result<Option<Action>, String> {
    let Some(hook) = ctx.config.hooks.prepare_next_turn.clone() else {
        return Ok(None);
    };

    let update = match hook(PrepareNextTurnContext {
        messages: ctx.messages.clone(),
        turn: ctx.turn,
    })
    .await
    {
        Ok(update) => update,
        Err(error) => {
            ctx.should_finish = true;
            ctx.emit(AgentEvent::AgentError {
                error: error.clone(),
            });
            return Ok(Some(action(ACTION_ERROR)?));
        }
    };

    let Some(update) = update else {
        return Ok(None);
    };

    if let Some(messages) = update.messages {
        ctx.messages = messages;
    }
    if let Some(model) = update.model {
        ctx.config.model = model;
    }
    if let Some(thinking_level) = update.thinking_level {
        ctx.config.thinking_level = thinking_level;
    }
    if let Some(stream_options) = update.stream_options {
        ctx.config.stream_options = Some(stream_options);
    }
    Ok(None)
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

fn clear_assistant_usage(message: &mut AgentMessage) {
    if let AgentMessage::Assistant { message, .. } = message {
        message.usage = Usage::default();
    }
}

fn split_for_compaction_after_usage_anchor(
    messages: &[AgentMessage],
    anchor_index: Option<usize>,
) -> (Vec<AgentMessage>, Vec<AgentMessage>) {
    let Some(anchor_index) = anchor_index else {
        return (vec![], messages.to_vec());
    };
    if messages.is_empty() {
        return (vec![], vec![]);
    }

    let mut split = anchor_index.saturating_add(1).min(messages.len());
    while split < messages.len() && matches!(messages[split], AgentMessage::ToolResult { .. }) {
        split += 1;
    }

    (messages[..split].to_vec(), messages[split..].to_vec())
}
