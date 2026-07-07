use super::operation_control::OperationKind;
use super::prompt::{PromptTurnOptions, PromptTurnOutcome};

#[derive(Debug)]
pub(crate) enum Operation {
    Prompt(PromptTurnOptions),
}

impl Operation {
    pub(crate) fn kind(&self) -> OperationKind {
        match self {
            Self::Prompt(_) => OperationKind::Prompt,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn origin(&self) -> OperationOrigin {
        match self {
            Self::Prompt(_) => OperationOrigin::ClientRoot,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn class(&self) -> OperationClass {
        match self {
            Self::Prompt(_) => OperationClass::SessionWriteRoot,
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
}

#[cfg(test)]
mod tests {
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
