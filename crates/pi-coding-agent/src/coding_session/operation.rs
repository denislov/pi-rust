use super::operation_control::OperationKind;
use super::plugin_load_flow::{PluginLoadOptions, PluginLoadOutcome};
use super::prompt::{PromptTurnOptions, PromptTurnOutcome};

#[derive(Debug)]
pub(crate) enum Operation {
    Prompt(PromptTurnOptions),
    ManualCompaction(PromptTurnOptions),
    PluginLoad(PluginLoadOptions),
    BranchSummary {
        options: PromptTurnOptions,
        source_leaf_id: String,
        target_leaf_id: String,
        custom_instructions: Option<String>,
    },
}

impl Operation {
    pub(crate) fn kind(&self) -> OperationKind {
        match self {
            Self::Prompt(_) => OperationKind::Prompt,
            Self::ManualCompaction(_) => OperationKind::Compact,
            Self::PluginLoad(_) => OperationKind::PluginLoad,
            Self::BranchSummary { .. } => OperationKind::BranchSummary,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn origin(&self) -> OperationOrigin {
        match self {
            Self::Prompt(_)
            | Self::ManualCompaction(_)
            | Self::PluginLoad(_)
            | Self::BranchSummary { .. } => OperationOrigin::ClientRoot,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn class(&self) -> OperationClass {
        match self {
            Self::Prompt(_) | Self::ManualCompaction(_) | Self::BranchSummary { .. } => {
                OperationClass::SessionWriteRoot
            }
            Self::PluginLoad(_) => OperationClass::RuntimeWrite,
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
    BranchSummary(PromptTurnOutcome),
}

#[cfg(test)]
mod tests {
    use super::super::plugin_load_flow::PluginLoadOptions;
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
