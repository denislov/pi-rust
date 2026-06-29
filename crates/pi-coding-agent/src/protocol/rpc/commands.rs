use crate::CliError;
use crate::protocol::rpc::state::RpcState;
use crate::protocol::rpc::state::RunningPrompt;
use crate::protocol::rpc::wire::write_rpc_response;
use crate::protocol::types::{RpcCommand, RpcResponse};
use tokio::io::AsyncWrite;

impl RpcState {
    pub(super) async fn handle_command<W>(
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
                if matches!(self.running, Some(RunningPrompt::Coding(_))) {
                    write_rpc_response(
                        writer,
                        RpcResponse::error(
                            id,
                            "steer",
                            "agent is streaming; steer awaits AgentTurnFlow",
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
                if matches!(self.running, Some(RunningPrompt::Coding(_))) {
                    write_rpc_response(
                        writer,
                        RpcResponse::error(
                            id,
                            "follow_up",
                            "agent is streaming; follow-up awaits AgentTurnFlow",
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
                write_rpc_response(
                    writer,
                    RpcResponse::success(
                        id,
                        "abort",
                        Some(serde_json::json!({ "cancelled": false })),
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
                self.coding_session = None;
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
}

pub(super) fn has_images(images: &Option<Vec<pi_ai::types::ContentBlock>>) -> bool {
    images.as_ref().is_some_and(|images| !images.is_empty())
}
