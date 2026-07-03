use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use super::CodingSessionError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationKind {
    Prompt,
    Compact,
    PluginCommand,
    PluginLoad,
    BranchSummary,
    AgentInvocation,
    Export,
}

impl OperationKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Prompt => "prompt",
            Self::Compact => "compact",
            Self::PluginCommand => "plugin_command",
            Self::PluginLoad => "plugin_load",
            Self::BranchSummary => "branch_summary",
            Self::AgentInvocation => "agent_invocation",
            Self::Export => "export",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PromptControlCommand {
    Abort { reason: String },
    Steer { text: String },
    FollowUp { text: String },
}

pub(crate) type PromptControlReceiver = mpsc::UnboundedReceiver<PromptControlCommand>;

#[derive(Debug, Clone)]
pub(crate) struct PromptControlHandle {
    sender: mpsc::UnboundedSender<PromptControlCommand>,
}

impl PromptControlHandle {
    pub(crate) fn abort(&self, reason: impl Into<String>) -> Result<(), CodingSessionError> {
        self.send(PromptControlCommand::Abort {
            reason: reason.into(),
        })
    }

    pub(crate) fn steer(&self, text: impl Into<String>) -> Result<(), CodingSessionError> {
        self.send(PromptControlCommand::Steer { text: text.into() })
    }

    pub(crate) fn follow_up(&self, text: impl Into<String>) -> Result<(), CodingSessionError> {
        self.send(PromptControlCommand::FollowUp { text: text.into() })
    }

    fn send(&self, command: PromptControlCommand) -> Result<(), CodingSessionError> {
        self.sender
            .send(command)
            .map_err(|_| CodingSessionError::Session {
                message: "prompt control receiver is closed".into(),
            })
    }
}

pub(crate) fn prompt_control_channel() -> (PromptControlHandle, PromptControlReceiver) {
    let (sender, receiver) = mpsc::unbounded_channel();
    (PromptControlHandle { sender }, receiver)
}

#[derive(Debug, Default, Clone)]
pub(crate) struct OperationState {
    active: Arc<Mutex<Option<OperationKind>>>,
}

impl OperationState {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn active(&self) -> Option<OperationKind> {
        *self.active.lock().expect("operation state lock poisoned")
    }

    pub(crate) fn ensure_idle(&self) -> Result<(), CodingSessionError> {
        if let Some(active) = *self.active.lock().expect("operation state lock poisoned") {
            return Err(CodingSessionError::Busy {
                operation: active.as_str().into(),
            });
        }

        Ok(())
    }

    pub(crate) fn begin(&self, kind: OperationKind) -> Result<OperationGuard, CodingSessionError> {
        let mut active = self.active.lock().expect("operation state lock poisoned");
        if let Some(active) = *active {
            return Err(CodingSessionError::Busy {
                operation: active.as_str().into(),
            });
        }
        *active = Some(kind);
        Ok(OperationGuard {
            active: Arc::clone(&self.active),
            kind,
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct OperationControl {
    state: OperationState,
    prompt_control_receiver: Option<PromptControlReceiver>,
}

impl OperationControl {
    pub(crate) fn new() -> Self {
        Self {
            state: OperationState::new(),
            prompt_control_receiver: None,
        }
    }

    pub(crate) fn active(&self) -> Option<OperationKind> {
        self.state.active()
    }

    pub(crate) fn ensure_idle(&self) -> Result<(), CodingSessionError> {
        self.state.ensure_idle()
    }

    pub(crate) fn begin(&self, kind: OperationKind) -> Result<OperationGuard, CodingSessionError> {
        self.state.begin(kind)
    }

    pub(crate) fn prompt_control_handle(
        &mut self,
    ) -> Result<PromptControlHandle, CodingSessionError> {
        self.state.ensure_idle()?;
        if self.prompt_control_receiver.is_some() {
            return Err(CodingSessionError::Busy {
                operation: "prompt_control".into(),
            });
        }
        let (handle, receiver) = prompt_control_channel();
        self.prompt_control_receiver = Some(receiver);
        Ok(handle)
    }

    pub(crate) fn take_prompt_control_receiver(&mut self) -> Option<PromptControlReceiver> {
        self.prompt_control_receiver.take()
    }

    pub(crate) fn clear_prompt_control_receiver(&mut self) {
        self.prompt_control_receiver = None;
    }
}

#[derive(Debug)]
#[must_use = "dropping OperationGuard clears the active operation"]
pub(crate) struct OperationGuard {
    active: Arc<Mutex<Option<OperationKind>>>,
    kind: OperationKind,
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        let Ok(mut active) = self.active.lock() else {
            return;
        };
        if *active == Some(self.kind) {
            *active = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_guard_sets_active_operation_and_drop_clears_it() {
        let state = OperationState::new();

        let guard = state.begin(OperationKind::Prompt).unwrap();

        assert_eq!(state.active(), Some(OperationKind::Prompt));

        drop(guard);

        assert_eq!(state.active(), None);
    }

    #[test]
    fn operation_guard_clears_active_operation_after_error_return() {
        let state = OperationState::new();

        let result: Result<(), CodingSessionError> = (|| {
            let _guard = state.begin(OperationKind::Compact)?;
            Err(CodingSessionError::Flow {
                message: "node failed".into(),
            })
        })();

        assert!(result.is_err());
        assert_eq!(state.active(), None);
    }

    #[test]
    fn begin_reports_current_operation_when_busy() {
        let state = OperationState::new();
        let _guard = state.begin(OperationKind::PluginLoad).unwrap();

        let error = state.begin(OperationKind::Prompt).unwrap_err();

        assert_eq!(
            error,
            CodingSessionError::Busy {
                operation: "plugin_load".into(),
            }
        );
        assert_eq!(state.active(), Some(OperationKind::PluginLoad));
    }

    #[test]
    fn prompt_control_handle_sends_abort_steer_and_follow_up_commands() {
        let (handle, mut receiver) = prompt_control_channel();

        handle.abort("user cancelled").unwrap();
        handle.steer("prefer concise answer").unwrap();
        handle.follow_up("continue with tests").unwrap();

        assert_eq!(
            receiver.try_recv().unwrap(),
            PromptControlCommand::Abort {
                reason: "user cancelled".into(),
            }
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            PromptControlCommand::Steer {
                text: "prefer concise answer".into(),
            }
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            PromptControlCommand::FollowUp {
                text: "continue with tests".into(),
            }
        );
    }

    #[test]
    fn operation_control_owns_prompt_control_receiver_lifecycle() {
        let mut control = OperationControl::new();

        let handle = control.prompt_control_handle().unwrap();
        handle.steer("prefer tests").unwrap();

        let mut receiver = control
            .take_prompt_control_receiver()
            .expect("prompt receiver should be owned by operation control");

        assert_eq!(
            receiver.try_recv().unwrap(),
            PromptControlCommand::Steer {
                text: "prefer tests".into(),
            }
        );
        assert!(control.take_prompt_control_receiver().is_none());
    }

    #[test]
    fn operation_control_rejects_prompt_handle_while_busy_or_pending() {
        let mut control = OperationControl::new();
        let _guard = control.begin(OperationKind::PluginLoad).unwrap();

        assert_eq!(
            control.prompt_control_handle().unwrap_err(),
            CodingSessionError::Busy {
                operation: "plugin_load".into(),
            }
        );
        drop(_guard);

        let _handle = control.prompt_control_handle().unwrap();
        assert_eq!(
            control.prompt_control_handle().unwrap_err(),
            CodingSessionError::Busy {
                operation: "prompt_control".into(),
            }
        );
        control.clear_prompt_control_receiver();
        assert!(control.prompt_control_handle().is_ok());
    }

    #[test]
    fn prompt_control_handle_reports_closed_receiver() {
        let (handle, receiver) = prompt_control_channel();
        drop(receiver);

        let error = handle.abort("stop").unwrap_err();

        assert_eq!(
            error,
            CodingSessionError::Session {
                message: "prompt control receiver is closed".into(),
            }
        );
    }
}
