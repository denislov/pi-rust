use super::*;

impl CodingAgentSession {
    #[cfg(test)]
    pub(crate) fn arm_append_events_failure_for_tests(&self, successful_calls: usize) {
        self.persistent_session_service()
            .fail_store_after_for_tests(
                session_log::store::StoreFailurePoint::AppendEvents,
                successful_calls,
            );
    }

    #[cfg(test)]
    pub(crate) fn arm_update_manifest_failure_for_tests(&self, successful_calls: usize) {
        self.persistent_session_service()
            .fail_store_after_for_tests(
                session_log::store::StoreFailurePoint::UpdateManifest,
                successful_calls,
            );
    }

    #[cfg(test)]
    pub(crate) fn queue_pending_delegation_for_tests(
        &mut self,
        operation_id: impl Into<String>,
        tool_call_id: impl Into<String>,
    ) {
        let prompt = "delegated task";
        let prompt_options =
            PromptTurnOptions::from_prompt_run_options(crate::prompt_options::PromptRunOptions {
                prompt: prompt.into(),
                model: pi_ai::api::Model {
                    id: "test-model".into(),
                    name: "Test Model".into(),
                    api: "interactive-pending-delegation-fixture".into(),
                    provider: "test".into(),
                    base_url: String::new(),
                    reasoning: false,
                    thinking_level_map: None,
                    input: vec![pi_ai::api::ModelInput::Text],
                    cost: pi_ai::api::ModelCost::default(),
                    context_window: 0,
                    max_tokens: 0,
                    headers: None,
                    compat: None,
                },
                api_key: None,
                auth_diagnostics: Vec::new(),
                system_prompt: Some("system".into()),
                max_turns: Some(2),
                tools: Vec::new(),
                register_builtins: false,
                ai_client: None,
                session: Some(crate::runtime::SessionRunOptions::disabled(".".into())),
                session_target: None,
                session_name: None,
                thinking_level: None,
                tool_execution: None,
                resources: pi_agent_core::api::AgentResources::default(),
                settings: None,
                invocation: crate::runtime::PromptInvocation::Text(prompt.into()),
            });
        let pending = PendingDelegationConfirmationState {
            request: prompt::DelegationRequest {
                operation_id: operation_id.into(),
                turn_id: "turn_interactive_fixture".into(),
                tool_call_id: tool_call_id.into(),
                requesting_profile_id: ProfileId::from("parent"),
                target_kind: ProfileKind::Agent,
                target_id: ProfileId::from("default"),
                task: "delegated task".into(),
            },
            prompt_options,
            reason: "requires confirmation".into(),
            requested_at: SystemClock.now_rfc3339(),
            child_delegation_depth: 1,
            delegation_lineage: Vec::new(),
        };
        self.delegation_confirmation_service
            .queue_pending(
                &mut self.persistence,
                &mut self.pending_delegation_confirmations,
                &self.event_service,
                pending,
                true,
            )
            .expect("pending delegation fixture requires a persistent session");
    }

    #[cfg(test)]
    pub(super) fn persistent_session_service(&self) -> &SessionService {
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => session_service,
            SessionPersistence::NonPersistent(_) => {
                panic!("expected persistent coding agent session")
            }
        }
    }

    #[cfg(test)]
    pub(super) fn current_capability_generation_for_tests(
        &self,
    ) -> capability_snapshot::CapabilityGeneration {
        self.capability_snapshots.current_generation()
    }
}
