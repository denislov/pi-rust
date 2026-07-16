use crate::adapters::rpc::prompt::flush_session_product_events;
use crate::adapters::rpc::state::RpcState;
use crate::adapters::rpc::wire::{write_json_line, write_rpc_response};
use crate::api::operation::{
    CodingAgentOperation, CodingAgentOperationOutcome, CodingAgentPluginLoadOutcome,
};
use crate::app::bootstrap::{PromptInvocation, SessionRunOptions};
use crate::app::cli::error::CliError;
use crate::app::cli::prompt_options::PromptRunOptions;
use crate::app::session::{open_forked_runtime_session, open_new_runtime_session};
use crate::authorization::ToolAuthorizationDecision;
use crate::operations::prompt::context::QueuedPromptInput;
use crate::protocol::types::{
    RpcCommand, RpcDetachLifecycleEvent, RpcDetachResponse, RpcDetachStatus, RpcHelloResponse,
    RpcResponse, RpcSelfHealingEditModelRepair, RpcSelfHealingEditReplacement,
    RpcSessionNamePersistence, RpcSetSessionNameResponse, RpcShutdownLifecycleEvent,
    RpcShutdownResponse, RpcShutdownStatus, RpcToolAuthorizationApprovalScope,
};
use crate::protocol::version::{
    PRODUCT_EVENT_PROTOCOL_VERSION, RPC_PROTOCOL_VERSION, UI_SNAPSHOT_PROTOCOL_VERSION,
};
use crate::runtime::facade::{
    AgentProfile, CodingAgentControlId, CodingAgentSession, CodingAgentShutdownOutcome,
    CodingSessionError, DelegationConfirmationMode, DelegationPolicy, OperationKind,
    PendingDelegationConfirmation, ProductEventSequence, ProfileDiagnostic, ProfileId, ProfileKind,
    ProfileSource, PromptTurnMode, PromptTurnOptions, SelfHealingEditCheckOutput,
    SelfHealingEditModelRepairOptions, SelfHealingEditOutcome, SelfHealingEditRepairAttempt,
    SelfHealingEditReplacement, SelfHealingEditRequest, SupervisionPolicy, TeamProfile,
    TeamStrategy, TeamSupervisor,
};
use pi_agent_core::api::resources::AgentResources;
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
            RpcCommand::Hello { id, protocol } => {
                if !RPC_PROTOCOL_VERSION.is_compatible_with(&protocol) {
                    write_rpc_response(
                        writer,
                        RpcResponse::error_with_data(
                            id,
                            "hello",
                            format!(
                                "unsupported protocol version for rpc: requested {protocol}, supported {RPC_PROTOCOL_VERSION}"
                            ),
                            serde_json::json!({
                                "code": "unsupported_protocol_version",
                                "requested": {
                                    "family": protocol.family,
                                    "major": protocol.major,
                                    "minor": protocol.minor
                                },
                                "supported": {
                                    "family": RPC_PROTOCOL_VERSION.family,
                                    "major": RPC_PROTOCOL_VERSION.major,
                                    "minor": RPC_PROTOCOL_VERSION.minor
                                }
                            }),
                        ),
                    )
                    .await?;
                    return Ok(());
                }
                self.negotiated_protocol.rpc = Some(RPC_PROTOCOL_VERSION);
                write_rpc_response(
                    writer,
                    RpcResponse::success(
                        id,
                        "hello",
                        Some(
                            serde_json::to_value(RpcHelloResponse {
                                protocol: RPC_PROTOCOL_VERSION,
                                product_events: PRODUCT_EVENT_PROTOCOL_VERSION,
                                ui_snapshot: UI_SNAPSHOT_PROTOCOL_VERSION,
                            })
                            .expect("hello response serializes"),
                        ),
                    ),
                )
                .await
            }
            RpcCommand::Detach { id } => match self.detach_client().await {
                Ok(status) => {
                    if status == RpcDetachStatus::Detached {
                        write_json_line(writer, &RpcDetachLifecycleEvent { status }).await?;
                    }
                    write_rpc_response(
                        writer,
                        RpcResponse::success(
                            id,
                            "detach",
                            Some(
                                serde_json::to_value(RpcDetachResponse { status })
                                    .expect("detach response serializes"),
                            ),
                        ),
                    )
                    .await
                }
                Err(CodingSessionError::Lifecycle { reason }) => {
                    write_rpc_response(
                        writer,
                        RpcResponse::error_with_data(
                            id,
                            "detach",
                            reason.to_string(),
                            serde_json::json!({"code": reason.code()}),
                        ),
                    )
                    .await
                }
                Err(error) => Err(CliError::from(error)),
            },
            RpcCommand::Shutdown { id } => {
                if self.has_active_operations() {
                    if self.pending_shutdown_response.is_some() {
                        write_rpc_response(
                            writer,
                            RpcResponse::error_with_data(
                                id,
                                "shutdown",
                                "runtime shutdown is already pending",
                                serde_json::json!({"code": "shutdown_in_progress"}),
                            ),
                        )
                        .await?;
                        return Ok(());
                    }
                    let shutdown_handle =
                        self.active_shutdown_handle.as_ref().ok_or_else(|| {
                            CliError::AgentFailure(
                                "active RPC operation has no runtime shutdown authority".into(),
                            )
                        })?;
                    shutdown_handle.request_shutdown();
                    self.pending_shutdown_response = Some(id);
                    return Ok(());
                }

                let mut session = match self.coding_session.take() {
                    Some(session) => session,
                    None => self.open_reload_session().await?,
                };
                let outcome = session.shutdown().await?;
                let status = match outcome {
                    CodingAgentShutdownOutcome::ShutDown => RpcShutdownStatus::ShutDown,
                    CodingAgentShutdownOutcome::AlreadyShutDown => {
                        RpcShutdownStatus::AlreadyShutDown
                    }
                };
                if status == RpcShutdownStatus::ShutDown {
                    write_json_line(writer, &RpcShutdownLifecycleEvent { status }).await?;
                }
                let response = write_rpc_response(
                    writer,
                    RpcResponse::success(
                        id,
                        "shutdown",
                        Some(
                            serde_json::to_value(RpcShutdownResponse { status })
                                .expect("shutdown response serializes"),
                        ),
                    ),
                )
                .await;
                self.coding_session = Some(session);
                response
            }
            RpcCommand::Prompt {
                id,
                message,
                images,
                streaming_behavior,
                after_snapshot_cursor,
                idempotency_key,
            } => {
                self.handle_prompt(
                    id,
                    message,
                    images,
                    streaming_behavior,
                    after_snapshot_cursor,
                    idempotency_key,
                    writer,
                )
                .await
            }
            RpcCommand::Steer {
                id,
                message,
                images,
            } => {
                if let Some(foreground) = self.foreground.as_ref() {
                    if foreground.operation_kind != OperationKind::Prompt {
                        write_rpc_response(
                            writer,
                            RpcResponse::error(
                                id,
                                "steer",
                                format!(
                                    "cannot steer while {} is running",
                                    foreground.operation_kind.as_str()
                                ),
                            ),
                        )
                        .await?;
                        return Ok(());
                    }
                    let Some(control) = self.active_prompt_control()? else {
                        write_rpc_response(
                            writer,
                            RpcResponse::error(id, "steer", "agent is not streaming"),
                        )
                        .await?;
                        return Ok(());
                    };
                    let control_id = CodingAgentControlId(
                        id.clone().unwrap_or_else(|| format!("rpc-steer-{message}")),
                    );
                    let result = match crate::adapters::rpc::prompt::rpc_control_content(
                        message.clone(),
                        images,
                    ) {
                        Ok(Some(content)) => control.steer_content(control_id, content),
                        Ok(None) => control.steer(control_id, message),
                        Err(error) => {
                            write_rpc_response(
                                writer,
                                RpcResponse::error(id, "steer", error.to_string()),
                            )
                            .await?;
                            return Ok(());
                        }
                    };
                    match result {
                        Ok(_) => {
                            write_rpc_response(writer, RpcResponse::success(id, "steer", None))
                                .await?
                        }
                        Err(error) => {
                            write_rpc_response(
                                writer,
                                RpcResponse::error(id, "steer", format!("{:?}", error.reason)),
                            )
                            .await?
                        }
                    }
                    return Ok(());
                }
                let input = match crate::adapters::rpc::prompt::rpc_control_content(
                    message.clone(),
                    images,
                ) {
                    Ok(Some(content)) => QueuedPromptInput::Content(content),
                    Ok(None) => QueuedPromptInput::Text(message),
                    Err(error) => {
                        write_rpc_response(
                            writer,
                            RpcResponse::error(id, "steer", error.to_string()),
                        )
                        .await?;
                        return Ok(());
                    }
                };
                self.enqueue_steer(input);
                write_rpc_response(writer, RpcResponse::success(id, "steer", None)).await?;
                self.emit_queue_update(writer).await
            }
            RpcCommand::FollowUp {
                id,
                message,
                images,
            } => {
                if let Some(foreground) = self.foreground.as_ref() {
                    if foreground.operation_kind != OperationKind::Prompt {
                        write_rpc_response(
                            writer,
                            RpcResponse::error(
                                id,
                                "follow_up",
                                format!(
                                    "cannot follow up while {} is running",
                                    foreground.operation_kind.as_str()
                                ),
                            ),
                        )
                        .await?;
                        return Ok(());
                    }
                    let Some(control) = self.active_prompt_control()? else {
                        write_rpc_response(
                            writer,
                            RpcResponse::error(id, "follow_up", "agent is not streaming"),
                        )
                        .await?;
                        return Ok(());
                    };
                    let control_id = CodingAgentControlId(
                        id.clone()
                            .unwrap_or_else(|| format!("rpc-follow-up-{message}")),
                    );
                    let result = match crate::adapters::rpc::prompt::rpc_control_content(
                        message.clone(),
                        images,
                    ) {
                        Ok(Some(content)) => control.follow_up_content(control_id, content),
                        Ok(None) => control.follow_up(control_id, message),
                        Err(error) => {
                            write_rpc_response(
                                writer,
                                RpcResponse::error(id, "follow_up", error.to_string()),
                            )
                            .await?;
                            return Ok(());
                        }
                    };
                    match result {
                        Ok(_) => {
                            write_rpc_response(writer, RpcResponse::success(id, "follow_up", None))
                                .await?
                        }
                        Err(error) => {
                            write_rpc_response(
                                writer,
                                RpcResponse::error(id, "follow_up", format!("{:?}", error.reason)),
                            )
                            .await?
                        }
                    }
                    return Ok(());
                }
                let input = match crate::adapters::rpc::prompt::rpc_control_content(
                    message.clone(),
                    images,
                ) {
                    Ok(Some(content)) => QueuedPromptInput::Content(content),
                    Ok(None) => QueuedPromptInput::Text(message),
                    Err(error) => {
                        write_rpc_response(
                            writer,
                            RpcResponse::error(id, "follow_up", error.to_string()),
                        )
                        .await?;
                        return Ok(());
                    }
                };
                self.enqueue_follow_up(input);
                write_rpc_response(writer, RpcResponse::success(id, "follow_up", None)).await?;
                self.emit_queue_update(writer).await
            }
            RpcCommand::Abort { id, operation_id } => {
                let target_operation_id = match operation_id {
                    Some(operation_id) => {
                        if !self.background_operations.contains_key(&operation_id)
                            && self.active_foreground_operation_id()?.as_deref()
                                != Some(operation_id.as_str())
                        {
                            write_rpc_response(
                                writer,
                                RpcResponse::error(id, "abort", "operation is not running"),
                            )
                            .await?;
                            return Ok(());
                        }
                        Some(operation_id)
                    }
                    None if self.foreground.is_some() => self.active_foreground_operation_id()?,
                    None => None,
                };
                let cancelled = if let Some(operation_id) = target_operation_id {
                    let Some(control) = self.operation_control(&operation_id) else {
                        write_rpc_response(
                            writer,
                            RpcResponse::error(id, "abort", "operation has no control owner"),
                        )
                        .await?;
                        return Ok(());
                    };
                    match control.abort(
                        CodingAgentControlId(id.clone().unwrap_or_else(|| "rpc-abort".into())),
                        "rpc abort requested",
                    ) {
                        Ok(_) => true,
                        Err(error) => {
                            write_rpc_response(
                                writer,
                                RpcResponse::error(id, "abort", format!("{:?}", error.reason)),
                            )
                            .await?;
                            return Ok(());
                        }
                    }
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
            RpcCommand::NewSession { id, parent_session } => {
                self.handle_new_session(id, parent_session, writer).await
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
            RpcCommand::Reload { id } => self.handle_reload(id, writer).await,
            RpcCommand::PluginCommand {
                id,
                command_id,
                args,
            } => {
                self.handle_plugin_command(
                    id,
                    command_id,
                    args.unwrap_or_else(|| serde_json::json!({})),
                    writer,
                )
                .await
            }
            RpcCommand::SelfHealingEdit {
                id,
                path,
                edits,
                check_command,
                repair_attempts,
                model_repair,
                idempotency_key,
            } => {
                self.handle_self_healing_edit(
                    id,
                    path,
                    edits,
                    check_command,
                    repair_attempts,
                    model_repair,
                    idempotency_key,
                    writer,
                )
                .await
            }
            RpcCommand::ListAgentProfiles { id } => {
                self.handle_list_agent_profiles(id, writer).await
            }
            RpcCommand::ListTeamProfiles { id } => self.handle_list_team_profiles(id, writer).await,
            RpcCommand::SetDefaultAgentProfile {
                id,
                profile_id,
                idempotency_key,
            } => {
                self.handle_set_default_agent_profile(id, profile_id, idempotency_key, writer)
                    .await
            }
            RpcCommand::InvokeAgent {
                id,
                profile_id,
                task,
                idempotency_key,
            } => {
                self.handle_invoke_agent(id, profile_id, task, idempotency_key, writer)
                    .await
            }
            RpcCommand::InvokeTeam {
                id,
                team_id,
                task,
                idempotency_key,
            } => {
                self.handle_invoke_team(id, team_id, task, idempotency_key, writer)
                    .await
            }
            RpcCommand::ListDelegationConfirmations { id } => {
                self.handle_list_delegation_confirmations(id, writer).await
            }
            RpcCommand::ListToolAuthorizations { id } => {
                self.handle_list_tool_authorizations(id, writer).await
            }
            RpcCommand::ApproveToolAuthorization {
                id,
                authorization_id,
                scope,
            } => {
                self.handle_approve_tool_authorization(id, authorization_id, scope, writer)
                    .await
            }
            RpcCommand::DenyToolAuthorization {
                id,
                authorization_id,
                reason,
            } => {
                self.handle_deny_tool_authorization(id, authorization_id, reason, writer)
                    .await
            }
            RpcCommand::ApproveDelegation {
                id,
                operation_id,
                tool_call_id,
                idempotency_key,
            } => {
                self.handle_approve_delegation(
                    id,
                    operation_id,
                    tool_call_id,
                    idempotency_key,
                    writer,
                )
                .await
            }
            RpcCommand::RejectDelegation {
                id,
                operation_id,
                tool_call_id,
                reason,
                idempotency_key,
            } => {
                self.handle_reject_delegation(
                    id,
                    operation_id,
                    tool_call_id,
                    reason,
                    idempotency_key,
                    writer,
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
            RpcCommand::Compact {
                id,
                custom_instructions,
            } => self.handle_compact(id, custom_instructions, writer).await,
            RpcCommand::SetAutoCompaction { id, enabled } => {
                self.auto_compaction_enabled = enabled;
                write_rpc_response(
                    writer,
                    RpcResponse::success(id, "set_auto_compaction", None),
                )
                .await
            }
            RpcCommand::GetSessionStats { id } => match self.session_stats() {
                Ok(stats) => {
                    write_rpc_response(
                        writer,
                        RpcResponse::success(
                            id,
                            "get_session_stats",
                            Some(serde_json::to_value(stats).expect("rpc session stats serialize")),
                        ),
                    )
                    .await
                }
                Err(error) => {
                    write_rpc_response(
                        writer,
                        RpcResponse::error(id, "get_session_stats", error.to_string()),
                    )
                    .await
                }
            },
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
                self.session_name = Some(name.clone());
                write_rpc_response(
                    writer,
                    RpcResponse::success(
                        id,
                        "set_session_name",
                        Some(
                            serde_json::to_value(RpcSetSessionNameResponse {
                                name,
                                persistence: RpcSessionNamePersistence::AdapterLocal,
                            })
                            .expect("set-session-name response serializes"),
                        ),
                    ),
                )
                .await
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

    async fn handle_new_session<W>(
        &mut self,
        id: Option<String>,
        parent_session: Option<String>,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        if self.has_active_operations() {
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

        let parent_session = match parent_session {
            Some(parent) if parent.is_empty() || parent.trim() != parent => {
                write_rpc_response(
                    writer,
                    RpcResponse::error_with_data(
                        id,
                        "new_session",
                        "parentSession must be a non-empty session ID without surrounding whitespace",
                        serde_json::json!({ "code": "input" }),
                    ),
                )
                .await?;
                return Ok(());
            }
            parent => parent,
        };

        let forked = if let Some(parent) = parent_session.as_deref() {
            match open_forked_runtime_session(&self.options.session, parent).await {
                Ok(session) => Some(session),
                Err(error) => {
                    write_rpc_response(
                        writer,
                        RpcResponse::error_with_data(
                            id,
                            "new_session",
                            error.to_string(),
                            serde_json::json!({ "code": error.code() }),
                        ),
                    )
                    .await?;
                    return Ok(());
                }
            }
        } else {
            None
        };
        let forked_state = match forked.as_ref().map(CodingAgentSession::hydrate_current) {
            Some(Ok(Some(hydration))) => Some((
                hydration.summary.session_id,
                hydration.summary.session_dir,
                hydration.summary.active_leaf_id,
            )),
            Some(Ok(None)) => unreachable!("forked runtime sessions are persistent"),
            Some(Err(error)) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error_with_data(
                        id,
                        "new_session",
                        error.to_string(),
                        serde_json::json!({ "code": error.code() }),
                    ),
                )
                .await?;
                return Ok(());
            }
            None => None,
        };

        if let Err(error) = self.detach_client().await {
            write_rpc_response(
                writer,
                RpcResponse::error_with_data(
                    id,
                    "new_session",
                    error.to_string(),
                    serde_json::json!({ "code": error.code() }),
                ),
            )
            .await?;
            return Ok(());
        }
        self.messages.clear();
        self.steering.clear();
        self.follow_up.clear();
        self.session_name = None;
        self.session_event_stream_id = None;
        self.session_events = None;
        self.session_event_flush = None;
        self.session_events_closed = false;
        self.adapter_applied_sequence = ProductEventSequence::default();
        self.active_shutdown_handle = None;

        let response_data = if let Some((session_id, session_dir, active_leaf_id)) = forked_state {
            self.active_session_path = Some(session_dir);
            self.active_leaf_id = active_leaf_id;
            self.coding_session = forked;
            serde_json::json!({
                "cancelled": false,
                "sessionId": session_id,
                "parentSession": parent_session,
            })
        } else {
            self.active_session_path = None;
            self.active_leaf_id = None;
            self.coding_session = None;
            serde_json::json!({"cancelled": false})
        };

        write_rpc_response(
            writer,
            RpcResponse::success(id, "new_session", Some(response_data)),
        )
        .await
    }

    fn self_healing_model_repair_options(
        &self,
        policy: RpcSelfHealingEditModelRepair,
    ) -> SelfHealingEditModelRepairOptions {
        let prompt = "repair self-healing edit".to_owned();
        let prompt_options = PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
            prompt: prompt.clone(),
            model: self.model.clone(),
            api_key: self.api_key.clone(),
            auth_diagnostics: Vec::new(),
            system_prompt: Some("Return only self-healing edit repair JSON.".into()),
            max_turns: Some(1),
            tools: self.options.tools.clone(),
            register_builtins: false,
            ai_client: self.options.ai_client.clone(),
            session: Some(self.options.session.clone()),
            session_target: None,
            session_name: None,
            thinking_level: Some(self.thinking_level),
            tool_execution: None,
            resources: AgentResources::default(),
            settings: Some(self.effective_prompt_settings()),
            invocation: PromptInvocation::Text(prompt),
        })
        .with_mode(PromptTurnMode::Rpc);
        SelfHealingEditModelRepairOptions::new(prompt_options)
            .with_max_attempts(policy.max_attempts.unwrap_or(1))
    }

    async fn handle_self_healing_edit<W>(
        &mut self,
        id: Option<String>,
        path: String,
        edits: Vec<RpcSelfHealingEditReplacement>,
        check_command: Option<String>,
        repair_attempts: Option<Vec<Vec<RpcSelfHealingEditReplacement>>>,
        model_repair: Option<RpcSelfHealingEditModelRepair>,
        idempotency_key: Option<String>,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let idempotency_key = match self.parse_idempotency_key(idempotency_key) {
            Ok(key) => key,
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "self_healing_edit", error.to_string()),
                )
                .await?;
                return Ok(());
            }
        };
        match self.idempotent_retry_response(idempotency_key.as_ref(), "self_healing_edit") {
            Ok(Some(data)) => {
                write_rpc_response(
                    writer,
                    RpcResponse::success(id, "self_healing_edit", Some(data)),
                )
                .await?;
                return Ok(());
            }
            Ok(None) => {}
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "self_healing_edit", error.to_string()),
                )
                .await?;
                return Ok(());
            }
        }

        if self.is_streaming() {
            write_rpc_response(
                writer,
                RpcResponse::error(
                    id,
                    "self_healing_edit",
                    "cannot run self-healing edit while agent is streaming",
                ),
            )
            .await?;
            return Ok(());
        }

        let replacements = edits
            .into_iter()
            .map(rpc_self_healing_edit_replacement)
            .collect::<Vec<_>>();
        let repair_attempts = repair_attempts
            .unwrap_or_default()
            .into_iter()
            .map(|attempt| {
                attempt
                    .into_iter()
                    .map(rpc_self_healing_edit_replacement)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut session = match self.coding_session.take() {
            Some(session) => session,
            None => match self.open_reload_session().await {
                Ok(session) => session,
                Err(error) => {
                    write_rpc_response(
                        writer,
                        RpcResponse::error(id, "self_healing_edit", error.to_string()),
                    )
                    .await?;
                    return Ok(());
                }
            },
        };
        self.ensure_session_event_pump(&session);
        let event_flush = self
            .session_event_flush
            .as_ref()
            .expect("session event pump installed")
            .clone();

        let mut request = SelfHealingEditRequest::new(path, replacements);
        if let Some(command) = check_command {
            request = request.with_check_command(command);
        }
        if !repair_attempts.is_empty() {
            request = request.with_repair_attempts(repair_attempts);
        }
        if let Some(model_repair) = model_repair {
            request =
                request.with_model_repair(self.self_healing_model_repair_options(model_repair));
        }

        let complete_key = idempotency_key.clone();
        self.remember_idempotency_key(
            idempotency_key,
            "self_healing_edit",
            OperationKind::SelfHealingEdit,
        );

        let result = session
            .run(CodingAgentOperation::SelfHealingEdit(request))
            .await;
        flush_session_product_events(event_flush).await;
        match result {
            Ok(operation_outcome) => {
                let outcome = match operation_outcome {
                    CodingAgentOperationOutcome::SelfHealingEdit(outcome) => outcome,
                    _ => unreachable!(
                        "self-healing edit operation returned a different public outcome"
                    ),
                };
                let data = rpc_self_healing_edit_data(&outcome);
                self.coding_session = Some(session);
                write_rpc_response(
                    writer,
                    RpcResponse::success(id, "self_healing_edit", Some(data)),
                )
                .await?;
                self.drain_session_product_events(writer).await?;
                self.mark_idempotency_complete(complete_key.as_ref());
                Ok(())
            }
            Err(error) => {
                let response = match rpc_self_healing_edit_error_data(&error) {
                    Some(data) => RpcResponse::error_with_data(
                        id,
                        "self_healing_edit",
                        error.to_string(),
                        data,
                    ),
                    None => RpcResponse::error(id, "self_healing_edit", error.to_string()),
                };
                self.coding_session = Some(session);
                write_rpc_response(writer, response).await?;
                self.drain_session_product_events(writer).await?;
                self.mark_idempotency_complete(complete_key.as_ref());
                Ok(())
            }
        }
    }

    async fn handle_list_agent_profiles<W>(
        &mut self,
        id: Option<String>,
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
                    "list_agent_profiles",
                    "cannot list agent profiles while agent is streaming",
                ),
            )
            .await?;
            return Ok(());
        }

        let data = if let Some(session) = self.coding_session.as_ref() {
            rpc_agent_profiles_data(session)
        } else {
            match self.open_profile_listing_session().await {
                Ok(session) => rpc_agent_profiles_data(&session),
                Err(error) => {
                    write_rpc_response(
                        writer,
                        RpcResponse::error(id, "list_agent_profiles", error.to_string()),
                    )
                    .await?;
                    return Ok(());
                }
            }
        };
        write_rpc_response(
            writer,
            RpcResponse::success(id, "list_agent_profiles", Some(data)),
        )
        .await
    }

    async fn handle_list_team_profiles<W>(
        &mut self,
        id: Option<String>,
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
                    "list_team_profiles",
                    "cannot list team profiles while agent is streaming",
                ),
            )
            .await?;
            return Ok(());
        }

        let data = if let Some(session) = self.coding_session.as_ref() {
            rpc_team_profiles_data(session)
        } else {
            match self.open_profile_listing_session().await {
                Ok(session) => rpc_team_profiles_data(&session),
                Err(error) => {
                    write_rpc_response(
                        writer,
                        RpcResponse::error(id, "list_team_profiles", error.to_string()),
                    )
                    .await?;
                    return Ok(());
                }
            }
        };
        write_rpc_response(
            writer,
            RpcResponse::success(id, "list_team_profiles", Some(data)),
        )
        .await
    }

    async fn handle_set_default_agent_profile<W>(
        &mut self,
        id: Option<String>,
        profile_id: String,
        idempotency_key: Option<String>,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let idempotency_key = match self.parse_idempotency_key(idempotency_key) {
            Ok(key) => key,
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "set_default_agent_profile", error.to_string()),
                )
                .await?;
                return Ok(());
            }
        };
        match self.idempotent_retry_response(idempotency_key.as_ref(), "set_default_agent_profile")
        {
            Ok(Some(data)) => {
                write_rpc_response(
                    writer,
                    RpcResponse::success(id, "set_default_agent_profile", Some(data)),
                )
                .await?;
                return Ok(());
            }
            Ok(None) => {}
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "set_default_agent_profile", error.to_string()),
                )
                .await?;
                return Ok(());
            }
        }

        if self.is_streaming() {
            write_rpc_response(
                writer,
                RpcResponse::error(
                    id,
                    "set_default_agent_profile",
                    "cannot set default agent profile while agent is streaming",
                ),
            )
            .await?;
            return Ok(());
        }

        let profile_id = match ProfileId::new(profile_id) {
            Ok(profile_id) => profile_id,
            Err(message) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "set_default_agent_profile", message),
                )
                .await?;
                return Ok(());
            }
        };

        let mut session = match self.coding_session.take() {
            Some(session) => session,
            None => match self.open_reload_session().await {
                Ok(session) => session,
                Err(error) => {
                    write_rpc_response(
                        writer,
                        RpcResponse::error(id, "set_default_agent_profile", error.to_string()),
                    )
                    .await?;
                    return Ok(());
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
                    "set_default_agent_profile",
                    format!("Unknown agent profile: {profile_id}"),
                ),
            )
            .await?;
            return Ok(());
        }

        let complete_key = idempotency_key.clone();
        self.remember_idempotency_key(
            idempotency_key,
            "set_default_agent_profile",
            OperationKind::SetDefaultAgentProfile,
        );

        self.ensure_session_event_pump(&session);
        let event_flush = self
            .session_event_flush
            .as_ref()
            .expect("session event pump installed")
            .clone();

        let result = session
            .run(CodingAgentOperation::SetDefaultAgentProfile {
                profile_id: profile_id.clone(),
            })
            .await;
        flush_session_product_events(event_flush).await;
        match result {
            Ok(operation_outcome) => match operation_outcome {
                CodingAgentOperationOutcome::DefaultAgentProfileChanged => {
                    let data = serde_json::json!({ "defaultAgentProfileId": profile_id.as_str() });
                    self.coding_session = Some(session);
                    write_rpc_response(
                        writer,
                        RpcResponse::success(id, "set_default_agent_profile", Some(data)),
                    )
                    .await?;
                    self.drain_session_product_events(writer).await?;
                    self.mark_idempotency_complete(complete_key.as_ref());
                    Ok(())
                }
                _ => unreachable!(
                    "set default agent profile operation returned a different public outcome"
                ),
            },
            Err(error) => {
                self.coding_session = Some(session);
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "set_default_agent_profile", error.to_string()),
                )
                .await?;
                self.drain_session_product_events(writer).await?;
                self.mark_idempotency_complete(complete_key.as_ref());
                Ok(())
            }
        }
    }

    async fn handle_list_delegation_confirmations<W>(
        &mut self,
        id: Option<String>,
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
                    "list_delegation_confirmations",
                    "cannot list delegation confirmations while agent is streaming",
                ),
            )
            .await?;
            return Ok(());
        }

        let confirmations = self
            .coding_session
            .as_ref()
            .map(CodingAgentSession::pending_delegation_confirmations)
            .unwrap_or_default()
            .into_iter()
            .map(|pending| rpc_pending_delegation_confirmation(&pending))
            .collect::<Vec<_>>();
        write_rpc_response(
            writer,
            RpcResponse::success(
                id,
                "list_delegation_confirmations",
                Some(serde_json::json!({ "confirmations": confirmations })),
            ),
        )
        .await
    }

    async fn handle_list_tool_authorizations<W>(
        &mut self,
        id: Option<String>,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let authorizations = match self.pending_tool_authorizations() {
            Ok(authorizations) => authorizations,
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "list_tool_authorizations", error.to_string()),
                )
                .await?;
                return Ok(());
            }
        };
        write_rpc_response(
            writer,
            RpcResponse::success(
                id,
                "list_tool_authorizations",
                Some(serde_json::json!({ "authorizations": authorizations })),
            ),
        )
        .await
    }

    async fn handle_approve_tool_authorization<W>(
        &mut self,
        id: Option<String>,
        authorization_id: String,
        scope: RpcToolAuthorizationApprovalScope,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let decision = match scope {
            RpcToolAuthorizationApprovalScope::Once => ToolAuthorizationDecision::AllowOnce,
            RpcToolAuthorizationApprovalScope::Operation => {
                ToolAuthorizationDecision::AllowForOperation
            }
        };
        match self.decide_tool_authorization(&authorization_id, decision) {
            Ok(()) => {
                write_rpc_response(
                    writer,
                    RpcResponse::success(
                        id,
                        "approve_tool_authorization",
                        Some(serde_json::json!({
                            "authorizationId": authorization_id,
                            "scope": match scope {
                                RpcToolAuthorizationApprovalScope::Once => "once",
                                RpcToolAuthorizationApprovalScope::Operation => "operation",
                            },
                        })),
                    ),
                )
                .await
            }
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "approve_tool_authorization", error.to_string()),
                )
                .await
            }
        }
    }

    async fn handle_deny_tool_authorization<W>(
        &mut self,
        id: Option<String>,
        authorization_id: String,
        reason: Option<String>,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        match self.decide_tool_authorization(
            &authorization_id,
            ToolAuthorizationDecision::Deny { reason },
        ) {
            Ok(()) => {
                write_rpc_response(
                    writer,
                    RpcResponse::success(
                        id,
                        "deny_tool_authorization",
                        Some(serde_json::json!({ "authorizationId": authorization_id })),
                    ),
                )
                .await
            }
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "deny_tool_authorization", error.to_string()),
                )
                .await
            }
        }
    }

    fn pending_tool_authorizations(
        &self,
    ) -> Result<Vec<crate::authorization::ToolAuthorizationRequest>, CodingSessionError> {
        if let Some(connection) = self.client_connection.as_ref() {
            return connection.pending_tool_authorizations();
        }
        self.coding_session
            .as_ref()
            .map(CodingAgentSession::pending_tool_authorizations)
            .ok_or_else(|| CodingSessionError::Input {
                message: "no active coding session".into(),
            })
    }

    fn decide_tool_authorization(
        &self,
        authorization_id: &str,
        decision: ToolAuthorizationDecision,
    ) -> Result<(), CodingSessionError> {
        if let Some(connection) = self.client_connection.as_ref() {
            return connection.decide_tool_authorization(authorization_id, decision);
        }
        self.coding_session
            .as_ref()
            .ok_or_else(|| CodingSessionError::Input {
                message: "no active coding session".into(),
            })?
            .decide_tool_authorization(authorization_id, decision)
    }

    async fn handle_reject_delegation<W>(
        &mut self,
        id: Option<String>,
        operation_id: String,
        tool_call_id: String,
        reason: Option<String>,
        idempotency_key: Option<String>,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let idempotency_key = match self.parse_idempotency_key(idempotency_key) {
            Ok(key) => key,
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "reject_delegation", error.to_string()),
                )
                .await?;
                return Ok(());
            }
        };
        match self.idempotent_retry_response(idempotency_key.as_ref(), "reject_delegation") {
            Ok(Some(data)) => {
                write_rpc_response(
                    writer,
                    RpcResponse::success(id, "reject_delegation", Some(data)),
                )
                .await?;
                return Ok(());
            }
            Ok(None) => {}
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "reject_delegation", error.to_string()),
                )
                .await?;
                return Ok(());
            }
        }

        if self.is_streaming() {
            write_rpc_response(
                writer,
                RpcResponse::error(
                    id,
                    "reject_delegation",
                    "cannot reject delegation while agent is streaming",
                ),
            )
            .await?;
            return Ok(());
        }

        let Some(mut session) = self.coding_session.take() else {
            write_rpc_response(
                writer,
                RpcResponse::error(id, "reject_delegation", "no active coding session"),
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
                        "reject_delegation",
                        format!(
                        "pending delegation confirmation not found: operation_id={operation_id}, tool_call_id={tool_call_id}"
                    ),
                    ),
                )
                .await?;
                return Ok(());
            }
        };

        let complete_key = idempotency_key.clone();
        self.remember_idempotency_key(
            idempotency_key,
            "reject_delegation",
            OperationKind::DelegationConfirmation,
        );

        let reason = reason.unwrap_or_default();
        let reason = if reason.trim().is_empty() {
            "delegation rejected by user".to_string()
        } else {
            reason
        };
        self.ensure_session_event_pump(&session);
        let event_flush = self
            .session_event_flush
            .as_ref()
            .expect("session event pump installed")
            .clone();

        let result = session
            .run(CodingAgentOperation::RejectDelegation {
                operation_id,
                tool_call_id,
                reason: reason.clone(),
            })
            .await;
        flush_session_product_events(event_flush).await;
        match result {
            Ok(operation_outcome) => match operation_outcome {
                CodingAgentOperationOutcome::DelegationRejected => {
                    self.coding_session = Some(session);
                    write_rpc_response(
                        writer,
                        RpcResponse::success(
                            id,
                            "reject_delegation",
                            Some(serde_json::json!({
                                "delegation": rpc_pending_delegation_confirmation(&pending),
                                "reason": reason,
                            })),
                        ),
                    )
                    .await?;
                    self.drain_session_product_events(writer).await?;
                    self.mark_idempotency_complete(complete_key.as_ref());
                    Ok(())
                }
                _ => unreachable!(
                    "delegation rejection operation returned a different public outcome"
                ),
            },
            Err(error) => {
                self.coding_session = Some(session);
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "reject_delegation", error.to_string()),
                )
                .await?;
                self.drain_session_product_events(writer).await?;
                self.mark_idempotency_complete(complete_key.as_ref());
                Ok(())
            }
        }
    }

    async fn open_profile_listing_session(&self) -> Result<CodingAgentSession, CliError> {
        let options = SessionRunOptions::disabled(self.options.session.cwd.clone());
        Ok(open_new_runtime_session(&options).await?)
    }

    async fn handle_plugin_command<W>(
        &mut self,
        id: Option<String>,
        command_id: String,
        args: serde_json::Value,
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
                    "plugin_command",
                    "cannot run plugin command while agent is streaming",
                ),
            )
            .await?;
            return Ok(());
        }

        let should_load_plugins = self.coding_session.is_none();
        if should_load_plugins {
            let session = match self.open_reload_session().await {
                Ok(session) => session,
                Err(error) => {
                    write_rpc_response(
                        writer,
                        RpcResponse::error(id, "plugin_command", error.to_string()),
                    )
                    .await?;
                    return Ok(());
                }
            };
            self.coding_session = Some(session);
        }

        let session = self
            .coding_session
            .as_mut()
            .expect("plugin command session is initialized");

        if should_load_plugins {
            if let Err(error) = session
                .run(CodingAgentOperation::PluginLoad)
                .await
                .map(|outcome| match outcome {
                    CodingAgentOperationOutcome::PluginLoad(_) => (),
                    _ => unreachable!("plugin load operation returned a different public outcome"),
                })
            {
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "plugin_command", error.to_string()),
                )
                .await?;
                return Ok(());
            }
        }

        let task = match session.submit(CodingAgentOperation::PluginCommand {
            command_id: command_id.clone(),
            args,
        }) {
            Ok(task) => task,
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "plugin_command", error.to_string()),
                )
                .await?;
                return Ok(());
            }
        };

        match task.join().await {
            Ok(CodingAgentOperationOutcome::PluginCommand(output)) => {
                write_rpc_response(
                    writer,
                    RpcResponse::success(
                        id,
                        "plugin_command",
                        Some(serde_json::json!({
                            "commandId": command_id,
                            "output": output,
                        })),
                    ),
                )
                .await
            }
            Ok(_) => unreachable!("plugin command operation returned a different public outcome"),
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "plugin_command", error.to_string()),
                )
                .await
            }
        }
    }

    async fn handle_reload<W>(&mut self, id: Option<String>, writer: &mut W) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        if self.is_streaming() {
            write_rpc_response(
                writer,
                RpcResponse::error(
                    id,
                    "reload",
                    "cannot reload plugins while agent is streaming",
                ),
            )
            .await?;
            return Ok(());
        }

        let mut session = match self.coding_session.take() {
            Some(session) => session,
            None => match self.open_reload_session().await {
                Ok(session) => session,
                Err(error) => {
                    write_rpc_response(writer, RpcResponse::error(id, "reload", error.to_string()))
                        .await?;
                    return Ok(());
                }
            },
        };

        match session.run(CodingAgentOperation::PluginLoad).await {
            Ok(operation_outcome) => match operation_outcome {
                CodingAgentOperationOutcome::PluginLoad(outcome) => {
                    let data = rpc_plugin_reload_data(&outcome);
                    self.coding_session = Some(session);
                    write_rpc_response(writer, RpcResponse::success(id, "reload", Some(data))).await
                }
                _ => unreachable!("plugin load operation returned a different public outcome"),
            },
            Err(error) => {
                self.coding_session = Some(session);
                write_rpc_response(writer, RpcResponse::error(id, "reload", error.to_string()))
                    .await
            }
        }
    }

    async fn open_reload_session(&self) -> Result<CodingAgentSession, CliError> {
        Ok(open_new_runtime_session(&self.options.session).await?)
    }
}

fn rpc_self_healing_edit_replacement(
    edit: RpcSelfHealingEditReplacement,
) -> SelfHealingEditReplacement {
    SelfHealingEditReplacement::new(edit.old_text, edit.new_text)
}

fn rpc_self_healing_edit_data(outcome: &SelfHealingEditOutcome) -> serde_json::Value {
    serde_json::json!({
        "path": outcome.path,
        "message": outcome.message,
        "diff": outcome.diff,
        "patch": outcome.patch,
        "firstChangedLine": outcome.first_changed_line,
        "attempts": outcome.attempts,
        "diagnostics": outcome
            .diagnostics
            .iter()
            .map(|diagnostic| serde_json::json!({ "message": diagnostic.message }))
            .collect::<Vec<_>>(),
        "checkOutput": outcome
            .check_output
            .as_ref()
            .map(rpc_self_healing_check_output_data),
        "repairAttempts": outcome
            .repair_attempts
            .iter()
            .map(rpc_self_healing_repair_attempt_data)
            .collect::<Vec<_>>(),
    })
}

fn rpc_self_healing_repair_attempt_data(
    repair: &SelfHealingEditRepairAttempt,
) -> serde_json::Value {
    serde_json::json!({
        "attempt": repair.attempt,
        "edits": repair
            .replacements
            .iter()
            .map(|replacement| serde_json::json!({
                "oldText": replacement.old_text,
                "newText": replacement.new_text,
            }))
            .collect::<Vec<_>>(),
        "diagnostics": repair
            .diagnostics
            .iter()
            .map(|diagnostic| serde_json::json!({ "message": diagnostic.message }))
            .collect::<Vec<_>>(),
        "checkOutput": repair
            .check_output
            .as_ref()
            .map(rpc_self_healing_check_output_data),
    })
}

fn rpc_self_healing_edit_error_data(error: &CodingSessionError) -> Option<serde_json::Value> {
    match error {
        CodingSessionError::SelfHealingEditFailed {
            diagnostics,
            check_output,
            repair_attempts,
            ..
        } => Some(serde_json::json!({
            "diagnostics": diagnostics
                .iter()
                .map(|diagnostic| serde_json::json!({ "message": diagnostic.message }))
                .collect::<Vec<_>>(),
            "checkOutput": check_output
                .as_ref()
                .map(rpc_self_healing_check_output_data),
            "repairAttempts": repair_attempts
                .iter()
                .map(rpc_self_healing_repair_attempt_data)
                .collect::<Vec<_>>(),
        })),
        _ => None,
    }
}

fn rpc_self_healing_check_output_data(output: &SelfHealingEditCheckOutput) -> serde_json::Value {
    serde_json::json!({
        "command": output.command,
        "stdout": output.stdout,
        "stderr": output.stderr,
        "exitCode": output.exit_code,
    })
}

fn rpc_agent_profiles_data(session: &CodingAgentSession) -> serde_json::Value {
    let view = session.view();
    let default_profile_id = view.default_agent_profile_id;
    let agents = session
        .agent_profiles()
        .into_iter()
        .map(|profile| rpc_agent_profile(&profile, &default_profile_id))
        .collect::<Vec<_>>();

    serde_json::json!({
        "defaultAgentProfileId": default_profile_id.as_str(),
        "agents": agents,
        "diagnostics": rpc_profile_diagnostics(session),
    })
}

fn rpc_team_profiles_data(session: &CodingAgentSession) -> serde_json::Value {
    let teams = session
        .team_profiles()
        .into_iter()
        .map(|profile| rpc_team_profile(&profile))
        .collect::<Vec<_>>();

    serde_json::json!({
        "teams": teams,
        "diagnostics": rpc_profile_diagnostics(session),
    })
}

fn rpc_agent_profile(profile: &AgentProfile, default_profile_id: &ProfileId) -> serde_json::Value {
    serde_json::json!({
        "id": profile.id.as_str(),
        "displayName": profile.display_name,
        "description": profile.description.as_deref(),
        "source": rpc_profile_source(profile.source),
        "path": profile.path.as_ref().map(|path| path.display().to_string()),
        "isDefault": &profile.id == default_profile_id,
        "model": profile.model.as_deref(),
        "systemPrompt": profile.system_prompt.as_deref(),
        "tools": profile.tools,
        "skills": profile.skills,
        "supervision": rpc_supervision_policy(&profile.supervision),
        "delegation": rpc_delegation_policy(&profile.delegation),
    })
}

fn rpc_team_profile(profile: &TeamProfile) -> serde_json::Value {
    serde_json::json!({
        "id": profile.id.as_str(),
        "displayName": profile.display_name,
        "description": profile.description.as_deref(),
        "source": rpc_profile_source(profile.source),
        "path": profile.path.as_ref().map(|path| path.display().to_string()),
        "supervisor": rpc_team_supervisor(&profile.supervisor),
        "strategy": rpc_team_strategy(&profile.strategy),
        "members": rpc_profile_id_list(&profile.members),
        "delegation": rpc_delegation_policy(&profile.delegation),
    })
}

pub(super) fn rpc_pending_delegation_confirmation(
    pending: &PendingDelegationConfirmation,
) -> serde_json::Value {
    serde_json::json!({
        "operationId": pending.operation_id,
        "turnId": pending.turn_id,
        "toolCallId": pending.tool_call_id,
        "requestingProfileId": pending.requesting_profile_id.as_str(),
        "targetKind": rpc_profile_kind(pending.target_kind),
        "targetId": pending.target_id.as_str(),
        "task": pending.task,
        "reason": pending.reason,
    })
}

fn rpc_profile_diagnostics(session: &CodingAgentSession) -> Vec<serde_json::Value> {
    session
        .profile_diagnostics()
        .into_iter()
        .map(|diagnostic| rpc_profile_diagnostic(&diagnostic))
        .collect()
}

fn rpc_profile_diagnostic(diagnostic: &ProfileDiagnostic) -> serde_json::Value {
    serde_json::json!({
        "source": rpc_profile_source(diagnostic.source),
        "kind": rpc_profile_kind(diagnostic.kind),
        "path": diagnostic.path.as_ref().map(|path| path.display().to_string()),
        "profileId": diagnostic.profile_id.as_ref().map(ProfileId::as_str),
        "message": diagnostic.message,
    })
}

fn rpc_delegation_policy(policy: &DelegationPolicy) -> serde_json::Value {
    serde_json::json!({
        "allowDelegateAgent": policy.allow_delegate_agent,
        "allowDelegateTeam": policy.allow_delegate_team,
        "maxDepth": policy.max_depth,
        "maxParallelChildren": policy.max_parallel_children,
        "requireConfirmation": rpc_delegation_confirmation_mode(&policy.require_confirmation),
        "allowedAgents": rpc_profile_id_list(&policy.allowed_agents),
        "allowedTeams": rpc_profile_id_list(&policy.allowed_teams),
    })
}

fn rpc_profile_id_list(ids: &[ProfileId]) -> Vec<&str> {
    ids.iter().map(ProfileId::as_str).collect()
}

fn rpc_team_supervisor(supervisor: &TeamSupervisor) -> serde_json::Value {
    match supervisor {
        TeamSupervisor::Deterministic => serde_json::json!({ "mode": "deterministic" }),
        TeamSupervisor::Agent(profile_id) => serde_json::json!({
            "mode": "agent",
            "profileId": profile_id.as_str(),
        }),
    }
}

fn rpc_profile_source(source: ProfileSource) -> &'static str {
    match source {
        ProfileSource::BuiltIn => "built_in",
        ProfileSource::User => "user",
        ProfileSource::Project => "project",
    }
}

fn rpc_profile_kind(kind: ProfileKind) -> &'static str {
    match kind {
        ProfileKind::Agent => "agent",
        ProfileKind::Team => "team",
    }
}

fn rpc_supervision_policy(policy: &SupervisionPolicy) -> &'static str {
    match policy {
        SupervisionPolicy::Session => "session",
        SupervisionPolicy::SelfReview => "self_review",
        SupervisionPolicy::LlmSupervisor => "llm_supervisor",
    }
}

fn rpc_delegation_confirmation_mode(mode: &DelegationConfirmationMode) -> &'static str {
    match mode {
        DelegationConfirmationMode::Never => "never",
        DelegationConfirmationMode::Writes => "writes",
        DelegationConfirmationMode::Always => "always",
    }
}

fn rpc_team_strategy(strategy: &TeamStrategy) -> &'static str {
    match strategy {
        TeamStrategy::PlanExecuteReview => "plan_execute_review",
    }
}

fn rpc_plugin_reload_data(outcome: &CodingAgentPluginLoadOutcome) -> serde_json::Value {
    let diagnostics = outcome
        .diagnostics
        .iter()
        .map(|diagnostic| match diagnostic.plugin_id.as_deref() {
            Some(plugin_id) => serde_json::json!({
                "pluginId": plugin_id,
                "message": diagnostic.message,
            }),
            None => serde_json::json!({
                "message": diagnostic.message,
            }),
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "loadedPluginIds": outcome.loaded_plugin_ids,
        "diagnostics": diagnostics,
        "capabilityChanged": outcome.capability_changed,
    })
}

pub(super) fn has_images(images: &Option<Vec<pi_ai::api::conversation::ContentBlock>>) -> bool {
    images.as_ref().is_some_and(|images| !images.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rpc_prompt_uses_versioned_snapshot_cursor_wire_shape() {
        let command: RpcCommand = serde_json::from_value(json!({
            "type": "prompt",
            "message": "resume",
            "afterSnapshotCursor": {
                "streamId": "session-1",
                "snapshotProtocolMajor": 2,
                "lastEventSequence": 7,
                "capabilityGeneration": 3
            }
        }))
        .unwrap();

        let RpcCommand::Prompt {
            after_snapshot_cursor,
            ..
        } = command
        else {
            panic!("expected prompt command");
        };
        let cursor = after_snapshot_cursor.expect("cursor must be decoded");
        assert_eq!(cursor.stream_id, "session-1");
        assert_eq!(cursor.snapshot_protocol_major, 2);
        assert_eq!(cursor.last_event_sequence, 7);
        assert_eq!(cursor.capability_generation, 3);
    }

    #[test]
    fn rpc_tool_authorization_commands_are_typed() {
        let list: RpcCommand = serde_json::from_value(json!({
            "type": "list_tool_authorizations",
            "id": "list-1"
        }))
        .unwrap();
        assert!(matches!(
            list,
            RpcCommand::ListToolAuthorizations { id } if id.as_deref() == Some("list-1")
        ));

        let approve: RpcCommand = serde_json::from_value(json!({
            "type": "approve_tool_authorization",
            "authorizationId": "auth-1",
            "scope": "operation"
        }))
        .unwrap();
        assert!(matches!(
            approve,
            RpcCommand::ApproveToolAuthorization {
                authorization_id,
                scope: RpcToolAuthorizationApprovalScope::Operation,
                ..
            } if authorization_id == "auth-1"
        ));

        let deny: RpcCommand = serde_json::from_value(json!({
            "type": "deny_tool_authorization",
            "authorizationId": "auth-2",
            "reason": "not approved"
        }))
        .unwrap();
        assert!(matches!(
            deny,
            RpcCommand::DenyToolAuthorization {
                authorization_id,
                reason,
                ..
            } if authorization_id == "auth-2" && reason.as_deref() == Some("not approved")
        ));
    }

    #[test]
    fn rpc_sync_commands_use_product_event_stream_boundary() {
        let source = include_str!("commands.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let product_subscription = [".", "subscribe_product_events()"].concat();
        let compatibility_subscription = [".", "subscribe()"].concat();
        let product_adapter = ["adapter", ".push_product_event(&event)"].concat();
        let compatibility_adapter = ["adapter", ".push(&event)"].concat();

        assert_eq!(source.matches(&product_subscription).count(), 0);
        assert!(!source.contains(&compatibility_subscription));
        assert!(!source.contains(&product_adapter));
        assert_eq!(
            source
                .matches("ensure_session_event_pump(&session)")
                .count(),
            3
        );
        assert_eq!(
            source
                .matches("drain_session_product_events(writer)")
                .count(),
            6
        );
        assert!(!source.contains(&compatibility_adapter));
    }
}
