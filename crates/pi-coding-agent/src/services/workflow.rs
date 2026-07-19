use tokio_util::sync::CancellationToken;

use crate::operations::agent_invocation::runner::{
    AgentInvocationContext, AgentInvocationOutcome, AgentInvocationRunner,
};
use crate::operations::branch_summary::runner::{
    BranchSummaryContext, BranchSummaryOutcome, BranchSummaryRunner,
};
use crate::operations::compaction::runner::{
    ManualCompactionContext, ManualCompactionOutcome, ManualCompactionRunner,
};
use crate::operations::export::runner::{ExportContext, ExportOutcome, ExportRunner};
use crate::operations::plugin_load::runner::{
    PluginLoadContext, PluginLoadOutcome, PluginLoadRunner,
};
use crate::operations::prompt::context::{PromptTurnContext, PromptTurnOutcome};
use crate::operations::prompt::runner::PromptTurnRunner;
use crate::operations::self_healing_edit::runner::{
    SelfHealingEditContext, SelfHealingEditOutcome, SelfHealingEditRunner,
};
use crate::operations::team_invocation::runner::{
    AgentTeamContext, AgentTeamOutcome, AgentTeamRunner,
};
use crate::runtime::facade::CodingSessionError;

#[derive(Debug, Default)]
pub(crate) struct WorkflowService;

impl WorkflowService {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn run_export(
        &self,
        ctx: &mut ExportContext,
    ) -> Result<ExportOutcome, CodingSessionError> {
        match ExportRunner::new()?.run_typed(ctx) {
            Ok(outcome) => Ok(outcome),
            Err(error) => Err(ctx.take_failure_error().unwrap_or(error)),
        }
    }

    pub(crate) async fn run_self_healing_edit(
        &self,
        ctx: &mut SelfHealingEditContext,
    ) -> Result<SelfHealingEditOutcome, CodingSessionError> {
        self.run_self_healing_edit_inner(ctx, None).await
    }

    pub(crate) async fn run_self_healing_edit_with_cancellation(
        &self,
        ctx: &mut SelfHealingEditContext,
        cancellation: CancellationToken,
    ) -> Result<SelfHealingEditOutcome, CodingSessionError> {
        self.run_self_healing_edit_inner(ctx, Some(cancellation))
            .await
    }

    async fn run_self_healing_edit_inner(
        &self,
        ctx: &mut SelfHealingEditContext,
        cancellation: Option<CancellationToken>,
    ) -> Result<SelfHealingEditOutcome, CodingSessionError> {
        match SelfHealingEditRunner::new()?
            .run_typed(ctx, cancellation)
            .await
        {
            Ok(_) => ctx.finish_success(),
            Err(error) => Err(ctx.take_failure_error().unwrap_or(error)),
        }
    }

    pub(crate) async fn run_agent_invocation(
        &self,
        ctx: &mut AgentInvocationContext,
    ) -> Result<AgentInvocationOutcome, CodingSessionError> {
        self.run_agent_invocation_inner(ctx, None).await
    }

    pub(crate) async fn run_agent_invocation_with_cancellation(
        &self,
        ctx: &mut AgentInvocationContext,
        cancellation: CancellationToken,
    ) -> Result<AgentInvocationOutcome, CodingSessionError> {
        self.run_agent_invocation_inner(ctx, Some(cancellation))
            .await
    }

    async fn run_agent_invocation_inner(
        &self,
        ctx: &mut AgentInvocationContext,
        cancellation: Option<CancellationToken>,
    ) -> Result<AgentInvocationOutcome, CodingSessionError> {
        match AgentInvocationRunner::new()?
            .run_typed(ctx, cancellation)
            .await
        {
            Ok(_) => ctx.finish_success(),
            Err(error) => Err(ctx.take_failure_error().unwrap_or(error)),
        }
    }

    pub(crate) async fn run_agent_team(
        &self,
        ctx: &mut AgentTeamContext,
    ) -> Result<AgentTeamOutcome, CodingSessionError> {
        self.run_agent_team_inner(ctx, None).await
    }

    pub(crate) async fn run_agent_team_with_cancellation(
        &self,
        ctx: &mut AgentTeamContext,
        cancellation: CancellationToken,
    ) -> Result<AgentTeamOutcome, CodingSessionError> {
        self.run_agent_team_inner(ctx, Some(cancellation)).await
    }

    async fn run_agent_team_inner(
        &self,
        ctx: &mut AgentTeamContext,
        cancellation: Option<CancellationToken>,
    ) -> Result<AgentTeamOutcome, CodingSessionError> {
        match AgentTeamRunner::new()?.run_typed(ctx, cancellation).await {
            Ok(_) => ctx.finish_success(),
            Err(error) => Err(ctx.take_failure_error().unwrap_or(error)),
        }
    }

    pub(crate) async fn run_prompt_subflow_typed_for_agent_invocation(
        &self,
        ctx: &mut PromptTurnContext,
    ) -> Result<(), CodingSessionError> {
        PromptTurnRunner::new()?.run_typed(ctx).await
    }

    pub(crate) async fn run_prompt_subflow_typed_for_agent_team_member(
        &self,
        ctx: &mut PromptTurnContext,
    ) -> Result<(), CodingSessionError> {
        PromptTurnRunner::new()?.run_typed(ctx).await
    }

    pub(crate) async fn run_plugin_load(
        &self,
        ctx: &mut PluginLoadContext,
    ) -> Result<PluginLoadOutcome, CodingSessionError> {
        self.run_plugin_load_inner(ctx, None).await
    }

    pub(crate) async fn run_plugin_load_with_cancellation(
        &self,
        ctx: &mut PluginLoadContext,
        cancellation: CancellationToken,
    ) -> Result<PluginLoadOutcome, CodingSessionError> {
        self.run_plugin_load_inner(ctx, Some(cancellation)).await
    }

    async fn run_plugin_load_inner(
        &self,
        ctx: &mut PluginLoadContext,
        cancellation: Option<CancellationToken>,
    ) -> Result<PluginLoadOutcome, CodingSessionError> {
        match PluginLoadRunner::new()?.run_typed(ctx, cancellation).await {
            Ok(()) => ctx.finish_success(),
            Err(error) => Err(ctx.take_failure_error().unwrap_or(error)),
        }
    }

    pub(crate) async fn run_branch_summary(
        &self,
        ctx: &mut BranchSummaryContext,
    ) -> Result<BranchSummaryOutcome, CodingSessionError> {
        self.run_branch_summary_inner(ctx, None).await
    }

    pub(crate) async fn run_branch_summary_with_cancellation(
        &self,
        ctx: &mut BranchSummaryContext,
        cancellation: CancellationToken,
    ) -> Result<BranchSummaryOutcome, CodingSessionError> {
        self.run_branch_summary_inner(ctx, Some(cancellation)).await
    }

    async fn run_branch_summary_inner(
        &self,
        ctx: &mut BranchSummaryContext,
        cancellation: Option<CancellationToken>,
    ) -> Result<BranchSummaryOutcome, CodingSessionError> {
        match BranchSummaryRunner::new()?
            .run_typed(ctx, cancellation)
            .await
        {
            Ok(_) => ctx.finish_success(),
            Err(error) => Err(ctx.take_failure_error().unwrap_or(error)),
        }
    }

    pub(crate) async fn run_manual_compaction(
        &self,
        ctx: &mut ManualCompactionContext,
    ) -> Result<ManualCompactionOutcome, CodingSessionError> {
        match ManualCompactionRunner::new()?.run_typed(ctx).await {
            Ok(outcome) => Ok(outcome),
            Err(error) => Err(ctx.take_failure_error().unwrap_or(error)),
        }
    }

    pub(crate) async fn run_prompt_turn(
        &self,
        ctx: &mut PromptTurnContext,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        match PromptTurnRunner::new()?.run_typed(ctx).await {
            Ok(_) => {
                let session_id = ctx.session_id().map(str::to_owned);
                ctx.finish_success(session_id, None)
            }
            Err(error) => match ctx.abort_reason() {
                Some(reason) => {
                    Ok(ctx.finish_abort(reason.to_owned(), ctx.session_id().map(str::to_owned)))
                }
                None => Ok(ctx.finish_failure(error)),
            },
        }
    }
}
