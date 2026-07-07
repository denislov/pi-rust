use super::agent_invocation_flow::{AgentInvocationOptions, AgentInvocationOutcome};
use super::agent_team_flow::{AgentTeamOptions, AgentTeamOutcome};
use super::export_flow::{ExportOptions, ExportOutcome};
use super::operation_control::OperationKind;
use super::plugin_load_flow::{PluginLoadOptions, PluginLoadOutcome};
use super::prompt::{PromptTurnOptions, PromptTurnOutcome};
use super::self_healing_edit_flow::{SelfHealingEditOutcome, SelfHealingEditRequest};

#[derive(Debug)]
pub(crate) enum Operation {
    Prompt(PromptTurnOptions),
    ManualCompaction(PromptTurnOptions),
    PluginLoad(PluginLoadOptions),
    PluginCommand {
        command_id: String,
        args: serde_json::Value,
    },
    ApproveDelegationConfirmation {
        operation_id: String,
        tool_call_id: String,
    },
    BranchSummary {
        options: PromptTurnOptions,
        source_leaf_id: String,
        target_leaf_id: String,
        custom_instructions: Option<String>,
    },
    SelfHealingEdit(SelfHealingEditRequest),
    AgentInvocation(AgentInvocationOptions),
    AgentTeam(AgentTeamOptions),
    Export(ExportOptions),
}

impl Operation {
    pub(crate) fn kind(&self) -> OperationKind {
        self.static_kind()
            .expect("dynamic operation does not have a static kind")
    }

    pub(crate) fn static_kind(&self) -> Option<OperationKind> {
        match self {
            Self::Prompt(_) => Some(OperationKind::Prompt),
            Self::ManualCompaction(_) => Some(OperationKind::Compact),
            Self::PluginLoad(_) => Some(OperationKind::PluginLoad),
            Self::PluginCommand { .. } => Some(OperationKind::PluginCommand),
            Self::ApproveDelegationConfirmation { .. } => None,
            Self::BranchSummary { .. } => Some(OperationKind::BranchSummary),
            Self::SelfHealingEdit(_) => Some(OperationKind::SelfHealingEdit),
            Self::AgentInvocation(_) => Some(OperationKind::AgentInvocation),
            Self::AgentTeam(_) => Some(OperationKind::AgentTeam),
            Self::Export(_) => Some(OperationKind::Export),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn origin(&self) -> OperationOrigin {
        match self {
            Self::Prompt(_)
            | Self::ManualCompaction(_)
            | Self::PluginLoad(_)
            | Self::PluginCommand { .. }
            | Self::ApproveDelegationConfirmation { .. }
            | Self::BranchSummary { .. }
            | Self::SelfHealingEdit(_)
            | Self::AgentInvocation(_)
            | Self::AgentTeam(_)
            | Self::Export(_) => OperationOrigin::ClientRoot,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn class(&self) -> OperationClass {
        match self {
            Self::Prompt(_)
            | Self::ManualCompaction(_)
            | Self::BranchSummary { .. }
            | Self::SelfHealingEdit(_) => OperationClass::SessionWriteRoot,
            Self::PluginLoad(_) => OperationClass::RuntimeWrite,
            Self::AgentInvocation(_)
            | Self::AgentTeam(_)
            | Self::PluginCommand { .. }
            | Self::ApproveDelegationConfirmation { .. } => OperationClass::NonSessionRoot,
            Self::Export(_) => OperationClass::ReadOnly,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationOrigin {
    ClientRoot,
    ParentChild,
    RuntimeInternal,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationClass {
    Query,
    ReadOnly,
    SessionWriteRoot,
    NonSessionRoot,
    RuntimeWrite,
    Child,
    Control,
}

#[derive(Debug)]
pub(crate) enum OperationOutcome {
    Prompt(PromptTurnOutcome),
    ManualCompaction(PromptTurnOutcome),
    PluginLoad(PluginLoadOutcome),
    PluginCommand(String),
    DelegationApproval,
    BranchSummary(PromptTurnOutcome),
    SelfHealingEdit(SelfHealingEditOutcome),
    AgentInvocation(AgentInvocationOutcome),
    AgentTeam(AgentTeamOutcome),
    Export(ExportOutcome),
}

#[cfg(test)]
mod tests {
    use super::super::plugin_load_flow::PluginLoadOptions;
    use super::super::self_healing_edit_flow::{
        SelfHealingEditReplacement, SelfHealingEditRequest,
    };
    use super::*;
    use crate::runtime::PromptInvocation;

    #[test]
    fn prompt_operation_declares_root_session_write_metadata() {
        let operation = Operation::Prompt(PromptTurnOptions::new(PromptInvocation::Text(
            "hello".into(),
        )));

        assert_eq!(operation.kind(), OperationKind::Prompt);
        assert_eq!(operation.origin(), OperationOrigin::ClientRoot);
        assert_eq!(operation.class(), OperationClass::SessionWriteRoot);
    }

    #[test]
    fn manual_compaction_operation_declares_root_session_write_metadata() {
        let operation =
            Operation::ManualCompaction(PromptTurnOptions::new(PromptInvocation::Compact {
                custom_instructions: None,
            }));

        assert_eq!(operation.kind(), OperationKind::Compact);
        assert_eq!(operation.origin(), OperationOrigin::ClientRoot);
        assert_eq!(operation.class(), OperationClass::SessionWriteRoot);
    }

    #[test]
    fn plugin_load_operation_declares_runtime_write_metadata() {
        let operation = Operation::PluginLoad(PluginLoadOptions::new());

        assert_eq!(operation.kind(), OperationKind::PluginLoad);
        assert_eq!(operation.origin(), OperationOrigin::ClientRoot);
        assert_eq!(operation.class(), OperationClass::RuntimeWrite);
    }

    #[test]
    fn plugin_command_operation_declares_root_non_session_metadata() {
        let operation = Operation::PluginCommand {
            command_id: "plugin.echo".into(),
            args: serde_json::Value::Null,
        };

        assert_eq!(operation.kind(), OperationKind::PluginCommand);
        assert_eq!(operation.origin(), OperationOrigin::ClientRoot);
        assert_eq!(operation.class(), OperationClass::NonSessionRoot);
    }

    #[test]
    fn delegation_approval_operation_declares_dynamic_root_non_session_metadata() {
        let operation = Operation::ApproveDelegationConfirmation {
            operation_id: "op_parent".into(),
            tool_call_id: "tool_delegate".into(),
        };

        assert_eq!(operation.static_kind(), None);
        assert_eq!(operation.origin(), OperationOrigin::ClientRoot);
        assert_eq!(operation.class(), OperationClass::NonSessionRoot);
    }

    #[test]
    fn branch_summary_operation_declares_root_session_write_metadata() {
        let operation = Operation::BranchSummary {
            options: PromptTurnOptions::new(PromptInvocation::Text("summarize".into())),
            source_leaf_id: "source_leaf".into(),
            target_leaf_id: "target_leaf".into(),
            custom_instructions: Some("keep details".into()),
        };

        assert_eq!(operation.kind(), OperationKind::BranchSummary);
        assert_eq!(operation.origin(), OperationOrigin::ClientRoot);
        assert_eq!(operation.class(), OperationClass::SessionWriteRoot);
    }

    #[test]
    fn self_healing_edit_operation_declares_root_session_write_metadata() {
        let operation = Operation::SelfHealingEdit(SelfHealingEditRequest::new(
            "src/lib.rs",
            vec![SelfHealingEditReplacement::new("old", "new")],
        ));

        assert_eq!(operation.kind(), OperationKind::SelfHealingEdit);
        assert_eq!(operation.origin(), OperationOrigin::ClientRoot);
        assert_eq!(operation.class(), OperationClass::SessionWriteRoot);
    }

    #[test]
    fn agent_invocation_operation_declares_root_non_session_metadata() {
        let operation = Operation::AgentInvocation(AgentInvocationOptions::new(
            "helper",
            "summarize this",
            PromptTurnOptions::new(PromptInvocation::Text("task".into())),
        ));

        assert_eq!(operation.kind(), OperationKind::AgentInvocation);
        assert_eq!(operation.origin(), OperationOrigin::ClientRoot);
        assert_eq!(operation.class(), OperationClass::NonSessionRoot);
    }

    #[test]
    fn agent_team_operation_declares_root_non_session_metadata() {
        let operation = Operation::AgentTeam(AgentTeamOptions::new(
            "team",
            "summarize this",
            PromptTurnOptions::new(PromptInvocation::Text("task".into())),
        ));

        assert_eq!(operation.kind(), OperationKind::AgentTeam);
        assert_eq!(operation.origin(), OperationOrigin::ClientRoot);
        assert_eq!(operation.class(), OperationClass::NonSessionRoot);
    }

    #[test]
    fn export_operation_declares_root_read_only_metadata() {
        let operation = Operation::Export(ExportOptions::view());

        assert_eq!(operation.kind(), OperationKind::Export);
        assert_eq!(operation.origin(), OperationOrigin::ClientRoot);
        assert_eq!(operation.class(), OperationClass::ReadOnly);
    }

    #[test]
    fn prompt_operation_outcome_exposes_prompt_payload() {
        let outcome = OperationOutcome::Prompt(PromptTurnOutcome::Aborted {
            operation_id: "op_test".into(),
            turn_id: Some("turn_test".into()),
            reason: "user cancelled".into(),
            session_id: None,
        });

        assert!(matches!(
            outcome,
            OperationOutcome::Prompt(PromptTurnOutcome::Aborted { reason, .. })
                if reason == "user cancelled"
        ));
    }
}
