use crate::adapters::rpc::event_queue::{RpcProductEventQueue, RpcProductEventReceiver};
use crate::adapters::rpc::events::RpcCodingEventAdapter;
use crate::app::bootstrap::CliRunOptions;
use crate::app::cli::error::CliError;
use crate::app::cli::request::{render_diagnostics, resolve_runtime_defaults};
use crate::protocol::types::{RpcDetachStatus, RpcNegotiatedProtocolState};
use crate::protocol::version::{PRODUCT_EVENT_PROTOCOL_VERSION, UI_SNAPSHOT_PROTOCOL_VERSION};
use crate::runtime::facade::{
    AgentInvocationOutcome, AgentTeamOutcome, CodingAgentClientConnection, CodingAgentClientId,
    CodingAgentDetachOutcome, CodingAgentDraft, CodingAgentDraftId, CodingAgentDraftKind,
    CodingAgentOperationControl, CodingAgentPromptControl, CodingAgentSession, CodingSessionError,
    OperationIdempotencyKey, OperationKind, ProductEventSequence, PromptTurnOutcome,
};
use pi_agent_core::api::agent::{QueueMode, ThinkingLevel};
use pi_agent_core::api::transcript::StoredAgentMessage;
use pi_ai::api::model::Model;
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
    pub(super) session_event_stream_id: Option<String>,
    pub(super) session_events: Option<RpcProductEventReceiver>,
    pub(super) session_event_flush: Option<mpsc::UnboundedSender<oneshot::Sender<()>>>,
    pub(super) session_events_closed: bool,
    pub(super) event_adapter: RpcCodingEventAdapter,
    pub(super) adapter_applied_sequence: ProductEventSequence,
    pub(super) foreground: Option<RpcForegroundOperation>,
    pub(super) background_operations: HashMap<String, RpcBackgroundOperation>,
    pub(super) background_completion_tx: mpsc::UnboundedSender<RpcBackgroundCompletion>,
    pub(super) background_completion_rx: mpsc::UnboundedReceiver<RpcBackgroundCompletion>,
    pub(super) active_shutdown_handle:
        Option<crate::runtime::facade::CodingAgentRuntimeShutdownHandle>,
    pub(super) pending_shutdown_response: Option<Option<String>>,
    pub(super) is_compacting: bool,
    pub(super) steering: Vec<String>,
    pub(super) follow_up: Vec<String>,
    pub(super) negotiated_protocol: RpcNegotiatedProtocolState,
    pub(super) idempotency_records: HashMap<OperationIdempotencyKey, RpcIdempotencyRecord>,
    pub(super) idempotency_order: VecDeque<OperationIdempotencyKey>,
}

pub(super) struct RpcForegroundOperation {
    pub(super) done: oneshot::Receiver<CodingOperationTaskResult>,
    pub(super) operation_kind: OperationKind,
    pub(super) idempotency_key: Option<OperationIdempotencyKey>,
}

pub(super) struct RpcBackgroundOperation {
    pub(super) operation_kind: OperationKind,
    pub(super) idempotency_key: Option<OperationIdempotencyKey>,
}

pub(super) struct RpcBackgroundCompletion {
    pub(super) operation_id: String,
    pub(super) result: CodingOperationTaskResult,
}

pub(super) struct CodingOperationTaskResult {
    pub(super) session: Option<CodingAgentSession>,
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
        let resolved = resolve_runtime_defaults(&options)?;
        let diag_text = render_diagnostics(&resolved.diagnostics);
        if !diag_text.is_empty() {
            eprint!("{diag_text}");
        }
        let model = resolved.model;
        let api_key = resolved.api_key;

        let event_adapter = RpcCodingEventAdapter::new_with_provider(
            model.api.clone(),
            model.provider.clone(),
            model.id.clone(),
        );
        let (background_completion_tx, background_completion_rx) = mpsc::unbounded_channel();
        Ok(Self {
            options,
            model,
            api_key,
            settings: resolved.settings,
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
            session_event_stream_id: None,
            session_events: None,
            session_event_flush: None,
            session_events_closed: false,
            event_adapter,
            adapter_applied_sequence: ProductEventSequence::default(),
            foreground: None,
            background_operations: HashMap::new(),
            background_completion_tx,
            background_completion_rx,
            active_shutdown_handle: None,
            pending_shutdown_response: None,
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
        self.foreground.is_some()
    }

    pub(super) fn has_active_operations(&self) -> bool {
        self.foreground.is_some() || !self.background_operations.is_empty()
    }

    pub(super) fn effective_prompt_settings(&self) -> crate::config::Settings {
        let mut settings = self.settings.clone();
        settings.steering_mode = self.steering_mode.to_string();
        settings.follow_up_mode = self.follow_up_mode.to_string();
        settings.compaction.enabled = self.auto_compaction_enabled;
        settings
    }

    pub(super) fn ensure_session_event_pump(&mut self, session: &CodingAgentSession) {
        let stream_id = session.snapshot().cursor.stream_id;
        if self.session_event_stream_id.as_deref() == Some(stream_id.as_str())
            && self.session_events.is_some()
        {
            return;
        }

        let mut source = session.subscribe_product_events();
        let (sender, receiver) = RpcProductEventQueue::new();
        let (flush_tx, mut flush_rx) = mpsc::unbounded_channel::<oneshot::Sender<()>>();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    event = source.recv() => {
                        match event {
                            Ok(event) => {
                                if sender.send_event(event).await.is_err() {
                                    break;
                                }
                            }
                            Err(CodingSessionError::EventStreamLag { skipped }) => {
                                let _ = sender.send_overflow(skipped).await;
                            }
                            Err(_) => break,
                        }
                    }
                    flush = flush_rx.recv() => {
                        let Some(flush) = flush else {
                            break;
                        };
                        loop {
                            match source.try_recv() {
                                Ok(Some(event)) => {
                                    if sender.send_event(event).await.is_err() {
                                        break;
                                    }
                                }
                                Ok(None) => break,
                                Err(CodingSessionError::EventStreamLag { skipped }) => {
                                    let _ = sender.send_overflow(skipped).await;
                                    break;
                                }
                                Err(_) => break,
                            }
                        }
                        let _ = flush.send(());
                    }
                }
            }
        });
        self.session_event_stream_id = Some(stream_id);
        self.session_events = Some(receiver);
        self.session_event_flush = Some(flush_tx);
        self.session_events_closed = false;
        self.event_adapter = RpcCodingEventAdapter::new_with_provider(
            self.model.api.clone(),
            self.model.provider.clone(),
            self.model.id.clone(),
        );
        self.adapter_applied_sequence = ProductEventSequence::default();
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
        let Some(foreground) = self.foreground.as_ref() else {
            return Ok(None);
        };
        if foreground.operation_kind != OperationKind::Prompt {
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

    pub(super) fn operation_control(
        &self,
        operation_id: &str,
    ) -> Option<CodingAgentOperationControl> {
        self.client_connection
            .as_ref()
            .map(|connection| connection.operation_control(operation_id.to_owned()))
    }

    pub(super) fn active_foreground_operation_id(
        &self,
    ) -> Result<Option<String>, CodingSessionError> {
        let Some(connection) = self.client_connection.as_ref() else {
            return Ok(None);
        };
        Ok(connection
            .state()?
            .submitted_operation
            .map(|submitted| submitted.operation_id))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::bootstrap::build_agent_config_with_auth_diagnostics;
    use pi_agent_core::api::resources::AgentResources;

    #[test]
    fn rpc_mutable_modes_feed_the_next_agent_config() {
        let mut state = RpcState::new(CliRunOptions::default()).unwrap();
        state.steering_mode = QueueMode::All;
        state.follow_up_mode = QueueMode::OneAtATime;
        state.auto_compaction_enabled = false;

        let settings = state.effective_prompt_settings();
        let config = build_agent_config_with_auth_diagnostics(
            state.model.clone(),
            None,
            None,
            None,
            Vec::new(),
            None,
            None,
            AgentResources::default(),
            Some(&settings),
        );

        assert_eq!(config.steering_mode, QueueMode::All);
        assert_eq!(config.follow_up_mode, QueueMode::OneAtATime);
        assert!(config.compaction.is_none());

        state.auto_compaction_enabled = true;
        let settings = state.effective_prompt_settings();
        let config = build_agent_config_with_auth_diagnostics(
            state.model.clone(),
            None,
            None,
            None,
            Vec::new(),
            None,
            None,
            AgentResources::default(),
            Some(&settings),
        );
        assert!(config.compaction.is_some());
    }
}
