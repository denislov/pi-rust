use std::path::PathBuf;

use super::agent_invocation_flow::{AgentInvocationOptions, AgentInvocationOutcome};
use super::agent_team_flow::{AgentTeamOptions, AgentTeamOutcome};
use super::export::CodingAgentSessionExport;
use super::prompt::{PromptTurnOptions, PromptTurnOutcome};
use super::self_healing_edit_flow::{SelfHealingEditOutcome, SelfHealingEditRequest};

#[derive(Debug)]
pub enum CodingAgentOperation {
    Prompt(PromptTurnOptions),
    Compact(PromptTurnOptions),
    BranchSummary {
        options: PromptTurnOptions,
        source_leaf_id: String,
        target_leaf_id: String,
        custom_instructions: Option<String>,
    },
    SelfHealingEdit(SelfHealingEditRequest),
    InvokeAgent(AgentInvocationOptions),
    InvokeTeam(AgentTeamOptions),
    ExportCurrent,
    ExportCurrentHtml(PathBuf),
}

#[derive(Debug)]
pub enum CodingAgentOperationOutcome {
    Prompt(PromptTurnOutcome),
    Compact(PromptTurnOutcome),
    BranchSummary(PromptTurnOutcome),
    SelfHealingEdit(SelfHealingEditOutcome),
    AgentInvocation(AgentInvocationOutcome),
    AgentTeam(AgentTeamOutcome),
    Export(CodingAgentSessionExport),
    ExportHtml(PathBuf),
}
