use std::path::PathBuf;

use super::agent_invocation_flow::{AgentInvocationOptions, AgentInvocationOutcome};
use super::agent_team_flow::{AgentTeamOptions, AgentTeamOutcome};
use super::export::CodingAgentSessionExport;
use super::export_flow::ExportOptions;
use super::operation::{Operation, OperationOutcome};
use super::plugin_load_flow::{PluginLoadOptions, PluginLoadOutcome};
use super::profiles::ProfileId;
use super::prompt::{PromptTurnOptions, PromptTurnOutcome};
use super::self_healing_edit_flow::{SelfHealingEditOutcome, SelfHealingEditRequest};

/// Controls whether branch summarization may reuse a previously persisted summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchSummaryReusePolicy {
    /// Always create a new summary for the requested branch pair.
    AlwaysCreate,
    /// Reuse a matching persisted summary without emitting events or rewriting the session log.
    /// A new summary is created when no matching persisted summary exists.
    ReuseExisting,
}

/// A plugin-load diagnostic projected without internal runtime state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentPluginDiagnostic {
    pub plugin_id: Option<String>,
    pub message: String,
}

/// Public plugin-load results, excluding internal capability objects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentPluginLoadOutcome {
    /// Plugin identifiers installed by this load.
    pub loaded_plugin_ids: Vec<String>,
    /// Validation and loading diagnostics safe for public consumers.
    pub diagnostics: Vec<CodingAgentPluginDiagnostic>,
    /// Whether the load installed a new capability generation.
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
    /// Move this owner to a forked persistent session while retaining live runtime state.
    ForkSession {
        /// The leaf to fork from, or the current active leaf when omitted.
        target_leaf_id: Option<String>,
    },
    /// Make an existing committed leaf active in a persistent session.
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
    /// The session owner was replaced with a newly forked session.
    SessionForked,
    /// The requested existing leaf became active.
    ActiveLeafSwitched,
    Export(CodingAgentSessionExport),
    ExportHtml(PathBuf),
}

impl CodingAgentOperation {
    pub(crate) fn into_internal(self, plugin_load: PluginLoadOptions) -> Operation {
        match self {
            Self::Prompt(options) => Operation::Prompt(options),
            Self::Compact(options) => Operation::ManualCompaction(options),
            Self::BranchSummary {
                options,
                source_leaf_id,
                target_leaf_id,
                custom_instructions,
                reuse,
            } => Operation::BranchSummary {
                options,
                source_leaf_id,
                target_leaf_id,
                custom_instructions,
                reuse_existing: matches!(reuse, BranchSummaryReusePolicy::ReuseExisting),
            },
            Self::SelfHealingEdit(request) => Operation::SelfHealingEdit(request),
            Self::InvokeAgent(options) => Operation::AgentInvocation(options),
            Self::InvokeTeam(options) => Operation::AgentTeam(options),
            Self::PluginLoad => Operation::PluginLoad(plugin_load),
            Self::PluginCommand { command_id, args } => {
                Operation::PluginCommand { command_id, args }
            }
            Self::SetDefaultAgentProfile { profile_id } => {
                Operation::SetDefaultAgentProfile { profile_id }
            }
            Self::ApproveDelegation {
                operation_id,
                tool_call_id,
            } => Operation::ApproveDelegationConfirmation {
                operation_id,
                tool_call_id,
            },
            Self::RejectDelegation {
                operation_id,
                tool_call_id,
                reason,
            } => Operation::RejectDelegationConfirmation {
                operation_id,
                tool_call_id,
                reason,
            },
            Self::ForkSession { target_leaf_id } => Operation::ForkSession { target_leaf_id },
            Self::SwitchActiveLeaf { target_leaf_id } => {
                Operation::SwitchActiveLeaf { target_leaf_id }
            }
            Self::ExportCurrent => Operation::Export(ExportOptions::view()),
            Self::ExportCurrentHtml(path) => Operation::Export(ExportOptions::html(path)),
        }
    }
}

impl CodingAgentOperationOutcome {
    pub(crate) fn from_internal(outcome: OperationOutcome) -> Self {
        match outcome {
            OperationOutcome::Prompt(outcome) => Self::Prompt(outcome),
            OperationOutcome::ManualCompaction(outcome) => Self::Compact(outcome),
            OperationOutcome::PluginLoad(outcome) => Self::PluginLoad(outcome.into()),
            OperationOutcome::PluginCommand(output) => Self::PluginCommand(output),
            OperationOutcome::DelegationApproval => Self::DelegationApproved,
            OperationOutcome::DelegationRejection => Self::DelegationRejected,
            OperationOutcome::BranchSummary(outcome) => Self::BranchSummary(outcome),
            OperationOutcome::SelfHealingEdit(outcome) => Self::SelfHealingEdit(outcome),
            OperationOutcome::AgentInvocation(outcome) => Self::AgentInvocation(outcome),
            OperationOutcome::AgentTeam(outcome) => Self::AgentTeam(outcome),
            OperationOutcome::SetDefaultAgentProfile => Self::DefaultAgentProfileChanged,
            OperationOutcome::ForkSession => Self::SessionForked,
            OperationOutcome::SwitchActiveLeaf => Self::ActiveLeafSwitched,
            OperationOutcome::Export(outcome) => match outcome.path {
                Some(path) => Self::ExportHtml(path),
                None => Self::Export(outcome.export),
            },
        }
    }
}

impl From<PluginLoadOutcome> for CodingAgentPluginLoadOutcome {
    fn from(outcome: PluginLoadOutcome) -> Self {
        Self {
            loaded_plugin_ids: outcome.loaded_plugin_ids,
            diagnostics: outcome
                .diagnostics
                .into_iter()
                .map(|diagnostic| CodingAgentPluginDiagnostic {
                    plugin_id: diagnostic.plugin_id,
                    message: diagnostic.message,
                })
                .collect(),
            capability_changed: outcome.capability_changed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::context::CodingAgentSessionSummary;
    use crate::coding_session::export_flow::ExportOutcome;
    use crate::coding_session::plugin_service::PluginDiagnostic;
    use crate::plugins::PluginCapabilities;
    use crate::runtime::PromptInvocation;

    fn branch_summary_reuse_flag(reuse: BranchSummaryReusePolicy) -> bool {
        let operation = CodingAgentOperation::BranchSummary {
            options: PromptTurnOptions::new(PromptInvocation::Text("summarize".into())),
            source_leaf_id: "leaf_source".into(),
            target_leaf_id: "leaf_target".into(),
            custom_instructions: None,
            reuse,
        }
        .into_internal(PluginLoadOptions::new());

        let Operation::BranchSummary { reuse_existing, .. } = operation else {
            panic!("branch summary should map to the internal branch-summary operation")
        };
        reuse_existing
    }

    #[test]
    fn branch_summary_reuse_policy_maps_to_internal_flag() {
        assert!(!branch_summary_reuse_flag(
            BranchSummaryReusePolicy::AlwaysCreate
        ));
        assert!(branch_summary_reuse_flag(
            BranchSummaryReusePolicy::ReuseExisting
        ));
    }

    #[test]
    fn html_export_outcome_projects_to_public_path() {
        let path = PathBuf::from("session.html");
        let export = CodingAgentSessionExport {
            summary: CodingAgentSessionSummary {
                session_id: "sess_export".into(),
                session_dir: PathBuf::from("sessions/sess_export"),
                created_at: "2026-07-10T00:00:00Z".into(),
                updated_at: "2026-07-10T00:00:00Z".into(),
                active_leaf_id: None,
            },
            cwd: None,
            transcript: Vec::new(),
            diagnostics: Vec::new(),
        };

        let outcome =
            CodingAgentOperationOutcome::from_internal(OperationOutcome::Export(ExportOutcome {
                export,
                path: Some(path.clone()),
            }));

        assert!(matches!(
            outcome,
            CodingAgentOperationOutcome::ExportHtml(projected) if projected == path
        ));
    }

    #[test]
    fn plugin_load_outcome_projects_non_empty_public_fields() {
        let projected = CodingAgentPluginLoadOutcome::from(PluginLoadOutcome {
            loaded_plugin_ids: vec!["plugin.loaded".into()],
            diagnostics: vec![PluginDiagnostic {
                plugin_id: Some("plugin.diagnostic".into()),
                message: "plugin diagnostic message".into(),
            }],
            capabilities: PluginCapabilities::new(),
            capability_changed: true,
        });

        assert_eq!(projected.loaded_plugin_ids, vec!["plugin.loaded"]);
        assert_eq!(
            projected.diagnostics,
            vec![CodingAgentPluginDiagnostic {
                plugin_id: Some("plugin.diagnostic".into()),
                message: "plugin diagnostic message".into(),
            }]
        );
        assert!(projected.capability_changed);
    }
}
