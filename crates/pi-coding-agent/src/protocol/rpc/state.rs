use crate::protocol::rpc::event_queue::RpcQueuedProductEvent;
use crate::protocol::rpc::events::RpcCodingEventAdapter;
use crate::{
    CliArgs, CliError, CliRunOptions, coding_session::AgentInvocationOutcome,
    coding_session::AgentTeamOutcome, coding_session::ClientConnectionId,
    coding_session::ClientDraft, coding_session::ClientDraftKind,
    coding_session::CodingAgentSession, coding_session::OperationKind,
    coding_session::ProductEvent, coding_session::ProductEventReplayHandle,
    coding_session::ProductEventSequence, coding_session::PromptControlHandle,
    coding_session::PromptTurnOutcome, coding_session::SubmittedOperation, config, select_model,
};
use pi_agent_core::transcript::StoredAgentMessage;
use pi_agent_core::{QueueMode, ThinkingLevel};
use pi_ai::types::Model;
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};

pub(super) struct RpcState {
    pub(super) options: CliRunOptions,
    pub(super) model: Model,
    pub(super) api_key: Option<String>,
    pub(super) settings: crate::config::Settings,
    pub(super) thinking_level: ThinkingLevel,
    pub(super) steering_mode: QueueMode,
    pub(super) follow_up_mode: QueueMode,
    pub(super) auto_compaction_enabled: bool,
    pub(super) session_name: Option<String>,
    pub(super) active_session_path: Option<PathBuf>,
    pub(super) active_leaf_id: Option<String>,
    pub(super) messages: Vec<StoredAgentMessage>,
    pub(super) coding_session: Option<CodingAgentSession>,
    pub(super) client_id: Option<ClientConnectionId>,
    pub(super) client_drafts: Vec<ClientDraft>,
    pub(super) submitted_operation: Option<SubmittedOperation>,
    pub(super) running: Option<RunningPrompt>,
    pub(super) is_compacting: bool,
    pub(super) steering: Vec<String>,
    pub(super) follow_up: Vec<String>,
}

pub(super) enum RunningPrompt {
    Coding(CodingRunningPrompt),
}

pub(super) struct CodingRunningPrompt {
    pub(super) events: mpsc::Receiver<RpcQueuedProductEvent>,
    pub(super) done: oneshot::Receiver<CodingOperationTaskResult>,
    pub(super) control: Option<PromptControlHandle>,
    pub(super) operation_kind: OperationKind,
    pub(super) adapter: RpcCodingEventAdapter,
    pub(super) product_event_replay: Option<ProductEventReplayHandle>,
    pub(super) adapter_applied_sequence: ProductEventSequence,
    pub(super) replayed_through_sequence: ProductEventSequence,
    pub(super) events_closed: bool,
}

pub(super) struct CodingOperationTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) session_root: Option<PathBuf>,
    pub(super) outcome: CodingOperationOutcome,
}

pub(super) enum CodingOperationOutcome {
    Prompt(Result<PromptTurnOutcome, CliError>),
    AgentInvocation(Result<AgentInvocationOutcome, CliError>),
    AgentTeam(Result<AgentTeamOutcome, CliError>),
    DelegationApproval(Result<(), CliError>),
}

impl RpcState {
    pub(super) fn new(options: CliRunOptions) -> Result<Self, CliError> {
        let cwd = options.session.cwd.clone();
        let (config, config_diags) = config::load_config(&cwd);
        let diagnostics = config_diags
            .iter()
            .map(crate::request::CliDiagnostic::from_config)
            .collect::<Vec<_>>();
        let diag_text = crate::request::render_diagnostics(&diagnostics);
        if !diag_text.is_empty() {
            eprint!("{diag_text}");
        }
        let args = CliArgs::default();
        let model = select_model(
            &args,
            config.settings.default_provider.as_deref(),
            config.settings.default_model.as_deref(),
            options.model_override.clone(),
        )?;
        let api_key = {
            let mut key_diags = Vec::new();
            let resolved =
                config::auth::resolve_api_key(&model.provider, None, &config.auth, &mut key_diags);
            let key_diagnostics = key_diags
                .iter()
                .map(crate::request::CliDiagnostic::from_config)
                .collect::<Vec<_>>();
            let key_text = crate::request::render_diagnostics(&key_diagnostics);
            if !key_text.is_empty() {
                eprint!("{key_text}");
            }
            resolved.map(|r| r.value)
        };

        Ok(Self {
            options,
            model,
            api_key,
            settings: config.settings,
            thinking_level: ThinkingLevel::Off,
            steering_mode: QueueMode::OneAtATime,
            follow_up_mode: QueueMode::OneAtATime,
            auto_compaction_enabled: true,
            session_name: None,
            active_session_path: None,
            active_leaf_id: None,
            messages: Vec::new(),
            coding_session: None,
            client_id: Some(ClientConnectionId::new("rpc-primary")),
            client_drafts: Vec::new(),
            submitted_operation: None,
            running: None,
            is_compacting: false,
            steering: Vec::new(),
            follow_up: Vec::new(),
        })
    }

    pub(super) fn is_streaming(&self) -> bool {
        self.running.is_some()
    }

    pub(super) fn mirror_client_draft(&mut self, kind: ClientDraftKind, message: String) {
        self.client_drafts.push(ClientDraft::new(kind, message));
    }

    pub(super) fn clear_client_drafts(&mut self, kind: ClientDraftKind) {
        self.client_drafts.retain(|draft| draft.kind != kind);
    }

    pub(super) fn clear_client_state(&mut self) {
        self.client_drafts.clear();
        self.submitted_operation = None;
    }

    pub(super) fn mark_submitted(&mut self, submitted: SubmittedOperation) {
        if submitted.kind == OperationKind::Prompt {
            self.clear_client_drafts(ClientDraftKind::Prompt);
        }
        self.submitted_operation = Some(submitted);
    }

    pub(super) fn clear_submitted_operation(&mut self, operation_id: &str) {
        if self
            .submitted_operation
            .as_ref()
            .is_some_and(|submitted| submitted.operation_id == operation_id)
        {
            self.submitted_operation = None;
        }
    }

    pub(super) fn observe_product_event_submission_for_kind(
        &mut self,
        event: &ProductEvent,
        operation_kind: Option<OperationKind>,
    ) {
        let operation_id = event.operation_id();
        if self.submitted_operation.is_none()
            && operation_kind == Some(OperationKind::Prompt)
            && let Some(operation_id) = operation_id
        {
            self.mark_submitted(SubmittedOperation {
                operation_id: operation_id.to_owned(),
                kind: OperationKind::Prompt,
            });
        }

        if let Some(terminal) = event.terminal_operation()
            && terminal.kind == OperationKind::Prompt
            && let Some(operation_id) = operation_id
        {
            self.clear_submitted_operation(operation_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CliRunOptions;
    use crate::coding_session::CodingAgentEvent;

    #[test]
    fn queued_control_messages_are_mirrored_as_client_local_drafts() {
        let mut state = RpcState::new(CliRunOptions::default()).unwrap();

        state.enqueue_steer("look here".into());
        state.enqueue_follow_up("next step".into());

        assert_eq!(state.steering, vec!["look here"]);
        assert_eq!(state.follow_up, vec!["next step"]);
        assert_eq!(
            state.client_drafts,
            vec![
                ClientDraft::new(ClientDraftKind::Steer, "look here"),
                ClientDraft::new(ClientDraftKind::FollowUp, "next step"),
            ]
        );
        assert!(state.submitted_operation.is_none());
    }

    #[test]
    fn explicit_prompt_operation_observer_updates_submission_state_without_running_prompt() {
        let mut state = RpcState::new(CliRunOptions::default()).unwrap();
        state.client_drafts = vec![ClientDraft::new(ClientDraftKind::Prompt, "draft prompt")];
        let started = ProductEvent::from_compat_event(
            ProductEventSequence::new(1),
            CodingAgentEvent::PromptStarted {
                operation_id: "op_observed".into(),
                turn_id: "turn_observed".into(),
            },
        );

        state.observe_product_event_submission_for_kind(&started, Some(OperationKind::Prompt));

        assert!(
            state
                .client_drafts
                .iter()
                .all(|draft| draft.kind != ClientDraftKind::Prompt)
        );
        assert_eq!(
            state.submitted_operation,
            Some(SubmittedOperation {
                operation_id: "op_observed".into(),
                kind: OperationKind::Prompt,
            })
        );

        let completed = ProductEvent::from_compat_event(
            ProductEventSequence::new(2),
            CodingAgentEvent::PromptCompleted {
                operation_id: "op_observed".into(),
                turn_id: "turn_observed".into(),
            },
        );

        state.observe_product_event_submission_for_kind(&completed, Some(OperationKind::Prompt));

        assert!(state.submitted_operation.is_none());
    }
}
