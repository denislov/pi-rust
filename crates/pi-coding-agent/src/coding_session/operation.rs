use super::agent_invocation_flow::{AgentInvocationOptions, AgentInvocationOutcome};
use super::agent_team_flow::{AgentTeamOptions, AgentTeamOutcome};
use super::export_flow::{ExportOptions, ExportOutcome};
use super::operation_control::OperationKind;
use super::plugin_load_flow::{PluginLoadOptions, PluginLoadOutcome};
use super::profiles::ProfileId;
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
    RejectDelegationConfirmation {
        operation_id: String,
        tool_call_id: String,
        reason: String,
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
    SetDefaultAgentProfile {
        profile_id: ProfileId,
    },
    Export(ExportOptions),
}

impl Operation {
    pub(crate) fn kind(&self) -> OperationKind {
        self.static_kind()
            .expect("dynamic operation does not have a static kind")
    }

    pub(crate) fn static_kind(&self) -> Option<OperationKind> {
        self.metadata().static_kind
    }

    #[allow(dead_code)]
    pub(crate) fn origin(&self) -> OperationOrigin {
        self.metadata().origin
    }

    #[allow(dead_code)]
    pub(crate) fn class(&self) -> OperationClass {
        self.metadata().class
    }

    pub(crate) fn metadata(&self) -> OperationMetadata {
        match self {
            Self::Prompt(_) => OperationMetadata::new(
                Some(OperationKind::Prompt),
                OperationOrigin::ClientRoot,
                OperationClass::SessionWriteRoot,
                OperationDispatchMode::Async,
            ),
            Self::ManualCompaction(_) => OperationMetadata::new(
                Some(OperationKind::Compact),
                OperationOrigin::ClientRoot,
                OperationClass::SessionWriteRoot,
                OperationDispatchMode::Async,
            ),
            Self::PluginLoad(_) => OperationMetadata::new(
                Some(OperationKind::PluginLoad),
                OperationOrigin::ClientRoot,
                OperationClass::RuntimeWrite,
                OperationDispatchMode::Async,
            ),
            Self::PluginCommand { .. } => OperationMetadata::new(
                Some(OperationKind::PluginCommand),
                OperationOrigin::ClientRoot,
                OperationClass::NonSessionRoot,
                OperationDispatchMode::SyncReadOnly,
            ),
            Self::ApproveDelegationConfirmation { .. } => OperationMetadata::new(
                None,
                OperationOrigin::ClientRoot,
                OperationClass::NonSessionRoot,
                OperationDispatchMode::Async,
            ),
            Self::RejectDelegationConfirmation { .. } => OperationMetadata::new(
                Some(OperationKind::DelegationConfirmation),
                OperationOrigin::ClientRoot,
                OperationClass::Control,
                OperationDispatchMode::SyncMutable,
            ),
            Self::BranchSummary { .. } => OperationMetadata::new(
                Some(OperationKind::BranchSummary),
                OperationOrigin::ClientRoot,
                OperationClass::SessionWriteRoot,
                OperationDispatchMode::Async,
            ),
            Self::SelfHealingEdit(_) => OperationMetadata::new(
                Some(OperationKind::SelfHealingEdit),
                OperationOrigin::ClientRoot,
                OperationClass::SessionWriteRoot,
                OperationDispatchMode::Async,
            ),
            Self::AgentInvocation(_) => OperationMetadata::new(
                Some(OperationKind::AgentInvocation),
                OperationOrigin::ClientRoot,
                OperationClass::NonSessionRoot,
                OperationDispatchMode::Async,
            ),
            Self::AgentTeam(_) => OperationMetadata::new(
                Some(OperationKind::AgentTeam),
                OperationOrigin::ClientRoot,
                OperationClass::NonSessionRoot,
                OperationDispatchMode::Async,
            ),
            Self::SetDefaultAgentProfile { .. } => OperationMetadata::new(
                Some(OperationKind::SetDefaultAgentProfile),
                OperationOrigin::ClientRoot,
                OperationClass::RuntimeWrite,
                OperationDispatchMode::SyncMutable,
            ),
            Self::Export(_) => OperationMetadata::new(
                Some(OperationKind::Export),
                OperationOrigin::ClientRoot,
                OperationClass::ReadOnly,
                OperationDispatchMode::SyncReadOnly,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OperationMetadata {
    pub(crate) static_kind: Option<OperationKind>,
    pub(crate) origin: OperationOrigin,
    pub(crate) class: OperationClass,
    pub(crate) dispatch_mode: OperationDispatchMode,
}

impl OperationMetadata {
    fn new(
        static_kind: Option<OperationKind>,
        origin: OperationOrigin,
        class: OperationClass,
        dispatch_mode: OperationDispatchMode,
    ) -> Self {
        Self {
            static_kind,
            origin,
            class,
            dispatch_mode,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OperationAdmission {
    pub(crate) kind: OperationKind,
    pub(crate) metadata: OperationMetadata,
    pub(crate) admitted_at: Option<String>,
}

impl OperationAdmission {
    pub(crate) fn new(
        kind: OperationKind,
        metadata: OperationMetadata,
        admitted_at: Option<String>,
    ) -> Self {
        Self {
            kind,
            metadata,
            admitted_at,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationDispatchMode {
    Async,
    SyncReadOnly,
    SyncMutable,
}

impl OperationDispatchMode {
    pub(crate) fn dispatcher_label(self) -> &'static str {
        match self {
            Self::Async => "async",
            Self::SyncReadOnly => "read-only sync",
            Self::SyncMutable => "sync mutable",
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
    DelegationRejection,
    BranchSummary(PromptTurnOutcome),
    SelfHealingEdit(SelfHealingEditOutcome),
    AgentInvocation(AgentInvocationOutcome),
    AgentTeam(AgentTeamOutcome),
    SetDefaultAgentProfile,
    Export(ExportOutcome),
}

#[cfg(test)]
mod tests {
    use super::super::CodingSessionError;
    use super::super::intent_router::IntentRouter;
    use super::super::operation_control::OperationControl;
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
    fn operation_metadata_exposes_static_contract_and_dispatch_mode() {
        let operation = Operation::Export(ExportOptions::view());

        let metadata = operation.metadata();

        assert_eq!(metadata.static_kind, Some(OperationKind::Export));
        assert_eq!(metadata.origin, OperationOrigin::ClientRoot);
        assert_eq!(metadata.class, OperationClass::ReadOnly);
        assert_eq!(metadata.dispatch_mode, OperationDispatchMode::SyncReadOnly);
    }

    #[test]
    fn dynamic_operation_metadata_exposes_dispatch_without_static_kind() {
        let operation = Operation::ApproveDelegationConfirmation {
            operation_id: "op_parent".into(),
            tool_call_id: "tool_delegate".into(),
        };

        let metadata = operation.metadata();

        assert_eq!(metadata.static_kind, None);
        assert_eq!(metadata.origin, OperationOrigin::ClientRoot);
        assert_eq!(metadata.class, OperationClass::NonSessionRoot);
        assert_eq!(metadata.dispatch_mode, OperationDispatchMode::Async);
    }

    #[test]
    fn delegation_rejection_operation_declares_root_control_metadata() {
        let operation = Operation::RejectDelegationConfirmation {
            operation_id: "op_parent".into(),
            tool_call_id: "tool_delegate".into(),
            reason: "not needed".into(),
        };

        assert_eq!(operation.kind(), OperationKind::DelegationConfirmation);
        assert_eq!(operation.origin(), OperationOrigin::ClientRoot);
        assert_eq!(operation.class(), OperationClass::Control);
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
    fn set_default_agent_profile_operation_declares_runtime_write_metadata() {
        let profile_id = ProfileId::new("agent-main").expect("valid profile id");
        let operation = Operation::SetDefaultAgentProfile { profile_id };

        assert_eq!(operation.kind(), OperationKind::SetDefaultAgentProfile);
        assert_eq!(operation.origin(), OperationOrigin::ClientRoot);
        assert_eq!(operation.class(), OperationClass::RuntimeWrite);
        assert_eq!(
            operation.metadata().dispatch_mode,
            OperationDispatchMode::SyncMutable
        );
    }

    #[test]
    fn intent_router_rejects_dynamic_operation_without_owner_resolution() {
        let operation = Operation::ApproveDelegationConfirmation {
            operation_id: "op_parent".into(),
            tool_call_id: "tool_delegate".into(),
        };

        let error = IntentRouter::static_admission(&operation).unwrap_err();

        assert_eq!(
            error,
            CodingSessionError::UnsupportedCapability {
                capability: "dynamic operation requires async dispatcher".into(),
            }
        );
    }

    #[test]
    fn intent_router_validates_dispatch_mode_before_beginning_operation() {
        let operation = Operation::PluginCommand {
            command_id: "plugin.echo".into(),
            args: serde_json::json!({}),
        };
        let admission = IntentRouter::static_admission(&operation).unwrap();
        let control = OperationControl::new();

        let error =
            IntentRouter::begin(&control, &admission, OperationDispatchMode::Async).unwrap_err();

        assert_eq!(
            error,
            CodingSessionError::UnsupportedCapability {
                capability: "plugin_command operation requires read-only sync dispatcher".into(),
            }
        );
        assert_eq!(control.active(), None);
    }

    #[test]
    fn intent_router_begins_admitted_operation_and_uses_busy_guard() {
        let operation = Operation::PluginCommand {
            command_id: "plugin.echo".into(),
            args: serde_json::json!({}),
        };
        let admission = IntentRouter::static_admission(&operation).unwrap();
        let control = OperationControl::new();

        let guard =
            IntentRouter::begin(&control, &admission, OperationDispatchMode::SyncReadOnly).unwrap();

        assert_eq!(control.active(), Some(OperationKind::PluginCommand));
        assert_eq!(
            IntentRouter::begin(&control, &admission, OperationDispatchMode::SyncReadOnly)
                .unwrap_err(),
            CodingSessionError::Busy {
                operation: "plugin_command".into(),
            }
        );

        drop(guard);
        assert_eq!(control.active(), None);
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
