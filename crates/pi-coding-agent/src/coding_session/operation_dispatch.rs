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

    pub(super) fn run_sync_mut_operation(
        &mut self,
        operation: Operation,
        mut submission: Option<SubmissionCommitGuard>,
    ) -> Result<OperationOutcome, CodingSessionError> {
        let admission = self.resolve_operation_admission(&operation)?;
        let operation_permit = OperationScheduler::admit(
            &self.operation_control,
            &admission,
            OperationDispatchMode::SyncMutable,
        )
        .map_err(|rejection| rejection.into_error())?;
        if let Some(guard) = submission.as_mut() {
            guard.commit(operation_permit.capability_snapshot().operation_id.clone())?;
        }

        let result = (|| match operation {
            Operation::RejectDelegationConfirmation {
                operation_id,
                tool_call_id,
                reason,
            } => {
                let now = SystemClock.now_rfc3339();
                self.delegation_confirmation_service.reject_pending(
                    &mut self.persistence,
                    &mut self.pending_delegation_confirmations,
                    &self.event_service,
                    operation_id.as_str(),
                    tool_call_id.as_str(),
                    &now,
                    reason,
                )?;
                Ok(OperationOutcome::DelegationRejection)
            }
            Operation::ForkSession { target_leaf_id } => {
                let operation_id = operation_permit.capability_snapshot().operation_id.clone();
                let SessionPersistence::Persistent(session_service) = &self.persistence else {
                    return Err(CodingSessionError::UnsupportedCapability {
                        capability: "fork requires a persistent Rust-native session".into(),
                    });
                };
                let mut forked_service = session_service
                    .fork_current_admitted(target_leaf_id.as_deref(), &operation_id)?;
                let forked_session_id = forked_service.session_id().to_owned();
                let replay_state = match replay_derived_owner_state(&mut forked_service) {
                    Ok(replay_state) => replay_state,
                    Err(error) => {
                        return Err(forked_service.cleanup_failed_transition(&operation_id, error));
                    }
                };
                drop(operation_permit);
                self.persistence = SessionPersistence::Persistent(forked_service);
                self.pending_delegation_confirmations =
                    replay_state.pending_delegation_confirmations;
                *self.startup_recovery_markers.lock().unwrap() =
                    replay_state.startup_recovery_markers;
                self.refresh_snapshot_projection();
                self.event_service.emit_session_opened(forked_session_id);
                Ok(OperationOutcome::ForkSession)
            }
            Operation::SwitchActiveLeaf { target_leaf_id } => {
                let SessionPersistence::Persistent(session_service) = &mut self.persistence else {
                    return Err(CodingSessionError::UnsupportedCapability {
                        capability:
                            "active leaf navigation requires a persistent Rust-native session"
                                .into(),
                    });
                };
                session_service.switch_active_leaf(
                    &target_leaf_id,
                    &operation_permit.capability_snapshot().operation_id,
                )?;
                self.refresh_snapshot_projection();
                Ok(OperationOutcome::SwitchActiveLeaf)
            }
            Operation::SetDefaultAgentProfile { profile_id } => {
                match &mut self.persistence {
                    SessionPersistence::Persistent(session_service) => {
                        session_service.set_default_agent_profile_id(profile_id.clone())?;
                    }
                    SessionPersistence::NonPersistent(state) => {
                        state.default_agent_profile_id = profile_id.clone();
                    }
                }
                self.event_service
                    .emit_default_agent_profile_changed(profile_id);
                let installed = self
                    .capability_snapshots
                    .install_next_generation(CapabilityRevocationPolicy::FutureOnly);
                self.refresh_snapshot_projection();
                self.event_service.emit_capability_changed(installed);
                Ok(OperationOutcome::SetDefaultAgentProfile)
            }
            Operation::Export(_) | Operation::PluginCommand { .. } => {
                Err(IntentRouter::unsupported_dispatch(&admission))
            }
            Operation::Prompt(_)
            | Operation::ManualCompaction(_)
            | Operation::PluginLoad(_)
            | Operation::ApproveDelegationConfirmation { .. }
            | Operation::BranchSummary { .. }
            | Operation::SelfHealingEdit(_)
            | Operation::AgentInvocation(_)
            | Operation::AgentTeam(_) => Err(IntentRouter::unsupported_dispatch(&admission)),
        })();
        if let Some(guard) = submission.as_mut() {
            guard.finish(submitted_terminal_status(&result))?;
        }
        result
    }
}
