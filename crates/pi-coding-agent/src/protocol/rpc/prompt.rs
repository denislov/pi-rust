use crate::CliError;
use crate::protocol::events::ProtocolEventAdapter;
use crate::protocol::rpc::commands::has_images;
use crate::protocol::rpc::state::{RpcState, RunningPrompt};
use crate::protocol::rpc::wire::{write_json_line, write_rpc_response};
use crate::protocol::session_runner::{
    SessionPromptOptions, SessionPromptResult, spawn_session_prompt,
};
use crate::protocol::types::{ProtocolEvent, RpcResponse, StreamingBehavior};
use crate::runtime::PromptInvocation;
use pi_agent_core::{AgentEvent, AgentResources};
use tokio::io::AsyncWrite;
use tokio::sync::oneshot;

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
            match streaming_behavior {
                Some(StreamingBehavior::Steer) => {
                    self.enqueue_steer(message);
                    write_rpc_response(writer, RpcResponse::success(id, "prompt", None)).await?;
                    return self.emit_queue_update(writer).await;
                }
                Some(StreamingBehavior::FollowUp) => {
                    self.enqueue_follow_up(message);
                    write_rpc_response(writer, RpcResponse::success(id, "prompt", None)).await?;
                    return self.emit_queue_update(writer).await;
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
                    return Ok(());
                }
            }
        }

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

        self.running = Some(RunningPrompt {
            control: spawned.abort,
            events: spawned.events,
            done: spawned.done,
            adapter,
            abort_requested: false,
            events_closed: false,
        });

        Ok(())
    }

    pub(super) fn enqueue_steer(&mut self, message: String) {
        if let Some(running) = self.running.as_ref() {
            running.control.steer(message.clone());
        }
        self.steering.push(message);
    }

    pub(super) fn enqueue_follow_up(&mut self, message: String) {
        if let Some(running) = self.running.as_ref() {
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
        let Some(running) = self.running.as_mut() else {
            return Ok(());
        };
        for protocol_event in running.adapter.push(&event) {
            write_json_line(writer, &protocol_event).await?;
        }
        Ok(())
    }

    pub(super) async fn finish_running_prompt<W>(
        &mut self,
        result: Result<Result<SessionPromptResult, CliError>, oneshot::error::RecvError>,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let Some(mut running) = self.running.take() else {
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
