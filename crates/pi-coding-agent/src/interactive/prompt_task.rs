use std::path::PathBuf;

use crate::CliError;
use crate::coding_session::{
    AgentInvocationOptions, AgentInvocationOutcome, AgentTeamOptions, AgentTeamOutcome,
    CodingAgentEvent, CodingAgentSession, CodingAgentSessionOptions, CodingSessionError,
    PluginLoadOutcome, ProfileId, PromptTurnOptions, PromptTurnOutcome,
};
use crate::interactive::root::{
    PluginKeybinding, PluginSlashCommand, PluginUiAction, PluginUiDialog, PluginUiDialogField,
};
use crate::prompt_options::PromptRunOptions;
use crate::runtime::SessionMode;
use crate::session::{ResolvedSessionTarget, resolve_session_dir};
use tokio::sync::{mpsc, oneshot};

pub(super) enum PromptTaskEvent {
    Coding(CodingAgentEvent),
}

pub(super) enum PromptTaskResult {
    Coding(CodingPromptTaskResult),
    AgentInvocation(AgentInvocationTaskResult),
    AgentTeam(AgentTeamTaskResult),
    DelegationApproval(DelegationApprovalTaskResult),
    PluginReload(PluginReloadTaskResult),
    PluginCommand(PluginCommandTaskResult),
}

pub(super) struct CodingPromptTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) outcome: PromptTurnOutcome,
    pub(super) update_usage: bool,
    pub(super) completion_notice: Option<String>,
    pub(super) hydrate_transcript: bool,
}

pub(super) struct AgentInvocationTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) outcome: AgentInvocationOutcome,
}

pub(super) struct AgentTeamTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) outcome: AgentTeamOutcome,
}

pub(super) struct DelegationApprovalTaskResult {
    pub(super) session: CodingAgentSession,
}

pub(super) struct PluginReloadTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) outcome: PluginLoadOutcome,
    pub(super) plugin_commands: Vec<PluginSlashCommand>,
    pub(super) plugin_ui_actions: Vec<PluginUiAction>,
    pub(super) plugin_keybindings: Vec<PluginKeybinding>,
    pub(super) plugin_ui_dialogs: Vec<PluginUiDialog>,
}

pub(super) struct PluginCommandTaskResult {
    pub(super) session: CodingAgentSession,
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
    pub(super) done: oneshot::Receiver<Result<PromptTaskResult, CliError>>,
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
            let _ = done_tx.send(result.map(PromptTaskResult::Coding));
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
            let _ = done_tx.send(result.map(PromptTaskResult::Coding));
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
            let _ = done_tx.send(result.map(PromptTaskResult::AgentInvocation));
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
            let _ = done_tx.send(result.map(PromptTaskResult::AgentTeam));
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
            let _ = done_tx.send(result.map(PromptTaskResult::DelegationApproval));
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
            let _ = done_tx.send(result.map(PromptTaskResult::PluginReload));
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
            let _ = done_tx.send(result.map(PromptTaskResult::PluginCommand));
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
            let _ = done_tx.send(result.map(PromptTaskResult::Coding));
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
            let _ = done_tx.send(result.map(PromptTaskResult::Coding));
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

async fn run_coding_prompt_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    default_agent_profile_id: ProfileId,
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut control_rx: mpsc::UnboundedReceiver<PromptTaskControl>,
) -> Result<CodingPromptTaskResult, CliError> {
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            open_interactive_coding_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
                default_agent_profile_id,
            )
            .await?
        }
    };
    let prompt_control = session.prompt_control_handle()?;
    let mut receiver = session.subscribe();
    let prompt_options = PromptTurnOptions::from_prompt_run_options(options);

    let outcome = {
        let mut prompt = Box::pin(session.prompt(prompt_options));
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
                    break outcome.map_err(CliError::from);
                }
            }
        }
    }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

    Ok(CodingPromptTaskResult {
        session,
        outcome,
        update_usage: true,
        completion_notice: None,
        hydrate_transcript: false,
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
) -> Result<AgentInvocationTaskResult, CliError> {
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            open_interactive_coding_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
                default_agent_profile_id,
            )
            .await?
        }
    };
    let prompt_control = session.prompt_control_handle()?;
    let mut receiver = session.subscribe();
    let invocation_options = AgentInvocationOptions::new(
        profile_id,
        task,
        PromptTurnOptions::from_prompt_run_options(options),
    );

    let outcome = {
        let mut invocation = Box::pin(session.invoke_agent(invocation_options));
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
                    break outcome.map_err(CliError::from);
                }
            }
        }
    }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

    Ok(AgentInvocationTaskResult { session, outcome })
}

async fn run_coding_agent_team_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    team_id: ProfileId,
    task: String,
    default_agent_profile_id: ProfileId,
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut abort_rx: oneshot::Receiver<()>,
) -> Result<AgentTeamTaskResult, CliError> {
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            open_interactive_coding_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
                default_agent_profile_id,
            )
            .await?
        }
    };
    let mut receiver = session.subscribe();
    let team_options = AgentTeamOptions::new(
        team_id,
        task,
        PromptTurnOptions::from_prompt_run_options(options),
    );

    let outcome = {
        let mut invocation = Box::pin(session.invoke_team(team_options));
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
                    break outcome.map_err(CliError::from);
                }
            }
        }
    }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

    Ok(AgentTeamTaskResult { session, outcome })
}

async fn run_coding_delegation_approval_task(
    mut session: CodingAgentSession,
    operation_id: String,
    tool_call_id: String,
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut abort_rx: oneshot::Receiver<()>,
) -> Result<DelegationApprovalTaskResult, CliError> {
    let mut receiver = session.subscribe();

    {
        let mut approval =
            Box::pin(session.approve_delegation_confirmation(&operation_id, &tool_call_id));
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
                    break outcome.map_err(CliError::from);
                }
            }
        }
    }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

    Ok(DelegationApprovalTaskResult { session })
}

async fn run_coding_compact_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    default_agent_profile_id: ProfileId,
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut abort_rx: oneshot::Receiver<()>,
) -> Result<CodingPromptTaskResult, CliError> {
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            open_interactive_coding_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
                default_agent_profile_id,
            )
            .await?
        }
    };
    let mut receiver = session.subscribe();
    let compact_options = PromptTurnOptions::from_prompt_run_options(options);

    let outcome = {
        let mut compact = Box::pin(session.compact(compact_options));
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
                    break outcome.map_err(CliError::from);
                }
            }
        }
    }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

    Ok(CodingPromptTaskResult {
        session,
        outcome,
        update_usage: false,
        completion_notice: None,
        hydrate_transcript: false,
    })
}

async fn run_coding_plugin_reload_task(
    options: PromptRunOptions,
    existing_session: Option<CodingAgentSession>,
    default_agent_profile_id: ProfileId,
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut abort_rx: oneshot::Receiver<()>,
) -> Result<PluginReloadTaskResult, CliError> {
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            open_interactive_coding_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
                default_agent_profile_id,
            )
            .await?
        }
    };
    let mut receiver = session.subscribe();

    let outcome = {
        let mut reload = Box::pin(session.reload_plugins());
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
                    break outcome.map_err(CliError::from);
                }
            }
        }
    }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

    let plugin_commands = plugin_slash_commands(&session);
    let plugin_ui_actions = plugin_ui_actions(&session);
    let plugin_keybindings = plugin_keybindings(&session);
    let plugin_ui_dialogs = plugin_ui_dialogs(&session);
    Ok(PluginReloadTaskResult {
        session,
        outcome,
        plugin_commands,
        plugin_ui_actions,
        plugin_keybindings,
        plugin_ui_dialogs,
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
) -> Result<PluginCommandTaskResult, CliError> {
    let should_load_plugins = existing_session.is_none();
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            open_interactive_coding_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
                default_agent_profile_id,
            )
            .await?
        }
    };
    let mut receiver = session.subscribe();

    if should_load_plugins {
        let mut reload = Box::pin(session.reload_plugins());
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
                    outcome.map_err(CliError::from)?;
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
        .run_plugin_command(&command_id, args)
        .map_err(CliError::from)?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

    let plugin_commands = plugin_slash_commands(&session);
    let plugin_ui_actions = plugin_ui_actions(&session);
    let plugin_keybindings = plugin_keybindings(&session);
    let plugin_ui_dialogs = plugin_ui_dialogs(&session);
    Ok(PluginCommandTaskResult {
        session,
        command_id,
        output,
        plugin_commands,
        plugin_ui_actions,
        plugin_keybindings,
        plugin_ui_dialogs,
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
) -> Result<CodingPromptTaskResult, CliError> {
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            open_interactive_coding_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
                default_agent_profile_id,
            )
            .await?
        }
    };
    let mut receiver = session.subscribe();
    let branch_options = PromptTurnOptions::from_prompt_run_options(options);

    let outcome = {
        let mut branch_summary = Box::pin(session.summarize_branch(
            branch_options,
            source_leaf_id,
            target_leaf_id,
            custom_instructions,
        ));
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
                    break outcome.map_err(CliError::from);
                }
            }
        }
    }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

    Ok(CodingPromptTaskResult {
        session,
        outcome,
        update_usage: false,
        completion_notice: None,
        hydrate_transcript: false,
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
) -> Result<CodingPromptTaskResult, CliError> {
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            open_interactive_coding_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
                default_agent_profile_id,
            )
            .await?
        }
    };
    let mut receiver = session.subscribe();
    let branch_options = PromptTurnOptions::from_prompt_run_options(options);

    let outcome = {
        let mut branch_summary = Box::pin(session.summarize_branch_for_navigation(
            branch_options,
            source_leaf_id,
            target_leaf_id.clone(),
        ));
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
                    break outcome.map_err(CliError::from);
                }
            }
        }
    }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

    let forked_session = session.fork_current_session(Some(&target_leaf_id))?;
    Ok(CodingPromptTaskResult {
        session: forked_session,
        outcome,
        update_usage: false,
        completion_notice: Some("Navigated to selected point".to_string()),
        hydrate_transcript: true,
    })
}

fn interactive_coding_session_root(
    options: Option<&crate::runtime::SessionRunOptions>,
) -> Result<Option<PathBuf>, CliError> {
    let Some(options) = options else {
        return Ok(None);
    };
    if !matches!(options.mode, SessionMode::Enabled) {
        return Ok(None);
    }
    match options.session_dir.as_ref() {
        Some(root) => Ok(Some(root.clone())),
        None => Ok(Some(resolve_session_dir(&options.cwd, None, None)?)),
    }
}

async fn open_interactive_coding_session(
    session_options: Option<&crate::runtime::SessionRunOptions>,
    target: Option<&ResolvedSessionTarget>,
    default_agent_profile_id: ProfileId,
) -> Result<CodingAgentSession, CliError> {
    let Some(session_options) = session_options else {
        return Ok(CodingAgentSession::non_persistent(
            CodingAgentSessionOptions::new()
                .with_default_agent_profile_id(default_agent_profile_id),
        )
        .await?);
    };
    if !matches!(session_options.mode, SessionMode::Enabled) {
        return Ok(CodingAgentSession::non_persistent(
            CodingAgentSessionOptions::new()
                .with_cwd(session_options.cwd.clone())
                .with_default_agent_profile_id(default_agent_profile_id.clone()),
        )
        .await?);
    }

    let session_root =
        interactive_coding_session_root(Some(session_options))?.ok_or_else(|| {
            CodingSessionError::Session {
                message: "enabled interactive session is missing a session root".into(),
            }
        })?;

    let options = CodingAgentSessionOptions::new()
        .with_cwd(session_options.cwd.clone())
        .with_session_log_root(session_root)
        .with_default_agent_profile_id(default_agent_profile_id);
    match target.unwrap_or(&ResolvedSessionTarget::New) {
        ResolvedSessionTarget::New => Ok(CodingAgentSession::create(options).await?),
        ResolvedSessionTarget::OpenOrCreateId(session_id) => Ok(
            CodingAgentSession::open_or_create(options.with_session_id(session_id.clone())).await?,
        ),
        ResolvedSessionTarget::OpenTarget(target) => {
            if target_looks_like_rust_native_session_dir(target) {
                Ok(CodingAgentSession::open(options.with_session_path(target)).await?)
            } else if target_looks_like_legacy_jsonl(target) {
                Err(CodingSessionError::UnsupportedCapability {
                    capability: "legacy JSONL session targets".into(),
                }
                .into())
            } else {
                Ok(CodingAgentSession::open(options.with_session_id(target.clone())).await?)
            }
        }
        ResolvedSessionTarget::ContinueMostRecent => {
            let session_id = CodingAgentSession::list(options.clone())?
                .into_iter()
                .next()
                .map(|summary| summary.session_id)
                .ok_or_else(|| CodingSessionError::Session {
                    message: "no previous session to continue".into(),
                })?;
            Ok(CodingAgentSession::open(options.with_session_id(session_id)).await?)
        }
        ResolvedSessionTarget::ForkTarget(source) => {
            let forked = CodingAgentSession::fork_session(
                options.clone().with_session_id(source.clone()),
                None,
            )?;
            Ok(
                CodingAgentSession::open(options.with_session_id(forked.summary.session_id))
                    .await?,
            )
        }
    }
}

fn target_looks_like_rust_native_session_dir(target: &str) -> bool {
    let path = std::path::Path::new(target);
    path.is_dir() && path.join("session.json").is_file() && path.join("events.jsonl").is_file()
}

fn target_looks_like_legacy_jsonl(target: &str) -> bool {
    let path = std::path::Path::new(target);
    path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") || path.is_file()
}
