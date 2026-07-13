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
    pub(crate) fn submission_fingerprint(&self) -> Option<(&'static str, String)> {
        match self {
            Self::Prompt(options) => match options.invocation() {
                crate::runtime::PromptInvocation::Text(text) => Some(("prompt", text.clone())),
                _ => None,
            },
            _ => None,
        }
    }

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
    use std::collections::HashSet;

    use super::*;
    use crate::coding_session::context::CodingAgentSessionSummary;
    use crate::coding_session::export_flow::ExportOutcome;
    use crate::coding_session::operation::OperationDispatchMode;
    use crate::coding_session::plugin_service::PluginDiagnostic;
    use crate::plugins::PluginCapabilities;
    use crate::runtime::PromptInvocation;
    use pi_ai::types::AssistantMessage;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ExpectedInternalOperationVariant {
        Prompt,
        ManualCompaction,
        BranchSummary,
        SelfHealingEdit,
        AgentInvocation,
        AgentTeam,
        PluginLoad,
        PluginCommand,
        SetDefaultAgentProfile,
        ApproveDelegationConfirmation,
        RejectDelegationConfirmation,
        ForkSession,
        SwitchActiveLeaf,
        ExportView,
        ExportHtml,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum ExpectedPublicOutcomeFamily {
        Prompt,
        Compact,
        BranchSummary,
        SelfHealingEdit,
        AgentInvocation,
        AgentTeam,
        PluginLoad,
        PluginCommand,
        DefaultAgentProfileChanged,
        DelegationApproved,
        DelegationRejected,
        SessionForked,
        ActiveLeafSwitched,
        Export,
        ExportHtml,
    }

    struct OperationContractCase {
        public_variant: &'static str,
        build_operation: fn() -> CodingAgentOperation,
        expected_internal: ExpectedInternalOperationVariant,
        expected_dispatch: OperationDispatchMode,
        expected_outcome: ExpectedPublicOutcomeFamily,
    }

    struct OutcomeProjectionCase {
        internal_outcome: &'static str,
        build_outcome: fn() -> OperationOutcome,
        expected_outcome: ExpectedPublicOutcomeFamily,
    }

    fn prompt_operation_options() -> PromptTurnOptions {
        PromptTurnOptions::new(PromptInvocation::Text("contract".into()))
    }

    fn operation_contract_cases() -> [OperationContractCase; 15] {
        [
            OperationContractCase {
                public_variant: "Prompt",
                build_operation: || CodingAgentOperation::Prompt(prompt_operation_options()),
                expected_internal: ExpectedInternalOperationVariant::Prompt,
                expected_dispatch: OperationDispatchMode::Async,
                expected_outcome: ExpectedPublicOutcomeFamily::Prompt,
            },
            OperationContractCase {
                public_variant: "Compact",
                build_operation: || CodingAgentOperation::Compact(prompt_operation_options()),
                expected_internal: ExpectedInternalOperationVariant::ManualCompaction,
                expected_dispatch: OperationDispatchMode::Async,
                expected_outcome: ExpectedPublicOutcomeFamily::Compact,
            },
            OperationContractCase {
                public_variant: "BranchSummary",
                build_operation: || CodingAgentOperation::BranchSummary {
                    options: prompt_operation_options(),
                    source_leaf_id: "leaf_source".into(),
                    target_leaf_id: "leaf_target".into(),
                    custom_instructions: Some("contract instructions".into()),
                    reuse: BranchSummaryReusePolicy::ReuseExisting,
                },
                expected_internal: ExpectedInternalOperationVariant::BranchSummary,
                expected_dispatch: OperationDispatchMode::Async,
                expected_outcome: ExpectedPublicOutcomeFamily::BranchSummary,
            },
            OperationContractCase {
                public_variant: "SelfHealingEdit",
                build_operation: || {
                    CodingAgentOperation::SelfHealingEdit(SelfHealingEditRequest::new(
                        "src/lib.rs",
                        Vec::new(),
                    ))
                },
                expected_internal: ExpectedInternalOperationVariant::SelfHealingEdit,
                expected_dispatch: OperationDispatchMode::Async,
                expected_outcome: ExpectedPublicOutcomeFamily::SelfHealingEdit,
            },
            OperationContractCase {
                public_variant: "InvokeAgent",
                build_operation: || {
                    CodingAgentOperation::InvokeAgent(AgentInvocationOptions::new(
                        "agent",
                        "task",
                        prompt_operation_options(),
                    ))
                },
                expected_internal: ExpectedInternalOperationVariant::AgentInvocation,
                expected_dispatch: OperationDispatchMode::Async,
                expected_outcome: ExpectedPublicOutcomeFamily::AgentInvocation,
            },
            OperationContractCase {
                public_variant: "InvokeTeam",
                build_operation: || {
                    CodingAgentOperation::InvokeTeam(AgentTeamOptions::new(
                        "team",
                        "task",
                        prompt_operation_options(),
                    ))
                },
                expected_internal: ExpectedInternalOperationVariant::AgentTeam,
                expected_dispatch: OperationDispatchMode::Async,
                expected_outcome: ExpectedPublicOutcomeFamily::AgentTeam,
            },
            OperationContractCase {
                public_variant: "PluginLoad",
                build_operation: || CodingAgentOperation::PluginLoad,
                expected_internal: ExpectedInternalOperationVariant::PluginLoad,
                expected_dispatch: OperationDispatchMode::Async,
                expected_outcome: ExpectedPublicOutcomeFamily::PluginLoad,
            },
            OperationContractCase {
                public_variant: "PluginCommand",
                build_operation: || CodingAgentOperation::PluginCommand {
                    command_id: "plugin.command".into(),
                    args: serde_json::json!({"contract": true}),
                },
                expected_internal: ExpectedInternalOperationVariant::PluginCommand,
                expected_dispatch: OperationDispatchMode::SyncReadOnly,
                expected_outcome: ExpectedPublicOutcomeFamily::PluginCommand,
            },
            OperationContractCase {
                public_variant: "SetDefaultAgentProfile",
                build_operation: || CodingAgentOperation::SetDefaultAgentProfile {
                    profile_id: ProfileId::from("reviewer"),
                },
                expected_internal: ExpectedInternalOperationVariant::SetDefaultAgentProfile,
                expected_dispatch: OperationDispatchMode::SyncMutable,
                expected_outcome: ExpectedPublicOutcomeFamily::DefaultAgentProfileChanged,
            },
            OperationContractCase {
                public_variant: "ApproveDelegation",
                build_operation: || CodingAgentOperation::ApproveDelegation {
                    operation_id: "op_parent".into(),
                    tool_call_id: "tool_delegate".into(),
                },
                expected_internal: ExpectedInternalOperationVariant::ApproveDelegationConfirmation,
                expected_dispatch: OperationDispatchMode::Async,
                expected_outcome: ExpectedPublicOutcomeFamily::DelegationApproved,
            },
            OperationContractCase {
                public_variant: "RejectDelegation",
                build_operation: || CodingAgentOperation::RejectDelegation {
                    operation_id: "op_parent".into(),
                    tool_call_id: "tool_delegate".into(),
                    reason: "not now".into(),
                },
                expected_internal: ExpectedInternalOperationVariant::RejectDelegationConfirmation,
                expected_dispatch: OperationDispatchMode::SyncMutable,
                expected_outcome: ExpectedPublicOutcomeFamily::DelegationRejected,
            },
            OperationContractCase {
                public_variant: "ForkSession",
                build_operation: || CodingAgentOperation::ForkSession {
                    target_leaf_id: Some("leaf_target".into()),
                },
                expected_internal: ExpectedInternalOperationVariant::ForkSession,
                expected_dispatch: OperationDispatchMode::SyncMutable,
                expected_outcome: ExpectedPublicOutcomeFamily::SessionForked,
            },
            OperationContractCase {
                public_variant: "SwitchActiveLeaf",
                build_operation: || CodingAgentOperation::SwitchActiveLeaf {
                    target_leaf_id: "leaf_target".into(),
                },
                expected_internal: ExpectedInternalOperationVariant::SwitchActiveLeaf,
                expected_dispatch: OperationDispatchMode::SyncMutable,
                expected_outcome: ExpectedPublicOutcomeFamily::ActiveLeafSwitched,
            },
            OperationContractCase {
                public_variant: "ExportCurrent",
                build_operation: || CodingAgentOperation::ExportCurrent,
                expected_internal: ExpectedInternalOperationVariant::ExportView,
                expected_dispatch: OperationDispatchMode::SyncReadOnly,
                expected_outcome: ExpectedPublicOutcomeFamily::Export,
            },
            OperationContractCase {
                public_variant: "ExportCurrentHtml",
                build_operation: || {
                    CodingAgentOperation::ExportCurrentHtml(PathBuf::from("session.html"))
                },
                expected_internal: ExpectedInternalOperationVariant::ExportHtml,
                expected_dispatch: OperationDispatchMode::SyncReadOnly,
                expected_outcome: ExpectedPublicOutcomeFamily::ExportHtml,
            },
        ]
    }

    fn prompt_outcome_fixture() -> PromptTurnOutcome {
        PromptTurnOutcome::Aborted {
            operation_id: "op_contract".into(),
            turn_id: Some("turn_contract".into()),
            reason: "fixture".into(),
            session_id: Some("sess_contract".into()),
        }
    }

    fn self_healing_outcome_fixture() -> SelfHealingEditOutcome {
        SelfHealingEditOutcome {
            path: "src/lib.rs".into(),
            message: "updated".into(),
            diff: "diff".into(),
            patch: "patch".into(),
            first_changed_line: Some(1),
            attempts: 1,
            diagnostics: Vec::new(),
            check_output: None,
            repair_attempts: Vec::new(),
        }
    }

    fn agent_invocation_outcome_fixture() -> AgentInvocationOutcome {
        AgentInvocationOutcome {
            operation_id: "op_agent".into(),
            child_operation_id: "op_child".into(),
            turn_id: "turn_agent".into(),
            profile_id: ProfileId::from("agent"),
            final_text: "agent result".into(),
            final_message: AssistantMessage::empty("test", "test-model"),
            diagnostics: Vec::new(),
        }
    }

    fn agent_team_outcome_fixture() -> AgentTeamOutcome {
        AgentTeamOutcome {
            operation_id: "op_team".into(),
            team_id: ProfileId::from("team"),
            final_text: "team result".into(),
            member_results: Vec::new(),
            supervisor_result: None,
            diagnostics: Vec::new(),
        }
    }

    fn plugin_load_outcome_fixture() -> PluginLoadOutcome {
        PluginLoadOutcome {
            loaded_plugin_ids: vec!["plugin.contract".into()],
            diagnostics: Vec::new(),
            capabilities: PluginCapabilities::new(),
            capability_changed: true,
        }
    }

    fn export_fixture() -> CodingAgentSessionExport {
        CodingAgentSessionExport {
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
        }
    }

    fn operation_outcome_projection_cases() -> [OutcomeProjectionCase; 15] {
        [
            OutcomeProjectionCase {
                internal_outcome: "Prompt",
                build_outcome: || OperationOutcome::Prompt(prompt_outcome_fixture()),
                expected_outcome: ExpectedPublicOutcomeFamily::Prompt,
            },
            OutcomeProjectionCase {
                internal_outcome: "ManualCompaction",
                build_outcome: || OperationOutcome::ManualCompaction(prompt_outcome_fixture()),
                expected_outcome: ExpectedPublicOutcomeFamily::Compact,
            },
            OutcomeProjectionCase {
                internal_outcome: "BranchSummary",
                build_outcome: || OperationOutcome::BranchSummary(prompt_outcome_fixture()),
                expected_outcome: ExpectedPublicOutcomeFamily::BranchSummary,
            },
            OutcomeProjectionCase {
                internal_outcome: "SelfHealingEdit",
                build_outcome: || OperationOutcome::SelfHealingEdit(self_healing_outcome_fixture()),
                expected_outcome: ExpectedPublicOutcomeFamily::SelfHealingEdit,
            },
            OutcomeProjectionCase {
                internal_outcome: "AgentInvocation",
                build_outcome: || {
                    OperationOutcome::AgentInvocation(agent_invocation_outcome_fixture())
                },
                expected_outcome: ExpectedPublicOutcomeFamily::AgentInvocation,
            },
            OutcomeProjectionCase {
                internal_outcome: "AgentTeam",
                build_outcome: || OperationOutcome::AgentTeam(agent_team_outcome_fixture()),
                expected_outcome: ExpectedPublicOutcomeFamily::AgentTeam,
            },
            OutcomeProjectionCase {
                internal_outcome: "PluginLoad",
                build_outcome: || OperationOutcome::PluginLoad(plugin_load_outcome_fixture()),
                expected_outcome: ExpectedPublicOutcomeFamily::PluginLoad,
            },
            OutcomeProjectionCase {
                internal_outcome: "PluginCommand",
                build_outcome: || OperationOutcome::PluginCommand("command output".into()),
                expected_outcome: ExpectedPublicOutcomeFamily::PluginCommand,
            },
            OutcomeProjectionCase {
                internal_outcome: "SetDefaultAgentProfile",
                build_outcome: || OperationOutcome::SetDefaultAgentProfile,
                expected_outcome: ExpectedPublicOutcomeFamily::DefaultAgentProfileChanged,
            },
            OutcomeProjectionCase {
                internal_outcome: "DelegationApproval",
                build_outcome: || OperationOutcome::DelegationApproval,
                expected_outcome: ExpectedPublicOutcomeFamily::DelegationApproved,
            },
            OutcomeProjectionCase {
                internal_outcome: "DelegationRejection",
                build_outcome: || OperationOutcome::DelegationRejection,
                expected_outcome: ExpectedPublicOutcomeFamily::DelegationRejected,
            },
            OutcomeProjectionCase {
                internal_outcome: "ForkSession",
                build_outcome: || OperationOutcome::ForkSession,
                expected_outcome: ExpectedPublicOutcomeFamily::SessionForked,
            },
            OutcomeProjectionCase {
                internal_outcome: "SwitchActiveLeaf",
                build_outcome: || OperationOutcome::SwitchActiveLeaf,
                expected_outcome: ExpectedPublicOutcomeFamily::ActiveLeafSwitched,
            },
            OutcomeProjectionCase {
                internal_outcome: "Export(view)",
                build_outcome: || {
                    OperationOutcome::Export(ExportOutcome {
                        export: export_fixture(),
                        path: None,
                    })
                },
                expected_outcome: ExpectedPublicOutcomeFamily::Export,
            },
            OutcomeProjectionCase {
                internal_outcome: "Export(html)",
                build_outcome: || {
                    OperationOutcome::Export(ExportOutcome {
                        export: export_fixture(),
                        path: Some(PathBuf::from("session.html")),
                    })
                },
                expected_outcome: ExpectedPublicOutcomeFamily::ExportHtml,
            },
        ]
    }

    fn internal_operation_variant(operation: &Operation) -> ExpectedInternalOperationVariant {
        match operation {
            Operation::Prompt(_) => ExpectedInternalOperationVariant::Prompt,
            Operation::ManualCompaction(_) => ExpectedInternalOperationVariant::ManualCompaction,
            Operation::PluginLoad(_) => ExpectedInternalOperationVariant::PluginLoad,
            Operation::PluginCommand { .. } => ExpectedInternalOperationVariant::PluginCommand,
            Operation::ApproveDelegationConfirmation { .. } => {
                ExpectedInternalOperationVariant::ApproveDelegationConfirmation
            }
            Operation::RejectDelegationConfirmation { .. } => {
                ExpectedInternalOperationVariant::RejectDelegationConfirmation
            }
            Operation::BranchSummary { .. } => ExpectedInternalOperationVariant::BranchSummary,
            Operation::SelfHealingEdit(_) => ExpectedInternalOperationVariant::SelfHealingEdit,
            Operation::AgentInvocation(_) => ExpectedInternalOperationVariant::AgentInvocation,
            Operation::AgentTeam(_) => ExpectedInternalOperationVariant::AgentTeam,
            Operation::ForkSession { .. } => ExpectedInternalOperationVariant::ForkSession,
            Operation::SwitchActiveLeaf { .. } => {
                ExpectedInternalOperationVariant::SwitchActiveLeaf
            }
            Operation::SetDefaultAgentProfile { .. } => {
                ExpectedInternalOperationVariant::SetDefaultAgentProfile
            }
            Operation::Export(options) => {
                if options == &ExportOptions::view() {
                    ExpectedInternalOperationVariant::ExportView
                } else if options == &ExportOptions::html("session.html") {
                    ExpectedInternalOperationVariant::ExportHtml
                } else {
                    panic!("unexpected export options in operation contract: {options:?}")
                }
            }
        }
    }

    fn public_outcome_family(outcome: &CodingAgentOperationOutcome) -> ExpectedPublicOutcomeFamily {
        match outcome {
            CodingAgentOperationOutcome::Prompt(_) => ExpectedPublicOutcomeFamily::Prompt,
            CodingAgentOperationOutcome::Compact(_) => ExpectedPublicOutcomeFamily::Compact,
            CodingAgentOperationOutcome::BranchSummary(_) => {
                ExpectedPublicOutcomeFamily::BranchSummary
            }
            CodingAgentOperationOutcome::SelfHealingEdit(_) => {
                ExpectedPublicOutcomeFamily::SelfHealingEdit
            }
            CodingAgentOperationOutcome::AgentInvocation(_) => {
                ExpectedPublicOutcomeFamily::AgentInvocation
            }
            CodingAgentOperationOutcome::AgentTeam(_) => ExpectedPublicOutcomeFamily::AgentTeam,
            CodingAgentOperationOutcome::PluginLoad(_) => ExpectedPublicOutcomeFamily::PluginLoad,
            CodingAgentOperationOutcome::PluginCommand(_) => {
                ExpectedPublicOutcomeFamily::PluginCommand
            }
            CodingAgentOperationOutcome::DefaultAgentProfileChanged => {
                ExpectedPublicOutcomeFamily::DefaultAgentProfileChanged
            }
            CodingAgentOperationOutcome::DelegationApproved => {
                ExpectedPublicOutcomeFamily::DelegationApproved
            }
            CodingAgentOperationOutcome::DelegationRejected => {
                ExpectedPublicOutcomeFamily::DelegationRejected
            }
            CodingAgentOperationOutcome::SessionForked => {
                ExpectedPublicOutcomeFamily::SessionForked
            }
            CodingAgentOperationOutcome::ActiveLeafSwitched => {
                ExpectedPublicOutcomeFamily::ActiveLeafSwitched
            }
            CodingAgentOperationOutcome::Export(_) => ExpectedPublicOutcomeFamily::Export,
            CodingAgentOperationOutcome::ExportHtml(_) => ExpectedPublicOutcomeFamily::ExportHtml,
        }
    }

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
    fn operation_contract_covers_all_public_variants() {
        let cases = operation_contract_cases();

        assert_eq!(cases.len(), 15);
        assert_eq!(
            cases
                .iter()
                .map(|case| case.expected_outcome)
                .collect::<HashSet<_>>()
                .len(),
            15
        );
        for case in &cases {
            let operation = (case.build_operation)().into_internal(PluginLoadOptions::new());
            assert_eq!(
                internal_operation_variant(&operation),
                case.expected_internal,
                "{} internal variant",
                case.public_variant
            );
            assert_eq!(
                operation.metadata().dispatch_mode,
                case.expected_dispatch,
                "{} dispatch mode",
                case.public_variant
            );
        }
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
    fn operation_outcome_projection_covers_all_families() {
        let cases = operation_outcome_projection_cases();
        let contract_outcomes = operation_contract_cases()
            .iter()
            .map(|case| case.expected_outcome)
            .collect::<HashSet<_>>();
        let projection_outcomes = cases
            .iter()
            .map(|case| case.expected_outcome)
            .collect::<HashSet<_>>();

        assert_eq!(cases.len(), 15);
        assert_eq!(projection_outcomes, contract_outcomes);
        for case in cases {
            let projected = CodingAgentOperationOutcome::from_internal((case.build_outcome)());
            assert_eq!(
                public_outcome_family(&projected),
                case.expected_outcome,
                "{} projection",
                case.internal_outcome
            );
        }
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
