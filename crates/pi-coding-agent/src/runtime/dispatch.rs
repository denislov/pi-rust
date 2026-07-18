use super::capability::CapabilityRevocationPolicy;
use super::control::PromptControlRegistration;
use super::facade::{CodingAgentSession, CodingSessionError, PromptControlCleanupGuard};
use super::intent::IntentRouter;
use super::operation::{Operation, OperationDispatchMode, OperationOutcome};
use super::scheduler::OperationScheduler;
use super::submission::{SubmissionCommitGuard, submitted_terminal_status};
use crate::operations::compaction::flow::ManualCompactionOptions;
use crate::runtime::capability::SessionWriteCapability;
use crate::session::id::{Clock, SystemClock};
use crate::session::service::SessionPersistence;

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

        let result = match operation {
            Operation::Export(options) => crate::operations::export::run(
                options,
                operation_permit.capability_snapshot(),
                &self.persistence,
                &self.flow_service,
            )
            .map(OperationOutcome::Export),
            Operation::PluginCommand { .. } => Err(IntentRouter::unsupported_dispatch(&admission)),
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
            | Operation::SetSessionTreeLabel { .. }
            | Operation::SetDefaultAgentProfile { .. } => {
                Err(IntentRouter::unsupported_dispatch(&admission))
            }
        };
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
                SessionWriteCapability::require(
                    operation_permit
                        .capability_snapshot()
                        .session_write
                        .as_ref(),
                )?;
                let now = SystemClock.now_rfc3339();
                crate::operations::delegation::confirmation::reject_pending(
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
                SessionWriteCapability::require(
                    operation_permit
                        .capability_snapshot()
                        .session_write
                        .as_ref(),
                )?;
                let operation_id = operation_permit.capability_snapshot().operation_id.clone();
                let transition = crate::operations::session_navigation::fork(
                    &self.persistence,
                    target_leaf_id.as_deref(),
                    &operation_id,
                )?;
                drop(operation_permit);
                self.persistence = SessionPersistence::Persistent(transition.session_service);
                self.pending_delegation_confirmations =
                    transition.owner_state.pending_delegation_confirmations;
                *self.startup_recovery_markers.lock().unwrap() =
                    transition.owner_state.startup_recovery_markers;
                self.refresh_snapshot_projection();
                self.event_service
                    .emit_session_opened(transition.session_id);
                Ok(OperationOutcome::ForkSession)
            }
            Operation::SwitchActiveLeaf { target_leaf_id } => {
                SessionWriteCapability::require(
                    operation_permit
                        .capability_snapshot()
                        .session_write
                        .as_ref(),
                )?;
                crate::operations::session_navigation::switch_active_leaf(
                    &mut self.persistence,
                    &target_leaf_id,
                    &operation_permit.capability_snapshot().operation_id,
                )?;
                self.refresh_snapshot_projection();
                Ok(OperationOutcome::SwitchActiveLeaf)
            }
            Operation::SetSessionTreeLabel { entry_id, label } => {
                SessionWriteCapability::require(
                    operation_permit
                        .capability_snapshot()
                        .session_write
                        .as_ref(),
                )?;
                let update = crate::operations::session_navigation::set_tree_label(
                    &mut self.persistence,
                    &entry_id,
                    label,
                    &operation_permit.capability_snapshot().operation_id,
                )?;
                self.refresh_snapshot_projection();
                Ok(OperationOutcome::SessionTreeLabelChanged {
                    entry_id: update.entry_id,
                    label: update.label,
                    updated_at: update.updated_at,
                })
            }
            Operation::SetDefaultAgentProfile { profile_id } => {
                SessionWriteCapability::require(
                    operation_permit
                        .capability_snapshot()
                        .session_write
                        .as_ref(),
                )?;
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
        self.prepare_operation_for_admission(&mut operation)?;
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
        let execution = operation_permit.execution().clone();
        let snapshot = execution.capability_snapshot.clone();
        let operation_cancellation = operation_permit.cancellation_token();
        let operation_cancellation_handle = operation_permit.cancellation_handle();
        if let (Some(submission), Some(cancellation)) =
            (submission.as_ref(), operation_cancellation_handle.clone())
        {
            self.snapshot_coordinator.bind_operation_cancellation(
                submission.handle.clone(),
                snapshot.operation_id.clone(),
                cancellation,
            );
        }

        let result =
            async {
                match operation {
                    Operation::Prompt(options) => {
                        let has_existing_prompt_control = self
                            .operation_control
                            .current_prompt_control_registration()
                            .is_some();
                        let prompt_control = if submission.is_some() || has_existing_prompt_control
                        {
                            Some(
                                self.operation_control
                                    .prompt_control_registration_for(&snapshot.operation_id)?,
                            )
                        } else {
                            None
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
                        let result = crate::operations::prompt::run(
                            &mut self.persistence,
                            &mut self.operation_control,
                            &self.profile_registry,
                            &self.plugin_service,
                            &self.event_service,
                            &self.flow_service,
                            &mut self.pending_delegation_confirmations,
                            &self.authorization_service,
                            options,
                            &snapshot,
                            operation_cancellation.clone(),
                        )
                        .await;
                        if let Some(cleanup) = prompt_control_cleanup.as_mut() {
                            cleanup.cleanup();
                        }
                        result.map(OperationOutcome::Prompt)
                    }
                    Operation::ManualCompaction(options) => {
                        let mut options =
                            ManualCompactionOptions::from_prompt_turn_options(&options)?;
                        if let Some(cancellation) = operation_cancellation.clone() {
                            options = options.with_cancellation(cancellation);
                        }
                        let SessionPersistence::Persistent(session_service) = &mut self.persistence
                        else {
                            return Err(CodingSessionError::UnsupportedCapability {
                                capability: "manual compaction without persistent session".into(),
                            });
                        };
                        crate::operations::compaction::run(
                            session_service,
                            &self.flow_service,
                            &self.event_service,
                            options,
                            &snapshot,
                            operation_cancellation_handle.clone(),
                        )
                        .await
                        .map(OperationOutcome::ManualCompaction)
                    }
                    Operation::PluginLoad(options) => {
                        let execution = crate::operations::plugin_load::run(
                            &mut self.persistence,
                            &self.flow_service,
                            &self.event_service,
                            options,
                            &snapshot,
                            operation_cancellation.clone(),
                            operation_cancellation_handle.clone(),
                        )
                        .await?;
                        if let Some(plugin_service) = execution.loaded_plugin_service {
                            self.plugin_service = plugin_service;
                        }
                        if execution.outcome.capability_changed {
                            let installed = self
                                .capability_snapshots
                                .install_next_generation(CapabilityRevocationPolicy::FutureOnly);
                            self.refresh_snapshot_projection();
                            self.event_service.emit_capability_changed(installed);
                        }
                        self.event_service
                            .emit_plugin_load_outcome(&snapshot.operation_id, &execution.outcome);
                        Ok(OperationOutcome::PluginLoad(execution.outcome))
                    }
                    Operation::BranchSummary {
                        options,
                        source_leaf_id,
                        target_leaf_id,
                        custom_instructions,
                        reuse_existing,
                    } => {
                        if reuse_existing
                            && let Some(outcome) =
                                crate::operations::branch_summary::reused_outcome(
                                    &self.persistence,
                                    &options,
                                    source_leaf_id.as_str(),
                                    target_leaf_id.as_str(),
                                    operation_permit.capability_snapshot(),
                                )?
                        {
                            return Ok(OperationOutcome::BranchSummary(outcome));
                        }
                        let SessionPersistence::Persistent(session_service) = &mut self.persistence
                        else {
                            return Err(CodingSessionError::UnsupportedCapability {
                                capability: "branch summary without persistent session".into(),
                            });
                        };
                        crate::operations::branch_summary::run(
                            session_service,
                            &self.flow_service,
                            &self.event_service,
                            options,
                            source_leaf_id,
                            target_leaf_id,
                            custom_instructions,
                            &snapshot,
                            operation_cancellation.clone(),
                            operation_cancellation_handle.clone(),
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
                        let model_repair_policy = match model_repair {
                            Some(model_repair) => {
                                let (prompt_options, max_attempts) = model_repair.into_parts();
                                Some(crate::operations::self_healing_edit::model_repair_policy(
                                    prompt_options,
                                    max_attempts,
                                    &snapshot,
                                )?)
                            }
                            None => None,
                        };
                        let SessionPersistence::Persistent(session_service) = &mut self.persistence
                        else {
                            return Err(CodingSessionError::UnsupportedCapability {
                                capability:
                                    "self-healing edit requires a persistent Rust-native session"
                                        .into(),
                            });
                        };
                        let outcome = crate::operations::self_healing_edit::run(
                            session_service,
                            &self.flow_service,
                            self.event_service.clone(),
                            path,
                            replacements,
                            check_command,
                            repair_attempts,
                            model_repair_policy,
                            &snapshot,
                            operation_cancellation.clone(),
                            operation_cancellation_handle.clone(),
                        )
                        .await?;
                        self.event_service
                            .emit_session_write_events(&outcome.finalized);
                        outcome.result.map(OperationOutcome::SelfHealingEdit)
                    }
                    Operation::AgentInvocation(options) => {
                        let prompt_control_receiver =
                            self.operation_control.take_prompt_control_receiver();
                        self.operation_control.clear_prompt_control_receiver();
                        let result = crate::operations::agent_invocation::run(
                            options,
                            snapshot.operation_id.clone(),
                            prompt_control_receiver,
                            &self.profile_registry,
                            &self.plugin_service,
                            &self.event_service,
                            &self.flow_service,
                            &self.operation_control,
                            snapshot.clone(),
                            operation_cancellation.clone(),
                        )
                        .await;
                        result.map(OperationOutcome::AgentInvocation)
                    }
                    Operation::AgentTeam(options) => crate::operations::team_invocation::run(
                        options,
                        snapshot.operation_id.clone(),
                        &self.profile_registry,
                        &self.plugin_service,
                        &self.event_service,
                        &self.flow_service,
                        &self.operation_control,
                        snapshot.clone(),
                        operation_cancellation.clone(),
                    )
                    .await
                    .map(OperationOutcome::AgentTeam),
                    Operation::Export(_)
                    | Operation::PluginCommand { .. }
                    | Operation::RejectDelegationConfirmation { .. }
                    | Operation::ForkSession { .. }
                    | Operation::SwitchActiveLeaf { .. }
                    | Operation::SetSessionTreeLabel { .. }
                    | Operation::SetDefaultAgentProfile { .. } => {
                        Err(IntentRouter::unsupported_dispatch(&admission))
                    }
                    Operation::ApproveDelegationConfirmation {
                        operation_id,
                        tool_call_id,
                    } => crate::operations::delegation::execution::approve(
                        &mut self.persistence,
                        &mut self.pending_delegation_confirmations,
                        &self.runtime_service,
                        &self.flow_service,
                        &self.profile_registry,
                        &self.plugin_service,
                        &self.event_service,
                        &self.operation_control,
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
