use crate::CliError;
use crate::coding_session::{
    CodingAgentSession, CodingAgentSessionOptions, PromptTurnMode, PromptTurnOptions,
};
use crate::protocol::events::ProtocolEventAdapter;
use crate::protocol::rpc::commands::has_images;
use crate::protocol::rpc::events::RpcCodingEventAdapter;
use crate::protocol::rpc::state::{
    CodingPromptTaskResult, CodingRunningPrompt, LegacyRunningPrompt, RpcState, RunningPrompt,
};
use crate::protocol::rpc::wire::{write_json_line, write_rpc_response};
use crate::protocol::session_runner::{
    SessionPromptOptions, SessionPromptResult, spawn_session_prompt,
};
use crate::protocol::types::{ProtocolEvent, RpcResponse, StreamingBehavior};
use crate::runtime::{PromptInvocation, SessionMode, SessionRunOptions};
use crate::session::resolve_session_dir;
use pi_agent_core::{AgentEvent, AgentResources};
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

        if matches!(self.options.session.mode, SessionMode::Enabled) {
            self.start_coding_session_prompt(id, message, writer).await
        } else {
            self.start_legacy_prompt(id, message, writer).await
        }
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
        if matches!(self.running, Some(RunningPrompt::Coding(_))) {
            write_rpc_response(
                writer,
                RpcResponse::error(
                    id,
                    "prompt",
                    "agent is streaming; steer and follow-up await AgentTurnFlow",
                ),
            )
            .await?;
            return Ok(());
        }

        match streaming_behavior {
            Some(StreamingBehavior::Steer) => {
                self.enqueue_steer(message);
                write_rpc_response(writer, RpcResponse::success(id, "prompt", None)).await?;
                self.emit_queue_update(writer).await?;
            }
            Some(StreamingBehavior::FollowUp) => {
                self.enqueue_follow_up(message);
                write_rpc_response(writer, RpcResponse::success(id, "prompt", None)).await?;
                self.emit_queue_update(writer).await?;
            }
            None => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(
                        id,
                        "prompt",
                        "agent is streaming; set streamingBehavior to steer or followUp",
                    ),
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn start_legacy_prompt<W>(
        &mut self,
        id: Option<String>,
        message: String,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        write_rpc_response(writer, RpcResponse::success(id, "prompt", None)).await?;
        write_json_line(writer, &ProtocolEvent::AgentStart).await?;

        let mut adapter = ProtocolEventAdapter::new_with_provider(
            self.model.api.clone(),
            self.model.provider.clone(),
            self.model.id.clone(),
        );

        let spawned = match spawn_session_prompt(SessionPromptOptions {
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
        }) {
            Ok(spawned) => spawned,
            Err(error) => {
                for protocol_event in adapter.push(&AgentEvent::AgentError {
                    error: error.to_string(),
                }) {
                    write_json_line(writer, &protocol_event).await?;
                }
                return Ok(());
            }
        };

        self.running = Some(RunningPrompt::Legacy(LegacyRunningPrompt {
            control: spawned.abort,
            events: spawned.events,
            done: spawned.done,
            adapter,
            abort_requested: false,
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
        let session_root = rpc_coding_session_root(&self.options.session)?;
        let mut session = match self.coding_session.take() {
            Some(session) => session,
            None => {
                CodingAgentSession::create(
                    CodingAgentSessionOptions::new().with_session_log_root(session_root.clone()),
                )
                .await?
            }
        };
        let prompt_options = PromptTurnOptions::from_session_prompt_options(SessionPromptOptions {
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

            let _ = done_tx.send(CodingPromptTaskResult {
                session,
                session_root,
                outcome,
            });
        });

        self.running = Some(RunningPrompt::Coding(CodingRunningPrompt {
            events: event_rx,
            done: done_rx,
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
        if let Some(RunningPrompt::Legacy(running)) = self.running.as_ref() {
            running.control.steer(message.clone());
        }
        self.steering.push(message);
    }

    pub(super) fn enqueue_follow_up(&mut self, message: String) {
        if let Some(RunningPrompt::Legacy(running)) = self.running.as_ref() {
            running.control.follow_up(message.clone());
        }
        self.follow_up.push(message);
    }

    pub(super) async fn write_agent_event<W>(
        &mut self,
        event: AgentEvent,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let Some(RunningPrompt::Legacy(running)) = self.running.as_mut() else {
            return Ok(());
        };
        for protocol_event in running.adapter.push(&event) {
            write_json_line(writer, &protocol_event).await?;
        }
        Ok(())
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

    pub(super) async fn finish_legacy_running_prompt<W>(
        &mut self,
        result: Result<Result<SessionPromptResult, CliError>, oneshot::error::RecvError>,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let Some(RunningPrompt::Legacy(mut running)) = self.running.take() else {
            return Ok(());
        };

        while let Ok(event) = running.events.try_recv() {
            for protocol_event in running.adapter.push(&event) {
                write_json_line(writer, &protocol_event).await?;
            }
        }

        match result {
            Ok(Ok(result)) => {
                self.active_session_path = result.session_path;
                self.active_leaf_id = result.leaf_id;
                self.messages = result
                    .messages
                    .iter()
                    .filter_map(crate::session::agent_message_to_stored)
                    .collect();
            }
            Ok(Err(_error)) => {}
            Err(error) => {
                return Err(CliError::AgentFailure(format!(
                    "agent task ended before reporting completion: {error}"
                )));
            }
        }

        self.steering.clear();
        self.follow_up.clear();
        Ok(())
    }

    pub(super) async fn finish_coding_running_prompt<W>(
        &mut self,
        result: Result<CodingPromptTaskResult, oneshot::error::RecvError>,
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

        match &result.outcome {
            Ok(outcome) => {
                if let Some(session_id) = prompt_outcome_session_id(outcome) {
                    self.active_leaf_id = Some(session_id.to_string());
                    self.active_session_path = Some(result.session_root.join(session_id));
                }
            }
            Err(_) => {}
        }

        self.coding_session = Some(result.session);
        self.steering.clear();
        self.follow_up.clear();
        result.outcome.map(|_| ())
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
