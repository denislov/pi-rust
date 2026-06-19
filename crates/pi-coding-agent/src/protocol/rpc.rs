use crate::protocol::events::ProtocolEventAdapter;
use crate::protocol::jsonl::{JsonlLineReader, serialize_json_line};
use crate::protocol::session_runner::{SessionPromptOptions, run_session_prompt};
use crate::protocol::types::{
    ProtocolEvent, RpcCommand, RpcResponse, RpcSessionState, StreamingBehavior,
};
use crate::runtime::{DEFAULT_MODEL_ID, PromptInvocation};
use crate::{CliError, CliRunOptions};
use pi_agent_core::session::StoredAgentMessage;
use pi_agent_core::{AgentResources, QueueMode, ThinkingLevel};
use pi_ai::types::Model;
use serde::Serialize;
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

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

    while let Some(line) = lines
        .read_next_line()
        .await
        .map_err(|e| CliError::AgentFailure(e.to_string()))?
    {
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(None, "parse", format!("Failed to parse command: {error}")),
                )
                .await?;
                continue;
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
            continue;
        }

        let command: RpcCommand = match serde_json::from_value(value) {
            Ok(command) => command,
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(None, command_name, format!("Invalid command: {error}")),
                )
                .await?;
                continue;
            }
        };

        state.handle_command(command, writer).await?;
    }

    Ok(())
}

pub async fn run_rpc_mode_stdio(options: CliRunOptions) -> Result<(), CliError> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    run_rpc_mode_for_io(stdin, &mut stdout, options).await
}

struct RpcState {
    options: CliRunOptions,
    model: Model,
    thinking_level: ThinkingLevel,
    steering_mode: QueueMode,
    follow_up_mode: QueueMode,
    auto_compaction_enabled: bool,
    session_name: Option<String>,
    messages: Vec<StoredAgentMessage>,
    is_streaming: bool,
    is_compacting: bool,
    steering: Vec<String>,
    follow_up: Vec<String>,
}

impl RpcState {
    fn new(options: CliRunOptions) -> Result<Self, CliError> {
        if options.register_builtins {
            pi_ai::providers::register_builtins();
        }
        let model = match options.model_override.clone() {
            Some(model) => model,
            None => pi_ai::lookup_model(DEFAULT_MODEL_ID)
                .ok_or_else(|| CliError::UnknownModel(DEFAULT_MODEL_ID.to_string()))?,
        };

        Ok(Self {
            options,
            model,
            thinking_level: ThinkingLevel::Off,
            steering_mode: QueueMode::OneAtATime,
            follow_up_mode: QueueMode::OneAtATime,
            auto_compaction_enabled: true,
            session_name: None,
            messages: Vec::new(),
            is_streaming: false,
            is_compacting: false,
            steering: Vec::new(),
            follow_up: Vec::new(),
        })
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
                self.steering.push(message);
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
                self.follow_up.push(message);
                write_rpc_response(writer, RpcResponse::success(id, "follow_up", None)).await?;
                self.emit_queue_update(writer).await
            }
            RpcCommand::Abort { id } => {
                self.is_streaming = false;
                write_rpc_response(writer, RpcResponse::success(id, "abort", None)).await
            }
            RpcCommand::NewSession { id, .. } => {
                self.messages.clear();
                self.steering.clear();
                self.follow_up.clear();
                self.session_name = None;
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

        if self.is_streaming {
            match streaming_behavior {
                Some(StreamingBehavior::Steer) => {
                    self.steering.push(message);
                    write_rpc_response(writer, RpcResponse::success(id, "prompt", None)).await?;
                    return self.emit_queue_update(writer).await;
                }
                Some(StreamingBehavior::FollowUp) => {
                    self.follow_up.push(message);
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

        let mut event_lines = Vec::new();
        event_lines.push(
            serialize_json_line(&ProtocolEvent::AgentStart)
                .map_err(|e| CliError::AgentFailure(e.to_string()))?,
        );
        let mut adapter = ProtocolEventAdapter::new_with_provider(
            self.model.api.clone(),
            self.model.provider.clone(),
            self.model.id.clone(),
        );

        self.is_streaming = true;
        let run = run_session_prompt(
            SessionPromptOptions {
                prompt: message.clone(),
                model: self.model.clone(),
                api_key: None,
                system_prompt: None,
                max_turns: 5,
                tools: self.options.tools.clone(),
                register_builtins: false,
                session: Some(self.options.session.clone()),
                session_target: None,
                session_name: self.session_name.clone(),
                thinking_level: Some(self.thinking_level),
                tool_execution: None,
                resources: AgentResources::default(),
                invocation: PromptInvocation::Text(message),
            },
            Some(&mut |event| {
                for protocol_event in adapter.push(event) {
                    event_lines.push(
                        serialize_json_line(&protocol_event)
                            .map_err(|e| CliError::AgentFailure(e.to_string()))?,
                    );
                }
                Ok(())
            }),
        )
        .await;
        self.is_streaming = false;

        for line in event_lines {
            writer
                .write_all(line.as_bytes())
                .await
                .map_err(|e| CliError::AgentFailure(e.to_string()))?;
        }

        if let Ok(result) = run {
            self.messages = result
                .messages
                .iter()
                .filter_map(crate::session::agent_message_to_stored)
                .collect();
        }

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
            is_streaming: self.is_streaming,
            is_compacting: self.is_compacting,
            steering_mode: self.steering_mode,
            follow_up_mode: self.follow_up_mode,
            session_file: None,
            session_id: "in-memory".into(),
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
        serde_json::json!({
            "sessionFile": Value::Null,
            "sessionId": "in-memory",
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
