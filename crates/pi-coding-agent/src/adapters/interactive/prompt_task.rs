use crate::adapters::interactive::event_bridge::CodingEventBridge;
use crate::adapters::interactive::root::{
    PluginKeybinding, PluginSlashCommand, PluginUiAction, PluginUiDialog, PluginUiDialogField,
};
use crate::api::operation::{
    BranchSummaryReusePolicy, CodingAgentOperation, CodingAgentOperationOutcome,
    CodingAgentPluginLoadOutcome,
};
use crate::app::cli::error::CliError;
use crate::app::cli::prompt_options::PromptRunOptions;
use crate::app::session::{ResolvedSessionTarget, open_interactive_session};
use crate::runtime::facade::{
    AgentInvocationOptions, AgentTeamOptions, AgentTeamOutcome, CodingAgentOperationTask,
    CodingAgentSession, ProductEvent, ProfileId, PromptTurnOptions, PromptTurnOutcome,
    SelfHealingEditOutcome, SelfHealingEditRequest, UiSnapshot,
};
use tokio::sync::{mpsc, oneshot};

pub(super) enum PromptTaskEvent {
    Snapshot(UiSnapshot),
    Coding(ProductEvent),
}

pub(super) enum PromptTaskResult {
    Coding(CodingPromptTaskResult),
    AgentInvocation(AgentInvocationTaskResult),
    AgentTeam(AgentTeamTaskResult),
    DelegationApproval(DelegationApprovalTaskResult),
    SelfHealingEdit(SelfHealingEditTaskResult),
    PluginReload(PluginReloadTaskResult),
    PluginCommand(PluginCommandTaskResult),
    SetDefaultAgentProfile(SetDefaultAgentProfileTaskResult),
    DelegationRejection(DelegationRejectionTaskResult),
    ForkSession(ForkSessionTaskResult),
}

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

pub(super) struct DelegationRejectionTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) fallback_notice: Option<String>,
}

pub(super) struct SelfHealingEditTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) outcome: SelfHealingEditOutcome,
}

pub(super) struct PluginReloadTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) outcome: CodingAgentPluginLoadOutcome,
    pub(super) plugin_commands: Vec<PluginSlashCommand>,
    pub(super) plugin_ui_actions: Vec<PluginUiAction>,
    pub(super) plugin_keybindings: Vec<PluginKeybinding>,
    pub(super) plugin_ui_dialogs: Vec<PluginUiDialog>,
}

pub(super) struct PluginCommandTaskResult {
    pub(super) session: Option<CodingAgentSession>,
    pub(super) command_id: String,
    pub(super) output: String,
    pub(super) plugin_commands: Vec<PluginSlashCommand>,
    pub(super) plugin_ui_actions: Vec<PluginUiAction>,
    pub(super) plugin_keybindings: Vec<PluginKeybinding>,
    pub(super) plugin_ui_dialogs: Vec<PluginUiDialog>,
}

enum PromptTaskControlHandle {
    Prompt(mpsc::UnboundedSender<PromptTaskControl>),
    AbortOnly(Option<oneshot::Sender<()>>),
}

#[derive(Debug)]
enum PromptTaskControl {
    Abort,
    Steer(String),
    FollowUp(String),
}

pub(super) struct PromptTask {
    control: PromptTaskControlHandle,
    pub(super) events: mpsc::UnboundedReceiver<PromptTaskEvent>,
    pub(super) done: oneshot::Receiver<PromptTaskCompletion>,
    abort_requested: bool,
    pub(super) events_closed: bool,
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

    pub(super) fn spawn_plugin_command(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        command_id: String,
        args: serde_json::Value,
        default_agent_profile_id: ProfileId,
    ) -> Result<Self, CliError> {
        Ok(Self::spawn_coding_plugin_command(
            options,
            existing_session,
            command_id,
            args,
            default_agent_profile_id,
        ))
    }

    pub(super) fn spawn_submitted_plugin_command(
        session: &CodingAgentSession,
        task: CodingAgentOperationTask,
        command_id: String,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        send_ui_snapshot(&event_tx, session);
        let plugin_commands = plugin_slash_commands(session);
        let plugin_ui_actions = plugin_ui_actions(session);
        let plugin_keybindings = plugin_keybindings(session);
        let plugin_ui_dialogs = plugin_ui_dialogs(session);

        tokio::spawn(async move {
            let completion = match task.join().await.map_err(CliError::from) {
                Ok(CodingAgentOperationOutcome::PluginCommand(output)) => {
                    PromptTaskCompletion::Completed(PromptTaskResult::PluginCommand(
                        PluginCommandTaskResult {
                            session: None,
                            command_id,
                            output,
                            plugin_commands,
                            plugin_ui_actions,
                            plugin_keybindings,
                            plugin_ui_dialogs,
                        },
                    ))
                }
                Ok(_) => {
                    unreachable!("plugin command operation returned a different public outcome")
                }
                Err(error) => PromptTaskCompletion::SetupFailed(error),
            };
            let _ = done_tx.send(completion);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(None),
            events: event_rx,
            done: done_rx,
            abort_requested: false,
            events_closed: false,
        }
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

    pub(super) fn abort_once(&mut self) {
        if self.abort_requested {
            return;
        }
        match &mut self.control {
            PromptTaskControlHandle::Prompt(control) => {
                let _ = control.send(PromptTaskControl::Abort);
            }
            PromptTaskControlHandle::AbortOnly(abort) => {
                if let Some(abort) = abort.take() {
                    let _ = abort.send(());
                }
            }
        }
        self.abort_requested = true;
    }

    pub(super) fn steer(&self, text: String) -> bool {
        match &self.control {
            PromptTaskControlHandle::Prompt(control) => {
                control.send(PromptTaskControl::Steer(text)).is_ok()
            }
            PromptTaskControlHandle::AbortOnly(_) => false,
        }
    }

    pub(super) fn follow_up(&self, text: String) -> bool {
        match &self.control {
            PromptTaskControlHandle::Prompt(control) => {
                control.send(PromptTaskControl::FollowUp(text)).is_ok()
            }
            PromptTaskControlHandle::AbortOnly(_) => false,
        }
    }

    fn spawn_coding(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (control_tx, control_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            let result = run_coding_prompt_task(
                options,
                existing_session,
                default_agent_profile_id,
                event_tx,
                control_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::Prompt(control_tx),
            events: event_rx,
            done: done_rx,
            abort_requested: false,
            events_closed: false,
        }
    }

    fn spawn_coding_compact(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_compact_task(
                options,
                existing_session,
                default_agent_profile_id,
                event_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            events: event_rx,
            done: done_rx,
            abort_requested: false,
            events_closed: false,
        }
    }

    fn spawn_coding_agent_invocation(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        profile_id: ProfileId,
        task: String,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (control_tx, control_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            let result = run_coding_agent_invocation_task(
                options,
                existing_session,
                profile_id,
                task,
                default_agent_profile_id,
                event_tx,
                control_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::Prompt(control_tx),
            events: event_rx,
            done: done_rx,
            abort_requested: false,
            events_closed: false,
        }
    }

    fn spawn_coding_agent_team(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        team_id: ProfileId,
        task: String,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_agent_team_task(
                options,
                existing_session,
                team_id,
                task,
                default_agent_profile_id,
                event_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            events: event_rx,
            done: done_rx,
            abort_requested: false,
            events_closed: false,
        }
    }

    fn spawn_coding_delegation_approval(
        existing_session: CodingAgentSession,
        operation_id: String,
        tool_call_id: String,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_delegation_approval_task(
                existing_session,
                operation_id,
                tool_call_id,
                event_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            events: event_rx,
            done: done_rx,
            abort_requested: false,
            events_closed: false,
        }
    }

    fn spawn_coding_set_default_agent_profile(
        existing_session: CodingAgentSession,
        profile_id: ProfileId,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_set_default_agent_profile_task(
                existing_session,
                profile_id,
                event_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            events: event_rx,
            done: done_rx,
            abort_requested: false,
            events_closed: false,
        }
    }

    fn spawn_coding_delegation_rejection(
        existing_session: CodingAgentSession,
        operation_id: String,
        tool_call_id: String,
        reason: String,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_delegation_rejection_task(
                existing_session,
                operation_id,
                tool_call_id,
                reason,
                event_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            events: event_rx,
            done: done_rx,
            abort_requested: false,
            events_closed: false,
        }
    }

    fn spawn_coding_plugin_reload(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_plugin_reload_task(
                options,
                existing_session,
                default_agent_profile_id,
                event_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            events: event_rx,
            done: done_rx,
            abort_requested: false,
            events_closed: false,
        }
    }

    fn spawn_coding_self_healing_edit(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        request: SelfHealingEditRequest,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_self_healing_edit_task(
                options,
                existing_session,
                request,
                default_agent_profile_id,
                event_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            events: event_rx,
            done: done_rx,
            abort_requested: false,
            events_closed: false,
        }
    }

    fn spawn_coding_plugin_command(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        command_id: String,
        args: serde_json::Value,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_plugin_command_task(
                options,
                existing_session,
                command_id,
                args,
                default_agent_profile_id,
                event_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            events: event_rx,
            done: done_rx,
            abort_requested: false,
            events_closed: false,
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
        let (event_tx, event_rx) = mpsc::unbounded_channel();
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
                event_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            events: event_rx,
            done: done_rx,
            abort_requested: false,
            events_closed: false,
        }
    }

    fn spawn_coding_branch_summary_navigation(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        source_leaf_id: String,
        target_leaf_id: String,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_branch_summary_navigation_task(
                options,
                existing_session,
                source_leaf_id,
                target_leaf_id,
                default_agent_profile_id,
                event_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            events: event_rx,
            done: done_rx,
            abort_requested: false,
            events_closed: false,
        }
    }

    fn spawn_coding_fork_session(
        options: PromptRunOptions,
        existing_session: Option<CodingAgentSession>,
        target_leaf_id: Option<String>,
        completion_notice: Option<String>,
        default_agent_profile_id: ProfileId,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = run_coding_fork_session_task(
                options,
                existing_session,
                target_leaf_id,
                completion_notice,
                default_agent_profile_id,
                event_tx,
                abort_rx,
            )
            .await;
            let _ = done_tx.send(result);
        });

        Self {
            control: PromptTaskControlHandle::AbortOnly(Some(abort_tx)),
            events: event_rx,
            done: done_rx,
            abort_requested: false,
            events_closed: false,
        }
    }
}

fn send_ui_snapshot(
    event_tx: &mpsc::UnboundedSender<PromptTaskEvent>,
    session: &CodingAgentSession,
) {
    let _ = event_tx.send(PromptTaskEvent::Snapshot(session.ui_snapshot(Vec::new())));
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

fn product_event_has_visible_ui(bridge: &mut CodingEventBridge, event: &ProductEvent) -> bool {
    !bridge.push_product_event(event).is_empty()
}

fn active_session_target(session: &CodingAgentSession) -> ResolvedSessionTarget {
    ResolvedSessionTarget::OpenOrCreateId(session.view().session_id.clone())
}

async fn run_coding_prompt_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    default_agent_profile_id: ProfileId,
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut control_rx: mpsc::UnboundedReceiver<PromptTaskControl>,
) -> PromptTaskCompletion {
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
        let mut receiver = session.subscribe_product_events();
        send_ui_snapshot(&event_tx, &session);
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
                            Some(PromptTaskControl::Abort) => {}
                            None => {
                                controls_open = false;
                            }
                        }
                    }
                    event = receiver.recv() => {
                        if let Ok(event) = event {
                            let _ = event_tx.send(PromptTaskEvent::Coding(event));
                        }
                    }
                    outcome = &mut prompt => {
                        break outcome
                            .map_err(CliError::from)
                            .map(|operation_outcome| match operation_outcome {
                                CodingAgentOperationOutcome::Prompt(outcome) => outcome,
                                _ => unreachable!("prompt operation returned a different public outcome"),
                            });
                    }
                }
            }
        }?;

        while let Ok(Some(event)) = receiver.try_recv() {
            let _ = event_tx.send(PromptTaskEvent::Coding(event));
        }
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
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut control_rx: mpsc::UnboundedReceiver<PromptTaskControl>,
) -> PromptTaskCompletion {
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
        let mut receiver = session.subscribe_product_events();
        send_ui_snapshot(&event_tx, &session);
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
                        Some(PromptTaskControl::Abort) => {}
                        None => {
                            controls_open = false;
                        }
                    }
                }
                event = receiver.recv() => {
                    if let Ok(event) = event {
                        let _ = event_tx.send(PromptTaskEvent::Coding(event));
                    }
                }
                outcome = &mut invocation => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| match operation_outcome {
                            CodingAgentOperationOutcome::AgentInvocation(_) => (),
                            _ => unreachable!("agent invocation operation returned a different public outcome"),
                        });
                }
            }
        }?;

        while let Ok(Some(event)) = receiver.try_recv() {
            let _ = event_tx.send(PromptTaskEvent::Coding(event));
        }
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
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
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
        let mut receiver = session.subscribe_product_events();
        send_ui_snapshot(&event_tx, &session);
        let team_options = AgentTeamOptions::new(
            team_id,
            task,
            PromptTurnOptions::from_prompt_run_options(options),
        );

        let outcome = {
            let mut invocation =
                Box::pin(session.run(CodingAgentOperation::InvokeTeam(team_options)));
            loop {
            tokio::select! {
                _ = &mut abort_rx => {
                    break Err(CliError::UnsupportedMode(
                        "interactive agent team abort is not implemented yet".into(),
                    ));
                }
                event = receiver.recv() => {
                    if let Ok(event) = event {
                        let _ = event_tx.send(PromptTaskEvent::Coding(event));
                    }
                }
                outcome = &mut invocation => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| match operation_outcome {
                            CodingAgentOperationOutcome::AgentTeam(outcome) => outcome,
                            _ => unreachable!("agent team operation returned a different public outcome"),
                        });
                }
            }
            }
        }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

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
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
    let result = async {
        let mut receiver = session.subscribe_product_events();
        send_ui_snapshot(&event_tx, &session);

        let mut approval = Box::pin(session.run(CodingAgentOperation::ApproveDelegation {
            operation_id,
            tool_call_id,
        }));
        loop {
            tokio::select! {
                _ = &mut abort_rx => {
                    break Err(CliError::UnsupportedMode(
                        "interactive delegation approval abort is not implemented yet".into(),
                    ));
                }
                event = receiver.recv() => {
                    if let Ok(event) = event {
                        let _ = event_tx.send(PromptTaskEvent::Coding(event));
                    }
                }
                outcome = &mut approval => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| match operation_outcome {
                            CodingAgentOperationOutcome::DelegationApproved => (),
                            _ => unreachable!("delegation approval operation returned a different public outcome"),
                        });
                }
            }
        }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

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
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
    let result = async {
        let mut receiver = session.subscribe_product_events();
        send_ui_snapshot(&event_tx, &session);

        let mut mutation =
            Box::pin(session.run(CodingAgentOperation::SetDefaultAgentProfile { profile_id }));
        loop {
            tokio::select! {
                _ = &mut abort_rx => {
                    break Err(CliError::UnsupportedMode(
                        "interactive default profile mutation abort is not implemented yet".into(),
                    ));
                }
                event = receiver.recv() => {
                    if let Ok(event) = event {
                        let _ = event_tx.send(PromptTaskEvent::Coding(event));
                    }
                }
                outcome = &mut mutation => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| match operation_outcome {
                            CodingAgentOperationOutcome::DefaultAgentProfileChanged => (),
                            _ => unreachable!(
                                "set default agent profile operation returned a different public outcome"
                            ),
                        });
                }
            }
        }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

        Ok(())
    }
    .await;

    complete_owned_task(session, result, |session, ()| {
        PromptTaskResult::SetDefaultAgentProfile(SetDefaultAgentProfileTaskResult { session })
    })
}

async fn run_coding_delegation_rejection_task(
    mut session: CodingAgentSession,
    operation_id: String,
    tool_call_id: String,
    reason: String,
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
    let fallback_text = format!("Delegation rejected: {operation_id} {tool_call_id}");
    let result = async {
        let mut receiver = session.subscribe_product_events();
        send_ui_snapshot(&event_tx, &session);

        let mut projection = CodingEventBridge::new();
        let mut had_visible_events = false;
        let mut rejection = Box::pin(session.run(CodingAgentOperation::RejectDelegation {
            operation_id,
            tool_call_id,
            reason,
        }));
        loop {
            tokio::select! {
                _ = &mut abort_rx => {
                    break Err(CliError::UnsupportedMode(
                        "interactive delegation rejection abort is not implemented yet".into(),
                    ));
                }
                event = receiver.recv() => {
                    if let Ok(event) = event {
                        had_visible_events |= product_event_has_visible_ui(&mut projection, &event);
                        let _ = event_tx.send(PromptTaskEvent::Coding(event));
                    }
                }
                outcome = &mut rejection => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| match operation_outcome {
                            CodingAgentOperationOutcome::DelegationRejected => (),
                            _ => unreachable!(
                                "delegation rejection operation returned a different public outcome"
                            ),
                        });
                }
            }
        }?;

        while let Ok(Some(event)) = receiver.try_recv() {
            had_visible_events |= product_event_has_visible_ui(&mut projection, &event);
            let _ = event_tx.send(PromptTaskEvent::Coding(event));
        }

        Ok(if had_visible_events {
            None
        } else {
            Some(fallback_text)
        })
    }
    .await;

    complete_owned_task(session, result, |session, fallback_notice| {
        PromptTaskResult::DelegationRejection(DelegationRejectionTaskResult {
            session,
            fallback_notice,
        })
    })
}

async fn run_coding_compact_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    default_agent_profile_id: ProfileId,
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
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
        let mut receiver = session.subscribe_product_events();
        send_ui_snapshot(&event_tx, &session);
        let compact_options = PromptTurnOptions::from_prompt_run_options(options);

        let outcome = {
        let mut compact = Box::pin(session.run(CodingAgentOperation::Compact(compact_options)));
        loop {
            tokio::select! {
                _ = &mut abort_rx => {
                    break Err(CliError::UnsupportedMode(
                        "interactive manual compaction abort is not implemented yet".into(),
                    ));
                }
                event = receiver.recv() => {
                    if let Ok(event) = event {
                        let _ = event_tx.send(PromptTaskEvent::Coding(event));
                    }
                }
                outcome = &mut compact => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| match operation_outcome {
                            CodingAgentOperationOutcome::Compact(outcome) => outcome,
                            _ => unreachable!("manual compaction operation returned a different public outcome"),
                        });
                }
            }
        }
        }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

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
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
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
        let mut receiver = session.subscribe_product_events();
        send_ui_snapshot(&event_tx, &session);

        let outcome = {
        let mut edit = Box::pin(session.run(CodingAgentOperation::SelfHealingEdit(request)));
        loop {
            tokio::select! {
                _ = &mut abort_rx => {
                    break Err(CliError::UnsupportedMode(
                        "interactive self-healing edit abort is not implemented yet".into(),
                    ));
                }
                event = receiver.recv() => {
                    if let Ok(event) = event {
                        let _ = event_tx.send(PromptTaskEvent::Coding(event));
                    }
                }
                outcome = &mut edit => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| match operation_outcome {
                            CodingAgentOperationOutcome::SelfHealingEdit(outcome) => outcome,
                            _ => unreachable!("self-healing edit operation returned a different public outcome"),
                        });
                }
            }
        }
        }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

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
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
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
        let mut receiver = session.subscribe_product_events();
        send_ui_snapshot(&event_tx, &session);

        let outcome = {
        let mut reload = Box::pin(session.run(CodingAgentOperation::PluginLoad));
        loop {
            tokio::select! {
                _ = &mut abort_rx => {
                    break Err(CliError::UnsupportedMode(
                        "interactive plugin reload abort is not implemented yet".into(),
                    ));
                }
                event = receiver.recv() => {
                    if let Ok(event) = event {
                        let _ = event_tx.send(PromptTaskEvent::Coding(event));
                    }
                }
                outcome = &mut reload => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| match operation_outcome {
                            CodingAgentOperationOutcome::PluginLoad(outcome) => outcome,
                            _ => unreachable!("plugin load operation returned a different public outcome"),
                        });
                }
            }
        }
        }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

        Ok(outcome)
    }
    .await;

    complete_owned_task(session, result, |session, outcome| {
        let plugin_commands = plugin_slash_commands(&session);
        let plugin_ui_actions = plugin_ui_actions(&session);
        let plugin_keybindings = plugin_keybindings(&session);
        let plugin_ui_dialogs = plugin_ui_dialogs(&session);
        PromptTaskResult::PluginReload(PluginReloadTaskResult {
            session,
            outcome,
            plugin_commands,
            plugin_ui_actions,
            plugin_keybindings,
            plugin_ui_dialogs,
        })
    })
}

async fn run_coding_plugin_command_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    command_id: String,
    args: serde_json::Value,
    default_agent_profile_id: ProfileId,
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
    let should_load_plugins = existing_session.is_none();
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
        let mut receiver = session.subscribe_product_events();
        send_ui_snapshot(&event_tx, &session);

        if should_load_plugins {
            let mut reload = Box::pin(session.run(CodingAgentOperation::PluginLoad));
            loop {
                tokio::select! {
                    _ = &mut abort_rx => {
                        return Err(CliError::UnsupportedMode(
                            "interactive plugin command abort is not implemented yet".into(),
                        ));
                    }
                    event = receiver.recv() => {
                        if let Ok(event) = event {
                            let _ = event_tx.send(PromptTaskEvent::Coding(event));
                        }
                    }
                    outcome = &mut reload => {
                        outcome
                            .map_err(CliError::from)
                            .map(|operation_outcome| match operation_outcome {
                                CodingAgentOperationOutcome::PluginLoad(_) => (),
                                _ => unreachable!("plugin load operation returned a different public outcome"),
                            })?;
                        break;
                    }
                }
            }
        } else if abort_rx.try_recv().is_ok() {
            return Err(CliError::UnsupportedMode(
                "interactive plugin command abort is not implemented yet".into(),
            ));
        }

        let output = session
            .submit(CodingAgentOperation::PluginCommand {
                command_id: command_id.clone(),
                args,
            })
            .map_err(CliError::from)
            ?
            .join()
            .await
            .map_err(CliError::from)
            .map(|operation_outcome| match operation_outcome {
                CodingAgentOperationOutcome::PluginCommand(output) => output,
                _ => unreachable!("plugin command operation returned a different public outcome"),
            })?;

        while let Ok(Some(event)) = receiver.try_recv() {
            let _ = event_tx.send(PromptTaskEvent::Coding(event));
        }
        Ok(output)
    }
    .await;

    complete_owned_task(session, result, |session, output| {
        let plugin_commands = plugin_slash_commands(&session);
        let plugin_ui_actions = plugin_ui_actions(&session);
        let plugin_keybindings = plugin_keybindings(&session);
        let plugin_ui_dialogs = plugin_ui_dialogs(&session);
        PromptTaskResult::PluginCommand(PluginCommandTaskResult {
            session: Some(session),
            command_id,
            output,
            plugin_commands,
            plugin_ui_actions,
            plugin_keybindings,
            plugin_ui_dialogs,
        })
    })
}

fn plugin_slash_commands(session: &CodingAgentSession) -> Vec<PluginSlashCommand> {
    session
        .plugin_commands()
        .into_iter()
        .map(|command| PluginSlashCommand::new(command.id, command.description))
        .collect()
}

fn plugin_ui_actions(session: &CodingAgentSession) -> Vec<PluginUiAction> {
    session
        .plugin_ui_actions()
        .into_iter()
        .map(|action| {
            PluginUiAction::new(
                action.id,
                action.label,
                action.description,
                action.action_id,
            )
        })
        .collect()
}

fn plugin_keybindings(session: &CodingAgentSession) -> Vec<PluginKeybinding> {
    session
        .plugin_keybindings()
        .into_iter()
        .map(|keybinding| {
            PluginKeybinding::new(
                keybinding.id,
                keybinding.key,
                keybinding.description,
                keybinding.action_id,
            )
        })
        .collect()
}

fn plugin_ui_dialogs(session: &CodingAgentSession) -> Vec<PluginUiDialog> {
    session
        .plugin_ui_dialogs()
        .into_iter()
        .map(|dialog| {
            let fields = dialog
                .fields
                .into_iter()
                .map(|field| {
                    PluginUiDialogField::new(
                        field.id,
                        field.label,
                        field.description,
                        field.kind,
                        field.default_value,
                        field.required,
                    )
                    .with_options(field.options)
                })
                .collect();
            PluginUiDialog::new(
                dialog.id,
                dialog.title,
                dialog.description,
                dialog.action_id,
            )
            .with_fields(fields)
        })
        .collect()
}

async fn run_coding_branch_summary_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    source_leaf_id: String,
    target_leaf_id: String,
    custom_instructions: Option<String>,
    default_agent_profile_id: ProfileId,
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
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
        let mut receiver = session.subscribe_product_events();
        send_ui_snapshot(&event_tx, &session);
        let branch_options = PromptTurnOptions::from_prompt_run_options(options);

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
                _ = &mut abort_rx => {
                    break Err(CliError::UnsupportedMode(
                        "interactive branch summary abort is not implemented yet".into(),
                    ));
                }
                event = receiver.recv() => {
                    if let Ok(event) = event {
                        let _ = event_tx.send(PromptTaskEvent::Coding(event));
                    }
                }
                outcome = &mut branch_summary => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| match operation_outcome {
                            CodingAgentOperationOutcome::BranchSummary(outcome) => outcome,
                            _ => unreachable!("branch summary operation returned a different public outcome"),
                        });
                }
            }
        }
        }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

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
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
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
        let mut receiver = session.subscribe_product_events();
        send_ui_snapshot(&event_tx, &session);
        let branch_options = PromptTurnOptions::from_prompt_run_options(options);

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
                _ = &mut abort_rx => {
                    break Err(CliError::UnsupportedMode(
                        "interactive branch summary navigation abort is not implemented yet".into(),
                    ));
                }
                event = receiver.recv() => {
                    if let Ok(event) = event {
                        let _ = event_tx.send(PromptTaskEvent::Coding(event));
                    }
                }
                outcome = &mut branch_summary => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| match operation_outcome {
                            CodingAgentOperationOutcome::BranchSummary(outcome) => outcome,
                            _ => unreachable!("branch summary navigation operation returned a different public outcome"),
                        });
                }
            }
        }
        }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

        {
        let mut fork = Box::pin(session.run(CodingAgentOperation::ForkSession {
            target_leaf_id: Some(target_leaf_id),
        }));
        loop {
            tokio::select! {
                _ = &mut abort_rx => {
                    break Err(CliError::UnsupportedMode(
                        "interactive navigation fork abort is not implemented yet".into(),
                    ));
                }
                event = receiver.recv() => {
                    if let Ok(event) = event {
                        let _ = event_tx.send(PromptTaskEvent::Coding(event));
                    }
                }
                outcome = &mut fork => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| match operation_outcome {
                            CodingAgentOperationOutcome::SessionForked => (),
                            _ => unreachable!("navigation fork operation returned a different public outcome"),
                        });
                }
            }
        }
        }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

        Ok(outcome)
    }
    .await;

    complete_owned_task(session, result, |session, outcome| {
        let session_target = active_session_target(&session);
        PromptTaskResult::Coding(CodingPromptTaskResult {
            session,
            outcome,
            session_target: Some(session_target),
            completion_notice: Some("Navigated to selected point".to_string()),
            hydrate_transcript: true,
        })
    })
}

async fn run_coding_fork_session_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    target_leaf_id: Option<String>,
    completion_notice: Option<String>,
    default_agent_profile_id: ProfileId,
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut abort_rx: oneshot::Receiver<()>,
) -> PromptTaskCompletion {
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
        let mut receiver = session.subscribe_product_events();
        send_ui_snapshot(&event_tx, &session);

        let mut fork = Box::pin(session.run(CodingAgentOperation::ForkSession { target_leaf_id }));
        loop {
            tokio::select! {
                _ = &mut abort_rx => {
                    break Err(CliError::UnsupportedMode(
                        "interactive fork abort is not implemented yet".into(),
                    ));
                }
                event = receiver.recv() => {
                    if let Ok(event) = event {
                        let _ = event_tx.send(PromptTaskEvent::Coding(event));
                    }
                }
                outcome = &mut fork => {
                    break outcome
                        .map_err(CliError::from)
                        .map(|operation_outcome| match operation_outcome {
                            CodingAgentOperationOutcome::SessionForked => (),
                            _ => unreachable!(
                                "fork session operation returned a different public outcome"
                            ),
                        });
                }
            }
        }?;

        while let Ok(Some(event)) = receiver.try_recv() {
            let _ = event_tx.send(PromptTaskEvent::Coding(event));
        }

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
    use crate::events::message::MessageEvent;
    use crate::events::prompt_stream::PromptStreamEvent;
    use crate::runtime::facade::ProductEventSequence;

    fn product_event(sequence: u64, event: PromptStreamEvent) -> ProductEvent {
        ProductEvent::from_draft_for_tests(
            ProductEventSequence::new(sequence),
            event.into_product_draft(),
            None,
        )
    }

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

    #[test]
    fn delegation_fallback_visibility_follows_ui_event_projection() {
        let invisible = product_event(
            1,
            PromptStreamEvent::Message(MessageEvent::Started {
                operation_id: "op_reject".into(),
                turn_id: "turn_reject".into(),
                message_id: Some("msg_reject".into()),
            }),
        );
        let visible = product_event(
            2,
            PromptStreamEvent::Message(MessageEvent::Delta {
                operation_id: "op_reject".into(),
                turn_id: "turn_reject".into(),
                message_id: Some("msg_reject".into()),
                text: "visible rejection event".into(),
            }),
        );

        let mut bridge = CodingEventBridge::new();
        assert!(!product_event_has_visible_ui(&mut bridge, &invisible));
        assert!(product_event_has_visible_ui(&mut bridge, &visible));
    }

    #[test]
    fn interactive_prompt_tasks_use_product_event_stream_boundary() {
        let source = include_str!("prompt_task.rs");
        let product_subscription = [".", "subscribe_product_events()"].concat();
        let compatibility_subscription = [".", "subscribe()"].concat();

        for function_name in [
            "run_coding_prompt_task",
            "run_coding_agent_invocation_task",
            "run_coding_agent_team_task",
            "run_coding_delegation_approval_task",
            "run_coding_set_default_agent_profile_task",
            "run_coding_delegation_rejection_task",
            "run_coding_compact_task",
            "run_coding_self_healing_edit_task",
            "run_coding_plugin_reload_task",
            "run_coding_plugin_command_task",
            "run_coding_branch_summary_task",
            "run_coding_branch_summary_navigation_task",
            "run_coding_fork_session_task",
        ] {
            let body = function_body(source, function_name);
            assert!(
                body.contains(&product_subscription),
                "interactive task runner `{function_name}` must subscribe through the product event boundary"
            );
            assert!(
                body.contains("complete_owned_task("),
                "interactive task runner `{function_name}` must return its live owner on completion"
            );
        }
        assert!(!source.contains(&compatibility_subscription));
        assert!(source.contains("Coding(ProductEvent)"));
    }
}
