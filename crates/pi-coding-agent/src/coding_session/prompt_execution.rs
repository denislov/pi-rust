use super::*;

impl CodingAgentSession {
    pub(super) async fn prompt_inner(
        &mut self,
        options: PromptTurnOptions,
        snapshot: &OperationCapabilitySnapshot,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        if options.runtime().is_none() {
            return Err(CodingSessionError::Config {
                message: "prompt turn options do not include a runtime snapshot".into(),
            });
        }
        let options = self.apply_default_agent_profile(options)?;
        let mut context = self.prepare_prompt_context(options, snapshot)?;
        let operation_id = context.operation_id().to_owned();
        let turn_id = context.turn_id().to_owned();

        self.event_service
            .emit_prompt_started(operation_id, turn_id);
        let mut outcome = match self.flow_service.run_prompt_turn(&mut context).await {
            Ok(outcome) => outcome,
            Err(error) => match context.abort_reason() {
                Some(reason) => {
                    context.finish_abort(reason.to_owned(), context.session_id().map(str::to_owned))
                }
                None => context.finish_failure(error),
            },
        };
        if outcome.is_success() {
            match context.authorize_delegation_requests(0) {
                Ok(decisions) => {
                    let decisions = decisions.to_vec();
                    let prompt_options = context.options().clone();
                    if let Err(error) = self
                        .execute_authorized_delegations(&mut context, &decisions, prompt_options)
                        .await
                    {
                        self.event_service.emit_diagnostic(
                            Some(context.operation_id().to_owned()),
                            format!("delegation execution failed: {error}"),
                        );
                    }
                }
                Err(error) => {
                    outcome = context.finish_failure(error);
                }
            }
        }
        let finalized = match self.finalize_prompt_transaction(&mut context, &outcome) {
            Ok(finalized) => finalized,
            Err(error) => {
                outcome = context.finish_failure(error.clone());
                SessionService::skip_prompt_transaction(
                    context.operation_id().to_owned(),
                    format!("session write finalization failed: {error}"),
                )
            }
        };
        apply_finalized_session_write(&mut outcome, &finalized);

        if !context.live_events_enabled() {
            self.event_service
                .emit_events_before_prompt_outcome(context.coding_events());
        }
        self.event_service.emit_session_write_events(&finalized);
        self.event_service.emit_prompt_outcome(&outcome);
        Ok(outcome)
    }

    async fn execute_authorized_delegations(
        &mut self,
        context: &mut PromptTurnContext,
        decisions: &[DelegationAuthorizationDecision],
        prompt_options: PromptTurnOptions,
    ) -> Result<(), CodingSessionError> {
        let parent_capability_snapshot = context.capability_snapshot().cloned();
        for decision in decisions {
            match decision {
                DelegationAuthorizationDecision::Approved {
                    request,
                    child_delegation_depth,
                } => {
                    self.event_service.emit_delegation_approved(request);
                    let outcome = match request.target_kind {
                        ProfileKind::Agent => {
                            self.delegation_execution_service
                                .execute_agent(
                                    &self.flow_service,
                                    self.profile_registry.clone(),
                                    self.plugin_service.clone(),
                                    self.event_service.clone(),
                                    request,
                                    prompt_options.clone(),
                                    *child_delegation_depth,
                                    delegation_lineage_for_request(&[], request),
                                    parent_capability_snapshot.clone(),
                                )
                                .await
                        }
                        ProfileKind::Team => {
                            self.delegation_execution_service
                                .execute_team(
                                    &self.flow_service,
                                    self.profile_registry.clone(),
                                    self.plugin_service.clone(),
                                    self.event_service.clone(),
                                    request,
                                    prompt_options.clone(),
                                    *child_delegation_depth,
                                    delegation_lineage_for_request(&[], request),
                                    parent_capability_snapshot.clone(),
                                )
                                .await
                        }
                    };
                    self.delegation_confirmation_service.adopt_pending(
                        &mut self.persistence,
                        &mut self.pending_delegation_confirmations,
                        &self.event_service,
                        outcome.pending_confirmations,
                    )?;
                    match outcome.execution {
                        Ok(execution) => {
                            context.record_delegation_folded_update(
                                request,
                                PersistedDelegationStatus::Completed,
                                Some(execution.child_operation_id),
                                Some(execution.final_text),
                            )?;
                        }
                        Err(error) => {
                            context.record_delegation_folded_update(
                                request,
                                PersistedDelegationStatus::Failed,
                                None,
                                Some(error.to_string()),
                            )?;
                            return Err(error);
                        }
                    }
                }
                DelegationAuthorizationDecision::RequiresConfirmation {
                    request,
                    reason,
                    child_delegation_depth,
                } => {
                    context.record_delegation_folded_update(
                        request,
                        PersistedDelegationStatus::ConfirmationRequired,
                        None,
                        Some(reason.clone()),
                    )?;
                    let pending = PendingDelegationConfirmationState {
                        request: request.clone(),
                        prompt_options: prompt_options.clone(),
                        reason: reason.clone(),
                        requested_at: SystemClock.now_rfc3339(),
                        child_delegation_depth: *child_delegation_depth,
                        delegation_lineage: delegation_lineage_for_request(&[], request),
                    };
                    self.delegation_confirmation_service.queue_pending(
                        &mut self.persistence,
                        &mut self.pending_delegation_confirmations,
                        &self.event_service,
                        pending,
                        true,
                    )?;
                }
                DelegationAuthorizationDecision::Rejected { request, reason } => {
                    self.event_service.emit_delegation_rejected(request, reason);
                    context.record_delegation_folded_update(
                        request,
                        PersistedDelegationStatus::Rejected,
                        None,
                        Some(reason.clone()),
                    )?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn apply_default_agent_profile(
        &self,
        mut options: PromptTurnOptions,
    ) -> Result<PromptTurnOptions, CodingSessionError> {
        let profile_id = self.default_agent_profile_id();
        let mut diagnostics = Vec::new();
        let profile = match self.profile_registry.agent(profile_id.as_str()) {
            Some(profile) => profile,
            None => {
                diagnostics.push(CodingDiagnostic::warning(format!(
                    "default agent profile {} could not be resolved; using built-in default profile",
                    profile_id
                )));
                self.profile_registry.agent("default").ok_or_else(|| {
                    CodingSessionError::Config {
                        message: "built-in default agent profile is not available".into(),
                    }
                })?
            }
        };
        options.apply_agent_profile(profile, diagnostics)?;
        Ok(options)
    }

    pub(super) fn default_agent_profile_id(&self) -> ProfileId {
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => {
                session_service.default_agent_profile_id().clone()
            }
            SessionPersistence::NonPersistent(state) => state.default_agent_profile_id.clone(),
        }
    }

    fn prepare_prompt_context(
        &mut self,
        options: PromptTurnOptions,
        snapshot: &OperationCapabilitySnapshot,
    ) -> Result<PromptTurnContext, CodingSessionError> {
        let event_service = self.event_service.clone();
        let prompt_control_receiver = self.operation_control.take_prompt_control_receiver();
        match &mut self.persistence {
            SessionPersistence::Persistent(session_service) => {
                let replay = session_service.replay()?;
                let transaction = session_service.begin_prompt_transaction_with_snapshot(snapshot);
                let operation_id = transaction.operation_id().to_owned();
                let turn_id = transaction.turn_id().to_owned();
                let mut context =
                    PromptTurnContext::new(PromptTurnIds::new(operation_id, turn_id), options);
                context.set_plugin_service(self.plugin_service.clone());
                context.set_session_id(session_service.session_id().to_owned());
                context.set_replay(replay);
                context.set_transaction(transaction);
                if let Some(receiver) = prompt_control_receiver {
                    context.set_prompt_control_receiver(receiver);
                }
                context.enable_live_events(event_service);
                context.set_capability_snapshot(snapshot.clone());
                Ok(context)
            }
            SessionPersistence::NonPersistent(state) => {
                let mut ids = SystemIdGenerator;
                let mut context = PromptTurnContext::new(
                    PromptTurnIds::new(snapshot.operation_id.clone(), ids.next_turn_id()),
                    options,
                );
                context.set_plugin_service(self.plugin_service.clone());
                context
                    .set_non_persistent_session(state.runtime_id.clone(), state.transcript.clone());
                if let Some(receiver) = prompt_control_receiver {
                    context.set_prompt_control_receiver(receiver);
                }
                context.enable_live_events(event_service);
                context.set_capability_snapshot(snapshot.clone());
                Ok(context)
            }
        }
    }

    fn finalize_prompt_transaction(
        &mut self,
        context: &mut PromptTurnContext,
        outcome: &PromptTurnOutcome,
    ) -> Result<FinalizedSessionWrite, CodingSessionError> {
        let operation_id = context.operation_id().to_owned();
        let transaction = context.take_transaction();
        match &mut self.persistence {
            SessionPersistence::Persistent(session_service) => {
                let snapshot = context.capability_snapshot().ok_or_else(|| {
                    CodingSessionError::UnsupportedCapability {
                        capability: "prompt session write requires operation capability snapshot"
                            .into(),
                    }
                })?;
                SessionWriteCapability::require(snapshot.session_write.as_ref())?;
                session_service.finalize_prompt_transaction(transaction, operation_id, outcome)
            }
            SessionPersistence::NonPersistent(state) => {
                Ok(state.finalize_prompt_transaction(context, outcome))
            }
        }
    }
}

pub(super) fn apply_finalized_session_write(
    outcome: &mut PromptTurnOutcome,
    finalized: &FinalizedSessionWrite,
) {
    outcome.apply_success_session_write_metadata(
        finalized.session_id.clone(),
        finalized.leaf_id.clone(),
    );
}
