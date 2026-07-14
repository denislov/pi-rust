use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::CodingSessionError;
use super::snapshot_coordinator::SnapshotCoordinator;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationKind {
    Prompt,
    Compact,
    PluginCommand,
    PluginLoad,
    DelegationConfirmation,
    BranchSummary,
    AgentInvocation,
    AgentTeam,
    Export,
    ForkSession,
    SwitchActiveLeaf,
    SetDefaultAgentProfile,
    #[allow(dead_code)]
    SelfHealingEdit,
}

impl OperationKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Prompt => "prompt",
            Self::Compact => "compact",
            Self::PluginCommand => "plugin_command",
            Self::PluginLoad => "plugin_load",
            Self::DelegationConfirmation => "delegation_confirmation",
            Self::BranchSummary => "branch_summary",
            Self::AgentInvocation => "agent_invocation",
            Self::AgentTeam => "agent_team",
            Self::Export => "export",
            Self::ForkSession => "fork_session",
            Self::SwitchActiveLeaf => "switch_active_leaf",
            Self::SetDefaultAgentProfile => "set_default_agent_profile",
            Self::SelfHealingEdit => "self_healing_edit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PromptControlCommand {
    Abort { reason: String },
    Steer { text: String },
    FollowUp { text: String },
}

pub(crate) type PromptControlReceiver = mpsc::Receiver<PromptControlCommand>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PromptControlGeneration(pub(crate) u64);

#[derive(Debug, Clone)]
pub(crate) struct PromptControlHandle {
    sender: mpsc::Sender<PromptControlCommand>,
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
        self.sender.try_send(command).map_err(|error| match error {
            mpsc::error::TrySendError::Closed(_) => CodingSessionError::Session {
                message: "prompt control receiver is closed".into(),
            },
            mpsc::error::TrySendError::Full(_) => CodingSessionError::Busy {
                operation: "prompt_control_queue".into(),
            },
        })
    }
}

pub(crate) fn prompt_control_channel() -> (PromptControlHandle, PromptControlReceiver) {
    let (sender, receiver) = mpsc::channel(64);
    (PromptControlHandle { sender }, receiver)
}

#[derive(Debug, Clone)]
pub(crate) struct PromptControlRegistration {
    pub(crate) generation: PromptControlGeneration,
    pub(crate) handle: PromptControlHandle,
}

#[derive(Debug)]
struct PromptControlOwnership {
    generation: PromptControlGeneration,
    handle: PromptControlHandle,
    receiver: Option<PromptControlReceiver>,
}

#[derive(Debug)]
struct PromptControlStateInner {
    next_generation: u64,
    active: Option<PromptControlOwnership>,
}

#[derive(Debug, Clone)]
pub(crate) struct PromptControlCleanup {
    shared: Arc<Mutex<PromptControlStateInner>>,
}

impl PromptControlCleanup {
    pub(crate) fn clear_if_generation(&self, generation: PromptControlGeneration) {
        let Ok(mut shared) = self.shared.lock() else {
            return;
        };
        if shared
            .active
            .as_ref()
            .is_some_and(|active| active.generation == generation)
        {
            shared.active = None;
        }
    }
}

#[derive(Debug, Clone)]
struct PromptControlState {
    shared: Arc<Mutex<PromptControlStateInner>>,
}

impl PromptControlState {
    fn new() -> Self {
        Self {
            shared: Arc::new(Mutex::new(PromptControlStateInner {
                next_generation: 1,
                active: None,
            })),
        }
    }

    fn create(&self) -> Result<PromptControlRegistration, CodingSessionError> {
        let mut shared = self
            .shared
            .lock()
            .expect("prompt control state lock poisoned");
        if shared
            .active
            .as_ref()
            .is_some_and(|active| active.receiver.is_some())
        {
            return Err(CodingSessionError::Busy {
                operation: "prompt_control".into(),
            });
        }
        let generation = PromptControlGeneration(shared.next_generation);
        shared.next_generation = shared.next_generation.saturating_add(1);
        let (handle, receiver) = prompt_control_channel();
        shared.active = Some(PromptControlOwnership {
            generation,
            handle: handle.clone(),
            receiver: Some(receiver),
        });
        Ok(PromptControlRegistration { generation, handle })
    }

    fn current(&self) -> Option<PromptControlRegistration> {
        self.shared
            .lock()
            .expect("prompt control state lock poisoned")
            .active
            .as_ref()
            .map(|active| PromptControlRegistration {
                generation: active.generation,
                handle: active.handle.clone(),
            })
    }

    fn take_receiver(&self) -> Option<PromptControlReceiver> {
        self.shared
            .lock()
            .expect("prompt control state lock poisoned")
            .active
            .as_mut()
            .and_then(|active| active.receiver.take())
    }

    fn clear(&self) {
        self.shared
            .lock()
            .expect("prompt control state lock poisoned")
            .active = None;
    }

    fn cleanup(&self) -> PromptControlCleanup {
        PromptControlCleanup {
            shared: Arc::clone(&self.shared),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct OperationState {
    shared: Arc<Mutex<OperationStateInner>>,
    snapshot_coordinator: Arc<SnapshotCoordinator>,
}

#[derive(Debug)]
struct OperationStateInner {
    active: Option<ActiveOperationIdentity>,
    next_generation: u64,
}

#[derive(Debug, Clone)]
struct ActiveOperationIdentity {
    kind: OperationKind,
    operation_id: String,
    generation: u64,
    cancellation: Option<CancellationToken>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompactCancellationRejection {
    NoActiveOperation,
    ActiveOperationNotCompact,
    OperationMismatch,
}

#[derive(Debug, Clone)]
pub(crate) struct CompactCancellationHandle {
    shared: Arc<Mutex<OperationStateInner>>,
}

impl CompactCancellationHandle {
    pub(crate) fn cancel(&self, operation_id: &str) -> Result<(), CompactCancellationRejection> {
        let shared = self.shared.lock().expect("operation state lock poisoned");
        let Some(active) = shared.active.as_ref() else {
            return Err(CompactCancellationRejection::NoActiveOperation);
        };
        if active.kind != OperationKind::Compact {
            return Err(CompactCancellationRejection::ActiveOperationNotCompact);
        }
        if active.operation_id != operation_id {
            return Err(CompactCancellationRejection::OperationMismatch);
        }
        active
            .cancellation
            .as_ref()
            .expect("active Compact identity must carry cancellation")
            .cancel();
        Ok(())
    }
}

impl OperationState {
    pub(crate) fn new() -> Self {
        Self::with_snapshot_coordinator(SnapshotCoordinator::new())
    }

    pub(crate) fn with_snapshot_coordinator(
        snapshot_coordinator: Arc<SnapshotCoordinator>,
    ) -> Self {
        Self {
            shared: Arc::new(Mutex::new(OperationStateInner {
                active: None,
                next_generation: 1,
            })),
            snapshot_coordinator,
        }
    }

    pub(crate) fn active(&self) -> Option<OperationKind> {
        self.shared
            .lock()
            .expect("operation state lock poisoned")
            .active
            .as_ref()
            .map(|active| active.kind)
    }

    pub(crate) fn ensure_idle(&self) -> Result<(), CodingSessionError> {
        if let Some(active) = self
            .shared
            .lock()
            .expect("operation state lock poisoned")
            .active
            .as_ref()
        {
            return Err(CodingSessionError::Busy {
                operation: active.kind.as_str().into(),
            });
        }

        Ok(())
    }

    pub(crate) fn begin(
        &self,
        kind: OperationKind,
        operation_id: String,
    ) -> Result<OperationGuard, CodingSessionError> {
        let mut shared = self.shared.lock().expect("operation state lock poisoned");
        if let Some(active) = shared.active.as_ref() {
            return Err(CodingSessionError::Busy {
                operation: active.kind.as_str().into(),
            });
        }
        let generation = shared.next_generation;
        shared.next_generation = shared.next_generation.saturating_add(1);
        let cancellation = (kind == OperationKind::Compact).then(CancellationToken::new);
        shared.active = Some(ActiveOperationIdentity {
            kind,
            operation_id,
            generation,
            cancellation: cancellation.clone(),
        });
        drop(shared);
        self.snapshot_coordinator.set_active_operation(Some(kind));
        Ok(OperationGuard {
            shared: Arc::clone(&self.shared),
            snapshot_coordinator: Arc::clone(&self.snapshot_coordinator),
            kind,
            generation,
            cancellation,
        })
    }
}

#[derive(Debug)]
pub(crate) struct OperationControl {
    state: OperationState,
    prompt_control: PromptControlState,
}

impl OperationControl {
    pub(crate) fn new() -> Self {
        Self::with_snapshot_coordinator(SnapshotCoordinator::new())
    }

    pub(crate) fn with_snapshot_coordinator(
        snapshot_coordinator: Arc<SnapshotCoordinator>,
    ) -> Self {
        Self {
            state: OperationState::with_snapshot_coordinator(snapshot_coordinator),
            prompt_control: PromptControlState::new(),
        }
    }

    pub(crate) fn active(&self) -> Option<OperationKind> {
        self.state.active()
    }

    pub(crate) fn ensure_idle(&self) -> Result<(), CodingSessionError> {
        self.state.ensure_idle()
    }

    pub(crate) fn begin(
        &self,
        kind: OperationKind,
        operation_id: String,
    ) -> Result<OperationGuard, CodingSessionError> {
        self.state.begin(kind, operation_id)
    }

    pub(crate) fn compact_cancellation_handle(&self) -> CompactCancellationHandle {
        CompactCancellationHandle {
            shared: Arc::clone(&self.state.shared),
        }
    }

    pub(crate) fn prompt_control_handle(
        &mut self,
    ) -> Result<PromptControlHandle, CodingSessionError> {
        if self.state.active() != Some(OperationKind::Prompt) {
            self.state.ensure_idle()?;
        }
        self.prompt_control
            .create()
            .map(|registration| registration.handle)
    }

    pub(crate) fn current_prompt_control_registration(&self) -> Option<PromptControlRegistration> {
        self.prompt_control.current()
    }

    pub(crate) fn prompt_control_registration(
        &mut self,
    ) -> Result<PromptControlRegistration, CodingSessionError> {
        if self.state.active() != Some(OperationKind::Prompt) {
            self.state.ensure_idle()?;
        }
        self.prompt_control.create()
    }

    pub(crate) fn prompt_control_cleanup(&self) -> PromptControlCleanup {
        self.prompt_control.cleanup()
    }

    pub(crate) fn take_prompt_control_receiver(&mut self) -> Option<PromptControlReceiver> {
        self.prompt_control.take_receiver()
    }

    pub(crate) fn clear_prompt_control_receiver(&mut self) {
        self.prompt_control.clear();
    }
}

#[derive(Debug)]
#[must_use = "dropping OperationGuard clears the active operation"]
pub(crate) struct OperationGuard {
    shared: Arc<Mutex<OperationStateInner>>,
    snapshot_coordinator: Arc<SnapshotCoordinator>,
    kind: OperationKind,
    generation: u64,
    cancellation: Option<CancellationToken>,
}

impl OperationGuard {
    pub(crate) fn cancellation_token(&self) -> Option<CancellationToken> {
        self.cancellation.clone()
    }
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        let Ok(mut shared) = self.shared.lock() else {
            return;
        };
        if shared
            .active
            .as_ref()
            .is_some_and(|active| active.kind == self.kind && active.generation == self.generation)
        {
            shared.active = None;
            drop(shared);
            self.snapshot_coordinator.set_active_operation(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_cancellation_handle_is_exact_and_fail_closed() {
        let state = OperationState::new();
        let handle = CompactCancellationHandle {
            shared: Arc::clone(&state.shared),
        };

        assert_eq!(
            handle.cancel("op-compact"),
            Err(CompactCancellationRejection::NoActiveOperation)
        );
        let guard = state
            .begin(OperationKind::Compact, "op-compact".into())
            .unwrap();
        let token = guard.cancellation_token().unwrap();
        assert_eq!(
            handle.cancel("op-stale"),
            Err(CompactCancellationRejection::OperationMismatch)
        );
        assert!(!token.is_cancelled());
        handle.cancel("op-compact").unwrap();
        assert!(token.is_cancelled());
        drop(guard);

        let prompt = state
            .begin(OperationKind::Prompt, "op-prompt".into())
            .unwrap();
        assert_eq!(
            handle.cancel("op-prompt"),
            Err(CompactCancellationRejection::ActiveOperationNotCompact)
        );
        drop(prompt);
    }

    #[test]
    fn operation_guard_sets_active_operation_and_drop_clears_it() {
        let state = OperationState::new();

        let guard = state
            .begin(OperationKind::Prompt, "op_test".into())
            .unwrap();

        assert_eq!(state.active(), Some(OperationKind::Prompt));

        drop(guard);

        assert_eq!(state.active(), None);
    }

    #[test]
    fn operation_guard_clears_active_operation_after_error_return() {
        let state = OperationState::new();

        let result: Result<(), CodingSessionError> = (|| {
            let _guard = state.begin(OperationKind::Compact, "op_test".into())?;
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
        let _guard = state
            .begin(OperationKind::PluginLoad, "op_test".into())
            .unwrap();

        let error = state
            .begin(OperationKind::Prompt, "op_test".into())
            .unwrap_err();

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
        let _guard = control
            .begin(OperationKind::PluginLoad, "op_test".into())
            .unwrap();

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

    #[test]
    fn prompt_control_cleanup_is_idempotent_and_preserves_newer_generation() {
        let mut control = OperationControl::new();
        let first = control.prompt_control_registration().unwrap();
        let cleanup = control.prompt_control_cleanup();
        let _first_receiver = control
            .take_prompt_control_receiver()
            .expect("first generation receiver");
        let second = control.prompt_control_registration().unwrap();

        cleanup.clear_if_generation(first.generation);
        cleanup.clear_if_generation(first.generation);

        assert_eq!(
            control
                .current_prompt_control_registration()
                .expect("newer generation must remain")
                .generation,
            second.generation
        );
        let mut second_receiver = control
            .take_prompt_control_receiver()
            .expect("newer receiver must remain");
        second.handle.follow_up("newer control").unwrap();
        assert_eq!(
            second_receiver.try_recv().unwrap(),
            PromptControlCommand::FollowUp {
                text: "newer control".into(),
            }
        );

        cleanup.clear_if_generation(second.generation);
        cleanup.clear_if_generation(second.generation);
        assert!(control.current_prompt_control_registration().is_none());
    }
}
