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

    pub(super) async fn run_operation(
        &mut self,
        mut operation: Operation,
        mut submission: Option<SubmissionCommitGuard>,
    ) -> Result<OperationOutcome, CodingSessionError> {
        if let Some(options) = operation.prompt_options_mut()
            && let Some(runtime) = options.runtime_mut()
        {
            self.runtime_service.install_provider_runtime(runtime);
        }
        let admission = self.resolve_operation_admission(&operation)?;
        let operation_permit = OperationScheduler::admit(
            &self.operation_control,
            &admission,
            OperationDispatchMode::Async,
        )
        .map_err(|rejection| rejection.into_error())?;
        if let Some(guard) = submission.as_mut() {
            guard.commit(operation_permit.capability_snapshot().operation_id.clone())?;
        }
        let snapshot = operation_permit.capability_snapshot().clone();
        let operation_cancellation = operation_permit.cancellation_token();

        let result =
            async {
                match operation {
                    Operation::Prompt(options) => {
                        let prompt_control = if submission.is_some() {
                            Some(
                                match self.operation_control.current_prompt_control_registration() {
                                    Some(registration) => registration,
                                    None => self.operation_control.prompt_control_registration()?,
                                },
                            )
                        } else {
                            self.operation_control.current_prompt_control_registration()
                        };
                        if let (Some(submission), Some(prompt_control)) =
                            (submission.as_ref(), prompt_control.as_ref())
                        {
                            self.snapshot_coordinator.bind_prompt_control(
                                submission.handle.clone(),
                                snapshot.operation_id.clone(),
                                prompt_control.generation,
                                prompt_control.handle.clone(),
                            );
                        }
                        let mut prompt_control_cleanup = prompt_control.map(
                            |PromptControlRegistration { generation, .. }| {
                                PromptControlCleanupGuard::new(
                                    self.operation_control.prompt_control_cleanup(),
                                    self.snapshot_coordinator.clone(),
                                    snapshot.operation_id.clone(),
                                    generation,
                                )
                            },
                        );
                        let result = self.prompt_inner(options, &snapshot).await;
                        if let Some(cleanup) = prompt_control_cleanup.as_mut() {
                            cleanup.cleanup();
                        }
                        result.map(OperationOutcome::Prompt)
                    }
                    Operation::ManualCompaction(options) => {
                        let mut options =
                            ManualCompactionOptions::from_prompt_turn_options(&options)?;
                        if let Some(cancellation) = operation_cancellation {
                            options = options.with_cancellation(cancellation);
                        }
                        let SessionPersistence::Persistent(session_service) = &mut self.persistence
                        else {
                            return Err(CodingSessionError::UnsupportedCapability {
                                capability: "manual compaction without persistent session".into(),
                            });
                        };
                        self.manual_compaction_service
                            .run_persistent(
                                session_service,
                                &self.flow_service,
                                &self.event_service,
                                options,
                                &snapshot,
                            )
                            .await
                            .map(OperationOutcome::ManualCompaction)
                    }
                    Operation::PluginLoad(options) => self
                        .load_plugins_inner(options, &snapshot)
                        .await
                        .map(OperationOutcome::PluginLoad),
                    Operation::BranchSummary {
                        options,
                        source_leaf_id,
                        target_leaf_id,
                        custom_instructions,
                        reuse_existing,
                    } => {
                        if reuse_existing
                            && let Some(outcome) = self.branch_summary_service.reused_outcome(
                                &self.persistence,
                                &options,
                                source_leaf_id.as_str(),
                                target_leaf_id.as_str(),
                                operation_permit.capability_snapshot(),
                            )?
                        {
                            return Ok(OperationOutcome::BranchSummary(outcome));
                        }
                        self.run_branch_summary_admitted(
                            options,
                            source_leaf_id,
                            target_leaf_id,
                            custom_instructions,
                            &snapshot,
                        )
                        .await
                        .map(OperationOutcome::BranchSummary)
                    }
                    Operation::SelfHealingEdit(request) => {
                        let (path, replacements, check_command, repair_attempts, model_repair) =
                            request.into_parts();
                        if !repair_attempts.is_empty() && model_repair.is_some() {
                            return Err(CodingSessionError::Input {
                            message:
                                "configure either planned repair attempts or model repair, not both"
                                    .into(),
                        });
                        }
                        let model_repair_policy =
                            self.self_healing_model_repair_policy(model_repair)?;
                        let SessionPersistence::Persistent(session_service) = &mut self.persistence
                        else {
                            return Err(CodingSessionError::UnsupportedCapability {
                                capability:
                                    "self-healing edit requires a persistent Rust-native session"
                                        .into(),
                            });
                        };
                        let outcome = self
                            .self_healing_edit_service
                            .run_persistent(
                                session_service,
                                &self.flow_service,
                                self.event_service.clone(),
                                path,
                                replacements,
                                check_command,
                                repair_attempts,
                                model_repair_policy,
                                &snapshot,
                            )
                            .await?;
                        self.event_service
                            .emit_session_write_events(&outcome.finalized);
                        outcome.result.map(OperationOutcome::SelfHealingEdit)
                    }
                    Operation::AgentInvocation(options) => {
                        let result = self
                            .invoke_agent_inner(options, snapshot.operation_id.clone())
                            .await;
                        self.operation_control.clear_prompt_control_receiver();
                        result.map(OperationOutcome::AgentInvocation)
                    }
                    Operation::AgentTeam(options) => self
                        .invoke_team_inner(options, snapshot.operation_id.clone())
                        .await
                        .map(OperationOutcome::AgentTeam),
                    Operation::Export(_)
                    | Operation::PluginCommand { .. }
                    | Operation::RejectDelegationConfirmation { .. }
                    | Operation::ForkSession { .. }
                    | Operation::SwitchActiveLeaf { .. }
                    | Operation::SetDefaultAgentProfile { .. } => {
                        Err(IntentRouter::unsupported_dispatch(&admission))
                    }
                    Operation::ApproveDelegationConfirmation {
                        operation_id,
                        tool_call_id,
                    } => self
                        .approve_delegation_confirmation_inner(
                            operation_id,
                            tool_call_id,
                            admission
                                .admitted_at
                                .expect("delegation approval admission time is resolved"),
                            snapshot.clone(),
                        )
                        .await
                        .map(|_| OperationOutcome::DelegationApproval),
                }
            }
            .await;
        if let Some(guard) = submission.as_mut() {
            guard.finish(submitted_terminal_status(&result))?;
        }
        result
    }
}
