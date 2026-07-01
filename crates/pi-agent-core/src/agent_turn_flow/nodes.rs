use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::compaction::estimate::estimate_context_tokens;
use crate::compaction::prepare::{prepare_compaction, should_compact};
use crate::compaction::summarize::summarize;
use crate::convert::convert_to_context;
use crate::flow::{Action, FlowNode};
use crate::hooks::{AfterToolCallContext, AfterToolCallHook, BeforeToolCallContext};
use crate::loop_runtime::context::stream_options_for_turn;
use crate::loop_runtime::tools::{
    ToolCallExecution, ToolCallRequest, append_tool_result_messages, extract_tool_calls,
    should_use_sequential_tools,
};
use crate::types::{
    AgentEvent, AgentMessage, AgentTool, AgentToolOutput, AgentToolResult, ProviderRequestSnapshot,
    ToolUpdateCallback,
};
use futures::channel::mpsc;
use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt};
use pi_ai::types::{AssistantMessage, AssistantMessageEvent, StopReason, Usage};

use super::context::{AgentTurnContext, PendingToolCall, RuntimeCompactionState};

pub struct PrepareContextNode;

impl FlowNode<AgentTurnContext> for PrepareContextNode {
    fn name(&self) -> &str {
        "prepare_context"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            prepare_context(ctx)?;
            default_action()
        })
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

    ctx.provider_request = Some(ProviderRequestSnapshot {
        model: ctx.config.model.clone(),
        context,
        stream_options,
    });
    Ok(())
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

    let summary = summarize(
        &ctx.config.model,
        &to_summarize,
        config.custom_instructions.as_deref(),
        ctx.config.stream_options.clone(),
        Some(ctx.cancel_token.clone()),
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
    ctx.events.push(AgentEvent::SessionCompacted {
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
    let mut llm_stream = pi_ai::stream_model(
        &request.model,
        request.context,
        Some(request.stream_options),
    );
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
        ctx.events.push(AgentEvent::LlmEvent(event));
        if is_terminal {
            break;
        }
    }

    if let Some(message) = assistant_message {
        ctx.assistant_message = Some(message);
        return default_action();
    }

    let error = stream_error.unwrap_or_else(|| "LLM stream ended without Done event".into());
    ctx.events.push(AgentEvent::AgentError { error });
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
            ctx.events.push(AgentEvent::AgentError { error });
            Action::new("error").map_err(|err| err.to_string())
        }
        StopReason::Aborted => {
            ctx.events.push(AgentEvent::AgentError {
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

pub async fn execute_tools(ctx: &mut AgentTurnContext) -> Result<Action, String> {
    let pending = std::mem::take(&mut ctx.pending_tool_calls);
    if pending.is_empty() {
        return Action::new("continue").map_err(|err| err.to_string());
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
            ctx.events.push(AgentEvent::ToolCallStart {
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

            ctx.events.push(AgentEvent::ToolCallEnd {
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
            ctx.events.push(AgentEvent::ToolCallStart {
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
            ctx.events.push(AgentEvent::ToolCallEnd {
                tool_call_id: execution.tool_call_id.clone(),
                tool_name: execution.tool_name.clone(),
                result: execution.result.clone(),
            });
            executions.push(execution);
        }
        executions.sort_by_key(|execution| execution.index);
        executions
    };

    let all_terminate = !executions.is_empty()
        && executions
            .iter()
            .all(|execution| execution.result.terminate);
    ctx.tool_results
        .extend(executions.iter().map(|execution| execution.result.clone()));
    append_tool_result_messages(&mut ctx.messages, &executions);

    if all_terminate && let Some(message) = ctx.assistant_message.clone() {
        ctx.events.push(AgentEvent::AgentDone { message });
        return Action::new("done").map_err(|err| err.to_string());
    }

    Action::new("continue").map_err(|err| err.to_string())
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
                    ctx.events.push(AgentEvent::ToolCallUpdate {
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
        ctx.events.push(AgentEvent::ToolCallUpdate {
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
    Action::new("default").map_err(|err| err.to_string())
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
