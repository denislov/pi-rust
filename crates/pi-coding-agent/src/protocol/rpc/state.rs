use crate::args::CliArgs;
use crate::error::CliError;
use crate::protocol::rpc::event_queue::RpcQueuedProductEvent;
use crate::protocol::rpc::events::RpcCodingEventAdapter;
use crate::protocol::types::{RpcDetachStatus, RpcNegotiatedProtocolState};
use crate::protocol::version::{PRODUCT_EVENT_PROTOCOL_VERSION, UI_SNAPSHOT_PROTOCOL_VERSION};
use crate::runtime::{CliRunOptions, select_model};
use crate::{
    coding_session::AgentInvocationOutcome, coding_session::AgentTeamOutcome,
    coding_session::CodingAgentClientConnection, coding_session::CodingAgentClientId,
    coding_session::CodingAgentDetachOutcome, coding_session::CodingAgentDraft,
    coding_session::CodingAgentDraftId, coding_session::CodingAgentDraftKind,
    coding_session::CodingAgentPromptControl, coding_session::CodingAgentSession,
    coding_session::CodingSessionError, coding_session::OperationIdempotencyKey,
    coding_session::OperationKind, coding_session::ProductEventSequence,
    coding_session::PromptTurnOutcome, config,
};
use pi_agent_core::api::StoredAgentMessage;
use pi_agent_core::api::{QueueMode, ThinkingLevel};
use pi_ai::api::Model;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};

const RPC_IDEMPOTENCY_RECORD_LIMIT: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RpcIdempotencyRecord {
    pub(super) command: &'static str,
    pub(super) operation_kind: OperationKind,
    pub(super) completed: bool,
}

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
    pub(super) client_connection: Option<CodingAgentClientConnection>,
    pub(super) running: Option<RunningPrompt>,
    pub(super) is_compacting: bool,
    pub(super) steering: Vec<String>,
    pub(super) follow_up: Vec<String>,
    pub(super) negotiated_protocol: RpcNegotiatedProtocolState,
    pub(super) idempotency_records: HashMap<OperationIdempotencyKey, RpcIdempotencyRecord>,
    pub(super) idempotency_order: VecDeque<OperationIdempotencyKey>,
}

pub(super) enum RunningPrompt {
    Coding(CodingRunningPrompt),
}

pub(super) struct CodingRunningPrompt {
    pub(super) events: mpsc::Receiver<RpcQueuedProductEvent>,
    pub(super) done: oneshot::Receiver<CodingOperationTaskResult>,
    pub(super) operation_kind: OperationKind,
    pub(super) adapter: RpcCodingEventAdapter,
    pub(super) adapter_applied_sequence: ProductEventSequence,
    pub(super) events_closed: bool,
    pub(super) idempotency_key: Option<OperationIdempotencyKey>,
    pub(super) shutdown_handle: crate::coding_session::CodingAgentRuntimeShutdownHandle,
    pub(super) pending_shutdown_response: Option<Option<String>>,
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
            client_connection: None,
            running: None,
            is_compacting: false,
            steering: Vec::new(),
            follow_up: Vec::new(),
            negotiated_protocol: RpcNegotiatedProtocolState {
                rpc: None,
                product_events: PRODUCT_EVENT_PROTOCOL_VERSION,
                ui_snapshot: UI_SNAPSHOT_PROTOCOL_VERSION,
            },
            idempotency_records: HashMap::new(),
            idempotency_order: VecDeque::new(),
        })
    }

    pub(super) fn is_streaming(&self) -> bool {
        self.running.is_some()
    }

    pub(super) async fn detach_client(&mut self) -> Result<RpcDetachStatus, CodingSessionError> {
        let Some(connection) = self.client_connection.take() else {
            return Ok(RpcDetachStatus::AlreadyDetached);
        };
        match connection.detach() {
            Ok(outcome) => Ok(match outcome {
                CodingAgentDetachOutcome::Detached => RpcDetachStatus::Detached,
                CodingAgentDetachOutcome::AlreadyDetached => RpcDetachStatus::AlreadyDetached,
                CodingAgentDetachOutcome::StaleGeneration => RpcDetachStatus::StaleGeneration,
            }),
            Err(error) => {
                self.client_connection = Some(connection);
                Err(error)
            }
        }
    }

    pub(super) fn ensure_client_connection(
        &mut self,
        session: &CodingAgentSession,
    ) -> Result<CodingAgentClientConnection, CliError> {
        if let Some(connection) = &self.client_connection {
            return Ok(connection.clone());
        }
        let connection = session
            .connect(CodingAgentClientId::new("rpc-primary"))
            .map_err(CliError::from)?;
        for (index, text) in self.steering.iter().enumerate() {
            connection
                .enqueue_control_draft(CodingAgentDraft {
                    id: CodingAgentDraftId(format!("rpc-steer-{index}")),
                    kind: CodingAgentDraftKind::Steer,
                    text: text.clone(),
                })
                .map_err(|reason| CliError::SessionFailure(format!("{reason:?}")))?;
        }
        for (index, text) in self.follow_up.iter().enumerate() {
            connection
                .enqueue_control_draft(CodingAgentDraft {
                    id: CodingAgentDraftId(format!("rpc-follow-up-{index}")),
                    kind: CodingAgentDraftKind::FollowUp,
                    text: text.clone(),
                })
                .map_err(|reason| CliError::SessionFailure(format!("{reason:?}")))?;
        }
        self.client_connection = Some(connection.clone());
        Ok(connection)
    }

    pub(super) fn active_prompt_control(
        &self,
    ) -> Result<Option<CodingAgentPromptControl>, CodingSessionError> {
        let Some(RunningPrompt::Coding(running)) = self.running.as_ref() else {
            return Ok(None);
        };
        if running.operation_kind != OperationKind::Prompt {
            return Ok(None);
        }
        let Some(connection) = self.client_connection.as_ref() else {
            return Ok(None);
        };
        Ok(connection
            .state()?
            .submitted_operation
            .map(|submitted| connection.prompt_control(submitted.operation_id)))
    }

    pub(super) fn parse_idempotency_key(
        &self,
        key: Option<String>,
    ) -> Result<Option<OperationIdempotencyKey>, CliError> {
        key.map(OperationIdempotencyKey::parse)
            .transpose()
            .map_err(CliError::from)
    }

    pub(super) fn idempotent_retry_response(
        &self,
        key: Option<&OperationIdempotencyKey>,
        command: &'static str,
    ) -> Result<Option<serde_json::Value>, CliError> {
        let Some(key) = key else {
            return Ok(None);
        };
        let Some(record) = self.idempotency_records.get(key) else {
            return Ok(None);
        };
        if record.command == command {
            return Ok(Some(serde_json::json!({
                "deduplicated": true,
                "operation": record.operation_kind.as_str(),
                "completed": record.completed
            })));
        }
        Err(CliError::SessionFailure(format!(
            "idempotency key was already used for {}, not {command}",
            record.command
        )))
    }

    pub(super) fn remember_idempotency_key(
        &mut self,
        key: Option<OperationIdempotencyKey>,
        command: &'static str,
        operation_kind: OperationKind,
    ) {
        let Some(key) = key else {
            return;
        };
        if !self.idempotency_records.contains_key(&key) {
            self.idempotency_order.push_back(key.clone());
        }
        self.idempotency_records.insert(
            key,
            RpcIdempotencyRecord {
                command,
                operation_kind,
                completed: false,
            },
        );
        while self.idempotency_order.len() > RPC_IDEMPOTENCY_RECORD_LIMIT {
            if let Some(expired) = self.idempotency_order.pop_front() {
                self.idempotency_records.remove(&expired);
            }
        }
    }

    pub(super) fn mark_idempotency_complete(&mut self, key: Option<&OperationIdempotencyKey>) {
        let Some(key) = key else {
            return;
        };
        if let Some(record) = self.idempotency_records.get_mut(key) {
            record.completed = true;
        }
    }
}

#[cfg(all(test, any()))]
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
        let started = ProductEvent::from_event_for_tests(
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

        let completed = ProductEvent::from_event_for_tests(
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
