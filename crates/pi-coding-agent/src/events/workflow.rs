use super::emission::ProductEventDraft;
use super::{
    CodingAgentProductEventCheckOutput, CodingAgentProductEventDiagnostic,
    CodingAgentProductEventDurability, CodingAgentProductEventError, CodingAgentProductEventKind,
    CodingAgentProductEventReplacement, CodingAgentProductEventTerminalStatus,
    CodingAgentWorkflowProductEvent,
};
use crate::operations::self_healing_edit::runner::{
    SelfHealingEditCheckOutput, SelfHealingEditDiagnostic, SelfHealingEditReplacement,
};
use crate::runtime::facade::CodingSessionError;
use crate::runtime::outcome::OperationRootTerminalEvidence;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SelfHealingEditEvent {
    Started {
        operation_id: String,
        path: String,
        replacements: usize,
    },
    RepairAttempted {
        operation_id: String,
        path: String,
        attempt: usize,
        replacements: Vec<SelfHealingEditReplacement>,
        diagnostics: Vec<SelfHealingEditDiagnostic>,
        check_output: Option<SelfHealingEditCheckOutput>,
    },
    Completed {
        operation_id: String,
        path: String,
        attempts: usize,
        first_changed_line: Option<usize>,
        check_output: Option<SelfHealingEditCheckOutput>,
    },
    Failed {
        operation_id: String,
        path: String,
        error: CodingSessionError,
    },
    Aborted {
        operation_id: String,
        path: String,
        reason: String,
    },
}

impl SelfHealingEditEvent {
    pub(crate) fn root_terminal_evidence(&self) -> Option<OperationRootTerminalEvidence> {
        match self {
            Self::Completed { .. } => Some(OperationRootTerminalEvidence::SelfHealingEditCompleted),
            Self::Failed { .. } => Some(OperationRootTerminalEvidence::SelfHealingEditFailed),
            Self::Aborted { .. } => Some(OperationRootTerminalEvidence::SelfHealingEditAborted),
            Self::Started { .. } | Self::RepairAttempted { .. } => None,
        }
    }

    pub(crate) fn into_product_draft(self) -> ProductEventDraft {
        let (event, operation_id, terminal_status) = match self {
            Self::Started {
                operation_id,
                path,
                replacements,
            } => (
                CodingAgentWorkflowProductEvent::SelfHealingEditStarted {
                    operation_id: operation_id.clone(),
                    path,
                    replacements,
                },
                operation_id,
                None,
            ),
            Self::RepairAttempted {
                operation_id,
                path,
                attempt,
                replacements,
                diagnostics,
                check_output,
            } => (
                CodingAgentWorkflowProductEvent::SelfHealingEditRepairAttempted {
                    operation_id: operation_id.clone(),
                    path,
                    attempt,
                    replacements: replacements
                        .into_iter()
                        .map(|replacement| CodingAgentProductEventReplacement {
                            old_text: replacement.old_text,
                            new_text: replacement.new_text,
                        })
                        .collect(),
                    diagnostics: diagnostics
                        .into_iter()
                        .map(|diagnostic| CodingAgentProductEventDiagnostic {
                            message: diagnostic.message,
                        })
                        .collect(),
                    check_output: check_output.map(product_check_output),
                },
                operation_id,
                None,
            ),
            Self::Completed {
                operation_id,
                path,
                attempts,
                first_changed_line,
                check_output,
            } => (
                CodingAgentWorkflowProductEvent::SelfHealingEditCompleted {
                    operation_id: operation_id.clone(),
                    path,
                    attempts,
                    first_changed_line,
                    check_output: check_output.map(product_check_output),
                },
                operation_id,
                Some(CodingAgentProductEventTerminalStatus::Completed),
            ),
            Self::Failed {
                operation_id,
                path,
                error,
            } => (
                CodingAgentWorkflowProductEvent::SelfHealingEditFailed {
                    operation_id: operation_id.clone(),
                    path,
                    error: CodingAgentProductEventError {
                        code: error.code().to_owned(),
                        message: error.to_string(),
                    },
                },
                operation_id,
                Some(CodingAgentProductEventTerminalStatus::Failed),
            ),
            Self::Aborted {
                operation_id,
                path,
                reason,
            } => (
                CodingAgentWorkflowProductEvent::SelfHealingEditAborted {
                    operation_id: operation_id.clone(),
                    path,
                    reason,
                },
                operation_id,
                Some(CodingAgentProductEventTerminalStatus::Aborted),
            ),
        };
        ProductEventDraft {
            event: CodingAgentProductEventKind::Workflow(event),
            operation_id: Some(operation_id),
            session_id: None,
            terminal_status,
            durability: CodingAgentProductEventDurability::LiveOnly,
        }
    }
}

fn product_check_output(value: SelfHealingEditCheckOutput) -> CodingAgentProductEventCheckOutput {
    CodingAgentProductEventCheckOutput {
        command: value.command,
        stdout: value.stdout,
        stderr: value.stderr,
        exit_code: value.exit_code,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PluginLoadEvent {
    Completed {
        operation_id: String,
    },
    Failed {
        operation_id: String,
        error: CodingSessionError,
    },
    Aborted {
        operation_id: String,
        reason: String,
    },
}

impl PluginLoadEvent {
    #[cfg(test)]
    pub(crate) fn root_terminal_evidence(&self) -> Option<OperationRootTerminalEvidence> {
        match self {
            Self::Completed { .. } => Some(OperationRootTerminalEvidence::PluginLoadCompleted),
            Self::Failed { .. } => Some(OperationRootTerminalEvidence::PluginLoadFailed),
            Self::Aborted { .. } => Some(OperationRootTerminalEvidence::PluginLoadAborted),
        }
    }

    pub(crate) fn into_product_draft(self) -> ProductEventDraft {
        let (event, operation_id, terminal_status) = match self {
            Self::Completed { operation_id } => (
                CodingAgentWorkflowProductEvent::PluginLoadCompleted {
                    operation_id: operation_id.clone(),
                },
                operation_id,
                Some(CodingAgentProductEventTerminalStatus::Completed),
            ),
            Self::Failed {
                operation_id,
                error,
            } => (
                CodingAgentWorkflowProductEvent::PluginLoadFailed {
                    operation_id: operation_id.clone(),
                    error: CodingAgentProductEventError {
                        code: error.code().to_owned(),
                        message: error.to_string(),
                    },
                },
                operation_id,
                Some(CodingAgentProductEventTerminalStatus::Failed),
            ),
            Self::Aborted {
                operation_id,
                reason,
            } => (
                CodingAgentWorkflowProductEvent::PluginLoadAborted {
                    operation_id: operation_id.clone(),
                    reason,
                },
                operation_id,
                Some(CodingAgentProductEventTerminalStatus::Aborted),
            ),
        };
        ProductEventDraft {
            event: CodingAgentProductEventKind::Workflow(event),
            operation_id: Some(operation_id),
            session_id: None,
            terminal_status,
            durability: CodingAgentProductEventDurability::LiveOnly,
        }
    }
}
