use crate::protocol::events::ProtocolEventAdapter;
use crate::protocol::jsonl::{JsonlLineReader, serialize_json_line};
use crate::protocol::session_runner::{
    SessionPromptAbortHandle, SessionPromptOptions, SessionPromptResult, spawn_session_prompt,
};
use crate::protocol::types::{
    ProtocolEvent, RpcCommand, RpcResponse, RpcSessionState, StreamingBehavior,
};
use crate::runtime::PromptInvocation;
use crate::{CliArgs, CliError, CliRunOptions, config, select_model};
use pi_agent_core::session::StoredAgentMessage;
use pi_agent_core::{AgentEvent, AgentResources, QueueMode, ThinkingLevel};
use pi_ai::types::Model;
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};

pub async fn write_rpc_response<W>(writer: &mut W, response: RpcResponse) -> Result<(), CliError>
where
    W: AsyncWrite + Unpin,
{
    write_json_line(writer, &response).await
}

async fn write_json_line<W, T>(writer: &mut W, value: &T) -> Result<(), CliError>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    let line = serialize_json_line(value).map_err(|e| CliError::AgentFailure(e.to_string()))?;
    writer
        .write_all(line.as_bytes())
        .await
        .map_err(|e| CliError::AgentFailure(e.to_string()))?;
    writer
        .flush()
        .await
        .map_err(|e| CliError::AgentFailure(e.to_string()))
}

pub async fn run_rpc_mode_for_io<R, W>(
    reader: R,
    writer: &mut W,
    options: CliRunOptions,
) -> Result<(), CliError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut state = RpcState::new(options)?;
    let mut lines = JsonlLineReader::new(reader);
    let mut input_closed = false;

    loop {
        if input_closed && !state.is_streaming() {
            break;
        }

        let event = match (input_closed, state.running.as_mut()) {
            (false, Some(running)) if !running.events_closed => {
                tokio::select! {
                    line = lines.read_next_line() => RpcLoopEvent::Input(line),
                    event = running.events.recv() => RpcLoopEvent::AgentEvent(event),
                    done = &mut running.done => RpcLoopEvent::PromptDone(done),
                }
            }
            (false, Some(running)) => {
                tokio::select! {
                    line = lines.read_next_line() => RpcLoopEvent::Input(line),
                    done = &mut running.done => RpcLoopEvent::PromptDone(done),
                }
            }
            (true, Some(running)) if !running.events_closed => {
                tokio::select! {
                    event = running.events.recv() => RpcLoopEvent::AgentEvent(event),
                    done = &mut running.done => RpcLoopEvent::PromptDone(done),
                }
            }
            (true, Some(running)) => RpcLoopEvent::PromptDone((&mut running.done).await),
            (false, None) => RpcLoopEvent::Input(lines.read_next_line().await),
            (true, None) => break,
        };

        match event {
            RpcLoopEvent::Input(line) => {
                let Some(line) = line.map_err(|e| CliError::AgentFailure(e.to_string()))? else {
                    input_closed = true;
                    continue;
                };
                handle_input_line(&mut state, &line, writer).await?;
            }
            RpcLoopEvent::AgentEvent(Some(event)) => {
                state.write_agent_event(event, writer).await?;
            }
            RpcLoopEvent::AgentEvent(None) => {
                if let Some(running) = state.running.as_mut() {
                    running.events_closed = true;
                }
            }
            RpcLoopEvent::PromptDone(result) => {
                state.finish_running_prompt(result, writer).await?;
            }
        }
    }

    Ok(())
}

enum RpcLoopEvent {
    Input(Result<Option<String>, std::io::Error>),
    AgentEvent(Option<AgentEvent>),
    PromptDone(Result<Result<SessionPromptResult, CliError>, oneshot::error::RecvError>),
}

async fn handle_input_line<W>(
    state: &mut RpcState,
    line: &str,
    writer: &mut W,
) -> Result<(), CliError>
where
    W: AsyncWrite + Unpin,
{
    let value: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(error) => {
            write_rpc_response(
                writer,
                RpcResponse::error(None, "parse", format!("Failed to parse command: {error}")),
            )
            .await?;
            return Ok(());
        }
    };

    let command_name = command_type(&value);
    if !is_supported_m5_command(&command_name) {
        write_rpc_response(
            writer,
            RpcResponse::error(
                command_id(&value),
                command_name.clone(),
                format!("unsupported command in Rust M5: {command_name}"),
            ),
        )
        .await?;
        return Ok(());
    }

    let command: RpcCommand = match serde_json::from_value(value) {
        Ok(command) => command,
        Err(error) => {
            write_rpc_response(
                writer,
                RpcResponse::error(None, command_name, format!("Invalid command: {error}")),
            )
            .await?;
            return Ok(());
        }
    };

    state.handle_command(command, writer).await
}

pub async fn run_rpc_mode_stdio(options: CliRunOptions) -> Result<(), CliError> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    run_rpc_mode_for_io(stdin, &mut stdout, options).await
}

struct RpcState {
    options: CliRunOptions,
    model: Model,
    api_key: Option<String>,
    settings: crate::config::Settings,
    thinking_level: ThinkingLevel,
    steering_mode: QueueMode,
    follow_up_mode: QueueMode,
    auto_compaction_enabled: bool,
    session_name: Option<String>,
    active_session_path: Option<PathBuf>,
    active_leaf_id: Option<String>,
    messages: Vec<StoredAgentMessage>,
    running: Option<RunningPrompt>,
    is_compacting: bool,
    steering: Vec<String>,
    follow_up: Vec<String>,
}

struct RunningPrompt {
    control: SessionPromptAbortHandle,
    events: mpsc::UnboundedReceiver<AgentEvent>,
    done: oneshot::Receiver<Result<SessionPromptResult, CliError>>,
    adapter: ProtocolEventAdapter,
    abort_requested: bool,
    events_closed: bool,
}

impl RpcState {
    fn new(options: CliRunOptions) -> Result<Self, CliError> {
        if options.register_builtins {
            pi_ai::providers::register_builtins();
        }
        let cwd = options.session.cwd.clone();
        let (config, config_diags) = config::load_config(&cwd);
        let diag_text = config::drain_diagnostics(&config_diags);
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
            let key_text = config::drain_diagnostics(&key_diags);
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
            running: None,
            is_compacting: false,
            steering: Vec::new(),
            follow_up: Vec::new(),
        })
    }

    fn is_streaming(&self) -> bool {
        self.running.is_some()
    }

    async fn handle_command<W>(
        &mut self,
        command: RpcCommand,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        match command {
            RpcCommand::Prompt {
                id,
                message,
                images,
                streaming_behavior,
            } => {
                self.handle_prompt(id, message, images, streaming_behavior, writer)
                    .await
            }
            RpcCommand::Steer {
                id,
                message,
                images,
            } => {
                if has_images(&images) {
                    write_rpc_response(
                        writer,
                        RpcResponse::error(
                            id,
                            "steer",
                            "image prompt payloads are not supported in Rust M5 RPC mode",
                        ),
                    )
                    .await?;
                    return Ok(());
                }
                self.enqueue_steer(message);
                write_rpc_response(writer, RpcResponse::success(id, "steer", None)).await?;
                self.emit_queue_update(writer).await
            }
            RpcCommand::FollowUp {
                id,
                message,
                images,
            } => {
                if has_images(&images) {
                    write_rpc_response(
                        writer,
                        RpcResponse::error(
                            id,
                            "follow_up",
                            "image prompt payloads are not supported in Rust M5 RPC mode",
                        ),
                    )
                    .await?;
                    return Ok(());
                }
                self.enqueue_follow_up(message);
                write_rpc_response(writer, RpcResponse::success(id, "follow_up", None)).await?;
                self.emit_queue_update(writer).await
            }
            RpcCommand::Abort { id } => {
                let cancelled = if let Some(running) = self.running.as_mut() {
                    if !running.abort_requested {
                        running.control.abort();
                        running.abort_requested = true;
                    }
                    true
                } else {
                    false
                };
                write_rpc_response(
                    writer,
                    RpcResponse::success(
                        id,
                        "abort",
                        Some(serde_json::json!({ "cancelled": cancelled })),
                    ),
                )
                .await
            }
            RpcCommand::NewSession { id, .. } => {
                if self.is_streaming() {
                    write_rpc_response(
                        writer,
                        RpcResponse::error(
                            id,
                            "new_session",
                            "cannot start new session while agent is streaming",
                        ),
                    )
                    .await?;
                    return Ok(());
                }
                self.messages.clear();
                self.steering.clear();
                self.follow_up.clear();
                self.session_name = None;
                self.active_session_path = None;
                self.active_leaf_id = None;
                write_rpc_response(
                    writer,
                    RpcResponse::success(
                        id,
                        "new_session",
                        Some(serde_json::json!({"cancelled": false})),
                    ),
                )
                .await
            }
            RpcCommand::GetState { id } => {
                write_rpc_response(
                    writer,
                    RpcResponse::success(
                        id,
                        "get_state",
                        Some(
                            serde_json::to_value(self.session_state())
                                .expect("rpc state serializes"),
                        ),
                    ),
                )
                .await
            }
            RpcCommand::SetThinkingLevel { id, level } => {
                self.thinking_level = level;
                write_rpc_response(writer, RpcResponse::success(id, "set_thinking_level", None))
                    .await
            }
            RpcCommand::SetSteeringMode { id, mode } => {
                self.steering_mode = mode;
                write_rpc_response(writer, RpcResponse::success(id, "set_steering_mode", None))
                    .await
            }
            RpcCommand::SetFollowUpMode { id, mode } => {
                self.follow_up_mode = mode;
                write_rpc_response(writer, RpcResponse::success(id, "set_follow_up_mode", None))
                    .await
            }
            RpcCommand::Compact { id, .. } => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(
                        id,
                        "compact",
                        "manual compaction is not available in Rust M5",
                    ),
                )
                .await
            }
            RpcCommand::SetAutoCompaction { id, enabled } => {
                self.auto_compaction_enabled = enabled;
                write_rpc_response(
                    writer,
                    RpcResponse::success(id, "set_auto_compaction", None),
                )
                .await
            }
            RpcCommand::GetSessionStats { id } => {
                write_rpc_response(
                    writer,
                    RpcResponse::success(id, "get_session_stats", Some(self.session_stats())),
                )
                .await
            }
            RpcCommand::GetLastAssistantText { id } => {
                write_rpc_response(
                    writer,
                    RpcResponse::success(
                        id,
                        "get_last_assistant_text",
                        Some(serde_json::json!({ "text": self.last_assistant_text() })),
                    ),
                )
                .await
            }
            RpcCommand::SetSessionName { id, name } => {
                let name = name.trim().to_string();
                if name.is_empty() {
                    write_rpc_response(
                        writer,
                        RpcResponse::error(id, "set_session_name", "Session name cannot be empty"),
                    )
                    .await?;
                    return Ok(());
                }
                self.session_name = Some(name);
                write_rpc_response(writer, RpcResponse::success(id, "set_session_name", None)).await
            }
            RpcCommand::GetMessages { id } => {
                write_rpc_response(
                    writer,
                    RpcResponse::success(
                        id,
                        "get_messages",
                        Some(serde_json::json!({ "messages": self.messages })),
                    ),
                )
                .await
            }
        }
    }

    async fn handle_prompt<W>(
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

    fn enqueue_steer(&mut self, message: String) {
        if let Some(running) = self.running.as_ref() {
            running.control.steer(message.clone());
        }
        self.steering.push(message);
    }

    fn enqueue_follow_up(&mut self, message: String) {
        if let Some(running) = self.running.as_ref() {
            running.control.follow_up(message.clone());
        }
        self.follow_up.push(message);
    }

    async fn write_agent_event<W>(
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

    async fn finish_running_prompt<W>(
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

    async fn emit_queue_update<W>(&self, writer: &mut W) -> Result<(), CliError>
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

    fn session_state(&self) -> RpcSessionState {
        RpcSessionState {
            model: Some(self.model.clone()),
            thinking_level: self.thinking_level,
            is_streaming: self.is_streaming(),
            is_compacting: self.is_compacting,
            steering_mode: self.steering_mode,
            follow_up_mode: self.follow_up_mode,
            session_file: self
                .active_session_path
                .as_ref()
                .map(|path| path.display().to_string()),
            session_id: self
                .active_leaf_id
                .clone()
                .or_else(|| {
                    self.active_session_path
                        .as_ref()
                        .and_then(|path| path.file_stem())
                        .and_then(|stem| stem.to_str())
                        .map(ToString::to_string)
                })
                .unwrap_or_else(|| "in-memory".into()),
            session_name: self.session_name.clone(),
            auto_compaction_enabled: self.auto_compaction_enabled,
            message_count: self.messages.len(),
            pending_message_count: self.steering.len() + self.follow_up.len(),
        }
    }

    fn session_stats(&self) -> Value {
        let mut user_messages = 0;
        let mut assistant_messages = 0;
        let mut tool_results = 0;
        for message in &self.messages {
            match message {
                StoredAgentMessage::User { .. } => user_messages += 1,
                StoredAgentMessage::Assistant { .. } => assistant_messages += 1,
                StoredAgentMessage::ToolResult { .. } => tool_results += 1,
                StoredAgentMessage::BashExecution { .. }
                | StoredAgentMessage::Custom { .. }
                | StoredAgentMessage::BranchSummary { .. } => user_messages += 1,
            }
        }
        let session_file = self
            .active_session_path
            .as_ref()
            .map(|path| Value::String(path.display().to_string()))
            .unwrap_or(Value::Null);
        let session_id = self
            .active_leaf_id
            .clone()
            .or_else(|| {
                self.active_session_path
                    .as_ref()
                    .and_then(|path| path.file_stem())
                    .and_then(|stem| stem.to_str())
                    .map(ToString::to_string)
            })
            .unwrap_or_else(|| "in-memory".into());

        serde_json::json!({
            "sessionFile": session_file,
            "sessionId": session_id,
            "userMessages": user_messages,
            "assistantMessages": assistant_messages,
            "toolCalls": 0,
            "toolResults": tool_results,
            "totalMessages": self.messages.len(),
            "tokens": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "total": 0
            },
            "cost": 0.0
        })
    }

    fn last_assistant_text(&self) -> Option<String> {
        self.messages
            .iter()
            .rev()
            .find_map(|message| match message {
                StoredAgentMessage::Assistant { content, .. } => Some(
                    content
                        .iter()
                        .filter_map(|block| match block {
                            pi_ai::types::ContentBlock::Text { text, .. } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                _ => None,
            })
    }
}

fn has_images(images: &Option<Vec<pi_ai::types::ContentBlock>>) -> bool {
    images.as_ref().is_some_and(|images| !images.is_empty())
}

fn command_type(value: &Value) -> String {
    value
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string()
}

fn command_id(value: &Value) -> Option<String> {
    value
        .get("id")
        .and_then(|v| v.as_str())
        .map(ToString::to_string)
}

fn is_supported_m5_command(command: &str) -> bool {
    matches!(
        command,
        "prompt"
            | "steer"
            | "follow_up"
            | "abort"
            | "new_session"
            | "get_state"
            | "set_thinking_level"
            | "set_steering_mode"
            | "set_follow_up_mode"
            | "compact"
            | "set_auto_compaction"
            | "get_session_stats"
            | "get_last_assistant_text"
            | "set_session_name"
            | "get_messages"
    )
}
