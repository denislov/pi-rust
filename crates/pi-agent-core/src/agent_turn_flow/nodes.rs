use std::future::Future;
use std::pin::Pin;

use crate::compaction::estimate::estimate_context_tokens;
use crate::compaction::prepare::{prepare_compaction, should_compact};
use crate::compaction::summarize::summarize;
use crate::convert::convert_to_context;
use crate::flow::{Action, FlowNode};
use crate::loop_runtime::context::stream_options_for_turn;
use crate::types::{AgentEvent, AgentMessage, ProviderRequestSnapshot};
use pi_ai::types::Usage;

use super::context::{AgentTurnContext, RuntimeCompactionState};

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
            Action::new("default").map_err(|err| err.to_string())
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
            Action::new("default").map_err(|err| err.to_string())
        })
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
