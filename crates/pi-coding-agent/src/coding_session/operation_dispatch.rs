use super::*;

impl CodingAgentSession {
    pub(super) fn run_sync_operation(
        &self,
        operation: Operation,
        mut submission: Option<SubmissionCommitGuard>,
    ) -> Result<OperationOutcome, CodingSessionError> {
        let admission = self.resolve_operation_admission(&operation)?;
        let operation_permit = OperationScheduler::admit(
            &self.operation_control,
            &admission,
            OperationDispatchMode::SyncReadOnly,
        )
        .map_err(|rejection| rejection.into_error())?;
        if let Some(guard) = submission.as_mut() {
            guard.commit(operation_permit.capability_snapshot().operation_id.clone())?;
        }

        let result = (|| match operation {
            Operation::Export(options) => self
                .export_current_inner(options, operation_permit.capability_snapshot())
                .map(OperationOutcome::Export),
            Operation::PluginCommand { command_id, args } => self
                .plugin_service
                .run_command_with_capabilities(
                    &command_id,
                    args,
                    &operation_permit.capability_snapshot().plugin,
                )
                .map(OperationOutcome::PluginCommand),
            Operation::RejectDelegationConfirmation { .. } => {
                Err(IntentRouter::unsupported_dispatch(&admission))
            }
            Operation::Prompt(_)
            | Operation::ManualCompaction(_)
            | Operation::PluginLoad(_)
            | Operation::ApproveDelegationConfirmation { .. }
            | Operation::BranchSummary { .. }
            | Operation::SelfHealingEdit(_)
            | Operation::AgentInvocation(_)
            | Operation::AgentTeam(_)
            | Operation::ForkSession { .. }
            | Operation::SwitchActiveLeaf { .. }
            | Operation::SetDefaultAgentProfile { .. } => {
                Err(IntentRouter::unsupported_dispatch(&admission))
            }
        })();
        if let Some(guard) = submission.as_mut() {
            guard.finish(submitted_terminal_status(&result))?;
        }
        result
    }
}
