use crate::CliError;
use crate::coding_session::{
    AgentInvocationOptions, AgentTeamOptions, ClientDraftKind, CodingAgentSession,
    CodingAgentSessionOptions, CodingSessionError, OperationKind, ProductEvent,
    ProductEventSequence, ProfileId, ProfileKind, PromptTurnMode, PromptTurnOptions,
};
use crate::prompt_options::PromptRunOptions;
use crate::protocol::rpc::commands::{has_images, rpc_pending_delegation_confirmation};
use crate::protocol::rpc::events::RpcCodingEventAdapter;
use crate::protocol::rpc::state::{
    CodingOperationOutcome, CodingOperationTaskResult, CodingRunningPrompt, RpcState, RunningPrompt,
};
use crate::protocol::rpc::wire::{write_json_line, write_rpc_response};
use crate::protocol::types::{ProtocolEvent, RpcResponse, StreamingBehavior};
use crate::runtime::{PromptInvocation, SessionMode, SessionRunOptions};
use crate::session::resolve_session_dir;
use pi_agent_core::AgentResources;
use std::path::PathBuf;
use tokio::io::AsyncWrite;
use tokio::sync::{mpsc, oneshot};

impl RpcState {
    pub(super) async fn handle_prompt<W>(
        &mut self,
        id: Option<String>,
        message: String,
        images: Option<Vec<pi_ai::types::ContentBlock>>,
        streaming_behavior: Option<StreamingBehavior>,
        after_snapshot_sequence: Option<ProductEventSequence>,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        if has_images(&images) {
            write_rpc_response(
                writer,
                RpcResponse::error(
                    id,
                    "prompt",
                    "image prompt payloads are not supported in Rust M5 RPC mode",
                ),
            )
            .await?;
            return Ok(());
        }

        if self.is_streaming() {
            self.handle_streaming_prompt(
                id,
                message,
                streaming_behavior,
                after_snapshot_sequence,
                writer,
            )
            .await?;
            return Ok(());
        }

        self.start_coding_session_prompt(id, message, writer).await
    }

    async fn handle_streaming_prompt<W>(
        &mut self,
        id: Option<String>,
        message: String,
        streaming_behavior: Option<StreamingBehavior>,
        after_snapshot_sequence: Option<ProductEventSequence>,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        if let Some(cursor) = after_snapshot_sequence {
            let replayed = match reconnect_running_prompt_after(self, cursor).await {
                Ok(replayed) => replayed,
                Err(error @ CodingSessionError::EventStreamGap { .. }) => {
                    write_rpc_response(
                        writer,
                        RpcResponse::error_with_data(
                            id,
                            "prompt",
                            error.to_string(),
                            serde_json::json!({ "code": error.code() }),
                        ),
                    )
                    .await?;
                    return Ok(());
                }
                Err(error) => return Err(CliError::from(error)),
            };

            if streaming_behavior.is_none() {
                write_rpc_response(writer, RpcResponse::success(id, "prompt", None)).await?;
                for event in replayed {
                    write_json_line(writer, &event).await?;
                }
                return Ok(());
            }

            self.handle_streaming_prompt_control(id, message, streaming_behavior, writer)
                .await?;
            for event in replayed {
                write_json_line(writer, &event).await?;
            }
            return Ok(());
        }

        self.handle_streaming_prompt_control(id, message, streaming_behavior, writer)
            .await
    }

    async fn handle_streaming_prompt_control<W>(
        &mut self,
        id: Option<String>,
        message: String,
        streaming_behavior: Option<StreamingBehavior>,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let Some(RunningPrompt::Coding(running)) = self.running.as_ref() else {
            write_rpc_response(
                writer,
                RpcResponse::error(id, "prompt", "agent is not streaming"),
            )
            .await?;
            return Ok(());
        };

        let Some(control) = running.control.as_ref() else {
            write_rpc_response(
                writer,
                RpcResponse::error(
                    id,
                    "prompt",
                    format!(
                        "cannot send prompt control while {} is running",
                        running.operation_kind.as_str()
                    ),
                ),
            )
            .await?;
            return Ok(());
        };

        let result = match streaming_behavior {
            Some(StreamingBehavior::Steer) => control.steer(message),
            Some(StreamingBehavior::FollowUp) => control.follow_up(message),
            None => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(
                        id,
                        "prompt",
                        "agent is streaming; prompt requires streamingBehavior steer or followUp",
                    ),
                )
                .await?;
                return Ok(());
            }
        };

        match result {
            Ok(()) => write_rpc_response(writer, RpcResponse::success(id, "prompt", None)).await,
            Err(error) => {
                write_rpc_response(writer, RpcResponse::error(id, "prompt", error.to_string()))
                    .await
            }
        }
    }

    pub(super) async fn handle_invoke_agent<W>(
        &mut self,
        id: Option<String>,
        profile_id: String,
        task: String,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        if self.is_streaming() {
            write_rpc_response(
                writer,
                RpcResponse::error(
                    id,
                    "invoke_agent",
                    "cannot invoke agent while agent is streaming",
                ),
            )
            .await?;
            return Ok(());
        }

        if task.trim().is_empty() {
            write_rpc_response(
                writer,
                RpcResponse::error(
                    id,
                    "invoke_agent",
                    "agent invocation requires a non-empty task",
                ),
            )
            .await?;
            return Ok(());
        }

        let profile_id = match ProfileId::new(profile_id) {
            Ok(profile_id) => profile_id,
            Err(message) => {
                write_rpc_response(writer, RpcResponse::error(id, "invoke_agent", message)).await?;
                return Ok(());
            }
        };

        let session_root = if matches!(self.options.session.mode, SessionMode::Enabled) {
            Some(rpc_coding_session_root(&self.options.session)?)
        } else {
            None
        };
        let mut session = match self.coding_session.take() {
            Some(session) => session,
            None => match session_root.as_ref() {
                Some(session_root) => {
                    CodingAgentSession::create(
                        CodingAgentSessionOptions::new()
                            .with_cwd(self.options.session.cwd.clone())
                            .with_session_log_root(session_root.clone()),
                    )
                    .await?
                }
                None => {
                    CodingAgentSession::non_persistent(
                        CodingAgentSessionOptions::new().with_cwd(self.options.session.cwd.clone()),
                    )
                    .await?
                }
            },
        };

        if !session
            .agent_profiles()
            .iter()
            .any(|profile| profile.id.as_str() == profile_id.as_str())
        {
            self.coding_session = Some(session);
            write_rpc_response(
                writer,
                RpcResponse::error(
                    id,
                    "invoke_agent",
                    format!("Unknown agent profile: {profile_id}"),
                ),
            )
            .await?;
            return Ok(());
        }

        let prompt_options = PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
            prompt: task.clone(),
            model: self.model.clone(),
            api_key: self.api_key.clone(),
            auth_diagnostics: Vec::new(),
            system_prompt: None,
            max_turns: None,
            tools: self.options.tools.clone(),
            register_builtins: false,
            session: Some(self.options.session.clone()),
            session_target: None,
            session_name: self.session_name.clone(),
            thinking_level: Some(self.thinking_level),
            tool_execution: None,
            resources: AgentResources::default(),
            settings: Some(self.settings.clone()),
            invocation: PromptInvocation::Text(task.clone()),
        })
        .with_mode(PromptTurnMode::Rpc);
        let invocation_options =
            AgentInvocationOptions::new(profile_id.clone(), task.clone(), prompt_options);
        let mut receiver = session.subscribe_product_events();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        let product_event_replay = session.product_event_replay_handle();

        write_rpc_response(
            writer,
            RpcResponse::success(
                id,
                "invoke_agent",
                Some(serde_json::json!({
                    "profileId": profile_id.as_str(),
                    "task": task,
                })),
            ),
        )
        .await?;
        write_json_line(writer, &ProtocolEvent::AgentStart).await?;

        tokio::spawn(async move {
            let outcome = {
                let mut invocation = Box::pin(session.invoke_agent(invocation_options));
                loop {
                    tokio::select! {
                        event = receiver.recv() => {
                            if let Ok(event) = event {
                                let _ = event_tx.send(event);
                            }
                        }
                        outcome = &mut invocation => {
                            break outcome.map_err(CliError::from);
                        }
                    }
                }
            };

            while let Ok(Some(event)) = receiver.try_recv() {
                let _ = event_tx.send(event);
            }

            let _ = done_tx.send(CodingOperationTaskResult {
                session,
                session_root,
                outcome: CodingOperationOutcome::AgentInvocation(outcome),
            });
        });

        self.running = Some(RunningPrompt::Coding(CodingRunningPrompt {
            events: event_rx,
            done: done_rx,
            control: None,
            operation_kind: OperationKind::AgentInvocation,
            adapter: RpcCodingEventAdapter::new_with_provider(
                self.model.api.clone(),
                self.model.provider.clone(),
                self.model.id.clone(),
            ),
            product_event_replay: Some(product_event_replay),
            adapter_applied_sequence: ProductEventSequence::default(),
            replayed_through_sequence: ProductEventSequence::default(),
            events_closed: false,
        }));

        Ok(())
    }

    pub(super) async fn handle_invoke_team<W>(
        &mut self,
        id: Option<String>,
        team_id: String,
        task: String,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        if self.is_streaming() {
            write_rpc_response(
                writer,
                RpcResponse::error(
                    id,
                    "invoke_team",
                    "cannot invoke team while agent is streaming",
                ),
            )
            .await?;
            return Ok(());
        }

        if task.trim().is_empty() {
            write_rpc_response(
                writer,
                RpcResponse::error(
                    id,
                    "invoke_team",
                    "agent team invocation requires a non-empty task",
                ),
            )
            .await?;
            return Ok(());
        }

        let team_id = match ProfileId::new(team_id) {
            Ok(team_id) => team_id,
            Err(message) => {
                write_rpc_response(writer, RpcResponse::error(id, "invoke_team", message)).await?;
                return Ok(());
            }
        };

        let session_root = if matches!(self.options.session.mode, SessionMode::Enabled) {
            Some(rpc_coding_session_root(&self.options.session)?)
        } else {
            None
        };
        let mut session = match self.coding_session.take() {
            Some(session) => session,
            None => match session_root.as_ref() {
                Some(session_root) => {
                    CodingAgentSession::create(
                        CodingAgentSessionOptions::new()
                            .with_cwd(self.options.session.cwd.clone())
                            .with_session_log_root(session_root.clone()),
                    )
                    .await?
                }
                None => {
                    CodingAgentSession::non_persistent(
                        CodingAgentSessionOptions::new().with_cwd(self.options.session.cwd.clone()),
                    )
                    .await?
                }
            },
        };

        if !session
            .team_profiles()
            .iter()
            .any(|team| team.id.as_str() == team_id.as_str())
        {
            self.coding_session = Some(session);
            write_rpc_response(
                writer,
                RpcResponse::error(
                    id,
                    "invoke_team",
                    format!("Unknown team profile: {team_id}"),
                ),
            )
            .await?;
            return Ok(());
        }

        let prompt_options = PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
            prompt: task.clone(),
            model: self.model.clone(),
            api_key: self.api_key.clone(),
            auth_diagnostics: Vec::new(),
            system_prompt: None,
            max_turns: None,
            tools: self.options.tools.clone(),
            register_builtins: false,
            session: Some(self.options.session.clone()),
            session_target: None,
            session_name: self.session_name.clone(),
            thinking_level: Some(self.thinking_level),
            tool_execution: None,
            resources: AgentResources::default(),
            settings: Some(self.settings.clone()),
            invocation: PromptInvocation::Text(task.clone()),
        })
        .with_mode(PromptTurnMode::Rpc);
        let team_options = AgentTeamOptions::new(team_id.clone(), task.clone(), prompt_options);
        let mut receiver = session.subscribe_product_events();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        let product_event_replay = session.product_event_replay_handle();

        write_rpc_response(
            writer,
            RpcResponse::success(
                id,
                "invoke_team",
                Some(serde_json::json!({
                    "teamId": team_id.as_str(),
                    "task": task,
                })),
            ),
        )
        .await?;
        write_json_line(writer, &ProtocolEvent::AgentStart).await?;

        tokio::spawn(async move {
            let outcome = {
                let mut invocation = Box::pin(session.invoke_team(team_options));
                loop {
                    tokio::select! {
                        event = receiver.recv() => {
                            if let Ok(event) = event {
                                let _ = event_tx.send(event);
                            }
                        }
                        outcome = &mut invocation => {
                            break outcome.map_err(CliError::from);
                        }
                    }
                }
            };

            while let Ok(Some(event)) = receiver.try_recv() {
                let _ = event_tx.send(event);
            }

            let _ = done_tx.send(CodingOperationTaskResult {
                session,
                session_root,
                outcome: CodingOperationOutcome::AgentTeam(outcome),
            });
        });

        self.running = Some(RunningPrompt::Coding(CodingRunningPrompt {
            events: event_rx,
            done: done_rx,
            control: None,
            operation_kind: OperationKind::AgentTeam,
            adapter: RpcCodingEventAdapter::new_with_provider(
                self.model.api.clone(),
                self.model.provider.clone(),
                self.model.id.clone(),
            ),
            product_event_replay: Some(product_event_replay),
            adapter_applied_sequence: ProductEventSequence::default(),
            replayed_through_sequence: ProductEventSequence::default(),
            events_closed: false,
        }));

        Ok(())
    }

    pub(super) async fn handle_approve_delegation<W>(
        &mut self,
        id: Option<String>,
        operation_id: String,
        tool_call_id: String,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        if self.is_streaming() {
            write_rpc_response(
                writer,
                RpcResponse::error(
                    id,
                    "approve_delegation",
                    "cannot approve delegation while agent is streaming",
                ),
            )
            .await?;
            return Ok(());
        }

        let Some(mut session) = self.coding_session.take() else {
            write_rpc_response(
                writer,
                RpcResponse::error(id, "approve_delegation", "no active coding session"),
            )
            .await?;
            return Ok(());
        };

        let pending = match session
            .pending_delegation_confirmations()
            .into_iter()
            .find(|pending| {
                pending.operation_id == operation_id && pending.tool_call_id == tool_call_id
            }) {
            Some(pending) => pending,
            None => {
                self.coding_session = Some(session);
                write_rpc_response(
                    writer,
                    RpcResponse::error(
                        id,
                        "approve_delegation",
                        format!(
                            "pending delegation confirmation not found: operation_id={operation_id}, tool_call_id={tool_call_id}"
                        ),
                    ),
                )
                .await?;
                return Ok(());
            }
        };
        let operation_kind = match pending.target_kind {
            ProfileKind::Agent => OperationKind::AgentInvocation,
            ProfileKind::Team => OperationKind::AgentTeam,
        };
        let session_root = if matches!(self.options.session.mode, SessionMode::Enabled) {
            Some(rpc_coding_session_root(&self.options.session)?)
        } else {
            None
        };
        let mut receiver = session.subscribe_product_events();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        let product_event_replay = session.product_event_replay_handle();

        write_rpc_response(
            writer,
            RpcResponse::success(
                id,
                "approve_delegation",
                Some(serde_json::json!({
                    "delegation": rpc_pending_delegation_confirmation(&pending),
                })),
            ),
        )
        .await?;
        write_json_line(writer, &ProtocolEvent::AgentStart).await?;

        tokio::spawn(async move {
            let outcome = {
                let mut approval =
                    Box::pin(session.approve_delegation_confirmation(operation_id, tool_call_id));
                loop {
                    tokio::select! {
                        event = receiver.recv() => {
                            if let Ok(event) = event {
                                let _ = event_tx.send(event);
                            }
                        }
                        outcome = &mut approval => {
                            break outcome.map_err(CliError::from);
                        }
                    }
                }
            };

            while let Ok(Some(event)) = receiver.try_recv() {
                let _ = event_tx.send(event);
            }

            let _ = done_tx.send(CodingOperationTaskResult {
                session,
                session_root,
                outcome: CodingOperationOutcome::DelegationApproval(outcome),
            });
        });

        self.running = Some(RunningPrompt::Coding(CodingRunningPrompt {
            events: event_rx,
            done: done_rx,
            control: None,
            operation_kind,
            adapter: RpcCodingEventAdapter::new_with_provider(
                self.model.api.clone(),
                self.model.provider.clone(),
                self.model.id.clone(),
            ),
            product_event_replay: Some(product_event_replay),
            adapter_applied_sequence: ProductEventSequence::default(),
            replayed_through_sequence: ProductEventSequence::default(),
            events_closed: false,
        }));

        Ok(())
    }

    async fn start_coding_session_prompt<W>(
        &mut self,
        id: Option<String>,
        message: String,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let session_root = if matches!(self.options.session.mode, SessionMode::Enabled) {
            Some(rpc_coding_session_root(&self.options.session)?)
        } else {
            None
        };
        let mut session = match self.coding_session.take() {
            Some(session) => session,
            None => match session_root.as_ref() {
                Some(session_root) => {
                    CodingAgentSession::create(
                        CodingAgentSessionOptions::new()
                            .with_cwd(self.options.session.cwd.clone())
                            .with_session_log_root(session_root.clone()),
                    )
                    .await?
                }
                None => {
                    CodingAgentSession::non_persistent(
                        CodingAgentSessionOptions::new().with_cwd(self.options.session.cwd.clone()),
                    )
                    .await?
                }
            },
        };
        let prompt_options = PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
            prompt: message.clone(),
            model: self.model.clone(),
            api_key: self.api_key.clone(),
            auth_diagnostics: Vec::new(),
            system_prompt: None,
            max_turns: None,
            tools: self.options.tools.clone(),
            register_builtins: false,
            session: Some(self.options.session.clone()),
            session_target: None,
            session_name: self.session_name.clone(),
            thinking_level: Some(self.thinking_level),
            tool_execution: None,
            resources: AgentResources::default(),
            settings: Some(self.settings.clone()),
            invocation: PromptInvocation::Text(message),
        })
        .with_mode(PromptTurnMode::Rpc);
        let control = session.prompt_control_handle()?;
        let mut receiver = session.subscribe_product_events();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        let product_event_replay = session.product_event_replay_handle();

        write_rpc_response(writer, RpcResponse::success(id, "prompt", None)).await?;
        write_json_line(writer, &ProtocolEvent::AgentStart).await?;

        tokio::spawn(async move {
            let outcome = {
                let mut prompt = Box::pin(session.prompt(prompt_options));
                loop {
                    tokio::select! {
                        event = receiver.recv() => {
                            if let Ok(event) = event {
                                if event_tx.send(event).is_err() {
                                    continue;
                                }
                            }
                        }
                        outcome = &mut prompt => {
                            break outcome.map_err(CliError::from);
                        }
                    }
                }
            };

            while let Ok(Some(event)) = receiver.try_recv() {
                let _ = event_tx.send(event);
            }

            let _ = done_tx.send(CodingOperationTaskResult {
                session,
                session_root,
                outcome: CodingOperationOutcome::Prompt(outcome),
            });
        });

        self.running = Some(RunningPrompt::Coding(CodingRunningPrompt {
            events: event_rx,
            done: done_rx,
            control: Some(control),
            operation_kind: OperationKind::Prompt,
            adapter: RpcCodingEventAdapter::new_with_provider(
                self.model.api.clone(),
                self.model.provider.clone(),
                self.model.id.clone(),
            ),
            product_event_replay: Some(product_event_replay),
            adapter_applied_sequence: ProductEventSequence::default(),
            replayed_through_sequence: ProductEventSequence::default(),
            events_closed: false,
        }));

        Ok(())
    }

    pub(super) fn enqueue_steer(&mut self, message: String) {
        self.mirror_client_draft(ClientDraftKind::Steer, message.clone());
        self.steering.push(message);
    }

    pub(super) fn enqueue_follow_up(&mut self, message: String) {
        self.mirror_client_draft(ClientDraftKind::FollowUp, message.clone());
        self.follow_up.push(message);
    }

    pub(super) async fn write_product_event<W>(
        &mut self,
        event: ProductEvent,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let (operation_kind, pushed) = {
            let Some(RunningPrompt::Coding(running)) = self.running.as_mut() else {
                return Ok(());
            };
            let operation_kind = running.operation_kind;
            let pushed = push_live_product_event(running, &event);
            (operation_kind, pushed)
        };
        if pushed.accepted {
            self.observe_product_event_submission_for_kind(&event, Some(operation_kind));
        }
        for protocol_event in pushed.protocol_events {
            write_json_line(writer, &protocol_event).await?;
        }
        Ok(())
    }

    pub(super) async fn finish_coding_running_prompt<W>(
        &mut self,
        result: Result<CodingOperationTaskResult, oneshot::error::RecvError>,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let Some(RunningPrompt::Coding(mut running)) = self.running.take() else {
            return Ok(());
        };
        let operation_kind = running.operation_kind;

        while let Ok(event) = running.events.try_recv() {
            let pushed = push_live_product_event(&mut running, &event);
            if pushed.accepted {
                self.observe_product_event_submission_for_kind(&event, Some(operation_kind));
            }
            for protocol_event in pushed.protocol_events {
                write_json_line(writer, &protocol_event).await?;
            }
        }

        let result = result.map_err(|error| {
            CliError::AgentFailure(format!(
                "coding agent task ended before reporting completion: {error}"
            ))
        })?;

        let outcome = match &result.outcome {
            CodingOperationOutcome::Prompt(outcome) => {
                if let Ok(outcome) = outcome
                    && let (Some(session_root), Some(session_id)) = (
                        result.session_root.as_ref(),
                        prompt_outcome_session_id(outcome),
                    )
                {
                    self.active_leaf_id = prompt_outcome_leaf_id(outcome).map(ToString::to_string);
                    self.active_session_path = Some(session_root.join(session_id));
                } else if outcome.is_ok() {
                    self.active_leaf_id = None;
                    self.active_session_path = None;
                }
                outcome.as_ref().map(|_| ()).map_err(Clone::clone)
            }
            CodingOperationOutcome::AgentInvocation(outcome) => {
                outcome.as_ref().map(|_| ()).map_err(Clone::clone)
            }
            CodingOperationOutcome::AgentTeam(outcome) => {
                outcome.as_ref().map(|_| ()).map_err(Clone::clone)
            }
            CodingOperationOutcome::DelegationApproval(outcome) => {
                outcome.as_ref().map(|_| ()).map_err(Clone::clone)
            }
        };

        self.coding_session = Some(result.session);
        self.steering.clear();
        self.follow_up.clear();
        self.clear_client_drafts(ClientDraftKind::Steer);
        self.clear_client_drafts(ClientDraftKind::FollowUp);
        outcome
    }

    pub(super) async fn emit_queue_update<W>(&self, writer: &mut W) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        write_json_line(
            writer,
            &ProtocolEvent::QueueUpdate {
                steering: self.steering.clone(),
                follow_up: self.follow_up.clone(),
            },
        )
        .await
    }
}

pub(super) async fn reconnect_running_prompt_after(
    state: &mut RpcState,
    cursor: ProductEventSequence,
) -> Result<Vec<ProtocolEvent>, CodingSessionError> {
    let Some(RunningPrompt::Coding(mut running)) = state.running.take() else {
        return Ok(Vec::new());
    };
    let Some(replay) = running.product_event_replay.as_ref() else {
        state.running = Some(RunningPrompt::Coding(running));
        return Ok(Vec::new());
    };
    let retained_events = match replay.product_events_after(cursor) {
        Ok(events) => events,
        Err(error) => {
            state.running = Some(RunningPrompt::Coding(running));
            return Err(error);
        }
    };

    let mut protocol_events = Vec::new();
    let operation_kind = running.operation_kind;
    for event in retained_events {
        if event.sequence() <= running.adapter_applied_sequence {
            continue;
        }
        let sequence = event.sequence();
        state.observe_product_event_submission_for_kind(&event, Some(operation_kind));
        protocol_events.extend(running.adapter.push_product_event(&event));
        running.adapter_applied_sequence = running.adapter_applied_sequence.max(sequence);
        running.replayed_through_sequence = running.replayed_through_sequence.max(sequence);
    }
    state.running = Some(RunningPrompt::Coding(running));
    Ok(protocol_events)
}

struct LiveProductEventPush {
    accepted: bool,
    protocol_events: Vec<ProtocolEvent>,
}

fn push_live_product_event(
    running: &mut CodingRunningPrompt,
    event: &ProductEvent,
) -> LiveProductEventPush {
    let sequence = event.sequence();
    if sequence <= running.replayed_through_sequence || sequence <= running.adapter_applied_sequence
    {
        return LiveProductEventPush {
            accepted: false,
            protocol_events: Vec::new(),
        };
    }
    let protocol_events = running.adapter.push_product_event(event);
    running.adapter_applied_sequence = running.adapter_applied_sequence.max(sequence);
    LiveProductEventPush {
        accepted: true,
        protocol_events,
    }
}

fn rpc_coding_session_root(options: &SessionRunOptions) -> Result<PathBuf, CliError> {
    match options.session_dir.as_ref() {
        Some(root) => Ok(root.clone()),
        None => resolve_session_dir(&options.cwd, None, None),
    }
}

fn prompt_outcome_session_id(outcome: &crate::coding_session::PromptTurnOutcome) -> Option<&str> {
    match outcome {
        crate::coding_session::PromptTurnOutcome::Success { session_id, .. } => {
            session_id.as_deref()
        }
        crate::coding_session::PromptTurnOutcome::Aborted { session_id, .. } => {
            session_id.as_deref()
        }
        crate::coding_session::PromptTurnOutcome::Failed { .. } => None,
    }
}

fn prompt_outcome_leaf_id(outcome: &crate::coding_session::PromptTurnOutcome) -> Option<&str> {
    match outcome {
        crate::coding_session::PromptTurnOutcome::Success { leaf_id, .. } => leaf_id.as_deref(),
        crate::coding_session::PromptTurnOutcome::Aborted { .. }
        | crate::coding_session::PromptTurnOutcome::Failed { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CliRunOptions;
    use crate::coding_session::{
        ClientDraft, CodingAgentEvent, CodingAgentSessionOptions, CodingSessionError, ProductEvent,
        ProductEventReplayHandle, ProductEventSequence, PromptTurnOutcome, SubmittedOperation,
    };
    use std::pin::Pin;
    use std::task::{Context, Poll};

    #[derive(Default)]
    struct TestWriter {
        bytes: Vec<u8>,
    }

    impl tokio::io::AsyncWrite for TestWriter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            self.bytes.extend_from_slice(buf);
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    fn state_with_running_prompt_replay(replay: ProductEventReplayHandle) -> RpcState {
        let mut state = RpcState::new(CliRunOptions::default()).unwrap();
        let (_event_tx, event_rx) = mpsc::unbounded_channel();
        let (_done_tx, done_rx) = oneshot::channel();
        state.running = Some(RunningPrompt::Coding(CodingRunningPrompt {
            events: event_rx,
            done: done_rx,
            control: None,
            operation_kind: OperationKind::Prompt,
            adapter: RpcCodingEventAdapter::new_with_provider(
                "test-api".into(),
                "test-provider".into(),
                "test-model".into(),
            ),
            product_event_replay: Some(replay),
            adapter_applied_sequence: ProductEventSequence::default(),
            replayed_through_sequence: ProductEventSequence::default(),
            events_closed: false,
        }));
        state
    }

    fn state_with_running_prompt_replay_and_pending(
        replay: ProductEventReplayHandle,
        pending: ProductEvent,
    ) -> RpcState {
        let mut state = RpcState::new(CliRunOptions::default()).unwrap();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        event_tx.send(pending).unwrap();
        drop(event_tx);
        let (_done_tx, done_rx) = oneshot::channel();
        state.running = Some(RunningPrompt::Coding(CodingRunningPrompt {
            events: event_rx,
            done: done_rx,
            control: None,
            operation_kind: OperationKind::Prompt,
            adapter: RpcCodingEventAdapter::new_with_provider(
                "test-api".into(),
                "test-provider".into(),
                "test-model".into(),
            ),
            product_event_replay: Some(replay),
            adapter_applied_sequence: ProductEventSequence::default(),
            replayed_through_sequence: ProductEventSequence::default(),
            events_closed: false,
        }));
        state
    }

    fn assistant_delta_event(text: &str) -> CodingAgentEvent {
        CodingAgentEvent::AssistantMessageDelta {
            operation_id: "op_reconnect".into(),
            turn_id: "turn_reconnect".into(),
            message_id: Some("msg_reconnect".into()),
            text: text.into(),
        }
    }

    fn prompt_started_event(sequence: u64, operation_id: &str) -> ProductEvent {
        ProductEvent::from_compat_event(
            ProductEventSequence::new(sequence),
            CodingAgentEvent::PromptStarted {
                operation_id: operation_id.into(),
                turn_id: "turn_prompt".into(),
            },
        )
    }

    fn prompt_completed_event(sequence: u64, operation_id: &str) -> ProductEvent {
        ProductEvent::from_compat_event(
            ProductEventSequence::new(sequence),
            CodingAgentEvent::PromptCompleted {
                operation_id: operation_id.into(),
                turn_id: "turn_prompt".into(),
            },
        )
    }

    #[tokio::test]
    async fn rpc_finish_drain_updates_client_submission_state_for_prompt_events() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let replay = session.product_event_replay_handle();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        event_tx.send(prompt_started_event(1, "op_finish")).unwrap();
        event_tx
            .send(prompt_completed_event(2, "op_finish"))
            .unwrap();
        drop(event_tx);
        let (_done_tx, done_rx) = oneshot::channel();
        let mut state = RpcState::new(CliRunOptions::default()).unwrap();
        state.client_drafts = vec![ClientDraft::new(ClientDraftKind::Prompt, "draft prompt")];
        state.running = Some(RunningPrompt::Coding(CodingRunningPrompt {
            events: event_rx,
            done: done_rx,
            control: None,
            operation_kind: OperationKind::Prompt,
            adapter: RpcCodingEventAdapter::new_with_provider(
                "test-api".into(),
                "test-provider".into(),
                "test-model".into(),
            ),
            product_event_replay: Some(replay),
            adapter_applied_sequence: ProductEventSequence::default(),
            replayed_through_sequence: ProductEventSequence::default(),
            events_closed: false,
        }));
        let mut writer = TestWriter::default();

        state
            .finish_coding_running_prompt(
                Ok(CodingOperationTaskResult {
                    session,
                    session_root: None,
                    outcome: CodingOperationOutcome::Prompt(Ok(PromptTurnOutcome::Aborted {
                        operation_id: "op_finish".into(),
                        turn_id: Some("turn_prompt".into()),
                        reason: "test completed".into(),
                        session_id: None,
                    })),
                }),
                &mut writer,
            )
            .await
            .unwrap();

        assert!(
            state
                .client_drafts
                .iter()
                .all(|draft| draft.kind != ClientDraftKind::Prompt)
        );
        assert!(state.submitted_operation.is_none());
    }

    #[tokio::test]
    async fn rpc_reconnect_replay_updates_client_submission_state() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let replay = session.product_event_replay_handle();
        session.emit_product_event_for_tests(CodingAgentEvent::PromptStarted {
            operation_id: "op_replay".into(),
            turn_id: "turn_prompt".into(),
        });
        let mut state = state_with_running_prompt_replay(replay);
        state.client_drafts = vec![ClientDraft::new(ClientDraftKind::Prompt, "draft prompt")];

        reconnect_running_prompt_after(&mut state, ProductEventSequence::default())
            .await
            .unwrap();

        assert!(
            state
                .client_drafts
                .iter()
                .all(|draft| draft.kind != ClientDraftKind::Prompt)
        );
        assert_eq!(
            state.submitted_operation,
            Some(SubmittedOperation {
                operation_id: "op_replay".into(),
                kind: OperationKind::Prompt,
            })
        );

        session.emit_product_event_for_tests(CodingAgentEvent::PromptCompleted {
            operation_id: "op_replay".into(),
            turn_id: "turn_prompt".into(),
        });
        reconnect_running_prompt_after(&mut state, ProductEventSequence::new(1))
            .await
            .unwrap();

        assert!(state.submitted_operation.is_none());
    }

    #[tokio::test]
    async fn rpc_live_overlap_after_reconnect_does_not_mutate_submission_state() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let replay = session.product_event_replay_handle();
        let started = session.emit_product_event_for_tests(CodingAgentEvent::PromptStarted {
            operation_id: "op_overlap".into(),
            turn_id: "turn_prompt".into(),
        });
        session.emit_product_event_for_tests(CodingAgentEvent::PromptCompleted {
            operation_id: "op_overlap".into(),
            turn_id: "turn_prompt".into(),
        });
        let mut state = state_with_running_prompt_replay(replay);
        state.client_drafts = vec![ClientDraft::new(ClientDraftKind::Prompt, "draft prompt")];

        reconnect_running_prompt_after(&mut state, ProductEventSequence::default())
            .await
            .unwrap();

        assert!(state.submitted_operation.is_none());
        assert!(
            state
                .client_drafts
                .iter()
                .all(|draft| draft.kind != ClientDraftKind::Prompt)
        );

        state.client_drafts = vec![ClientDraft::new(
            ClientDraftKind::Prompt,
            "new prompt draft",
        )];
        let mut live_writer = TestWriter::default();
        state
            .write_product_event(started, &mut live_writer)
            .await
            .unwrap();

        assert_eq!(
            state.client_drafts,
            vec![ClientDraft::new(
                ClientDraftKind::Prompt,
                "new prompt draft"
            )]
        );
        assert!(state.submitted_operation.is_none());
    }

    #[tokio::test]
    async fn rpc_reconnect_after_live_adapter_consumed_event_does_not_duplicate_text() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let replay = session.product_event_replay_handle();
        let event = session
            .emit_product_event_for_tests(assistant_delta_event("already live reconnect marker"));
        let mut state = state_with_running_prompt_replay(replay);
        let mut live_writer = TestWriter::default();
        state
            .write_product_event(event, &mut live_writer)
            .await
            .unwrap();
        assert!(
            String::from_utf8(live_writer.bytes)
                .unwrap()
                .contains("already live reconnect marker")
        );

        let events = reconnect_running_prompt_after(&mut state, ProductEventSequence::default())
            .await
            .unwrap();

        let encoded = serde_json::to_string(&events).unwrap();
        assert!(!encoded.contains("already live reconnect marker"));
    }

    #[tokio::test]
    async fn rpc_reconnect_skips_later_live_channel_overlap() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let replay = session.product_event_replay_handle();
        let event =
            session.emit_product_event_for_tests(assistant_delta_event("pending overlap marker"));
        let mut state = state_with_running_prompt_replay_and_pending(replay, event.clone());

        let replayed = reconnect_running_prompt_after(&mut state, ProductEventSequence::default())
            .await
            .unwrap();
        let encoded = serde_json::to_string(&replayed).unwrap();
        assert!(encoded.contains("pending overlap marker"));

        let pending = match state.running.as_mut().unwrap() {
            RunningPrompt::Coding(running) => running.events.try_recv().unwrap(),
        };
        let mut live_writer = TestWriter::default();
        state
            .write_product_event(pending, &mut live_writer)
            .await
            .unwrap();

        let live_output = String::from_utf8(live_writer.bytes).unwrap();
        assert!(!live_output.contains("pending overlap marker"));
    }

    #[tokio::test]
    async fn rpc_reconnect_replayed_event_advances_live_adapter_for_following_delta() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let replay = session.product_event_replay_handle();
        session.emit_product_event_for_tests(assistant_delta_event("replayed prefix "));
        let mut state = state_with_running_prompt_replay(replay);

        let replayed = reconnect_running_prompt_after(&mut state, ProductEventSequence::default())
            .await
            .unwrap();
        let replayed_json = serde_json::to_string(&replayed).unwrap();
        assert!(replayed_json.contains("replayed prefix "));

        let later_event =
            session.emit_product_event_for_tests(assistant_delta_event("live suffix"));
        let mut live_writer = TestWriter::default();
        state
            .write_product_event(later_event, &mut live_writer)
            .await
            .unwrap();

        let live_output = String::from_utf8(live_writer.bytes).unwrap();
        assert!(live_output.contains("replayed prefix "));
        assert!(live_output.contains("live suffix"));
        for line in live_output.lines().filter(|line| !line.trim().is_empty()) {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_ne!(value["type"], "message_start");
        }
    }

    #[tokio::test]
    async fn rpc_reconnect_replays_retained_product_events_after_snapshot_cursor() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let replay = session.product_event_replay_handle();
        let cursor = session.ui_snapshot(Vec::new()).cursor.last_event_sequence;
        session.emit_product_event_for_tests(assistant_delta_event("retained reconnect marker"));
        let mut state = state_with_running_prompt_replay(replay);

        let events = reconnect_running_prompt_after(&mut state, cursor)
            .await
            .unwrap();

        let encoded = serde_json::to_string(&events).unwrap();
        assert!(encoded.contains("retained reconnect marker"));
    }

    #[tokio::test]
    async fn rpc_reconnect_gap_returns_fresh_snapshot_required_error() {
        let session = CodingAgentSession::non_persistent_with_event_capacity_for_tests(
            CodingAgentSessionOptions::new(),
            1,
        )
        .await
        .unwrap();
        let replay = session.product_event_replay_handle();
        session.emit_product_event_for_tests(assistant_delta_event("evicted reconnect marker"));
        session.emit_product_event_for_tests(assistant_delta_event("retained reconnect marker"));
        let mut state = state_with_running_prompt_replay(replay);

        let error = reconnect_running_prompt_after(&mut state, ProductEventSequence::new(1))
            .await
            .unwrap_err();

        assert_eq!(error.code(), "event_stream_gap");
        assert!(matches!(
            error,
            CodingSessionError::EventStreamGap {
                requested_after: 1,
                oldest_available: 2,
            }
        ));
    }

    #[test]
    fn rpc_running_prompt_uses_product_event_stream_boundary() {
        let prompt_source = include_str!("prompt.rs");
        let state_source = include_str!("state.rs");
        let rpc_source = include_str!("../rpc.rs");
        let product_subscription = [".", "subscribe_product_events()"].concat();
        let compatibility_subscription = [".", "subscribe()"].concat();
        let product_adapter = ["adapter", ".push_product_event(&event)"].concat();

        assert_eq!(prompt_source.matches(&product_subscription).count(), 4);
        assert!(!prompt_source.contains(&compatibility_subscription));
        assert!(state_source.contains("UnboundedReceiver<ProductEvent>"));
        assert!(rpc_source.contains("CodingEvent(Option<crate::coding_session::ProductEvent>)"));
        assert!(prompt_source.contains(&product_adapter));
    }
}
