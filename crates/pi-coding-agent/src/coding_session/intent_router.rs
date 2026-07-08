use super::CodingSessionError;
use super::operation::{Operation, OperationAdmission, OperationClass, OperationDispatchMode};
use super::operation_control::{
    OperationControl, OperationGuard, OperationKind, PromptControlHandle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControlIntent {
    PromptControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ControlIntentMetadata {
    pub(crate) operation_kind: OperationKind,
    pub(crate) class: OperationClass,
}

impl ControlIntent {
    pub(crate) fn metadata(self) -> ControlIntentMetadata {
        match self {
            Self::PromptControl => ControlIntentMetadata {
                operation_kind: OperationKind::Prompt,
                class: OperationClass::Control,
            },
        }
    }
}

pub(crate) struct IntentRouter;

impl IntentRouter {
    pub(crate) fn static_admission(
        operation: &Operation,
    ) -> Result<OperationAdmission, CodingSessionError> {
        let metadata = operation.metadata();
        if operation.static_kind().is_none() {
            return Err(CodingSessionError::UnsupportedCapability {
                capability: "dynamic operation requires async dispatcher".into(),
            });
        }
        Ok(OperationAdmission::new(operation.kind(), metadata, None))
    }

    pub(crate) fn begin(
        control: &OperationControl,
        admission: &OperationAdmission,
        expected: OperationDispatchMode,
    ) -> Result<OperationGuard, CodingSessionError> {
        if admission.metadata.dispatch_mode != expected {
            return Err(Self::unsupported_dispatch(admission));
        }

        control.begin(admission.kind)
    }

    pub(crate) fn prompt_control_handle(
        control: &mut OperationControl,
        intent: ControlIntent,
    ) -> Result<PromptControlHandle, CodingSessionError> {
        let metadata = intent.metadata();
        debug_assert_eq!(metadata.class, OperationClass::Control);
        match metadata.operation_kind {
            OperationKind::Prompt => control.prompt_control_handle(),
            _ => unreachable!("unsupported control intent target"),
        }
    }

    pub(crate) fn unsupported_dispatch(admission: &OperationAdmission) -> CodingSessionError {
        CodingSessionError::UnsupportedCapability {
            capability: format!(
                "{} operation requires {} dispatcher",
                admission.kind.as_str(),
                admission.metadata.dispatch_mode.dispatcher_label(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::operation::OperationClass;
    use crate::coding_session::operation_control::{OperationKind, PromptControlCommand};

    #[test]
    fn control_intent_prompt_control_is_classified_as_control() {
        let metadata = ControlIntent::PromptControl.metadata();

        assert_eq!(metadata.operation_kind, OperationKind::Prompt);
        assert_eq!(metadata.class, OperationClass::Control);
    }

    #[test]
    fn intent_router_prompt_control_handle_rejects_busy_or_pending_receiver() {
        let mut control = OperationControl::new();
        let guard = control.begin(OperationKind::PluginLoad).unwrap();

        assert_eq!(
            IntentRouter::prompt_control_handle(&mut control, ControlIntent::PromptControl)
                .unwrap_err(),
            CodingSessionError::Busy {
                operation: "plugin_load".into(),
            }
        );
        drop(guard);

        let _handle =
            IntentRouter::prompt_control_handle(&mut control, ControlIntent::PromptControl)
                .unwrap();
        assert_eq!(
            IntentRouter::prompt_control_handle(&mut control, ControlIntent::PromptControl)
                .unwrap_err(),
            CodingSessionError::Busy {
                operation: "prompt_control".into(),
            }
        );
    }

    #[test]
    fn intent_router_prompt_control_handle_preserves_prompt_control_commands() {
        let mut control = OperationControl::new();

        let handle =
            IntentRouter::prompt_control_handle(&mut control, ControlIntent::PromptControl)
                .unwrap();
        handle.abort("stop").unwrap();
        handle.steer("focus").unwrap();
        handle.follow_up("continue").unwrap();

        let mut receiver = control
            .take_prompt_control_receiver()
            .expect("router should leave receiver owned by operation control");
        assert_eq!(
            receiver.try_recv().unwrap(),
            PromptControlCommand::Abort {
                reason: "stop".into(),
            }
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            PromptControlCommand::Steer {
                text: "focus".into(),
            }
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            PromptControlCommand::FollowUp {
                text: "continue".into(),
            }
        );
    }
}
