use crate::api::operation::{
    BranchSummaryReusePolicy, CodingAgentOperation, CodingAgentPluginLoadOutcome,
};
use crate::app::cli::error::CliError;
use crate::app::cli::prompt_options::PromptRunOptions;
use crate::app::session::{ResolvedSessionTarget, open_interactive_session};
use crate::runtime::control::{OperationControl, OperationKind, operation_control_for_adapter};
use crate::runtime::facade::{
    AgentInvocationOptions, AgentTeamOptions, AgentTeamOutcome, CodingAgentClientConnection,
    CodingAgentClientId, CodingAgentSession, CodingSessionError, ProfileId, PromptTurnOptions,
    PromptTurnOutcome, SelfHealingEditOutcome, SelfHealingEditRequest,
};
use tokio::sync::{mpsc, oneshot};

const PROMPT_TASK_CONTROL_CAPACITY: usize = 32;

#[allow(
    clippy::large_enum_variant,
    reason = "single-owner task completion preserves exhaustive typed operation outcomes"
)]
pub(super) enum PromptTaskResult {
    Coding(CodingPromptTaskResult),
    AgentInvocation(AgentInvocationTaskResult),
    AgentTeam(AgentTeamTaskResult),
    DelegationApproval(DelegationApprovalTaskResult),
    SelfHealingEdit(SelfHealingEditTaskResult),
    PluginReload(PluginReloadTaskResult),
    SetDefaultAgentProfile(SetDefaultAgentProfileTaskResult),
    SessionTreeLabel(SessionTreeLabelTaskResult),
    DelegationRejection(DelegationRejectionTaskResult),
    ForkSession(ForkSessionTaskResult),
}

#[allow(
    clippy::large_enum_variant,
    reason = "task completion is moved once and retains typed failure recovery state"
)]
pub(super) enum PromptTaskCompletion {
    Completed(PromptTaskResult),
    Failed(PromptTaskFailure),
    SetupFailed(CliError),
}

pub(super) struct PromptTaskFailure {
    pub(super) session: CodingAgentSession,
    pub(super) error: CliError,
}

pub(super) struct CodingPromptTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) outcome: PromptTurnOutcome,
    pub(super) session_target: Option<ResolvedSessionTarget>,
    pub(super) completion_notice: Option<String>,
    pub(super) hydrate_transcript: bool,
}

pub(super) struct ForkSessionTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) session_target: ResolvedSessionTarget,
    pub(super) completion_notice: Option<String>,
    pub(super) hydrate_transcript: bool,
}

pub(super) struct AgentInvocationTaskResult {
    pub(super) session: CodingAgentSession,
}

pub(super) struct AgentTeamTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) outcome: AgentTeamOutcome,
}

pub(super) struct DelegationApprovalTaskResult {
    pub(super) session: CodingAgentSession,
}

pub(super) struct SetDefaultAgentProfileTaskResult {
    pub(super) session: CodingAgentSession,
}

pub(super) struct SessionTreeLabelTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) entry_id: String,
    pub(super) label: Option<String>,
    pub(super) updated_at: String,
}

pub(super) struct DelegationRejectionTaskResult {
    pub(super) session: CodingAgentSession,
}

pub(super) struct SelfHealingEditTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) outcome: SelfHealingEditOutcome,
}

pub(super) struct PluginReloadTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) outcome: CodingAgentPluginLoadOutcome,
}

enum PromptTaskControlHandle {
    Prompt(mpsc::Sender<PromptTaskControl>),
    Operation(mpsc::Sender<PromptTaskControl>),
    AbortOnly(Option<oneshot::Sender<()>>),
}

#[derive(Debug)]
enum PromptTaskControl {
    Abort,
    Steer(String),
    FollowUp(String),
    DecideToolAuthorization {
        authorization_id: String,
        decision: crate::authorization::ToolAuthorizationDecision,
    },
}

pub(super) struct PromptTask {
    control: PromptTaskControlHandle,
    pub(super) connection_handoff:
        Option<oneshot::Receiver<Result<Option<CodingAgentClientConnection>, CliError>>>,
    pub(super) done: oneshot::Receiver<PromptTaskCompletion>,
    abort_requested: bool,
}

impl PromptTask {
    pub(super) fn spawn_prompt(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        default_agent_profile_id: ProfileId,
    ) -> Result<Self, CliError> {
        Ok(Self::spawn_coding(
            options,
            existing_session,
            default_agent_profile_id,
        ))
    }

    pub(super) fn spawn_compact(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        default_agent_profile_id: ProfileId,
    ) -> Result<Self, CliError> {
        Ok(Self::spawn_coding_compact(
            options,
            existing_session,
            default_agent_profile_id,
        ))
    }

    pub(super) fn spawn_agent_invocation(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        profile_id: ProfileId,
        task: String,
        default_agent_profile_id: ProfileId,
    ) -> Result<Self, CliError> {
        Ok(Self::spawn_coding_agent_invocation(
            options,
            existing_session,
            profile_id,
            task,
            default_agent_profile_id,
        ))
    }

    pub(super) fn spawn_agent_team(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        team_id: ProfileId,
        task: String,
        default_agent_profile_id: ProfileId,
    ) -> Result<Self, CliError> {
        Ok(Self::spawn_coding_agent_team(
            options,
            existing_session,
            team_id,
            task,
            default_agent_profile_id,
        ))
    }

    pub(super) fn spawn_delegation_approval(
        existing_session: CodingAgentSession,
        operation_id: String,
        tool_call_id: String,
    ) -> Result<Self, CliError> {
        Ok(Self::spawn_coding_delegation_approval(
            existing_session,
            operation_id,
            tool_call_id,
        ))
    }

    pub(super) fn spawn_set_default_agent_profile(
        existing_session: CodingAgentSession,
        profile_id: ProfileId,
    ) -> Result<Self, CliError> {
        Ok(Self::spawn_coding_set_default_agent_profile(
            existing_session,
            profile_id,
        ))
    }

    pub(super) fn spawn_session_tree_label(
        existing_session: CodingAgentSession,
        entry_id: String,
        label: Option<String>,
    ) -> Result<Self, CliError> {
        Ok(Self::spawn_coding_session_tree_label(
            existing_session,
            entry_id,
            label,
        ))
    }

    pub(super) fn spawn_delegation_rejection(
        existing_session: CodingAgentSession,
        operation_id: String,
        tool_call_id: String,
        reason: String,
    ) -> Result<Self, CliError> {
        Ok(Self::spawn_coding_delegation_rejection(
            existing_session,
            operation_id,
            tool_call_id,
            reason,
        ))
    }

    pub(super) fn spawn_plugin_reload(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        default_agent_profile_id: ProfileId,
    ) -> Result<Self, CliError> {
        Ok(Self::spawn_coding_plugin_reload(
            options,
            existing_session,
            default_agent_profile_id,
        ))
    }

    pub(super) fn spawn_self_healing_edit(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        request: SelfHealingEditRequest,
        default_agent_profile_id: ProfileId,
    ) -> Result<Self, CliError> {
        Ok(Self::spawn_coding_self_healing_edit(
            options,
            existing_session,
            request,
            default_agent_profile_id,
        ))
    }

    pub(super) fn spawn_branch_summary(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        source_leaf_id: String,
        target_leaf_id: String,
        custom_instructions: Option<String>,
        default_agent_profile_id: ProfileId,
    ) -> Result<Self, CliError> {
        Ok(Self::spawn_coding_branch_summary(
            options,
            existing_session,
            source_leaf_id,
            target_leaf_id,
            custom_instructions,
            default_agent_profile_id,
        ))
    }

    pub(super) fn spawn_branch_summary_navigation(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        source_leaf_id: String,
        target_leaf_id: String,
        default_agent_profile_id: ProfileId,
    ) -> Result<Self, CliError> {
        Ok(Self::spawn_coding_branch_summary_navigation(
            options,
            existing_session,
            source_leaf_id,
            target_leaf_id,
            default_agent_profile_id,
        ))
    }

    pub(super) fn spawn_fork_session(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        target_leaf_id: Option<String>,
        completion_notice: Option<String>,
        default_agent_profile_id: ProfileId,
    ) -> Result<Self, CliError> {
        Ok(Self::spawn_coding_fork_session(
            options,
            existing_session,
            target_leaf_id,
            completion_notice,
            default_agent_profile_id,
        ))
    }

    pub(super) async fn abort_once(&mut self) {
        if self.abort_requested {
            return;
        }
        match &mut self.control {
            PromptTaskControlHandle::Prompt(control) => {
                let _ = control.send(PromptTaskControl::Abort).await;
            }
            PromptTaskControlHandle::Operation(control) => {
                let _ = control.send(PromptTaskControl::Abort).await;
            }
            PromptTaskControlHandle::AbortOnly(abort) => {
                if let Some(abort) = abort.take() {
                    let _ = abort.send(());
                }
            }
        }
        self.abort_requested = true;
    }

    pub(super) async fn steer(&self, text: String) -> bool {
        match &self.control {
            PromptTaskControlHandle::Prompt(control) => {
                control.send(PromptTaskControl::Steer(text)).await.is_ok()
            }
            PromptTaskControlHandle::Operation(_) | PromptTaskControlHandle::AbortOnly(_) => false,
        }
    }

    pub(super) async fn follow_up(&self, text: String) -> bool {
        match &self.control {
            PromptTaskControlHandle::Prompt(control) => control
                .send(PromptTaskControl::FollowUp(text))
                .await
                .is_ok(),
            PromptTaskControlHandle::Operation(_) | PromptTaskControlHandle::AbortOnly(_) => false,
        }
    }

    pub(super) async fn decide_tool_authorization(
        &self,
        authorization_id: String,
        decision: crate::authorization::ToolAuthorizationDecision,
    ) -> bool {
        match &self.control {
            PromptTaskControlHandle::Prompt(control)
            | PromptTaskControlHandle::Operation(control) => control
                .send(PromptTaskControl::DecideToolAuthorization {
                    authorization_id,
                    decision,
                })
                .await
                .is_ok(),
            PromptTaskControlHandle::AbortOnly(_) => false,
        }
    }

    fn spawn_coding(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (connection_tx, connection_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (control_tx, control_rx) = mpsc::channel(PROMPT_TASK_CONTROL_CAPACITY);

        tokio::spawn(async move {
            let result = run_coding_prompt_task(
                options,
                existing_session,
                default_agent_profile_id,
                connection_tx,
                control_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::Prompt(control_tx),
            connection_handoff: Some(connection_rx),
            done: done_rx,
            abort_requested: false,
        }
    }

    fn spawn_coding_compact(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (connection_tx, connection_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_compact_task(
                options,
                existing_session,
                default_agent_profile_id,
                connection_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            connection_handoff: Some(connection_rx),
            done: done_rx,
            abort_requested: false,
        }
    }

    fn spawn_coding_agent_invocation(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        profile_id: ProfileId,
        task: String,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (connection_tx, connection_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (control_tx, control_rx) = mpsc::channel(PROMPT_TASK_CONTROL_CAPACITY);

        tokio::spawn(async move {
            let result = run_coding_agent_invocation_task(
                options,
                existing_session,
                profile_id,
                task,
                default_agent_profile_id,
                connection_tx,
                control_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::Prompt(control_tx),
            connection_handoff: Some(connection_rx),
            done: done_rx,
            abort_requested: false,
        }
    }

    fn spawn_coding_agent_team(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        team_id: ProfileId,
        task: String,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (connection_tx, connection_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (control_tx, control_rx) = mpsc::channel(PROMPT_TASK_CONTROL_CAPACITY);

        tokio::spawn(async move {
            let result = run_coding_agent_team_task(
                options,
                existing_session,
                team_id,
                task,
                default_agent_profile_id,
                connection_tx,
                control_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::Operation(control_tx),
            connection_handoff: Some(connection_rx),
            done: done_rx,
            abort_requested: false,
        }
    }

    fn spawn_coding_delegation_approval(
        existing_session: CodingAgentSession,
        operation_id: String,
        tool_call_id: String,
    ) -> Self {
        let (connection_tx, connection_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (control_tx, control_rx) = mpsc::channel(PROMPT_TASK_CONTROL_CAPACITY);

        tokio::spawn(async move {
            let result = run_coding_delegation_approval_task(
                existing_session,
                operation_id,
                tool_call_id,
                connection_tx,
                control_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::Operation(control_tx),
            connection_handoff: Some(connection_rx),
            done: done_rx,
            abort_requested: false,
        }
    }

    fn spawn_coding_set_default_agent_profile(
        existing_session: CodingAgentSession,
        profile_id: ProfileId,
    ) -> Self {
        let (connection_tx, connection_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_set_default_agent_profile_task(
                existing_session,
                profile_id,
                connection_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            connection_handoff: Some(connection_rx),
            done: done_rx,
            abort_requested: false,
        }
    }

    fn spawn_coding_session_tree_label(
        existing_session: CodingAgentSession,
        entry_id: String,
        label: Option<String>,
    ) -> Self {
        let (connection_tx, connection_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_session_tree_label_task(
                existing_session,
                entry_id,
                label,
                connection_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            connection_handoff: Some(connection_rx),
            done: done_rx,
            abort_requested: false,
        }
    }

    fn spawn_coding_delegation_rejection(
        existing_session: CodingAgentSession,
        operation_id: String,
        tool_call_id: String,
        reason: String,
    ) -> Self {
        let (connection_tx, connection_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_delegation_rejection_task(
                existing_session,
                operation_id,
                tool_call_id,
                reason,
                connection_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            connection_handoff: Some(connection_rx),
            done: done_rx,
            abort_requested: false,
        }
    }

    fn spawn_coding_plugin_reload(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (connection_tx, connection_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_plugin_reload_task(
                options,
                existing_session,
                default_agent_profile_id,
                connection_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            connection_handoff: Some(connection_rx),
            done: done_rx,
            abort_requested: false,
        }
    }

    fn spawn_coding_self_healing_edit(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        request: SelfHealingEditRequest,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (connection_tx, connection_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_self_healing_edit_task(
                options,
                existing_session,
                request,
                default_agent_profile_id,
                connection_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            connection_handoff: Some(connection_rx),
            done: done_rx,
            abort_requested: false,
        }
    }

    fn spawn_coding_branch_summary(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        source_leaf_id: String,
        target_leaf_id: String,
        custom_instructions: Option<String>,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (connection_tx, connection_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_branch_summary_task(
                options,
                existing_session,
                source_leaf_id,
                target_leaf_id,
                custom_instructions,
                default_agent_profile_id,
                connection_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            connection_handoff: Some(connection_rx),
            done: done_rx,
            abort_requested: false,
        }
    }

    fn spawn_coding_branch_summary_navigation(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        source_leaf_id: String,
        target_leaf_id: String,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (connection_tx, connection_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_branch_summary_navigation_task(
                options,
                existing_session,
                source_leaf_id,
                target_leaf_id,
                default_agent_profile_id,
                connection_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            connection_handoff: Some(connection_rx),
            done: done_rx,
            abort_requested: false,
        }
    }

    fn spawn_coding_fork_session(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        target_leaf_id: Option<String>,
        completion_notice: Option<String>,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (connection_tx, connection_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_fork_session_task(
                options,
                existing_session,
                target_leaf_id,
                completion_notice,
                default_agent_profile_id,
                connection_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            connection_handoff: Some(connection_rx),
            done: done_rx,
            abort_requested: false,
        }
    }
}

fn handoff_interactive_connection(
    connection_tx: oneshot::Sender<Result<Option<CodingAgentClientConnection>, CliError>>,
    session: &CodingAgentSession,
    connect: bool,
) -> Result<(), CliError> {
    if !connect {
        return connection_tx.send(Ok(None)).map_err(|_| {
            CliError::AgentFailure("interactive connection handoff receiver closed".to_string())
        });
    }
    match session.connect(CodingAgentClientId::new("interactive")) {
        Ok(connection) => connection_tx.send(Ok(Some(connection))).map_err(|_| {
            CliError::AgentFailure("interactive connection handoff receiver closed".to_string())
        }),
        Err(error) => {
            connection_tx
                .send(Err(CliError::from(error.clone())))
                .map_err(|_| {
                    CliError::AgentFailure(
                        "interactive connection handoff receiver closed".to_string(),
                    )
                })?;
            Err(CliError::from(error))
        }
    }
}

fn complete_owned_task<T>(
    session: CodingAgentSession,
    result: Result<T, CliError>,
    completed: impl FnOnce(CodingAgentSession, T) -> PromptTaskResult,
) -> PromptTaskCompletion {
    match result {
        Ok(value) => PromptTaskCompletion::Completed(completed(session, value)),
        Err(error) => PromptTaskCompletion::Failed(PromptTaskFailure { session, error }),
    }
}

fn request_interactive_abort(
    control: &OperationControl,
    kind: OperationKind,
) -> Result<(), CliError> {
    normalize_interactive_abort_result(control.cancel_active(kind))
}

fn normalize_interactive_abort_result(
    result: Result<crate::runtime::control::OperationCancellationOutcome, CodingSessionError>,
) -> Result<(), CliError> {
    match result {
        Ok(_) => Ok(()),
        Err(CodingSessionError::UnsupportedCapability { capability })
            if capability.contains("no longer cancellable") =>
        {
            Ok(())
        }
        Err(error) => Err(CliError::from(error)),
    }
}

fn active_session_target(session: &CodingAgentSession) -> ResolvedSessionTarget {
    ResolvedSessionTarget::OpenOrCreateId(session.view().session_id.clone())
}

async fn run_coding_prompt_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    default_agent_profile_id: ProfileId,
    connection_tx: oneshot::Sender<Result<Option<CodingAgentClientConnection>, CliError>>,
    mut control_rx: mpsc::Receiver<PromptTaskControl>,
) -> PromptTaskCompletion {
    let needs_connection = existing_session.is_none();
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            match open_interactive_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
                default_agent_profile_id,
            )
            .await
            {
                Ok(session) => session,
                Err(error) => return PromptTaskCompletion::SetupFailed(error),
            }
        }
    };
    let result = async {
        let prompt_control = session.prompt_control_handle()?;
        let tool_authorization_control = session.tool_authorization_control();
        handoff_interactive_connection(connection_tx, &session, needs_connection)?;
        let prompt_options = PromptTurnOptions::from_prompt_run_options(options);

        let outcome = {
            let mut prompt = Box::pin(session.run(CodingAgentOperation::Prompt(prompt_options)));
            let mut abort_requested = false;
            let mut controls_open = true;
            loop {
                tokio::select! {
                    control = control_rx.recv(), if controls_open => {
                        match control {
                            Some(PromptTaskControl::Abort) if !abort_requested => {
                                abort_requested = true;
                                prompt_control.abort("user cancelled")?;
                            }
                            Some(PromptTaskControl::Steer(text)) => {
                                prompt_control.steer(text)?;
                            }
                            Some(PromptTaskControl::FollowUp(text)) => {
                                prompt_control.follow_up(text)?;
                            }
                            Some(PromptTaskControl::DecideToolAuthorization {
                                authorization_id,
                                decision,
                            }) => {
                                tool_authorization_control.decide(&authorization_id, decision)?;
                            }
                            Some(PromptTaskControl::Abort) => {}
                            None => {
                                controls_open = false;
                            }
                        }
                    }
                    outcome = &mut prompt => {
                        break outcome
                            .map_err(CliError::from)
                            .map(|operation_outcome| operation_outcome
                                .into_prompt()
                                .expect("prompt operation returned a different public outcome"));
                    }
                }
            }
        }?;
        Ok(outcome)
    }
    .await;

    complete_owned_task(session, result, |session, outcome| {
        PromptTaskResult::Coding(CodingPromptTaskResult {
            session,
            outcome,
            session_target: None,
            completion_notice: None,
            hydrate_transcript: false,
        })
    })
}

async fn run_coding_agent_invocation_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    profile_id: ProfileId,
    task: String,
    default_agent_profile_id: ProfileId,
    connection_tx: oneshot::Sender<Result<Option<CodingAgentClientConnection>, CliError>>,
    mut control_rx: mpsc::Receiver<PromptTaskControl>,
) -> PromptTaskCompletion {
    let needs_connection = existing_session.is_none();
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            match open_interactive_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
                default_agent_profile_id,
            )
            .await
            {
                Ok(session) => session,
                Err(error) => return PromptTaskCompletion::SetupFailed(error),
            }
        }
    };
    let result = async {
        let prompt_control = session.prompt_control_handle()?;
        let tool_authorization_control = session.tool_authorization_control();
        handoff_interactive_connection(connection_tx, &session, needs_connection)?;
        let invocation_options = AgentInvocationOptions::new(
            profile_id,
            task,
            PromptTurnOptions::from_prompt_run_options(options),
        );
        let mut invocation =
            Box::pin(session.run(CodingAgentOperation::InvokeAgent(invocation_options)));
        let mut abort_requested = false;
        let mut controls_open = true;
        loop {
            tokio::select! {
                control = control_rx.recv(), if controls_open => {
                    match control {
                        Some(PromptTaskControl::Abort) if !abort_requested => {
                            abort_requested = true;
                            prompt_control.abort("user cancelled")?;
                        }
                        Some(PromptTaskControl::Steer(text)) => {
                            prompt_control.steer(text)?;
                        }
                        Some(PromptTaskControl::FollowUp(text)) => {
                            prompt_control.follow_up(text)?;
                        }
                        Some(PromptTaskControl::DecideToolAuthorization {
                            authorization_id,
                            decision,
                        }) => {
                            tool_authorization_control.decide(&authorization_id, decision)?;
                        }
                        Some(PromptTaskControl::Abort) => {}
                        None => {
                            controls_open = false;
                        }
                    }
                }
                outcome = &mut invocation => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| {
                            operation_outcome
                                .into_agent_invocation()
                                .expect("agent invocation operation returned a different public outcome");
                        });
                }
            }
        }?;
        Ok(())
    }
    .await;

    complete_owned_task(session, result, |session, ()| {
        PromptTaskResult::AgentInvocation(AgentInvocationTaskResult { session })
    })
}

async fn run_coding_agent_team_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    team_id: ProfileId,
    task: String,
    default_agent_profile_id: ProfileId,
    connection_tx: oneshot::Sender<Result<Option<CodingAgentClientConnection>, CliError>>,
    mut control_rx: mpsc::Receiver<PromptTaskControl>,
) -> PromptTaskCompletion {
    let needs_connection = existing_session.is_none();
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            match open_interactive_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
                default_agent_profile_id,
            )
            .await
            {
                Ok(session) => session,
                Err(error) => return PromptTaskCompletion::SetupFailed(error),
            }
        }
    };
    let result = async {
        let operation_control = operation_control_for_adapter(&session);
        let tool_authorization_control = session.tool_authorization_control();
        handoff_interactive_connection(connection_tx, &session, needs_connection)?;
        let team_options = AgentTeamOptions::new(
            team_id,
            task,
            PromptTurnOptions::from_prompt_run_options(options),
        );

        let outcome = {
            let mut invocation =
                Box::pin(session.run(CodingAgentOperation::InvokeTeam(team_options)));
            let mut abort_requested = false;
            loop {
            tokio::select! {
                control = control_rx.recv() => {
                    match control {
                        Some(PromptTaskControl::Abort) if !abort_requested => {
                            abort_requested = true;
                            request_interactive_abort(&operation_control, OperationKind::AgentTeam)?;
                        }
                        Some(PromptTaskControl::DecideToolAuthorization {
                            authorization_id,
                            decision,
                        }) => {
                            tool_authorization_control.decide(&authorization_id, decision)?;
                        }
                        Some(PromptTaskControl::Abort)
                        | Some(PromptTaskControl::Steer(_))
                        | Some(PromptTaskControl::FollowUp(_))
                        | None => {}
                    }
                }
                outcome = &mut invocation => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| operation_outcome
                            .into_agent_team()
                            .expect("agent team operation returned a different public outcome"));
                }
            }
            }
        }?;

        Ok(outcome)
    }
    .await;

    complete_owned_task(session, result, |session, outcome| {
        PromptTaskResult::AgentTeam(AgentTeamTaskResult { session, outcome })
    })
}

async fn run_coding_delegation_approval_task(
    mut session: CodingAgentSession,
    operation_id: String,
    tool_call_id: String,
    connection_tx: oneshot::Sender<Result<Option<CodingAgentClientConnection>, CliError>>,
    mut control_rx: mpsc::Receiver<PromptTaskControl>,
) -> PromptTaskCompletion {
    let needs_connection = false;
    let result = async {
        let operation_control = operation_control_for_adapter(&session);
        let tool_authorization_control = session.tool_authorization_control();
        handoff_interactive_connection(connection_tx, &session, needs_connection)?;

        let mut approval = Box::pin(session.run(CodingAgentOperation::ApproveDelegation {
            operation_id,
            tool_call_id,
        }));
        let mut abort_requested = false;
        loop {
            tokio::select! {
                control = control_rx.recv() => {
                    match control {
                        Some(PromptTaskControl::Abort) if !abort_requested => {
                            abort_requested = true;
                            request_interactive_abort(
                                &operation_control,
                                OperationKind::DelegationConfirmation,
                            )?;
                        }
                        Some(PromptTaskControl::DecideToolAuthorization {
                            authorization_id,
                            decision,
                        }) => {
                            tool_authorization_control.decide(&authorization_id, decision)?;
                        }
                        Some(PromptTaskControl::Abort)
                        | Some(PromptTaskControl::Steer(_))
                        | Some(PromptTaskControl::FollowUp(_))
                        | None => {}
                    }
                }
                outcome = &mut approval => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| operation_outcome
                            .into_delegation_approved()
                            .expect("delegation approval operation returned a different public outcome"));
                }
            }
        }?;

        Ok(())
    }
    .await;

    complete_owned_task(session, result, |session, ()| {
        PromptTaskResult::DelegationApproval(DelegationApprovalTaskResult { session })
    })
}

async fn run_coding_set_default_agent_profile_task(
    mut session: CodingAgentSession,
    profile_id: ProfileId,
    connection_tx: oneshot::Sender<Result<Option<CodingAgentClientConnection>, CliError>>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
    let needs_connection = false;
    let result = async {
        handoff_interactive_connection(connection_tx, &session, needs_connection)?;

        let mut mutation =
            Box::pin(session.run(CodingAgentOperation::SetDefaultAgentProfile { profile_id }));
        tokio::select! {
            _ = &mut abort_rx => Err(CliError::from(CodingSessionError::Cancelled)),
            outcome = &mut mutation => {
                outcome
                    .map_err(CliError::from)
                    .map(|operation_outcome| operation_outcome
                        .into_default_agent_profile_changed()
                        .expect("set default agent profile operation returned a different public outcome"))
            }
        }?;

        Ok(())
    }
    .await;

    complete_owned_task(session, result, |session, ()| {
        PromptTaskResult::SetDefaultAgentProfile(SetDefaultAgentProfileTaskResult { session })
    })
}

async fn run_coding_session_tree_label_task(
    mut session: CodingAgentSession,
    entry_id: String,
    label: Option<String>,
    connection_tx: oneshot::Sender<Result<Option<CodingAgentClientConnection>, CliError>>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
    let needs_connection = false;
    let result = async {
        handoff_interactive_connection(connection_tx, &session, needs_connection)?;

        let mut mutation =
            Box::pin(session.run(CodingAgentOperation::SetSessionTreeLabel { entry_id, label }));
        let update = tokio::select! {
            _ = &mut abort_rx => Err(CliError::from(CodingSessionError::Cancelled)),
            outcome = &mut mutation => {
                outcome
                    .map_err(CliError::from)
                    .map(|operation_outcome| operation_outcome
                        .into_session_tree_label_changed()
                        .expect("session tree label operation returned a different public outcome"))
            }
        }?;

        Ok(update)
    }
    .await;

    complete_owned_task(session, result, |session, (entry_id, label, updated_at)| {
        PromptTaskResult::SessionTreeLabel(SessionTreeLabelTaskResult {
            session,
            entry_id,
            label,
            updated_at,
        })
    })
}

async fn run_coding_delegation_rejection_task(
    mut session: CodingAgentSession,
    operation_id: String,
    tool_call_id: String,
    reason: String,
    connection_tx: oneshot::Sender<Result<Option<CodingAgentClientConnection>, CliError>>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
    let needs_connection = false;
    let result = async {
        handoff_interactive_connection(connection_tx, &session, needs_connection)?;

        let mut rejection = Box::pin(session.run(CodingAgentOperation::RejectDelegation {
            operation_id,
            tool_call_id,
            reason,
        }));
        tokio::select! {
            _ = &mut abort_rx => Err(CliError::from(CodingSessionError::Cancelled)),
            outcome = &mut rejection => {
                outcome
                    .map_err(CliError::from)
                    .map(|operation_outcome| operation_outcome
                        .into_delegation_rejected()
                        .expect("delegation rejection operation returned a different public outcome"))
            }
        }?;
        Ok(())
    }
    .await;

    complete_owned_task(session, result, |session, ()| {
        PromptTaskResult::DelegationRejection(DelegationRejectionTaskResult { session })
    })
}

async fn run_coding_compact_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    default_agent_profile_id: ProfileId,
    connection_tx: oneshot::Sender<Result<Option<CodingAgentClientConnection>, CliError>>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
    let needs_connection = existing_session.is_none();
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            match open_interactive_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
                default_agent_profile_id,
            )
            .await
            {
                Ok(session) => session,
                Err(error) => return PromptTaskCompletion::SetupFailed(error),
            }
        }
    };
    let result = async {
        let operation_control = operation_control_for_adapter(&session);
        handoff_interactive_connection(connection_tx, &session, needs_connection)?;
        let compact_options = PromptTurnOptions::from_prompt_run_options(options);

        let outcome = {
        let mut compact = Box::pin(session.run(CodingAgentOperation::Compact(compact_options)));
        let mut abort_requested = false;
        loop {
            tokio::select! {
                _ = &mut abort_rx, if !abort_requested => {
                    abort_requested = true;
                    request_interactive_abort(&operation_control, OperationKind::Compact)?;
                }
                outcome = &mut compact => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| operation_outcome
                            .into_compact()
                            .expect("manual compaction operation returned a different public outcome"));
                }
            }
        }
        }?;

        Ok(outcome)
    }
    .await;

    complete_owned_task(session, result, |session, outcome| {
        PromptTaskResult::Coding(CodingPromptTaskResult {
            session,
            outcome,
            session_target: None,
            completion_notice: None,
            hydrate_transcript: false,
        })
    })
}

async fn run_coding_self_healing_edit_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    request: SelfHealingEditRequest,
    default_agent_profile_id: ProfileId,
    connection_tx: oneshot::Sender<Result<Option<CodingAgentClientConnection>, CliError>>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
    let needs_connection = existing_session.is_none();
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            match open_interactive_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
                default_agent_profile_id,
            )
            .await
            {
                Ok(session) => session,
                Err(error) => return PromptTaskCompletion::SetupFailed(error),
            }
        }
    };
    let result = async {
        let operation_control = operation_control_for_adapter(&session);
        handoff_interactive_connection(connection_tx, &session, needs_connection)?;

        let outcome = {
        let mut edit = Box::pin(session.run(CodingAgentOperation::SelfHealingEdit(request)));
        let mut abort_requested = false;
        loop {
            tokio::select! {
                _ = &mut abort_rx, if !abort_requested => {
                    abort_requested = true;
                    request_interactive_abort(
                        &operation_control,
                        OperationKind::SelfHealingEdit,
                    )?;
                }
                outcome = &mut edit => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| operation_outcome
                            .into_self_healing_edit()
                            .expect("self-healing edit operation returned a different public outcome"));
                }
            }
        }
        }?;

        Ok(outcome)
    }
    .await;

    complete_owned_task(session, result, |session, outcome| {
        PromptTaskResult::SelfHealingEdit(SelfHealingEditTaskResult { session, outcome })
    })
}

async fn run_coding_plugin_reload_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    default_agent_profile_id: ProfileId,
    connection_tx: oneshot::Sender<Result<Option<CodingAgentClientConnection>, CliError>>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
    let needs_connection = existing_session.is_none();
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            match open_interactive_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
                default_agent_profile_id,
            )
            .await
            {
                Ok(session) => session,
                Err(error) => return PromptTaskCompletion::SetupFailed(error),
            }
        }
    };
    let result = async {
        let operation_control = operation_control_for_adapter(&session);
        handoff_interactive_connection(connection_tx, &session, needs_connection)?;

        let outcome = {
        let mut reload = Box::pin(session.run(CodingAgentOperation::PluginLoad));
        let mut abort_requested = false;
        loop {
            tokio::select! {
                _ = &mut abort_rx, if !abort_requested => {
                    abort_requested = true;
                    request_interactive_abort(&operation_control, OperationKind::PluginLoad)?;
                }
                outcome = &mut reload => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| operation_outcome
                            .into_plugin_load()
                            .expect("plugin load operation returned a different public outcome"));
                }
            }
        }
        }?;

        Ok(outcome)
    }
    .await;

    complete_owned_task(session, result, |session, outcome| {
        PromptTaskResult::PluginReload(PluginReloadTaskResult { session, outcome })
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "branch-summary task startup keeps session ownership and cancellation inputs explicit"
)]
async fn run_coding_branch_summary_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    source_leaf_id: String,
    target_leaf_id: String,
    custom_instructions: Option<String>,
    default_agent_profile_id: ProfileId,
    connection_tx: oneshot::Sender<Result<Option<CodingAgentClientConnection>, CliError>>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
    let needs_connection = existing_session.is_none();
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            match open_interactive_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
                default_agent_profile_id,
            )
            .await
            {
                Ok(session) => session,
                Err(error) => return PromptTaskCompletion::SetupFailed(error),
            }
        }
    };
    let result = async {
        let operation_control = operation_control_for_adapter(&session);
        handoff_interactive_connection(connection_tx, &session, needs_connection)?;
        let branch_options = PromptTurnOptions::from_prompt_run_options(options);
        let mut abort_requested = false;

        let outcome = {
        let mut branch_summary = Box::pin(session.run(CodingAgentOperation::BranchSummary {
            options: branch_options,
            source_leaf_id,
            target_leaf_id,
            custom_instructions,
            reuse: BranchSummaryReusePolicy::AlwaysCreate,
        }));
        loop {
            tokio::select! {
                _ = &mut abort_rx, if !abort_requested => {
                    abort_requested = true;
                    request_interactive_abort(
                        &operation_control,
                        OperationKind::BranchSummary,
                    )?;
                }
                outcome = &mut branch_summary => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| operation_outcome
                            .into_branch_summary()
                            .expect("branch summary operation returned a different public outcome"));
                }
            }
        }
        }?;

        Ok(outcome)
    }
    .await;

    complete_owned_task(session, result, |session, outcome| {
        PromptTaskResult::Coding(CodingPromptTaskResult {
            session,
            outcome,
            session_target: None,
            completion_notice: None,
            hydrate_transcript: false,
        })
    })
}

async fn run_coding_branch_summary_navigation_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    source_leaf_id: String,
    target_leaf_id: String,
    default_agent_profile_id: ProfileId,
    connection_tx: oneshot::Sender<Result<Option<CodingAgentClientConnection>, CliError>>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
    let needs_connection = existing_session.is_none();
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            match open_interactive_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
                default_agent_profile_id,
            )
            .await
            {
                Ok(session) => session,
                Err(error) => return PromptTaskCompletion::SetupFailed(error),
            }
        }
    };
    let result = async {
        let operation_control = operation_control_for_adapter(&session);
        handoff_interactive_connection(connection_tx, &session, needs_connection)?;
        let branch_options = PromptTurnOptions::from_prompt_run_options(options);
        let mut abort_requested = false;

        let outcome = {
        let mut branch_summary = Box::pin(session.run(CodingAgentOperation::BranchSummary {
            options: branch_options,
            source_leaf_id,
            target_leaf_id: target_leaf_id.clone(),
            custom_instructions: None,
            reuse: BranchSummaryReusePolicy::ReuseExisting,
        }));
        loop {
            tokio::select! {
                _ = &mut abort_rx, if !abort_requested => {
                    abort_requested = true;
                    request_interactive_abort(
                        &operation_control,
                        OperationKind::BranchSummary,
                    )?;
                }
                outcome = &mut branch_summary => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| operation_outcome
                            .into_branch_summary()
                            .expect("branch summary navigation operation returned a different public outcome"));
                }
            }
        }
        }?;

        if !branch_summary_allows_navigation(&outcome) {
            return Ok((outcome, false));
        }

        let mut fork = Box::pin(session.run(CodingAgentOperation::ForkSession {
            target_leaf_id: Some(target_leaf_id),
        }));
        tokio::select! {
            _ = &mut abort_rx => Err(CliError::from(CodingSessionError::Cancelled)),
            outcome = &mut fork => {
                outcome
                    .map_err(CliError::from)
                    .map(|operation_outcome| operation_outcome
                        .into_session_forked()
                        .expect("navigation fork operation returned a different public outcome"))
            }
        }?;

        Ok((outcome, true))
    }
    .await;

    complete_owned_task(session, result, |session, (outcome, navigated)| {
        let session_target = navigated.then(|| active_session_target(&session));
        PromptTaskResult::Coding(CodingPromptTaskResult {
            session,
            outcome,
            session_target,
            completion_notice: navigated.then(|| "Navigated to selected point".to_string()),
            hydrate_transcript: navigated,
        })
    })
}

fn branch_summary_allows_navigation(outcome: &PromptTurnOutcome) -> bool {
    matches!(outcome, PromptTurnOutcome::Success { .. })
}

async fn run_coding_fork_session_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    target_leaf_id: Option<String>,
    completion_notice: Option<String>,
    default_agent_profile_id: ProfileId,
    connection_tx: oneshot::Sender<Result<Option<CodingAgentClientConnection>, CliError>>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
    let needs_connection = existing_session.is_none();
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            match open_interactive_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
                default_agent_profile_id,
            )
            .await
            {
                Ok(session) => session,
                Err(error) => return PromptTaskCompletion::SetupFailed(error),
            }
        }
    };
    let result = async {
        handoff_interactive_connection(connection_tx, &session, needs_connection)?;

        let mut fork = Box::pin(session.run(CodingAgentOperation::ForkSession { target_leaf_id }));
        tokio::select! {
            _ = &mut abort_rx => Err(CliError::from(CodingSessionError::Cancelled)),
            outcome = &mut fork => {
                outcome
                    .map_err(CliError::from)
                    .map(|operation_outcome| operation_outcome
                        .into_session_forked()
                        .expect("fork session operation returned a different public outcome"))
            }
        }?;

        Ok(())
    }
    .await;

    complete_owned_task(session, result, |session, ()| {
        let session_target = active_session_target(&session);
        PromptTaskResult::ForkSession(ForkSessionTaskResult {
            session,
            session_target,
            completion_notice,
            hydrate_transcript: true,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::bootstrap::{PromptInvocation, SessionRunOptions};
    use crate::authorization::ToolAuthorizationDecision;
    use crate::runtime::facade::{CodingAgentReconnect, CodingAgentReconnectDelivery};
    use pi_ai::api::conversation::StopReason;
    use pi_ai::api::model::{Model, ModelCost, ModelInput};
    use pi_ai::api::testing::{FauxProvider, FauxResponse, FauxToolCall};
    use std::sync::Arc;

    fn function_body<'a>(source: &'a str, function_name: &str) -> &'a str {
        let signature = format!("async fn {function_name}(");
        let start = source
            .find(&signature)
            .unwrap_or_else(|| panic!("missing interactive task runner `{function_name}`"));
        let body_start = source[start..]
            .find('{')
            .map(|offset| start + offset)
            .unwrap_or_else(|| {
                panic!("missing body for interactive task runner `{function_name}`")
            });
        let mut depth = 0usize;
        for (offset, byte) in source.as_bytes()[body_start..].iter().enumerate() {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &source[body_start..=body_start + offset];
                    }
                }
                _ => {}
            }
        }
        panic!("unterminated body for interactive task runner `{function_name}`");
    }

    fn authorization_test_model(api: &str) -> Model {
        Model {
            id: "authorization-test".into(),
            name: "Authorization Test".into(),
            api: api.into(),
            provider: "test".into(),
            base_url: String::new(),
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost::default(),
            context_window: 8_000,
            max_tokens: 1_024,
            headers: None,
            compat: None,
        }
    }

    #[tokio::test]
    async fn prompt_task_delivers_tool_authorization_decision_to_running_session() {
        let temp = tempfile::tempdir().unwrap();
        let api = "interactive-prompt-task-authorization";
        let provider = FauxProvider::with_call_queue(vec![
            FauxProvider::single_call(
                vec![FauxResponse {
                    text_deltas: Vec::new(),
                    thinking_deltas: Vec::new(),
                    tool_calls: vec![FauxToolCall {
                        id: "tool-write".into(),
                        name: "write".into(),
                        deltas: Vec::new(),
                        final_arguments: serde_json::json!({
                            "path": "authorized.txt",
                            "content": "authorized"
                        }),
                    }],
                }],
                StopReason::ToolUse,
            ),
            FauxProvider::text_call("done", StopReason::Stop),
        ]);
        let provider_guard = crate::test_support::ProviderGuard::register(api, Arc::new(provider));
        let cwd = temp.path().to_path_buf();
        let mut task = PromptTask::spawn_prompt(
            PromptRunOptions {
                prompt: "write".into(),
                model: authorization_test_model(api),
                api_key: None,
                auth_diagnostics: Vec::new(),
                system_prompt: None,
                max_turns: Some(5),
                tools: crate::tools::builtin_tools(cwd.clone()),
                register_builtins: false,
                ai_client: Some(provider_guard.ai_client()),
                session: Some(SessionRunOptions::disabled(cwd.clone())),
                session_target: None,
                session_name: None,
                thinking_level: None,
                tool_execution: None,
                resources: pi_agent_core::api::resources::AgentResources::default(),
                settings: None,
                invocation: PromptInvocation::Text("write".into()),
            },
            None,
            ProfileId::from("default"),
        )
        .unwrap();

        let connection = task
            .connection_handoff
            .take()
            .expect("new-session task must expose a connection handoff")
            .await
            .expect("connection handoff sender")
            .expect("connection setup")
            .expect("new-session handoff contains a connection");
        let (replayed, mut receiver) = match connection
            .reconnect_from_cursor(&connection.snapshot.cursor)
            .expect("canonical reconnect")
        {
            CodingAgentReconnect::Replayed {
                events, receiver, ..
            } => (
                events
                    .into_iter()
                    .collect::<std::collections::VecDeque<_>>(),
                receiver,
            ),
            CodingAgentReconnect::FreshSnapshotRequired(_) => {
                panic!("freshly handed-off connection must not have a retained gap")
            }
        };
        let authorization_id = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            let mut replayed = replayed;
            loop {
                let event = match replayed.pop_front() {
                    Some(event) => event,
                    None => match receiver.recv().await.expect("canonical product event") {
                        CodingAgentReconnectDelivery::Event(event) => event,
                        CodingAgentReconnectDelivery::FreshSnapshotRequired(_) => continue,
                    },
                };
                if let crate::events::CodingAgentProductEventKind::Tool(
                    crate::events::CodingAgentToolProductEvent::AuthorizationRequired { request },
                ) = event.event()
                {
                    break request.authorization_id.clone();
                }
            }
        })
        .await
        .expect("authorization request must be projected");
        assert!(!cwd.join("authorized.txt").exists());
        assert!(
            task.decide_tool_authorization(authorization_id, ToolAuthorizationDecision::AllowOnce)
                .await
        );

        let completion = tokio::time::timeout(std::time::Duration::from_secs(2), task.done)
            .await
            .expect("prompt task must complete after approval")
            .expect("prompt task completion channel");
        assert!(matches!(completion, PromptTaskCompletion::Completed(_)));
        assert_eq!(
            std::fs::read_to_string(cwd.join("authorized.txt")).unwrap(),
            "authorized"
        );
    }

    #[test]
    fn interactive_prompt_tasks_handoff_connection_without_local_subscription() {
        let source = include_str!("prompt_task.rs");
        let product_subscription = [".", "subscribe_product_events()"].concat();
        let compatibility_subscription = [".", "subscribe()"].concat();

        for function_name in [
            "run_coding_prompt_task",
            "run_coding_agent_invocation_task",
            "run_coding_agent_team_task",
            "run_coding_delegation_approval_task",
            "run_coding_set_default_agent_profile_task",
            "run_coding_session_tree_label_task",
            "run_coding_delegation_rejection_task",
            "run_coding_compact_task",
            "run_coding_self_healing_edit_task",
            "run_coding_plugin_reload_task",
            "run_coding_branch_summary_task",
            "run_coding_branch_summary_navigation_task",
            "run_coding_fork_session_task",
        ] {
            let body = function_body(source, function_name);
            assert!(!body.contains(&product_subscription));
            assert!(body.contains("handoff_interactive_connection("));
            assert!(
                body.contains("complete_owned_task("),
                "interactive task runner `{function_name}` must return its live owner on completion"
            );
        }
        assert!(!source.contains(&compatibility_subscription));
    }

    #[test]
    fn every_prompt_task_uses_one_connection_handoff() {
        let source = include_str!("prompt_task.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("production source precedes the test module");
        assert_eq!(
            production
                .matches("let (connection_tx, connection_rx) = oneshot::channel()")
                .count(),
            13
        );
        assert_eq!(
            production
                .matches("mpsc::channel(PROMPT_TASK_CONTROL_CAPACITY)")
                .count(),
            4
        );
        assert!(!production.contains("subscribe_product_events()"));
        assert!(!production.contains("let (control_tx, control_rx) = mpsc::unbounded_channel()"));
    }

    #[tokio::test]
    async fn saturated_prompt_control_preserves_abort() {
        let (control_tx, mut control_rx) = mpsc::channel(PROMPT_TASK_CONTROL_CAPACITY);
        for index in 0..PROMPT_TASK_CONTROL_CAPACITY {
            control_tx
                .try_send(PromptTaskControl::Steer(index.to_string()))
                .unwrap();
        }
        let (_connection_tx, connection_rx) = oneshot::channel();
        let (_done_tx, done_rx) = oneshot::channel();
        let task = PromptTask {
            control: PromptTaskControlHandle::Prompt(control_tx),
            connection_handoff: Some(connection_rx),
            done: done_rx,
            abort_requested: false,
        };

        let blocked = tokio::spawn(async move {
            let mut task = task;
            task.abort_once().await;
            task
        });
        tokio::task::yield_now().await;
        assert!(!blocked.is_finished());

        assert!(matches!(
            control_rx.recv().await,
            Some(PromptTaskControl::Steer(_))
        ));
        let task = blocked.await.unwrap();
        assert!(task.abort_requested);

        let mut aborts = 0usize;
        let mut total = 1usize;
        while let Ok(control) = control_rx.try_recv() {
            total += 1;
            aborts += usize::from(matches!(control, PromptTaskControl::Abort));
        }
        assert_eq!(total, PROMPT_TASK_CONTROL_CAPACITY + 1);
        assert_eq!(aborts, 1);
    }

    #[test]
    fn cancelled_or_failed_branch_summary_does_not_continue_to_fork() {
        let aborted = PromptTurnOutcome::Aborted {
            operation_id: "op_branch".into(),
            turn_id: None,
            reason: "cancelled".into(),
            session_id: Some("session".into()),
        };
        let failed = PromptTurnOutcome::Failed {
            operation_id: "op_branch".into(),
            turn_id: None,
            error: CodingSessionError::Cancelled,
            diagnostics: Vec::new(),
        };

        assert!(!branch_summary_allows_navigation(&aborted));
        assert!(!branch_summary_allows_navigation(&failed));
    }
}
