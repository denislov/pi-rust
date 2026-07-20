use crate::adapters::rpc::commands::{has_images, rpc_pending_delegation_confirmation};
use crate::adapters::rpc::event_queue::RpcQueuedProductEvent;
use crate::adapters::rpc::events::RpcCodingEventAdapter;
use crate::adapters::rpc::state::{
    CodingOperationOutcome, CodingOperationTaskResult, RpcBackgroundCompletion,
    RpcBackgroundOperation, RpcForegroundOperation, RpcState,
};
use crate::adapters::rpc::wire::{write_json_line, write_rpc_response};
use crate::api::operation::CodingAgentOperation;
use crate::app::bootstrap::PromptInvocation;
use crate::app::bootstrap::SessionMode;
use crate::app::cli::error::CliError;
use crate::app::cli::prompt_options::PromptRunOptions;
use crate::app::session::{open_new_runtime_session, runtime_session_root};
use crate::operations::prompt::context::QueuedPromptInput;
use crate::protocol::types::{
    ProtocolEvent, RpcResponse, RpcShutdownLifecycleEvent, RpcShutdownResponse, RpcShutdownStatus,
    StreamingBehavior,
};
use crate::runtime::facade::{
    AgentInvocationOptions, AgentTeamOptions, CodingAgentControlId, CodingAgentDraft,
    CodingAgentDraftId, CodingAgentDraftKind, CodingAgentLifecycleRejection, CodingAgentReconnect,
    CodingAgentSession, CodingAgentShutdownOutcome, CodingAgentSnapshotCursor, CodingSessionError,
    OperationIdempotencyKey, OperationKind, ProductEvent, ProductEventSequence, ProfileId,
    ProfileKind, PromptTurnMode, PromptTurnOptions,
};
use pi_agent_core::api::resources::AgentResources;
use tokio::io::AsyncWrite;
use tokio::sync::oneshot;

impl RpcState {
    pub(super) async fn handle_compact<W>(
        &mut self,
        id: Option<String>,
        custom_instructions: Option<String>,
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
                    "compact",
                    "cannot compact while another operation is running",
                ),
            )
            .await?;
            return Ok(());
        }
        if !matches!(self.options.session.mode, SessionMode::Enabled) {
            write_rpc_response(
                writer,
                RpcResponse::error(
                    id,
                    "compact",
                    "manual compaction requires a persistent Rust-native session",
                ),
            )
            .await?;
            return Ok(());
        }

        let (mut session, session_root) = self.take_or_open_coding_session().await?;
        let options = PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
            prompt: String::new(),
            model: self.model.clone(),
            api_key: self.api_key.clone(),
            auth_diagnostics: Vec::new(),
            system_prompt: None,
            max_turns: None,
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
            invocation: PromptInvocation::Compact {
                custom_instructions,
            },
        })
        .with_mode(PromptTurnMode::Rpc);
        self.ensure_client_connection(&session)?;
        self.ensure_session_event_pump(&session);
        let event_flush = self
            .session_event_flush
            .as_ref()
            .expect("session event pump installed")
            .clone();
        let operation = CodingAgentOperation::Compact(options);
        let (done_tx, done_rx) = oneshot::channel();

        write_rpc_response(writer, RpcResponse::success(id, "compact", None)).await?;

        let shutdown_handle = session.runtime_shutdown_handle();
        self.active_shutdown_handle.get_or_insert(shutdown_handle);
        tokio::spawn(async move {
            let outcome =
                session
                    .run(operation)
                    .await
                    .map_err(CliError::from)
                    .map(|operation_outcome| {
                        operation_outcome.into_compact().expect(
                            "manual compaction operation returned a different public outcome",
                        )
                    });
            flush_session_product_events(event_flush).await;
            let _ = done_tx.send(CodingOperationTaskResult {
                session: Some(session),
                session_root,
                outcome: CodingOperationOutcome::Compact(outcome),
            });
        });

        self.is_compacting = true;
        self.foreground = Some(RpcForegroundOperation {
            done: done_rx,
            operation_kind: OperationKind::Compact,
            idempotency_key: None,
        });
        Ok(())
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "RPC prompt fields and stream cursor remain explicit protocol inputs"
    )]
    pub(super) async fn handle_prompt<W>(
        &mut self,
        id: Option<String>,
        message: String,
        images: Option<Vec<pi_ai::api::conversation::ContentBlock>>,
        streaming_behavior: Option<StreamingBehavior>,
        after_snapshot_cursor: Option<CodingAgentSnapshotCursor>,
        idempotency_key: Option<String>,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let idempotency_key = match self.parse_idempotency_key(idempotency_key) {
            Ok(key) => key,
            Err(error) => {
                write_rpc_response(writer, RpcResponse::error(id, "prompt", error.to_string()))
                    .await?;
                return Ok(());
            }
        };
        if self.has_active_operations() && after_snapshot_cursor.is_some() {
            self.handle_streaming_prompt(
                id,
                message,
                images,
                streaming_behavior,
                after_snapshot_cursor,
                writer,
            )
            .await?;
            return Ok(());
        }
        match self.idempotent_retry_response(idempotency_key.as_ref(), "prompt") {
            Ok(Some(data)) => {
                write_rpc_response(writer, RpcResponse::success(id, "prompt", Some(data))).await?;
                return Ok(());
            }
            Ok(None) => {}
            Err(error) => {
                write_rpc_response(writer, RpcResponse::error(id, "prompt", error.to_string()))
                    .await?;
                return Ok(());
            }
        }

        if self.is_streaming() {
            self.handle_streaming_prompt(
                id,
                message,
                images,
                streaming_behavior,
                after_snapshot_cursor,
                writer,
            )
            .await?;
            return Ok(());
        }

        let invocation = match rpc_prompt_invocation(message.clone(), images) {
            Ok(invocation) => invocation,
            Err(error) => {
                write_rpc_response(writer, RpcResponse::error(id, "prompt", error.to_string()))
                    .await?;
                return Ok(());
            }
        };
        self.start_coding_session_prompt(id, message, invocation, idempotency_key, writer)
            .await
    }

    async fn handle_streaming_prompt<W>(
        &mut self,
        id: Option<String>,
        message: String,
        images: Option<Vec<pi_ai::api::conversation::ContentBlock>>,
        streaming_behavior: Option<StreamingBehavior>,
        after_snapshot_cursor: Option<CodingAgentSnapshotCursor>,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        if let Some(cursor) = after_snapshot_cursor {
            let replayed = match reconnect_running_prompt_after(self, &cursor).await {
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
                if has_images(&images) {
                    write_rpc_response(
                        writer,
                        RpcResponse::error(
                            id,
                            "prompt",
                            "reconnect-only prompt requests cannot include image content",
                        ),
                    )
                    .await?;
                    return Ok(());
                }
                write_rpc_response(writer, RpcResponse::success(id, "prompt", None)).await?;
                for event in replayed {
                    write_json_line(writer, &event).await?;
                }
                return Ok(());
            }

            self.handle_streaming_prompt_control(id, message, images, streaming_behavior, writer)
                .await?;
            for event in replayed {
                write_json_line(writer, &event).await?;
            }
            return Ok(());
        }

        self.handle_streaming_prompt_control(id, message, images, streaming_behavior, writer)
            .await
    }

    async fn handle_streaming_prompt_control<W>(
        &mut self,
        id: Option<String>,
        message: String,
        images: Option<Vec<pi_ai::api::conversation::ContentBlock>>,
        streaming_behavior: Option<StreamingBehavior>,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let Some(foreground) = self.foreground.as_ref() else {
            write_rpc_response(
                writer,
                RpcResponse::error(id, "prompt", "agent is not streaming"),
            )
            .await?;
            return Ok(());
        };

        if foreground.operation_kind != OperationKind::Prompt {
            write_rpc_response(
                writer,
                RpcResponse::error(
                    id,
                    "prompt",
                    format!(
                        "cannot send prompt control while {} is running",
                        foreground.operation_kind.as_str()
                    ),
                ),
            )
            .await?;
            return Ok(());
        }
        let Some(connection) = self.client_connection.as_ref() else {
            write_rpc_response(
                writer,
                RpcResponse::error(id, "prompt", "agent is not streaming"),
            )
            .await?;
            return Ok(());
        };
        let Some(submitted) = connection.state()?.submitted_operation else {
            write_rpc_response(
                writer,
                RpcResponse::error(id, "prompt", "agent is not streaming"),
            )
            .await?;
            return Ok(());
        };
        let control = connection.prompt_control(submitted.operation_id);
        let control_id = CodingAgentControlId(
            id.clone()
                .unwrap_or_else(|| format!("rpc-prompt-control-{message}")),
        );

        let content = match rpc_control_content(message.clone(), images) {
            Ok(content) => content,
            Err(error) => {
                write_rpc_response(writer, RpcResponse::error(id, "prompt", error.to_string()))
                    .await?;
                return Ok(());
            }
        };
        let result = match streaming_behavior {
            Some(StreamingBehavior::Steer) => match content {
                Some(content) => control.steer_content(control_id, content),
                None => control.steer(control_id, message),
            },
            Some(StreamingBehavior::FollowUp) => match content {
                Some(content) => control.follow_up_content(control_id, content),
                None => control.follow_up(control_id, message),
            },
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
            Ok(_) => write_rpc_response(writer, RpcResponse::success(id, "prompt", None)).await,
            Err(rejection) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "prompt", format!("{:?}", rejection.reason)),
                )
                .await
            }
        }
    }

    pub(super) async fn handle_invoke_agent<W>(
        &mut self,
        id: Option<String>,
        profile_id: String,
        task: String,
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
                    RpcResponse::error(id, "invoke_agent", error.to_string()),
                )
                .await?;
                return Ok(());
            }
        };
        match self.idempotent_retry_response(idempotency_key.as_ref(), "invoke_agent") {
            Ok(Some(data)) => {
                write_rpc_response(writer, RpcResponse::success(id, "invoke_agent", Some(data)))
                    .await?;
                return Ok(());
            }
            Ok(None) => {}
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "invoke_agent", error.to_string()),
                )
                .await?;
                return Ok(());
            }
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

        let (mut session, session_root) = self.take_or_open_coding_session().await?;

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
            ai_client: self.options.ai_client.clone(),
            session: Some(self.options.session.clone()),
            session_target: None,
            session_name: None,
            thinking_level: Some(self.thinking_level),
            tool_execution: None,
            resources: AgentResources::default(),
            settings: Some(self.effective_prompt_settings()),
            invocation: PromptInvocation::Text(task.clone()),
        })
        .with_mode(PromptTurnMode::Rpc);
        let invocation_options =
            AgentInvocationOptions::new(profile_id.clone(), task.clone(), prompt_options);
        let connection = self.ensure_client_connection(&session)?;
        self.ensure_session_event_pump(&session);
        let event_flush = self
            .session_event_flush
            .as_ref()
            .expect("session event pump installed")
            .clone();
        let invocation = match session.submit(CodingAgentOperation::InvokeAgent(invocation_options))
        {
            Ok(invocation) => invocation,
            Err(error) => {
                self.coding_session = Some(session);
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "invoke_agent", error.to_string()),
                )
                .await?;
                return Ok(());
            }
        };
        let operation_id = invocation.operation_id().to_owned();
        invocation.bind_control_owner(&connection);

        write_rpc_response(
            writer,
            RpcResponse::success(
                id,
                "invoke_agent",
                Some(serde_json::json!({
                    "operationId": operation_id,
                    "profileId": profile_id.as_str(),
                    "task": task,
                })),
            ),
        )
        .await?;
        write_json_line(writer, &ProtocolEvent::AgentStart).await?;

        let running_idempotency_key = idempotency_key.clone();
        self.remember_idempotency_key(
            idempotency_key,
            "invoke_agent",
            OperationKind::AgentInvocation,
        );

        let shutdown_handle = session.runtime_shutdown_handle();
        self.active_shutdown_handle.get_or_insert(shutdown_handle);
        self.coding_session = Some(session);
        let completion_tx = self.background_completion_tx.clone();
        let completion_operation_id = operation_id.clone();
        tokio::spawn(async move {
            let outcome =
                invocation
                    .join()
                    .await
                    .map_err(CliError::from)
                    .map(|operation_outcome| {
                        operation_outcome.into_agent_invocation().expect(
                            "agent invocation operation returned a different public outcome",
                        )
                    });
            flush_session_product_events(event_flush).await;

            let _ = completion_tx.send(RpcBackgroundCompletion {
                operation_id: completion_operation_id,
                result: CodingOperationTaskResult {
                    session: None,
                    session_root,
                    outcome: CodingOperationOutcome::AgentInvocation(outcome),
                },
            });
        });

        self.background_operations.insert(
            operation_id,
            RpcBackgroundOperation {
                operation_kind: OperationKind::AgentInvocation,
                idempotency_key: running_idempotency_key,
            },
        );

        Ok(())
    }

    pub(super) async fn handle_invoke_team<W>(
        &mut self,
        id: Option<String>,
        team_id: String,
        task: String,
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
                    RpcResponse::error(id, "invoke_team", error.to_string()),
                )
                .await?;
                return Ok(());
            }
        };
        match self.idempotent_retry_response(idempotency_key.as_ref(), "invoke_team") {
            Ok(Some(data)) => {
                write_rpc_response(writer, RpcResponse::success(id, "invoke_team", Some(data)))
                    .await?;
                return Ok(());
            }
            Ok(None) => {}
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "invoke_team", error.to_string()),
                )
                .await?;
                return Ok(());
            }
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

        let (mut session, session_root) = self.take_or_open_coding_session().await?;

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
            ai_client: self.options.ai_client.clone(),
            session: Some(self.options.session.clone()),
            session_target: None,
            session_name: None,
            thinking_level: Some(self.thinking_level),
            tool_execution: None,
            resources: AgentResources::default(),
            settings: Some(self.effective_prompt_settings()),
            invocation: PromptInvocation::Text(task.clone()),
        })
        .with_mode(PromptTurnMode::Rpc);
        let team_options = AgentTeamOptions::new(team_id.clone(), task.clone(), prompt_options);
        let connection = self.ensure_client_connection(&session)?;
        self.ensure_session_event_pump(&session);
        let event_flush = self
            .session_event_flush
            .as_ref()
            .expect("session event pump installed")
            .clone();
        let invocation = match session.submit(CodingAgentOperation::InvokeTeam(team_options)) {
            Ok(invocation) => invocation,
            Err(error) => {
                self.coding_session = Some(session);
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "invoke_team", error.to_string()),
                )
                .await?;
                return Ok(());
            }
        };
        let operation_id = invocation.operation_id().to_owned();
        invocation.bind_control_owner(&connection);

        write_rpc_response(
            writer,
            RpcResponse::success(
                id,
                "invoke_team",
                Some(serde_json::json!({
                    "operationId": operation_id,
                    "teamId": team_id.as_str(),
                    "task": task,
                })),
            ),
        )
        .await?;
        write_json_line(writer, &ProtocolEvent::AgentStart).await?;

        let running_idempotency_key = idempotency_key.clone();
        self.remember_idempotency_key(idempotency_key, "invoke_team", OperationKind::AgentTeam);

        let shutdown_handle = session.runtime_shutdown_handle();
        self.active_shutdown_handle.get_or_insert(shutdown_handle);
        self.coding_session = Some(session);
        let completion_tx = self.background_completion_tx.clone();
        let completion_operation_id = operation_id.clone();
        tokio::spawn(async move {
            let outcome =
                invocation
                    .join()
                    .await
                    .map_err(CliError::from)
                    .map(|operation_outcome| {
                        operation_outcome
                            .into_agent_team()
                            .expect("agent team operation returned a different public outcome")
                    });
            flush_session_product_events(event_flush).await;

            let _ = completion_tx.send(RpcBackgroundCompletion {
                operation_id: completion_operation_id,
                result: CodingOperationTaskResult {
                    session: None,
                    session_root,
                    outcome: CodingOperationOutcome::AgentTeam(outcome),
                },
            });
        });

        self.background_operations.insert(
            operation_id,
            RpcBackgroundOperation {
                operation_kind: OperationKind::AgentTeam,
                idempotency_key: running_idempotency_key,
            },
        );

        Ok(())
    }

    pub(super) async fn handle_approve_delegation<W>(
        &mut self,
        id: Option<String>,
        operation_id: String,
        tool_call_id: String,
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
                    RpcResponse::error(id, "approve_delegation", error.to_string()),
                )
                .await?;
                return Ok(());
            }
        };
        match self.idempotent_retry_response(idempotency_key.as_ref(), "approve_delegation") {
            Ok(Some(data)) => {
                write_rpc_response(
                    writer,
                    RpcResponse::success(id, "approve_delegation", Some(data)),
                )
                .await?;
                return Ok(());
            }
            Ok(None) => {}
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(id, "approve_delegation", error.to_string()),
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
        let session_root = runtime_session_root(&self.options.session)?;
        self.ensure_session_event_pump(&session);
        let event_flush = self
            .session_event_flush
            .as_ref()
            .expect("session event pump installed")
            .clone();
        let (done_tx, done_rx) = oneshot::channel();

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

        let running_idempotency_key = idempotency_key.clone();
        self.remember_idempotency_key(idempotency_key, "approve_delegation", operation_kind);

        let shutdown_handle = session.runtime_shutdown_handle();
        self.active_shutdown_handle.get_or_insert(shutdown_handle);
        tokio::spawn(async move {
            let outcome = session
                .run(CodingAgentOperation::ApproveDelegation {
                    operation_id,
                    tool_call_id,
                })
                .await
                .map_err(CliError::from)
                .map(|operation_outcome| {
                    operation_outcome
                        .into_delegation_approved()
                        .expect("delegation approval operation returned a different public outcome")
                });
            flush_session_product_events(event_flush).await;

            let _ = done_tx.send(CodingOperationTaskResult {
                session: Some(session),
                session_root,
                outcome: CodingOperationOutcome::DelegationApproval(outcome),
            });
        });

        self.foreground = Some(RpcForegroundOperation {
            done: done_rx,
            operation_kind,
            idempotency_key: running_idempotency_key,
        });

        Ok(())
    }

    async fn start_coding_session_prompt<W>(
        &mut self,
        id: Option<String>,
        message: String,
        invocation: PromptInvocation,
        idempotency_key: Option<OperationIdempotencyKey>,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let (mut session, session_root) = self.take_or_open_coding_session().await?;
        let prompt_options = PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
            prompt: message.clone(),
            model: self.model.clone(),
            api_key: self.api_key.clone(),
            auth_diagnostics: Vec::new(),
            system_prompt: None,
            max_turns: None,
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
            invocation,
        })
        .with_mode(PromptTurnMode::Rpc);
        let prompt_options =
            prompt_options.with_queued_inputs(self.steering.clone(), self.follow_up.clone());
        let connection = self.ensure_client_connection(&session)?;
        let draft_id = CodingAgentDraftId("rpc-prompt".into());
        let operation = CodingAgentOperation::Prompt(prompt_options);
        connection.set_prompt_operation_draft(
            draft_id.clone(),
            prompt_draft_display(&message, &operation),
            &operation,
        )?;
        self.ensure_session_event_pump(&session);
        let event_flush = self
            .session_event_flush
            .as_ref()
            .expect("session event pump installed")
            .clone();
        let submission = connection.prepare_submission(&mut session, draft_id, &operation)?;
        let (done_tx, done_rx) = oneshot::channel();

        write_rpc_response(writer, RpcResponse::success(id, "prompt", None)).await?;
        write_json_line(writer, &ProtocolEvent::AgentStart).await?;

        let running_idempotency_key = idempotency_key.clone();
        self.remember_idempotency_key(idempotency_key, "prompt", OperationKind::Prompt);

        let shutdown_handle = session.runtime_shutdown_handle();
        self.active_shutdown_handle.get_or_insert(shutdown_handle);
        tokio::spawn(async move {
            let outcome =
                session
                    .run(operation)
                    .await
                    .map_err(CliError::from)
                    .map(|operation_outcome| {
                        operation_outcome
                            .into_prompt()
                            .expect("prompt operation returned a different public outcome")
                    });
            flush_session_product_events(event_flush).await;

            let _ = done_tx.send(CodingOperationTaskResult {
                session: Some(session),
                session_root,
                outcome: CodingOperationOutcome::Prompt(outcome),
            });
            drop(submission);
        });

        self.foreground = Some(RpcForegroundOperation {
            done: done_rx,
            operation_kind: OperationKind::Prompt,
            idempotency_key: running_idempotency_key,
        });

        Ok(())
    }

    pub(super) fn enqueue_steer(&mut self, input: QueuedPromptInput) {
        if let Some(connection) = &self.client_connection {
            let _ = connection.enqueue_control_draft(CodingAgentDraft {
                id: CodingAgentDraftId(format!("rpc-steer-{}", self.steering.len())),
                kind: CodingAgentDraftKind::Steer,
                text: input.display_text(),
            });
        }
        self.steering.push(input);
    }

    pub(super) fn enqueue_follow_up(&mut self, input: QueuedPromptInput) {
        if let Some(connection) = &self.client_connection {
            let _ = connection.enqueue_control_draft(CodingAgentDraft {
                id: CodingAgentDraftId(format!("rpc-follow-up-{}", self.follow_up.len())),
                kind: CodingAgentDraftKind::FollowUp,
                text: input.display_text(),
            });
        }
        self.follow_up.push(input);
    }

    pub(super) async fn write_product_event<W>(
        &mut self,
        event: ProductEvent,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let pushed = push_live_product_event(
            &mut self.event_adapter,
            &mut self.adapter_applied_sequence,
            &event,
        );
        for protocol_event in pushed.protocol_events {
            write_json_line(writer, &protocol_event).await?;
        }
        self.acknowledge_delivered_product_event(&event)?;
        Ok(())
    }

    fn acknowledge_delivered_product_event(&self, event: &ProductEvent) -> Result<(), CliError> {
        let Some(connection) = &self.client_connection else {
            return Ok(());
        };
        match connection.acknowledge(event.sequence()) {
            Ok(_) => Ok(()),
            Err(CodingSessionError::Lifecycle {
                reason: CodingAgentLifecycleRejection::RuntimeShutDown,
            }) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    pub(super) async fn finish_coding_running_prompt<W>(
        &mut self,
        result: Result<CodingOperationTaskResult, oneshot::error::RecvError>,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        let Some(running) = self.foreground.take() else {
            return Ok(());
        };
        if running.operation_kind == OperationKind::Compact {
            self.is_compacting = false;
        }
        self.mark_idempotency_complete(running.idempotency_key.as_ref());
        self.drain_session_product_events(writer).await?;

        let result = result.map_err(|error| {
            CliError::AgentFailure(format!(
                "coding agent task ended before reporting completion: {error}"
            ))
        })?;

        let consumed_prompt_queue = matches!(&result.outcome, CodingOperationOutcome::Prompt(_));
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
            CodingOperationOutcome::Compact(outcome) => {
                if let Ok(outcome) = outcome
                    && let (Some(session_root), Some(session_id)) = (
                        result.session_root.as_ref(),
                        prompt_outcome_session_id(outcome),
                    )
                {
                    self.active_leaf_id = prompt_outcome_leaf_id(outcome).map(ToString::to_string);
                    self.active_session_path = Some(session_root.join(session_id));
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

        let session = match result.session {
            Some(session) => session,
            None => self.coding_session.take().ok_or_else(|| {
                CliError::AgentFailure(
                    "runtime-owned operation completed without a retained session".into(),
                )
            })?,
        };
        self.coding_session = Some(session);
        if consumed_prompt_queue {
            self.steering.clear();
            self.follow_up.clear();
            if let Some(connection) = &self.client_connection {
                let _ = connection.clear_control_drafts();
            }
        }
        self.finish_pending_shutdown_if_idle(writer).await?;
        match outcome {
            Err(CliError::SessionFailure(message)) if message == "cancelled" => Ok(()),
            outcome => outcome,
        }
    }

    pub(super) async fn finish_background_operation<W>(
        &mut self,
        completion: RpcBackgroundCompletion,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        self.drain_session_product_events(writer).await?;
        let Some(operation) = self.background_operations.remove(&completion.operation_id) else {
            return Err(CliError::AgentFailure(format!(
                "background operation completed without registry ownership: {}",
                completion.operation_id
            )));
        };
        self.mark_idempotency_complete(operation.idempotency_key.as_ref());

        let outcome = match &completion.result.outcome {
            CodingOperationOutcome::AgentInvocation(outcome)
                if operation.operation_kind == OperationKind::AgentInvocation =>
            {
                outcome.as_ref().map(|_| ()).map_err(Clone::clone)
            }
            CodingOperationOutcome::AgentTeam(outcome)
                if operation.operation_kind == OperationKind::AgentTeam =>
            {
                outcome.as_ref().map(|_| ()).map_err(Clone::clone)
            }
            _ => Err(CliError::AgentFailure(format!(
                "background operation {} completed with a mismatched outcome",
                completion.operation_id
            ))),
        };

        self.finish_pending_shutdown_if_idle(writer).await?;
        match outcome {
            Err(CliError::SessionFailure(message)) if message == "cancelled" => Ok(()),
            outcome => outcome,
        }
    }

    pub(super) async fn drain_session_product_events<W>(
        &mut self,
        writer: &mut W,
    ) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        while let Some(events) = self.session_events.as_mut() {
            let Ok(item) = events.try_recv() else {
                break;
            };
            match item {
                RpcQueuedProductEvent::Event(event) => {
                    let pushed = push_live_product_event(
                        &mut self.event_adapter,
                        &mut self.adapter_applied_sequence,
                        &event,
                    );
                    for protocol_event in pushed.protocol_events {
                        write_json_line(writer, &protocol_event).await?;
                    }
                    self.acknowledge_delivered_product_event(&event)?;
                }
                RpcQueuedProductEvent::Overflow { skipped } => {
                    write_rpc_response(
                        writer,
                        RpcResponse::error_with_data(
                            None,
                            "event_stream",
                            format!(
                                "event stream lagged by {skipped} events; client must request a fresh UI snapshot"
                            ),
                            serde_json::json!({
                                "code": "event_stream_lag",
                                "skipped": skipped,
                                "recovery": "fresh_snapshot"
                            }),
                        ),
                    )
                    .await?;
                    self.session_events_closed = true;
                    break;
                }
            }
        }
        Ok(())
    }

    async fn finish_pending_shutdown_if_idle<W>(&mut self, writer: &mut W) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        if self.has_active_operations() {
            return Ok(());
        }
        self.active_shutdown_handle = None;
        let Some(id) = self.pending_shutdown_response.take() else {
            return Ok(());
        };
        let mut session = self.coding_session.take().ok_or_else(|| {
            CliError::AgentFailure(
                "runtime operations drained without returning the owned session".into(),
            )
        })?;
        let status = match session.shutdown().await? {
            CodingAgentShutdownOutcome::ShutDown => RpcShutdownStatus::ShutDown,
            CodingAgentShutdownOutcome::AlreadyShutDown => RpcShutdownStatus::AlreadyShutDown,
        };
        if status == RpcShutdownStatus::ShutDown {
            write_json_line(writer, &RpcShutdownLifecycleEvent { status }).await?;
        }
        write_rpc_response(
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
        .await?;
        self.coding_session = Some(session);
        Ok(())
    }

    pub(super) async fn take_or_open_coding_session(
        &mut self,
    ) -> Result<(CodingAgentSession, Option<std::path::PathBuf>), CliError> {
        let session_root = runtime_session_root(&self.options.session)?;
        let session = match self.coding_session.take() {
            Some(session) => session,
            None => open_new_runtime_session(&self.options.session).await?,
        };
        Ok((session, session_root))
    }

    pub(super) async fn emit_queue_update<W>(&self, writer: &mut W) -> Result<(), CliError>
    where
        W: AsyncWrite + Unpin,
    {
        write_json_line(
            writer,
            &ProtocolEvent::QueueUpdate {
                steering: self
                    .steering
                    .iter()
                    .map(QueuedPromptInput::display_text)
                    .collect(),
                follow_up: self
                    .follow_up
                    .iter()
                    .map(QueuedPromptInput::display_text)
                    .collect(),
            },
        )
        .await
    }
}

fn rpc_prompt_invocation(
    message: String,
    images: Option<Vec<pi_ai::api::conversation::ContentBlock>>,
) -> Result<PromptInvocation, CodingSessionError> {
    let Some(images) = images.filter(|images| !images.is_empty()) else {
        return Ok(PromptInvocation::Text(message));
    };
    let mut content = Vec::with_capacity(images.len() + usize::from(!message.is_empty()));
    if !message.is_empty() {
        content.push(pi_ai::api::conversation::ContentBlock::Text {
            text: message,
            text_signature: None,
        });
    }
    for image in images {
        if !matches!(image, pi_ai::api::conversation::ContentBlock::Image { .. }) {
            return Err(CodingSessionError::Input {
                message: "RPC prompt images must contain only image content blocks".into(),
            });
        }
        content.push(image);
    }
    Ok(PromptInvocation::Content(content))
}

pub(super) fn rpc_control_content(
    message: String,
    images: Option<Vec<pi_ai::api::conversation::ContentBlock>>,
) -> Result<Option<Vec<pi_ai::api::conversation::ContentBlock>>, CodingSessionError> {
    match rpc_prompt_invocation(message, images)? {
        PromptInvocation::Text(_) => Ok(None),
        PromptInvocation::Content(content) => Ok(Some(content)),
        _ => unreachable!("RPC control input only constructs text or content invocations"),
    }
}

fn prompt_draft_display(message: &str, operation: &CodingAgentOperation) -> String {
    let CodingAgentOperation::Prompt(options) = operation else {
        return message.to_owned();
    };
    match options.invocation() {
        PromptInvocation::Content(content) => content
            .iter()
            .filter_map(|block| match block {
                pi_ai::api::conversation::ContentBlock::Text { text, .. } => Some(text.clone()),
                pi_ai::api::conversation::ContentBlock::Image { mime_type, .. } => {
                    Some(format!("[image:{mime_type}]"))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => message.to_owned(),
    }
}

pub(super) async fn flush_session_product_events(
    flush: tokio::sync::mpsc::UnboundedSender<oneshot::Sender<()>>,
) {
    let (acknowledge, acknowledged) = oneshot::channel();
    if flush.send(acknowledge).is_ok() {
        let _ = acknowledged.await;
    }
}

pub(super) async fn reconnect_running_prompt_after(
    state: &mut RpcState,
    cursor: &CodingAgentSnapshotCursor,
) -> Result<Vec<ProtocolEvent>, CodingSessionError> {
    let Some(connection) = state.client_connection.as_ref() else {
        return Ok(Vec::new());
    };
    let recovery = connection.reconnect_from_cursor(cursor)?;
    let (retained_events, through) = match recovery {
        CodingAgentReconnect::Replayed { events, cursor, .. } => {
            (events, cursor.last_event_sequence)
        }
        CodingAgentReconnect::FreshSnapshotRequired(recovery) => {
            return Err(CodingSessionError::EventStreamGap {
                requested_after: recovery.requested_sequence,
                oldest_available: recovery.oldest_available_sequence,
            });
        }
    };

    let mut protocol_events = Vec::new();
    for event in retained_events {
        if event.sequence() <= state.adapter_applied_sequence.get() {
            continue;
        }
        let sequence = ProductEventSequence::new(event.sequence());
        protocol_events.extend(state.event_adapter.push_product_event(&event));
        state.adapter_applied_sequence = state.adapter_applied_sequence.max(sequence);
    }
    connection.acknowledge(through)?;
    state.session_events_closed = false;
    Ok(protocol_events)
}

struct LiveProductEventPush {
    protocol_events: Vec<ProtocolEvent>,
}

fn push_live_product_event(
    adapter: &mut RpcCodingEventAdapter,
    applied_sequence: &mut ProductEventSequence,
    event: &ProductEvent,
) -> LiveProductEventPush {
    let sequence = event.sequence_internal();
    if sequence <= *applied_sequence {
        return LiveProductEventPush {
            protocol_events: Vec::new(),
        };
    }
    let protocol_events = adapter.push_product_event(event);
    *applied_sequence = (*applied_sequence).max(sequence);
    LiveProductEventPush { protocol_events }
}

fn prompt_outcome_session_id(outcome: &crate::runtime::facade::PromptTurnOutcome) -> Option<&str> {
    match outcome {
        crate::runtime::facade::PromptTurnOutcome::Success { session_id, .. } => {
            session_id.as_deref()
        }
        crate::runtime::facade::PromptTurnOutcome::Aborted { session_id, .. } => {
            session_id.as_deref()
        }
        crate::runtime::facade::PromptTurnOutcome::Failed { .. } => None,
    }
}

fn prompt_outcome_leaf_id(outcome: &crate::runtime::facade::PromptTurnOutcome) -> Option<&str> {
    match outcome {
        crate::runtime::facade::PromptTurnOutcome::Success { leaf_id, .. } => leaf_id.as_deref(),
        crate::runtime::facade::PromptTurnOutcome::Aborted { .. }
        | crate::runtime::facade::PromptTurnOutcome::Failed { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::runtime::CliRunOptions;
    use crate::runtime::facade::{CodingAgentSession, CodingAgentSessionOptions};

    #[tokio::test]
    async fn rpc_session_event_pump_reports_receiver_lag() {
        let session = CodingAgentSession::non_persistent_with_event_capacity_for_tests(
            CodingAgentSessionOptions::new(),
            1,
        )
        .await
        .unwrap();
        let mut state = RpcState::new(CliRunOptions::default()).unwrap();
        state.ensure_session_event_pump(&session);
        for index in 0..3 {
            session.emit_diagnostic_for_tests(format!("lagged event {index}"));
        }

        assert_eq!(
            state.session_events.as_mut().unwrap().recv().await.unwrap(),
            RpcQueuedProductEvent::Overflow { skipped: 2 }
        );
    }

    #[test]
    fn rpc_state_owns_single_product_event_stream_boundary() {
        let prompt_source = include_str!("prompt.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let state_source = include_str!("state.rs");
        let rpc_source = include_str!("mod.rs");
        let product_subscription = [".", "subscribe_product_events()"].concat();
        let compatibility_subscription = [".", "subscribe()"].concat();
        let product_adapter = ["adapter", ".push_product_event(event)"].concat();

        assert_eq!(prompt_source.matches(&product_subscription).count(), 0);
        assert_eq!(state_source.matches(&product_subscription).count(), 1);
        assert!(!prompt_source.contains(&compatibility_subscription));
        assert!(state_source.contains("session_events: Option<RpcProductEventReceiver>"));
        let foreground_source = state_source
            .split("pub(super) struct RpcForegroundOperation")
            .nth(1)
            .unwrap()
            .split("pub(super) struct RpcBackgroundOperation")
            .next()
            .unwrap();
        assert!(!foreground_source.contains("RpcProductEventReceiver"));
        assert!(
            state_source.contains("background_operations: HashMap<String, RpcBackgroundOperation>")
        );
        assert!(rpc_source.contains("state.session_events.as_mut()"));
        assert!(rpc_source.contains("BackgroundOperationDone"));
        assert!(prompt_source.contains(&product_adapter));
        assert!(prompt_source.contains("connection.reconnect_from_cursor(cursor)"));
        assert!(!prompt_source.contains("connection.reconnect(cursor.get())"));
    }
}
