use super::*;

#[derive(Debug)]
#[must_use = "dropping PromptControlCleanupGuard clears exact Prompt control ownership"]
pub(super) struct PromptControlCleanupGuard {
    cleanup: PromptControlCleanup,
    snapshot_coordinator: Arc<SnapshotCoordinator>,
    operation_id: String,
    channel_generation: PromptControlGeneration,
    armed: bool,
}

impl PromptControlCleanupGuard {
    pub(super) fn new(
        cleanup: PromptControlCleanup,
        snapshot_coordinator: Arc<SnapshotCoordinator>,
        operation_id: String,
        channel_generation: PromptControlGeneration,
    ) -> Self {
        Self {
            cleanup,
            snapshot_coordinator,
            operation_id,
            channel_generation,
            armed: true,
        }
    }

    pub(super) fn cleanup(&mut self) {
        if !self.armed {
            return;
        }
        self.snapshot_coordinator
            .clear_prompt_control_if(&self.operation_id, self.channel_generation);
        self.cleanup.clear_if_generation(self.channel_generation);
        self.armed = false;
    }
}

impl Drop for PromptControlCleanupGuard {
    fn drop(&mut self) {
        self.cleanup();
    }
}

impl CodingAgentSession {
    pub(crate) fn prompt_control_handle(
        &mut self,
    ) -> Result<PromptControlHandle, CodingSessionError> {
        IntentRouter::prompt_control_handle(
            &mut self.operation_control,
            ControlIntent::PromptControl,
        )
    }
}
