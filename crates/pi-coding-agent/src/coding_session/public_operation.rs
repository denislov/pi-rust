use std::path::PathBuf;

use super::agent_invocation_flow::{AgentInvocationOptions, AgentInvocationOutcome};
use super::agent_team_flow::{AgentTeamOptions, AgentTeamOutcome};
use super::export::CodingAgentSessionExport;
use super::profiles::ProfileId;
use super::prompt::{PromptTurnOptions, PromptTurnOutcome};
use super::self_healing_edit_flow::{SelfHealingEditOutcome, SelfHealingEditRequest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchSummaryReusePolicy {
    AlwaysCreate,
    ReuseExisting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentPluginDiagnostic {
    pub plugin_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentPluginLoadOutcome {
    pub loaded_plugin_ids: Vec<String>,
    pub diagnostics: Vec<CodingAgentPluginDiagnostic>,
    pub capability_changed: bool,
}

#[derive(Debug)]
pub enum CodingAgentOperation {
    Prompt(PromptTurnOptions),
    Compact(PromptTurnOptions),
    BranchSummary {
        options: PromptTurnOptions,
        source_leaf_id: String,
        target_leaf_id: String,
        custom_instructions: Option<String>,
        reuse: BranchSummaryReusePolicy,
    },
    SelfHealingEdit(SelfHealingEditRequest),
    InvokeAgent(AgentInvocationOptions),
    InvokeTeam(AgentTeamOptions),
    PluginLoad,
    PluginCommand {
        command_id: String,
        args: serde_json::Value,
    },
    SetDefaultAgentProfile {
        profile_id: ProfileId,
    },
    ApproveDelegation {
        operation_id: String,
        tool_call_id: String,
    },
    RejectDelegation {
        operation_id: String,
        tool_call_id: String,
        reason: String,
    },
    ForkSession {
        target_leaf_id: Option<String>,
    },
    SwitchActiveLeaf {
        target_leaf_id: String,
    },
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
    PluginLoad(CodingAgentPluginLoadOutcome),
    PluginCommand(String),
    DefaultAgentProfileChanged,
    DelegationApproved,
    DelegationRejected,
    SessionForked,
    ActiveLeafSwitched,
    Export(CodingAgentSessionExport),
    ExportHtml(PathBuf),
}
