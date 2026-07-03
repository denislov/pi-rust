use crate::CliError;
use crate::coding_session::{
    AgentInvocationOptions, AgentTeamOptions, CodingAgentSession, CodingAgentSessionOptions,
    OperationKind, ProfileId, PromptTurnMode, PromptTurnOptions,
};
use crate::prompt_options::PromptRunOptions;
use crate::protocol::rpc::commands::has_images;
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
            self.handle_streaming_prompt(id, message, streaming_behavior, writer)
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
        let mut receiver = session.subscribe();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();

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
        let mut receiver = session.subscribe();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();

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
        let mut receiver = session.subscribe();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();

        write_rpc_response(writer, RpcResponse::success(id, "prompt", None)).await?;
        write_json_line(writer, &ProtocolEvent::AgentStart).await?;

        tokio::spawn(async move {
            let outcome = {
                let mut prompt = Box::pin(session.prompt(prompt_options));
                loop {
                    tokio::select! {
                        event = receiver.recv() => {
                            match event {
                                Ok(event) => {
                                    if event_tx.send(event).is_err() {
                                        continue;
                                    }
                                }
                                Err(_) => {}
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
            events_closed: false,
        }));

        Ok(())
    }

    pub(super) fn enqueue_steer(&mut self, message: String) {
        self.steering.push(message);
    }

    pub(super) fn enqueue_follow_up(&mut self, message: String) {
        self.follow_up.push(message);
    }

    pub(super) async fn write_coding_event<W>(
        &mut self,
        event: crate::coding_session::CodingAgentEvent,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let Some(RunningPrompt::Coding(running)) = self.running.as_mut() else {
            return Ok(());
        };
        for protocol_event in running.adapter.push(&event) {
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

        while let Ok(event) = running.events.try_recv() {
            for protocol_event in running.adapter.push(&event) {
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
        };

        self.coding_session = Some(result.session);
        self.steering.clear();
        self.follow_up.clear();
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
