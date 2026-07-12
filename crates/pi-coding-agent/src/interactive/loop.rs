use std::path::PathBuf;
use std::time::{Duration, Instant};

use pi_agent_core::AgentResources;
use pi_agent_core::transcript::create_session_id;
use pi_tui::{
    Component, InputEvent, RenderScheduler, StdinBuffer, Terminal, Tui, TuiError, is_key_release,
};

use crate::api::CodingAgentPluginLoadOutcome;
use crate::coding_session::{
    CodingAgentSession, ProfileId, PromptTurnOptions, PromptTurnOutcome,
    SelfHealingEditModelRepairOptions, SelfHealingEditRequest,
};
use crate::input::{self, ProcessedPromptInput};
use crate::interactive::app::{
    PromptContext, build_prompt_context, resolve_prompt_api_key, session_label,
};
use crate::interactive::event_bridge::UiProjection;
use crate::interactive::input::InputPump;
use crate::interactive::prompt_task::{PromptTask, PromptTaskEvent, PromptTaskResult};
use crate::interactive::root::{
    ActivePluginUiDialog, InteractiveAction, InteractiveRoot, InteractiveStatus,
    PendingAgentInvocationRequest, PendingAgentTeamRequest, PendingBranchSummaryRequest,
    PendingDelegationConfirmationCommand, PendingDelegationConfirmationSelection,
    PendingForkRequest, PendingPluginCommandRequest, PendingPluginUiAction, PendingPluginUiDialog,
    PendingSelfHealingEditRequest, PluginUiDialogField,
};
use crate::interactive::session_actions::{
    SessionChoiceKind, fork_rust_native_choice, hydrate_existing_session_target,
    hydrated_session_from_rust_native,
};
use crate::interactive::{TranscriptItem, UiEvent};
use crate::prompt_options::PromptRunOptions;
use crate::runtime::PromptInvocation;
use crate::session::ResolvedSessionTarget;
use crate::{CliArgs, CliError, CliRunOptions};

const NORMAL_RENDER_INTERVAL: Duration = Duration::from_millis(16);
const SPINNER_INTERVAL: Duration = Duration::from_millis(120);
const SHUTDOWN_DRAIN_MAX: Duration = Duration::from_millis(1000);
const SHUTDOWN_DRAIN_IDLE: Duration = Duration::from_millis(50);

/// Print startup resource summary to stderr before the TUI takes over.
/// Mirrors the TS startup banner with [Context], [Skills], [Extensions].
/// Respects the `quiet_startup` setting.
fn print_startup_banner(prompt_context: &PromptContext) {
    if prompt_context.settings.quiet_startup {
        return;
    }
    let cwd = prompt_context
        .session
        .as_ref()
        .map(|s| s.cwd.clone())
        .unwrap_or_else(|| PathBuf::from("."));
    let cwd = cwd.canonicalize().unwrap_or(cwd);

    // [Context]
    if !prompt_context.context_files.is_empty() {
        let names: Vec<String> = prompt_context
            .context_files
            .iter()
            .map(|f| {
                // If the file's parent directory equals cwd, show just the file name.
                if let Some(parent) = f.path.parent()
                    && parent == cwd
                {
                    f.path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| f.path.display().to_string())
                } else {
                    f.path.display().to_string()
                }
            })
            .collect();
        eprintln!("[Context] {}", names.join(", "));
    }

    // [Skills]
    let skill_names: Vec<&str> = prompt_context
        .resources
        .skills
        .iter()
        .filter(|s| !s.disable_model_invocation)
        .map(|s| s.name.as_str())
        .collect();
    if !skill_names.is_empty() {
        eprintln!("[Skills] {}", skill_names.join(", "));
    }

    // [Extensions] — placeholder, not yet implemented.
}

fn print_exit_resume_hint(active_session_path: Option<&std::path::Path>) {
    if let Some(path) = active_session_path {
        let session_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| {
                // Rust-native session paths use the session id as the directory name.
                s.to_string()
            })
            .unwrap_or_else(|| path.display().to_string());
        eprintln!("To resume this session: pi --session {session_id}");
    }
}

pub(super) struct LoopResult<T: Terminal> {
    pub(super) tui: Tui<T>,
    pub(super) exit_code: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RenderRequest {
    requested: bool,
    force: bool,
}

impl RenderRequest {
    const NONE: Self = Self {
        requested: false,
        force: false,
    };
    const FORCE: Self = Self {
        requested: true,
        force: true,
    };

    fn changed(changed: bool) -> Self {
        if changed {
            Self {
                requested: true,
                force: false,
            }
        } else {
            Self::NONE
        }
    }
}

enum LoopControl {
    Continue(RenderRequest),
    Exit,
}

pub(super) trait InteractiveClock {
    fn now(&self) -> Instant;
}

struct SystemInteractiveClock;

impl InteractiveClock for SystemInteractiveClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

#[cfg(any(test, feature = "test-harness", debug_assertions))]
#[derive(Clone)]
pub(super) struct ManualInteractiveClock {
    now: std::sync::Arc<std::sync::Mutex<Instant>>,
}

#[cfg(any(test, feature = "test-harness", debug_assertions))]
impl ManualInteractiveClock {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            now: std::sync::Arc::new(std::sync::Mutex::new(now)),
        }
    }

    pub(super) fn advance(&self, delay: Duration) {
        let mut now = self
            .now
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *now += delay;
    }
}

#[cfg(any(test, feature = "test-harness", debug_assertions))]
impl InteractiveClock for ManualInteractiveClock {
    fn now(&self) -> Instant {
        *self
            .now
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

pub(super) async fn run_interactive_loop<T: Terminal>(
    parsed: CliArgs,
    options: CliRunOptions,
    terminal: T,
    input: &mut InputPump,
) -> Result<LoopResult<T>, CliError> {
    let clock = SystemInteractiveClock;
    run_interactive_loop_with_clock(parsed, options, terminal, input, &clock).await
}

pub(super) async fn run_interactive_loop_with_clock<T, C>(
    parsed: CliArgs,
    options: CliRunOptions,
    mut terminal: T,
    input: &mut InputPump,
    clock: &C,
) -> Result<LoopResult<T>, CliError>
where
    T: Terminal,
    C: InteractiveClock + ?Sized,
{
    let prompt_context = build_prompt_context(&parsed, options.clone())?;

    print_startup_banner(&prompt_context);

    terminal.start().map_err(to_cli_error)?;
    let (mut tui, root_id) = initialize_started_tui(terminal, &prompt_context)?;

    let loop_result = run_started_interactive_loop(
        &mut tui,
        root_id,
        input,
        prompt_context,
        &parsed,
        &options,
        clock,
    )
    .await;
    // Drain in-flight Kitty key release events before stopping, matching TS `drainInput(1000)`.
    let _ = tui
        .terminal_mut()
        .drain_input(SHUTDOWN_DRAIN_MAX, SHUTDOWN_DRAIN_IDLE);
    let stop_result = tui.terminal_mut().stop().map_err(to_cli_error);

    // Print resume hint after terminal cleanup.
    if let Ok(root) = root_ref(&tui, root_id) {
        print_exit_resume_hint(root.active_session_path.as_deref());
    }

    match (loop_result, stop_result) {
        (Ok(exit_code), Ok(())) => Ok(LoopResult { tui, exit_code }),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

pub(super) async fn run_interactive_loop_with_input<T, F>(
    parsed: CliArgs,
    options: CliRunOptions,
    mut terminal: T,
    make_input: F,
) -> Result<LoopResult<T>, CliError>
where
    T: Terminal,
    F: FnOnce() -> InputPump,
{
    let prompt_context = build_prompt_context(&parsed, options.clone())?;

    print_startup_banner(&prompt_context);

    terminal.start().map_err(to_cli_error)?;
    let mut input = make_input();
    let (mut tui, root_id) = initialize_started_tui(terminal, &prompt_context)?;

    let clock = SystemInteractiveClock;
    let loop_result = run_started_interactive_loop(
        &mut tui,
        root_id,
        &mut input,
        prompt_context,
        &parsed,
        &options,
        &clock,
    )
    .await;
    // Drain in-flight Kitty key release events before stopping.
    let _ = tui
        .terminal_mut()
        .drain_input(SHUTDOWN_DRAIN_MAX, SHUTDOWN_DRAIN_IDLE);
    let stop_result = tui.terminal_mut().stop().map_err(to_cli_error);

    // Print resume hint after terminal cleanup.
    if let Ok(root) = root_ref(&tui, root_id) {
        print_exit_resume_hint(root.active_session_path.as_deref());
    }

    match (loop_result, stop_result) {
        (Ok(exit_code), Ok(())) => Ok(LoopResult { tui, exit_code }),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

fn initialize_started_tui<T: Terminal>(
    terminal: T,
    prompt_context: &PromptContext,
) -> Result<(Tui<T>, usize), CliError> {
    let cwd = prompt_context
        .session
        .as_ref()
        .map(|session| session.cwd.clone())
        .unwrap_or_else(|| PathBuf::from("."));
    let session_label = session_label(&prompt_context.session);
    let mut tui = Tui::new(terminal);
    let root_id = tui.add_child_with_id(Box::new(
        InteractiveRoot::new_with_theme_models_and_settings(
            cwd,
            prompt_context.model.id.clone(),
            session_label,
            prompt_context.theme.clone(),
            prompt_context.model_choices.clone(),
            prompt_context.settings.clone(),
            prompt_context.auth.clone(),
        )
        .with_resolved_theme(prompt_context.resolved_theme.clone()),
    ));
    {
        let root = root_mut(&mut tui, root_id)?;
        root.model_rotation = prompt_context.model_rotation.clone();
        root.session_choices = prompt_context.session_choices.clone();
        root.model = Some(prompt_context.model.clone());
        root.thinking_level = prompt_context.thinking_level.unwrap_or_default();
        root.profile_registry = prompt_context.profile_registry.clone();
        root.default_agent_profile_id = prompt_context.default_agent_profile_id.clone();
        if let Some(hydrated) = hydrate_existing_session_target(
            &prompt_context.session,
            prompt_context.session_target.as_ref(),
        )? {
            root.apply_hydrated_session(hydrated, None);
        }
    }
    tui.set_clear_on_shrink(prompt_context.settings.terminal.clear_on_shrink);
    tui.set_focus(Some(root_id));
    Ok((tui, root_id))
}

async fn run_started_interactive_loop<T, C>(
    tui: &mut Tui<T>,
    root_id: usize,
    input: &mut InputPump,
    mut prompt_context: PromptContext,
    parsed: &CliArgs,
    options: &CliRunOptions,
    clock: &C,
) -> Result<i32, CliError>
where
    T: Terminal,
    C: InteractiveClock + ?Sized,
{
    let mut stdin_buffer = StdinBuffer::new();
    let mut running: Option<PromptTask> = None;
    let mut coding_session: Option<CodingAgentSession> = None;
    let mut ui_projection = UiProjection::new();
    let mut input_open = true;
    let mut render_scheduler = RenderScheduler::new(NORMAL_RENDER_INTERVAL);
    render_scheduler.request(true);
    flush_render_if_ready(tui, &mut render_scheduler, clock.now())?;

    // Start the theme hot-reload watcher. Only custom themes (a name other
    // than dark/light) are watched; built-in themes return an idle watcher.
    let active_theme_name = prompt_context.settings.theme.as_deref().unwrap_or("dark");
    let (_theme_watcher, mut theme_reload) = crate::theme::ThemeWatcher::start(
        prompt_context.themes_dir.clone(),
        active_theme_name.to_string(),
        Duration::from_millis(100),
    )
    .map_err(to_cli_error)?;

    loop {
        flush_render_if_ready(tui, &mut render_scheduler, clock.now())?;
        if let Some(mut task) = running.take() {
            let render_delay = pending_render_delay(&render_scheduler, clock.now());
            let stdin_delay = stdin_pending_delay(&stdin_buffer, clock.now());
            tokio::select! {
                _ = sleep_render_delay(render_delay), if render_delay.is_some() => {
                    flush_render_if_ready(tui, &mut render_scheduler, clock.now())?;
                    running = Some(task);
                }
                _ = sleep_stdin_pending(stdin_delay), if stdin_delay.is_some() => {
                    running = Some(task);
                    let events = stdin_buffer.tick(clock.now());
                    if !events.is_empty() {
                        match process_input_events(
                            tui,
                            root_id,
                            events,
                            &mut prompt_context,
                            &mut running,
                            &mut coding_session,
                            &mut render_scheduler,
                            parsed,
                            options,
                            clock.now(),
                        )? {
                            LoopControl::Continue(_) => {}
                            LoopControl::Exit => return Ok(0),
                        }
                    }
                }
                chunk = input.recv(), if input_open => {
                    match chunk {
                        Some(chunk) => {
                            running = Some(task);
                            match process_input_events(
                                tui,
                                root_id,
                                stdin_buffer.process_at(&chunk, clock.now()),
                                &mut prompt_context,
                                &mut running,
                                &mut coding_session,
                                &mut render_scheduler,
                                parsed,
                                options,
                                clock.now(),
                            )? {
                                LoopControl::Continue(_) => {
                                    input.mark_processed(&chunk);
                                }
                                LoopControl::Exit => return Ok(0),
                            }
                        }
                        None => {
                            input_open = false;
                            running = Some(task);
                        }
                    }
                }
                maybe_event = task.events.recv(), if !task.events_closed => {
                    match maybe_event {
                        Some(event) => {
                            schedule_render(
                                &mut render_scheduler,
                                apply_prompt_task_event(
                                    tui,
                                    root_id,
                                    &mut ui_projection,
                                    event,
                                )?,
                            );
                        }
                        None => {
                            task.events_closed = true;
                        }
                    }
                    running = Some(task);
                }
                _ = tokio::time::sleep(SPINNER_INTERVAL) => {
                    if let Some(root) = tui.component_as_mut::<InteractiveRoot>(root_id) {
                        root.spinner_frame = root.spinner_frame.wrapping_add(1);
                    }
                    render_scheduler.request(true);
                    running = Some(task);
                }
                done = &mut task.done => {
                    let result = done.unwrap_or_else(|_| {
                        Err(CliError::AgentFailure(
                            "prompt task dropped before completion".to_string(),
                        ))
                    });
                    while let Ok(event) = task.events.try_recv() {
                        schedule_render(
                            &mut render_scheduler,
                            apply_prompt_task_event(
                                tui,
                                root_id,
                                &mut ui_projection,
                                event,
                            )?,
                        );
                    }
                    finish_prompt(tui, root_id, result, &mut coding_session)?;
                    if let Some(session) = coding_session.as_ref() {
                        prompt_context.default_agent_profile_id =
                            session.view().default_agent_profile_id.clone();
                    }
                    schedule_render(&mut render_scheduler, RenderRequest::FORCE);
                    flush_render_if_ready(tui, &mut render_scheduler, clock.now())?;
                    running = None;
                    input.mark_idle();
                }
                Some(reload) = theme_reload.recv() => {
                    apply_theme_reload(tui, root_id, reload);
                    render_scheduler.request(true);
                    running = Some(task);
                }
            }
        } else {
            if !input_open {
                flush_pending_render(tui, &mut render_scheduler, clock.now())?;
                return Ok(0);
            }

            let render_delay = pending_render_delay(&render_scheduler, clock.now());
            let stdin_delay = stdin_pending_delay(&stdin_buffer, clock.now());
            tokio::select! {
                _ = sleep_render_delay(render_delay), if render_delay.is_some() => {
                    flush_render_if_ready(tui, &mut render_scheduler, clock.now())?;
                }
                _ = sleep_stdin_pending(stdin_delay), if stdin_delay.is_some() => {
                    let events = stdin_buffer.tick(clock.now());
                    if !events.is_empty() {
                        match process_input_events(
                            tui,
                            root_id,
                            events,
                            &mut prompt_context,
                            &mut running,
                            &mut coding_session,
                            &mut render_scheduler,
                            parsed,
                            options,
                            clock.now(),
                        )? {
                            LoopControl::Continue(_) => {}
                            LoopControl::Exit => return Ok(0),
                        }
                    }
                }
                chunk = input.recv() => {
                    let Some(chunk) = chunk else {
                        input_open = false;
                        match process_input_events(
                            tui,
                            root_id,
                            stdin_buffer.flush(),
                            &mut prompt_context,
                            &mut running,
                            &mut coding_session,
                            &mut render_scheduler,
                            parsed,
                            options,
                            clock.now(),
                        )? {
                            LoopControl::Continue(_) => {}
                            LoopControl::Exit => return Ok(0),
                        }
                        if running.is_none() {
                            flush_pending_render(tui, &mut render_scheduler, clock.now())?;
                            return Ok(0);
                        }
                        tokio::task::yield_now().await;
                        continue;
                    };

                    match process_input_events(
                        tui,
                        root_id,
                        stdin_buffer.process_at(&chunk, clock.now()),
                        &mut prompt_context,
                        &mut running,
                        &mut coding_session,
                        &mut render_scheduler,
                        parsed,
                        options,
                        clock.now(),
                    )? {
                        LoopControl::Continue(_) => {
                            input.mark_processed(&chunk);
                        }
                        LoopControl::Exit => return Ok(0),
                    }
                    if running.is_some() {
                        tokio::task::yield_now().await;
                    }
                }
                Some(reload) = theme_reload.recv() => {
                    apply_theme_reload(tui, root_id, reload);
                    render_scheduler.request(true);
                }
            }
        }
    }
}

fn process_input_events<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    events: Vec<InputEvent>,
    prompt_context: &mut PromptContext,
    running: &mut Option<PromptTask>,
    coding_session: &mut Option<CodingAgentSession>,
    render_scheduler: &mut RenderScheduler,
    parsed: &CliArgs,
    options: &CliRunOptions,
    now: Instant,
) -> Result<LoopControl, CliError> {
    for event in events {
        let was_running = running.is_some();
        match handle_input_event(
            tui,
            root_id,
            event,
            prompt_context,
            running,
            coding_session,
            parsed,
            options,
        )? {
            LoopControl::Continue(request) => {
                schedule_render(render_scheduler, request);
                flush_render_if_ready(tui, render_scheduler, now)?;
            }
            LoopControl::Exit => return Ok(LoopControl::Exit),
        }
        if !was_running && running.is_some() {
            break;
        }
    }
    Ok(LoopControl::Continue(RenderRequest::NONE))
}

fn schedule_render(render_scheduler: &mut RenderScheduler, request: RenderRequest) {
    if request.requested {
        render_scheduler.request(request.force);
    }
}

fn pending_render_delay(render_scheduler: &RenderScheduler, now: Instant) -> Option<Duration> {
    render_scheduler
        .next_render_at(now)
        .map(|deadline| deadline.saturating_duration_since(now))
}

async fn sleep_render_delay(delay: Option<Duration>) {
    if let Some(delay) = delay {
        tokio::time::sleep(delay).await;
    }
}

fn stdin_pending_delay(stdin_buffer: &StdinBuffer, now: Instant) -> Option<Duration> {
    stdin_buffer.pending_timeout_at(now)
}

async fn sleep_stdin_pending(delay: Option<Duration>) {
    if let Some(delay) = delay {
        tokio::time::sleep(delay).await;
    }
}

fn flush_render_if_ready<T: Terminal>(
    tui: &mut Tui<T>,
    render_scheduler: &mut RenderScheduler,
    now: Instant,
) -> Result<(), CliError> {
    if render_scheduler.should_render_now(now) {
        render_tui(tui)?;
        render_scheduler.mark_rendered(now);
    }
    Ok(())
}

fn flush_pending_render<T: Terminal>(
    tui: &mut Tui<T>,
    render_scheduler: &mut RenderScheduler,
    now: Instant,
) -> Result<(), CliError> {
    if render_scheduler.has_pending() {
        render_scheduler.request(true);
        flush_render_if_ready(tui, render_scheduler, now)?;
    }
    Ok(())
}

fn handle_input_event<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    event: InputEvent,
    prompt_context: &mut PromptContext,
    running: &mut Option<PromptTask>,
    coding_session: &mut Option<CodingAgentSession>,
    parsed: &CliArgs,
    options: &CliRunOptions,
) -> Result<LoopControl, CliError> {
    if is_key_release(&event) {
        return Ok(LoopControl::Continue(RenderRequest::NONE));
    }

    let (
        action,
        prompt,
        selected_model,
        selected_thinking_level,
        selected_agent_profile_id,
        selected_session,
        selected_session_hydrate,
        settings_update,
        auth_update,
        compact_instructions,
        branch_summary_request,
        agent_invocation_request,
        agent_team_request,
        delegation_confirmation_command,
        self_healing_edit_request,
        plugin_command_request,
        plugin_ui_action,
        plugin_ui_dialog,
        fork_request,
        render_request,
    ) = {
        let root = root_mut(tui, root_id)?;
        let before = root.render_state();
        root.handle_input(&event);
        let action = root.take_action();
        let prompt = if matches!(
            action,
            InteractiveAction::Submit | InteractiveAction::FollowUp
        ) {
            root.take_pending_submit()
        } else {
            None
        };
        let selected_model = root.take_selected_model();
        let selected_thinking_level = root.take_selected_thinking_level();
        let selected_agent_profile_id = root.take_selected_agent_profile_id();
        let selected_session = root.take_selected_session();
        let selected_session_hydrate = root.take_selected_session_hydrate();
        let settings_update = root.take_settings_update();
        let auth_update = root.take_auth_update();
        let compact_instructions = if action == InteractiveAction::CompactSession {
            root.take_pending_compact_instructions()
        } else {
            None
        };
        let branch_summary_request = if action == InteractiveAction::BranchSummary {
            root.take_pending_branch_summary_request()
        } else {
            None
        };
        let agent_invocation_request = if action == InteractiveAction::AgentInvocation {
            root.take_pending_agent_invocation_request()
        } else {
            None
        };
        let agent_team_request = if action == InteractiveAction::AgentTeam {
            root.take_pending_agent_team_request()
        } else {
            None
        };
        let delegation_confirmation_command = if action == InteractiveAction::DelegationConfirmation
        {
            root.take_pending_delegation_confirmation_command()
        } else {
            None
        };
        let self_healing_edit_request = if action == InteractiveAction::SelfHealingEdit {
            root.take_pending_self_healing_edit_request()
        } else {
            None
        };
        let plugin_command_request = if action == InteractiveAction::PluginCommand {
            root.take_pending_plugin_command_request()
        } else {
            None
        };
        let plugin_ui_action = if action == InteractiveAction::PluginUiAction {
            root.take_pending_plugin_ui_action()
        } else {
            None
        };
        let plugin_ui_dialog = if action == InteractiveAction::PluginUiDialog {
            root.take_pending_plugin_ui_dialog()
        } else {
            None
        };
        let fork_request = if action == InteractiveAction::Fork {
            root.take_pending_fork_request()
        } else {
            None
        };
        let after = root.render_state();
        (
            action,
            prompt,
            selected_model,
            selected_thinking_level,
            selected_agent_profile_id,
            selected_session,
            selected_session_hydrate,
            settings_update,
            auth_update,
            compact_instructions,
            branch_summary_request,
            agent_invocation_request,
            agent_team_request,
            delegation_confirmation_command,
            self_healing_edit_request,
            plugin_command_request,
            plugin_ui_action,
            plugin_ui_dialog,
            fork_request,
            RenderRequest::changed(before != after),
        )
    };

    if let Some(model) = selected_model {
        let (api_key, auth_diagnostics, diagnostics) = resolve_prompt_api_key(
            &model.provider,
            prompt_context.cli_api_key.as_deref(),
            &prompt_context.auth,
        );
        let diagnostic_text = crate::request::render_diagnostics(&diagnostics);
        if !diagnostic_text.is_empty() {
            eprint!("{diagnostic_text}");
        }
        prompt_context.api_key = api_key;
        prompt_context.auth_diagnostics = auth_diagnostics;
        prompt_context.model = model;
    }
    if let Some(thinking_level) = selected_thinking_level {
        prompt_context.thinking_level = Some(thinking_level);
    }
    if let Some(profile_id) = selected_agent_profile_id {
        if coding_session.is_some() {
            if running.is_some() {
                let root = root_mut(tui, root_id)?;
                root.transcript.push(TranscriptItem::system(
                    "Wait for the current run to finish before changing the default profile.",
                ));
                return Ok(LoopControl::Continue(RenderRequest::FORCE));
            }
            start_set_default_agent_profile_task(
                tui,
                root_id,
                profile_id,
                prompt_context,
                running,
                coding_session,
            )?;
            return Ok(LoopControl::Continue(RenderRequest::FORCE));
        } else {
            prompt_context.default_agent_profile_id = profile_id;
        }
    }
    if let Some(session) = selected_session {
        *coding_session = None;
        prompt_context.session_target = Some(ResolvedSessionTarget::OpenTarget(
            session.path.display().to_string(),
        ));
        prompt_context.session_name = session.name.clone();
        if selected_session_hydrate
            && let Some(hydrated) = hydrate_existing_session_target(
                &prompt_context.session,
                prompt_context.session_target.as_ref(),
            )?
        {
            let root = root_mut(tui, root_id)?;
            root.apply_hydrated_session(
                hydrated,
                Some(format!("Session selected: {}", session.display_name())),
            );
        }
    }
    if let Some(settings) = settings_update {
        let clear_on_shrink = settings.terminal.clear_on_shrink;
        prompt_context.settings = settings;
        tui.set_clear_on_shrink(clear_on_shrink);

        // Persist settings delta to disk
        if let Ok(root) = root_mut(tui, root_id) {
            let delta = root.settings_delta();
            let cwd = prompt_context
                .session
                .as_ref()
                .map(|s| s.cwd.clone())
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            let paths = crate::config::resolve_paths(&cwd);
            let mut diags = Vec::new();
            crate::config::merge_and_save_settings(
                &paths,
                crate::config::SettingsScope::Global,
                delta,
                &mut diags,
            );
        }
    }
    if let Some(auth) = auth_update {
        prompt_context.auth = auth;
        let (api_key, auth_diagnostics, diagnostics) = resolve_prompt_api_key(
            &prompt_context.model.provider,
            prompt_context.cli_api_key.as_deref(),
            &prompt_context.auth,
        );
        let diagnostic_text = crate::request::render_diagnostics(&diagnostics);
        if !diagnostic_text.is_empty() {
            eprint!("{diagnostic_text}");
        }
        prompt_context.api_key = api_key;
        prompt_context.auth_diagnostics = auth_diagnostics;
    }

    // Tree label persistence for Rust-native sessions is not implemented yet.
    {
        let root = root_mut(tui, root_id)?;
        let _ = root.take_pending_tree_label_change();
    }

    // Process tree navigation.
    let mut tree_navigation_summary: Option<(String, String)> = None;
    let mut tree_navigation_fork: Option<(
        crate::interactive::session_actions::SessionChoice,
        String,
    )> = None;
    {
        let root = root_mut(tui, root_id)?;
        if let Some(target_id) = root.take_selected_tree_entry_id() {
            if let Some(choice) = root
                .active_session
                .as_ref()
                .filter(|choice| choice.kind == SessionChoiceKind::RustNative)
                .cloned()
            {
                let current_leaf_id = choice
                    .active_leaf_id
                    .clone()
                    .or_else(|| root.active_leaf_id.clone());
                if current_leaf_id.as_deref() == Some(target_id.as_str()) {
                    root.transcript
                        .push(TranscriptItem::system("Already at this point".to_string()));
                } else if let Some(source_leaf_id) = current_leaf_id {
                    tree_navigation_summary = Some((source_leaf_id, target_id));
                } else {
                    tree_navigation_fork = Some((choice, target_id));
                }
            } else {
                root.transcript.push(TranscriptItem::system(
                    "No active Rust-native session for tree navigation".to_string(),
                ));
            }
        }
    }
    if let Some((source_leaf_id, target_leaf_id)) = tree_navigation_summary {
        if running.is_some() {
            let root = root_mut(tui, root_id)?;
            root.transcript.push(TranscriptItem::system(
                "Wait for the current run to finish before navigating the session tree.",
            ));
            return Ok(LoopControl::Continue(RenderRequest::FORCE));
        }
        *running = Some(start_branch_summary_navigation_task(
            tui,
            root_id,
            source_leaf_id,
            target_leaf_id,
            prompt_context,
            coding_session,
        )?);
        return Ok(LoopControl::Continue(RenderRequest::FORCE));
    }
    if let Some((choice, target_id)) = tree_navigation_fork {
        let root = root_mut(tui, root_id)?;
        match fork_rust_native_choice(&choice, Some(&target_id)) {
            Ok(hydrated) => {
                root.apply_hydrated_session(
                    hydrated,
                    Some("Navigated to selected point".to_string()),
                );
                if let Some(active) = root.active_session.as_ref() {
                    prompt_context.session_target =
                        Some(ResolvedSessionTarget::OpenTarget(active.id.clone()));
                }
            }
            Err(error) => root.transcript.push(TranscriptItem::system(format!(
                "Failed to navigate tree: {error}"
            ))),
        }
    }

    match action {
        InteractiveAction::None => Ok(LoopControl::Continue(render_request)),
        InteractiveAction::Exit => {
            if root_settings_show_progress(tui, root_id)? {
                set_terminal_progress(tui, false)?;
            }
            Ok(LoopControl::Exit)
        }
        InteractiveAction::AbortRunning => {
            if let Some(task) = running.as_mut() {
                task.abort_once();
            }
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
        InteractiveAction::NewSession => {
            if prompt_context
                .session
                .as_ref()
                .is_some_and(|session| matches!(session.mode, crate::runtime::SessionMode::Enabled))
            {
                *coding_session = None;
                prompt_context.session_target =
                    Some(ResolvedSessionTarget::OpenOrCreateId(create_session_id()));
                prompt_context.session_name = None;
            }
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
        InteractiveAction::ReloadResources => {
            if running.is_some() {
                let root = root_mut(tui, root_id)?;
                root.transcript.push(TranscriptItem::system(
                    "Wait for the current run to finish before reloading plugins.",
                ));
                return Ok(LoopControl::Continue(RenderRequest::FORCE));
            }
            match build_prompt_context(parsed, options.clone()) {
                Ok(mut reloaded) => {
                    reloaded.default_agent_profile_id =
                        prompt_context.default_agent_profile_id.clone();
                    *prompt_context = reloaded;
                    let root = root_mut(tui, root_id)?;
                    root.apply_prompt_context(prompt_context);
                    root.transcript
                        .push(TranscriptItem::system("Reloaded keybindings and resources"));
                }
                Err(error) => {
                    let root = root_mut(tui, root_id)?;
                    root.transcript
                        .push(TranscriptItem::system(format!("Reload failed: {error}")));
                    return Ok(LoopControl::Continue(RenderRequest::FORCE));
                }
            }
            *running = Some(start_plugin_reload_task(
                tui,
                root_id,
                prompt_context,
                coding_session,
            )?);
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
        InteractiveAction::AgentProfileUse => Ok(LoopControl::Continue(RenderRequest::FORCE)),
        InteractiveAction::AgentInvocation => {
            if running.is_some() {
                return Ok(LoopControl::Continue(render_request));
            }
            let Some(request) = agent_invocation_request else {
                return Ok(LoopControl::Continue(render_request));
            };
            *running = Some(start_agent_invocation_task(
                tui,
                root_id,
                request,
                prompt_context,
                coding_session,
            )?);
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
        InteractiveAction::AgentTeam => {
            if running.is_some() {
                return Ok(LoopControl::Continue(render_request));
            }
            let Some(request) = agent_team_request else {
                return Ok(LoopControl::Continue(render_request));
            };
            *running = Some(start_agent_team_task(
                tui,
                root_id,
                request,
                prompt_context,
                coding_session,
            )?);
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
        InteractiveAction::DelegationConfirmation => {
            if running.is_some() {
                return Ok(LoopControl::Continue(render_request));
            }
            let Some(command) = delegation_confirmation_command else {
                return Ok(LoopControl::Continue(render_request));
            };
            handle_delegation_confirmation_command(
                tui,
                root_id,
                command,
                prompt_context,
                running,
                coding_session,
            )?;
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
        InteractiveAction::Submit => {
            let Some(prompt) = prompt else {
                return Ok(LoopControl::Continue(render_request));
            };
            if prompt.trim().is_empty() {
                return Ok(LoopControl::Continue(render_request));
            }
            if let Some(task) = running.as_ref() {
                if task.steer(prompt) {
                    return Ok(LoopControl::Continue(RenderRequest::FORCE));
                }
                return Ok(LoopControl::Continue(render_request));
            }
            *running = Some(start_prompt_task(
                tui,
                root_id,
                prompt,
                prompt_context,
                coding_session,
            )?);
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
        InteractiveAction::FollowUp => {
            let Some(prompt) = prompt else {
                return Ok(LoopControl::Continue(render_request));
            };
            if prompt.trim().is_empty() {
                return Ok(LoopControl::Continue(render_request));
            }
            if let Some(task) = running.as_ref() {
                if task.follow_up(prompt) {
                    return Ok(LoopControl::Continue(RenderRequest::FORCE));
                }
            }
            Ok(LoopControl::Continue(render_request))
        }
        InteractiveAction::CompactSession => {
            if running.is_some() {
                return Ok(LoopControl::Continue(render_request));
            }
            *running = Some(start_compact_task(
                tui,
                root_id,
                compact_instructions,
                prompt_context,
                coding_session,
            )?);
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
        InteractiveAction::BranchSummary => {
            if running.is_some() {
                return Ok(LoopControl::Continue(render_request));
            }
            let Some(request) = branch_summary_request else {
                return Ok(LoopControl::Continue(render_request));
            };
            *running = Some(start_branch_summary_task(
                tui,
                root_id,
                request,
                prompt_context,
                coding_session,
            )?);
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
        InteractiveAction::SelfHealingEdit => {
            if running.is_some() {
                return Ok(LoopControl::Continue(render_request));
            }
            let Some(request) = self_healing_edit_request else {
                return Ok(LoopControl::Continue(render_request));
            };
            *running = Some(start_self_healing_edit_task(
                tui,
                root_id,
                request,
                prompt_context,
                coding_session,
            )?);
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
        InteractiveAction::PluginCommand => {
            if running.is_some() {
                return Ok(LoopControl::Continue(render_request));
            }
            let Some(request) = plugin_command_request else {
                return Ok(LoopControl::Continue(render_request));
            };
            *running = Some(start_plugin_command_task(
                tui,
                root_id,
                request,
                prompt_context,
                coding_session,
            )?);
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
        InteractiveAction::PluginUiAction => {
            let Some(action) = plugin_ui_action else {
                return Ok(LoopControl::Continue(render_request));
            };
            dispatch_plugin_ui_action(tui, root_id, action)?;
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
        InteractiveAction::PluginUiDialog => {
            let Some(dialog) = plugin_ui_dialog else {
                return Ok(LoopControl::Continue(render_request));
            };
            dispatch_plugin_ui_dialog(tui, root_id, dialog)?;
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
        InteractiveAction::Fork => {
            if running.is_some() {
                return Ok(LoopControl::Continue(render_request));
            }
            let Some(request) = fork_request else {
                return Ok(LoopControl::Continue(render_request));
            };
            start_fork_task(
                tui,
                root_id,
                request,
                prompt_context,
                running,
                coding_session,
            )?;
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
    }
}

fn dispatch_plugin_ui_action<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    action: PendingPluginUiAction,
) -> Result<(), CliError> {
    let root = root_mut(tui, root_id)?;
    root.transcript.push(TranscriptItem::system(format!(
        "Plugin UI action {}: {}",
        action.action_id, action.label
    )));
    Ok(())
}

fn dispatch_plugin_ui_dialog<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    dialog: PendingPluginUiDialog,
) -> Result<(), CliError> {
    let root = root_mut(tui, root_id)?;
    let description = if dialog.description.trim().is_empty() {
        String::new()
    } else {
        format!(" - {}", dialog.description)
    };
    root.active_plugin_ui_dialog = Some(ActivePluginUiDialog::new(dialog.clone()));
    root.transcript.push(TranscriptItem::system(format!(
        "Plugin UI dialog {}: {}{}",
        dialog.dialog_id, dialog.title, description
    )));
    for field in &dialog.fields {
        root.transcript
            .push(TranscriptItem::system(plugin_dialog_field_line(field)));
    }
    root.editor.set_text(plugin_dialog_command_text(
        &dialog.action_id,
        &dialog.fields,
    ));
    Ok(())
}

fn handle_delegation_confirmation_command<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    command: PendingDelegationConfirmationCommand,
    prompt_context: &PromptContext,
    running: &mut Option<PromptTask>,
    coding_session: &mut Option<CodingAgentSession>,
) -> Result<(), CliError> {
    match command {
        PendingDelegationConfirmationCommand::List => {
            show_pending_delegation_confirmations(tui, root_id, coding_session.as_ref())
        }
        PendingDelegationConfirmationCommand::Approve { selection } => {
            start_delegation_approval_task(
                tui,
                root_id,
                selection,
                prompt_context,
                running,
                coding_session,
            )
        }
        PendingDelegationConfirmationCommand::Reject { selection, reason } => {
            reject_pending_delegation_confirmation(
                tui,
                root_id,
                selection,
                reason,
                prompt_context,
                running,
                coding_session,
            )
        }
    }
}

fn show_pending_delegation_confirmations<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    coding_session: Option<&CodingAgentSession>,
) -> Result<(), CliError> {
    let Some(session) = coding_session else {
        root_mut(tui, root_id)?
            .transcript
            .push(TranscriptItem::system("No active coding session."));
        return Ok(());
    };
    let pending = session.pending_delegation_confirmations();
    if pending.is_empty() {
        root_mut(tui, root_id)?
            .transcript
            .push(TranscriptItem::system(
                "No pending delegation confirmations.",
            ));
        return Ok(());
    }
    root_mut(tui, root_id)?.open_delegation_confirmation_menu(pending);
    Ok(())
}

fn start_delegation_approval_task<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    selection: PendingDelegationConfirmationSelection,
    prompt_context: &PromptContext,
    running: &mut Option<PromptTask>,
    coding_session: &mut Option<CodingAgentSession>,
) -> Result<(), CliError> {
    let Some(session) = coding_session.as_ref() else {
        root_mut(tui, root_id)?
            .transcript
            .push(TranscriptItem::system("No active coding session."));
        return Ok(());
    };
    let (operation_id, tool_call_id) =
        match resolve_pending_delegation_confirmation(session, &selection) {
            Ok(resolved) => resolved,
            Err(message) => {
                root_mut(tui, root_id)?
                    .transcript
                    .push(TranscriptItem::system(message));
                return Ok(());
            }
        };

    let session = coding_session
        .take()
        .expect("coding session was checked before starting delegation approval");
    {
        let root = root_mut(tui, root_id)?;
        root.transcript.push(TranscriptItem::system(format!(
            "Approving delegation: {operation_id} {tool_call_id}"
        )));
        root.set_status(InteractiveStatus::Running);
    }
    *running = Some(PromptTask::spawn_delegation_approval(
        session,
        operation_id,
        tool_call_id,
    )?);
    if prompt_context.settings.terminal.show_progress {
        set_terminal_progress(tui, true)?;
    }
    Ok(())
}

fn start_set_default_agent_profile_task<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    profile_id: ProfileId,
    prompt_context: &PromptContext,
    running: &mut Option<PromptTask>,
    coding_session: &mut Option<CodingAgentSession>,
) -> Result<(), CliError> {
    if coding_session.is_none() {
        root_mut(tui, root_id)?
            .transcript
            .push(TranscriptItem::system("No active coding session."));
        return Ok(());
    }
    let session = coding_session
        .take()
        .expect("coding session was checked before starting default profile mutation");
    {
        let root = root_mut(tui, root_id)?;
        root.set_status(InteractiveStatus::Running);
    }
    *running = Some(PromptTask::spawn_set_default_agent_profile(
        session, profile_id,
    )?);
    if prompt_context.settings.terminal.show_progress {
        set_terminal_progress(tui, true)?;
    }
    Ok(())
}

fn reject_pending_delegation_confirmation<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    selection: PendingDelegationConfirmationSelection,
    reason: Option<String>,
    prompt_context: &PromptContext,
    running: &mut Option<PromptTask>,
    coding_session: &mut Option<CodingAgentSession>,
) -> Result<(), CliError> {
    let Some(session) = coding_session.as_ref() else {
        root_mut(tui, root_id)?
            .transcript
            .push(TranscriptItem::system("No active coding session."));
        return Ok(());
    };
    let (operation_id, tool_call_id) =
        match resolve_pending_delegation_confirmation(session, &selection) {
            Ok(resolved) => resolved,
            Err(message) => {
                root_mut(tui, root_id)?
                    .transcript
                    .push(TranscriptItem::system(message));
                return Ok(());
            }
        };

    let session = coding_session
        .take()
        .expect("coding session was checked before starting delegation rejection");
    {
        let root = root_mut(tui, root_id)?;
        root.set_status(InteractiveStatus::Running);
    }
    *running = Some(PromptTask::spawn_delegation_rejection(
        session,
        operation_id,
        tool_call_id,
        reason.unwrap_or_else(|| "delegation rejected by user".to_string()),
    )?);
    if prompt_context.settings.terminal.show_progress {
        set_terminal_progress(tui, true)?;
    }
    Ok(())
}

fn resolve_pending_delegation_confirmation(
    session: &CodingAgentSession,
    selection: &PendingDelegationConfirmationSelection,
) -> Result<(String, String), String> {
    let pending = session.pending_delegation_confirmations();
    if pending.is_empty() {
        return Err("No pending delegation confirmations.".to_string());
    }
    if let Some(operation_id) = selection.operation_id.as_deref() {
        return pending
            .iter()
            .find(|pending| {
                pending.operation_id == operation_id
                    && pending.tool_call_id == selection.tool_call_id
            })
            .map(|pending| (pending.operation_id.clone(), pending.tool_call_id.clone()))
            .ok_or_else(|| {
                format!(
                    "Pending delegation confirmation not found: operation_id={operation_id}, tool_call_id={}",
                    selection.tool_call_id
                )
            });
    }

    let matches = pending
        .iter()
        .filter(|pending| pending.tool_call_id == selection.tool_call_id)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [pending] => Ok((pending.operation_id.clone(), pending.tool_call_id.clone())),
        [] => Err(format!(
            "Pending delegation confirmation not found: tool_call_id={}",
            selection.tool_call_id
        )),
        _ => Err(format!(
            "Multiple pending delegation confirmations match tool_call_id={}; include the operation id.",
            selection.tool_call_id
        )),
    }
}

fn plugin_dialog_field_line(field: &PluginUiDialogField) -> String {
    if field.description.trim().is_empty() {
        field.label.clone()
    } else {
        format!("{}: {}", field.label, field.description)
    }
}

fn plugin_dialog_command_text(action_id: &str, fields: &[PluginUiDialogField]) -> String {
    if fields.is_empty() {
        return format!("/plugin-command {action_id} ");
    }
    let mut args = serde_json::Map::new();
    for field in fields {
        args.insert(field.id.clone(), field.default_value.clone());
    }
    let args =
        serde_json::to_string(&serde_json::Value::Object(args)).unwrap_or_else(|_| "{}".into());
    format!("/plugin-command {action_id} {args}")
}

fn start_prompt_task<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    prompt: String,
    prompt_context: &PromptContext,
    coding_session: &mut Option<CodingAgentSession>,
) -> Result<PromptTask, CliError> {
    let processed_prompt = input::process_at_file_references_with_processing_options(
        &prompt,
        &prompt_cwd(prompt_context),
        input::ImageProcessingOptions::from_settings(&prompt_context.settings),
    )?;
    let invocation = prompt_invocation_from_processed(&processed_prompt);
    let task_prompt = match &invocation {
        PromptInvocation::Text(text) => text.clone(),
        PromptInvocation::Content(_) => processed_prompt.text.clone(),
        _ => prompt.clone(),
    };

    {
        let root = root_mut(tui, root_id)?;
        root.push_user(prompt.clone());
        root.set_status(InteractiveStatus::Running);
    }

    let options = PromptRunOptions {
        model: prompt_context.model.clone(),
        api_key: prompt_context.api_key.clone(),
        auth_diagnostics: prompt_context.auth_diagnostics.clone(),
        system_prompt: prompt_context.system_prompt.clone(),
        max_turns: prompt_context.max_turns,
        tools: prompt_context.tools.clone(),
        register_builtins: prompt_context.register_builtins,
        session: prompt_context.session.clone(),
        session_target: prompt_context.session_target.clone(),
        session_name: prompt_context.session_name.clone(),
        thinking_level: prompt_context.thinking_level,
        tool_execution: prompt_context.tool_execution,
        resources: prompt_context.resources.clone(),
        settings: Some(prompt_context.settings.clone()),
        invocation,
        prompt: task_prompt,
    };

    let existing_session = coding_session.take();
    let task = PromptTask::spawn_prompt(
        options,
        existing_session,
        prompt_context.default_agent_profile_id.clone(),
    )?;
    if prompt_context.settings.terminal.show_progress {
        set_terminal_progress(tui, true)?;
    }
    Ok(task)
}

fn prompt_invocation_from_processed(processed_prompt: &ProcessedPromptInput) -> PromptInvocation {
    if processed_prompt.images.is_empty() {
        PromptInvocation::Text(processed_prompt.text.clone())
    } else {
        PromptInvocation::Content(processed_prompt.content.clone())
    }
}

fn prompt_cwd(prompt_context: &PromptContext) -> PathBuf {
    prompt_context
        .session
        .as_ref()
        .map(|session| session.cwd.clone())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn start_agent_invocation_task<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    request: PendingAgentInvocationRequest,
    prompt_context: &PromptContext,
    coding_session: &mut Option<CodingAgentSession>,
) -> Result<PromptTask, CliError> {
    {
        let root = root_mut(tui, root_id)?;
        root.push_user(format!("/agent:{} {}", request.profile_id, request.task));
        root.set_status(InteractiveStatus::Running);
    }

    let task_prompt = request.task.clone();
    let options = PromptRunOptions {
        prompt: task_prompt.clone(),
        model: prompt_context.model.clone(),
        api_key: prompt_context.api_key.clone(),
        auth_diagnostics: prompt_context.auth_diagnostics.clone(),
        system_prompt: prompt_context.system_prompt.clone(),
        max_turns: prompt_context.max_turns,
        tools: prompt_context.tools.clone(),
        register_builtins: prompt_context.register_builtins,
        session: prompt_context.session.clone(),
        session_target: prompt_context.session_target.clone(),
        session_name: prompt_context.session_name.clone(),
        thinking_level: prompt_context.thinking_level,
        tool_execution: prompt_context.tool_execution,
        resources: prompt_context.resources.clone(),
        settings: Some(prompt_context.settings.clone()),
        invocation: PromptInvocation::Text(task_prompt),
    };

    let task = PromptTask::spawn_agent_invocation(
        options,
        coding_session.take(),
        request.profile_id,
        request.task,
        prompt_context.default_agent_profile_id.clone(),
    )?;
    if prompt_context.settings.terminal.show_progress {
        set_terminal_progress(tui, true)?;
    }
    Ok(task)
}

fn start_agent_team_task<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    request: PendingAgentTeamRequest,
    prompt_context: &PromptContext,
    coding_session: &mut Option<CodingAgentSession>,
) -> Result<PromptTask, CliError> {
    {
        let root = root_mut(tui, root_id)?;
        root.push_user(format!("/team:{} {}", request.team_id, request.task));
        root.set_status(InteractiveStatus::Running);
    }

    let task_prompt = request.task.clone();
    let options = PromptRunOptions {
        prompt: task_prompt.clone(),
        model: prompt_context.model.clone(),
        api_key: prompt_context.api_key.clone(),
        auth_diagnostics: prompt_context.auth_diagnostics.clone(),
        system_prompt: prompt_context.system_prompt.clone(),
        max_turns: prompt_context.max_turns,
        tools: prompt_context.tools.clone(),
        register_builtins: prompt_context.register_builtins,
        session: prompt_context.session.clone(),
        session_target: prompt_context.session_target.clone(),
        session_name: prompt_context.session_name.clone(),
        thinking_level: prompt_context.thinking_level,
        tool_execution: prompt_context.tool_execution,
        resources: prompt_context.resources.clone(),
        settings: Some(prompt_context.settings.clone()),
        invocation: PromptInvocation::Text(task_prompt),
    };

    let task = PromptTask::spawn_agent_team(
        options,
        coding_session.take(),
        request.team_id,
        request.task,
        prompt_context.default_agent_profile_id.clone(),
    )?;
    if prompt_context.settings.terminal.show_progress {
        set_terminal_progress(tui, true)?;
    }
    Ok(task)
}

fn start_plugin_reload_task<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    prompt_context: &PromptContext,
    coding_session: &mut Option<CodingAgentSession>,
) -> Result<PromptTask, CliError> {
    {
        let root = root_mut(tui, root_id)?;
        root.transcript
            .push(TranscriptItem::system("Reloading plugins..."));
        root.set_status(InteractiveStatus::Running);
    }

    let options = PromptRunOptions {
        prompt: String::new(),
        model: prompt_context.model.clone(),
        api_key: prompt_context.api_key.clone(),
        auth_diagnostics: prompt_context.auth_diagnostics.clone(),
        system_prompt: prompt_context.system_prompt.clone(),
        max_turns: prompt_context.max_turns,
        tools: prompt_context.tools.clone(),
        register_builtins: prompt_context.register_builtins,
        session: prompt_context.session.clone(),
        session_target: prompt_context.session_target.clone(),
        session_name: prompt_context.session_name.clone(),
        thinking_level: prompt_context.thinking_level,
        tool_execution: prompt_context.tool_execution,
        resources: prompt_context.resources.clone(),
        settings: Some(prompt_context.settings.clone()),
        invocation: PromptInvocation::Text(String::new()),
    };

    let task = PromptTask::spawn_plugin_reload(
        options,
        coding_session.take(),
        prompt_context.default_agent_profile_id.clone(),
    )?;
    if prompt_context.settings.terminal.show_progress {
        set_terminal_progress(tui, true)?;
    }
    Ok(task)
}

fn interactive_self_healing_model_repair_options(
    prompt_context: &PromptContext,
    max_attempts: usize,
) -> SelfHealingEditModelRepairOptions {
    let prompt = "repair self-healing edit".to_string();
    let prompt_options = PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
        prompt: prompt.clone(),
        model: prompt_context.model.clone(),
        api_key: prompt_context.api_key.clone(),
        auth_diagnostics: prompt_context.auth_diagnostics.clone(),
        system_prompt: Some("Return only self-healing edit repair JSON.".to_string()),
        max_turns: Some(1),
        tools: prompt_context.tools.clone(),
        register_builtins: false,
        session: prompt_context.session.clone(),
        session_target: None,
        session_name: prompt_context.session_name.clone(),
        thinking_level: prompt_context.thinking_level,
        tool_execution: None,
        resources: AgentResources::default(),
        settings: Some(prompt_context.settings.clone()),
        invocation: PromptInvocation::Text(prompt),
    });
    SelfHealingEditModelRepairOptions::new(prompt_options).with_max_attempts(max_attempts)
}

fn start_self_healing_edit_task<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    request: PendingSelfHealingEditRequest,
    prompt_context: &PromptContext,
    coding_session: &mut Option<CodingAgentSession>,
) -> Result<PromptTask, CliError> {
    {
        let root = root_mut(tui, root_id)?;
        root.transcript.push(TranscriptItem::system(format!(
            "Applying self-healing edit: {}",
            request.path
        )));
        root.set_status(InteractiveStatus::Running);
    }

    let options = PromptRunOptions {
        prompt: String::new(),
        model: prompt_context.model.clone(),
        api_key: prompt_context.api_key.clone(),
        auth_diagnostics: prompt_context.auth_diagnostics.clone(),
        system_prompt: prompt_context.system_prompt.clone(),
        max_turns: prompt_context.max_turns,
        tools: prompt_context.tools.clone(),
        register_builtins: prompt_context.register_builtins,
        session: prompt_context.session.clone(),
        session_target: prompt_context.session_target.clone(),
        session_name: prompt_context.session_name.clone(),
        thinking_level: prompt_context.thinking_level,
        tool_execution: prompt_context.tool_execution,
        resources: prompt_context.resources.clone(),
        settings: Some(prompt_context.settings.clone()),
        invocation: PromptInvocation::Text(String::new()),
    };

    let mut edit_request = SelfHealingEditRequest::new(request.path, request.replacements);
    if let Some(command) = request.check_command {
        edit_request = edit_request.with_check_command(command);
    }
    if let Some(model_repair) = request.model_repair {
        edit_request =
            edit_request.with_model_repair(interactive_self_healing_model_repair_options(
                prompt_context,
                model_repair.max_attempts,
            ));
    }
    let task = PromptTask::spawn_self_healing_edit(
        options,
        coding_session.take(),
        edit_request,
        prompt_context.default_agent_profile_id.clone(),
    )?;
    if prompt_context.settings.terminal.show_progress {
        set_terminal_progress(tui, true)?;
    }
    Ok(task)
}

fn start_plugin_command_task<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    request: PendingPluginCommandRequest,
    prompt_context: &PromptContext,
    coding_session: &mut Option<CodingAgentSession>,
) -> Result<PromptTask, CliError> {
    {
        let root = root_mut(tui, root_id)?;
        root.transcript.push(TranscriptItem::system(format!(
            "Running plugin command: {}",
            request.command_id
        )));
        root.set_status(InteractiveStatus::Running);
    }

    let options = PromptRunOptions {
        prompt: String::new(),
        model: prompt_context.model.clone(),
        api_key: prompt_context.api_key.clone(),
        auth_diagnostics: prompt_context.auth_diagnostics.clone(),
        system_prompt: prompt_context.system_prompt.clone(),
        max_turns: prompt_context.max_turns,
        tools: prompt_context.tools.clone(),
        register_builtins: prompt_context.register_builtins,
        session: prompt_context.session.clone(),
        session_target: prompt_context.session_target.clone(),
        session_name: prompt_context.session_name.clone(),
        thinking_level: prompt_context.thinking_level,
        tool_execution: prompt_context.tool_execution,
        resources: prompt_context.resources.clone(),
        settings: Some(prompt_context.settings.clone()),
        invocation: PromptInvocation::Text(String::new()),
    };

    let task = PromptTask::spawn_plugin_command(
        options,
        coding_session.take(),
        request.command_id,
        request.args,
        prompt_context.default_agent_profile_id.clone(),
    )?;
    if prompt_context.settings.terminal.show_progress {
        set_terminal_progress(tui, true)?;
    }
    Ok(task)
}

fn start_compact_task<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    custom_instructions: Option<String>,
    prompt_context: &PromptContext,
    coding_session: &mut Option<CodingAgentSession>,
) -> Result<PromptTask, CliError> {
    let use_rust_native = {
        let root = root_mut(tui, root_id)?;
        matches!(
            root.active_session.as_ref().map(|choice| choice.kind),
            Some(SessionChoiceKind::RustNative)
        )
    };

    {
        let root = root_mut(tui, root_id)?;
        root.transcript
            .push(TranscriptItem::system("Compacting session..."));
        root.set_status(InteractiveStatus::Running);
    }

    let options = PromptRunOptions {
        prompt: String::new(),
        model: prompt_context.model.clone(),
        api_key: prompt_context.api_key.clone(),
        auth_diagnostics: prompt_context.auth_diagnostics.clone(),
        system_prompt: prompt_context.system_prompt.clone(),
        max_turns: prompt_context.max_turns,
        tools: prompt_context.tools.clone(),
        register_builtins: prompt_context.register_builtins,
        session: prompt_context.session.clone(),
        session_target: prompt_context.session_target.clone(),
        session_name: prompt_context.session_name.clone(),
        thinking_level: prompt_context.thinking_level,
        tool_execution: prompt_context.tool_execution,
        resources: prompt_context.resources.clone(),
        settings: Some(prompt_context.settings.clone()),
        invocation: PromptInvocation::Compact {
            custom_instructions,
        },
    };

    if !use_rust_native {
        return Err(CliError::UnsupportedMode(
            "manual compaction requires an active Rust-native session".into(),
        ));
    }
    let task = PromptTask::spawn_compact(
        options,
        coding_session.take(),
        prompt_context.default_agent_profile_id.clone(),
    )?;
    if prompt_context.settings.terminal.show_progress {
        set_terminal_progress(tui, true)?;
    }
    Ok(task)
}

fn start_fork_task<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    request: PendingForkRequest,
    prompt_context: &PromptContext,
    running: &mut Option<PromptTask>,
    coding_session: &mut Option<CodingAgentSession>,
) -> Result<(), CliError> {
    if coding_session.is_none() {
        root_mut(tui, root_id)?
            .transcript
            .push(TranscriptItem::system("No active coding session."));
        return Ok(());
    }
    {
        let root = root_mut(tui, root_id)?;
        root.set_status(InteractiveStatus::Running);
    }
    let options = PromptRunOptions {
        prompt: String::new(),
        model: prompt_context.model.clone(),
        api_key: prompt_context.api_key.clone(),
        auth_diagnostics: prompt_context.auth_diagnostics.clone(),
        system_prompt: prompt_context.system_prompt.clone(),
        max_turns: prompt_context.max_turns,
        tools: prompt_context.tools.clone(),
        register_builtins: prompt_context.register_builtins,
        session: prompt_context.session.clone(),
        session_target: prompt_context.session_target.clone(),
        session_name: prompt_context.session_name.clone(),
        thinking_level: prompt_context.thinking_level,
        tool_execution: prompt_context.tool_execution,
        resources: prompt_context.resources.clone(),
        settings: Some(prompt_context.settings.clone()),
        invocation: PromptInvocation::Text(String::new()),
    };
    *running = Some(PromptTask::spawn_fork_session(
        options,
        coding_session.take(),
        request.target_leaf_id,
        prompt_context.default_agent_profile_id.clone(),
    )?);
    if prompt_context.settings.terminal.show_progress {
        set_terminal_progress(tui, true)?;
    }
    Ok(())
}

fn start_branch_summary_navigation_task<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    source_leaf_id: String,
    target_leaf_id: String,
    prompt_context: &PromptContext,
    coding_session: &mut Option<CodingAgentSession>,
) -> Result<PromptTask, CliError> {
    {
        let root = root_mut(tui, root_id)?;
        root.transcript.push(TranscriptItem::system(
            "Summarizing branch before navigation...",
        ));
        root.set_status(InteractiveStatus::Running);
    }

    let options = PromptRunOptions {
        prompt: String::new(),
        model: prompt_context.model.clone(),
        api_key: prompt_context.api_key.clone(),
        auth_diagnostics: prompt_context.auth_diagnostics.clone(),
        system_prompt: prompt_context.system_prompt.clone(),
        max_turns: prompt_context.max_turns,
        tools: prompt_context.tools.clone(),
        register_builtins: prompt_context.register_builtins,
        session: prompt_context.session.clone(),
        session_target: prompt_context.session_target.clone(),
        session_name: prompt_context.session_name.clone(),
        thinking_level: prompt_context.thinking_level,
        tool_execution: prompt_context.tool_execution,
        resources: prompt_context.resources.clone(),
        settings: Some(prompt_context.settings.clone()),
        invocation: PromptInvocation::Text(String::new()),
    };

    let task = PromptTask::spawn_branch_summary_navigation(
        options,
        coding_session.take(),
        source_leaf_id,
        target_leaf_id,
        prompt_context.default_agent_profile_id.clone(),
    )?;
    if prompt_context.settings.terminal.show_progress {
        set_terminal_progress(tui, true)?;
    }
    Ok(task)
}

fn start_branch_summary_task<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    request: PendingBranchSummaryRequest,
    prompt_context: &PromptContext,
    coding_session: &mut Option<CodingAgentSession>,
) -> Result<PromptTask, CliError> {
    let use_rust_native = {
        let root = root_mut(tui, root_id)?;
        matches!(
            root.active_session.as_ref().map(|choice| choice.kind),
            Some(SessionChoiceKind::RustNative)
        )
    };

    {
        let root = root_mut(tui, root_id)?;
        root.transcript
            .push(TranscriptItem::system("Summarizing branch..."));
        root.set_status(InteractiveStatus::Running);
    }

    let options = PromptRunOptions {
        prompt: String::new(),
        model: prompt_context.model.clone(),
        api_key: prompt_context.api_key.clone(),
        auth_diagnostics: prompt_context.auth_diagnostics.clone(),
        system_prompt: prompt_context.system_prompt.clone(),
        max_turns: prompt_context.max_turns,
        tools: prompt_context.tools.clone(),
        register_builtins: prompt_context.register_builtins,
        session: prompt_context.session.clone(),
        session_target: prompt_context.session_target.clone(),
        session_name: prompt_context.session_name.clone(),
        thinking_level: prompt_context.thinking_level,
        tool_execution: prompt_context.tool_execution,
        resources: prompt_context.resources.clone(),
        settings: Some(prompt_context.settings.clone()),
        invocation: PromptInvocation::Text(String::new()),
    };

    if !use_rust_native {
        return Err(CliError::UnsupportedMode(
            "branch summary requires an active Rust-native session".into(),
        ));
    }
    let task = PromptTask::spawn_branch_summary(
        options,
        coding_session.take(),
        request.source_leaf_id,
        request.target_leaf_id,
        request.custom_instructions,
        prompt_context.default_agent_profile_id.clone(),
    )?;
    if prompt_context.settings.terminal.show_progress {
        set_terminal_progress(tui, true)?;
    }
    Ok(task)
}

fn apply_prompt_task_event<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    ui_projection: &mut UiProjection,
    event: PromptTaskEvent,
) -> Result<RenderRequest, CliError> {
    match event {
        PromptTaskEvent::Snapshot(snapshot) => {
            *ui_projection = UiProjection::from_snapshot(snapshot);
        }
        PromptTaskEvent::Coding(event) => {
            ui_projection.apply_product_event(&event);
        }
    }
    let ui_events = ui_projection.drain();
    let force_render = ui_events.iter().any(ui_event_updates_visible_block);
    let root = root_mut(tui, root_id)?;
    let before = root.render_state();
    root.apply_events(ui_events);
    let after = root.render_state();
    let changed = before != after;
    Ok(if changed && force_render {
        RenderRequest::FORCE
    } else {
        RenderRequest::changed(changed)
    })
}

fn ui_event_updates_visible_block(event: &UiEvent) -> bool {
    matches!(
        event,
        UiEvent::AssistantDelta { .. }
            | UiEvent::ThinkingDelta { .. }
            | UiEvent::AssistantDone
            | UiEvent::ToolStarted { .. }
            | UiEvent::ToolFinished { .. }
            | UiEvent::ToolUpdated { .. }
            | UiEvent::AgentError { .. }
            | UiEvent::SystemNotice { .. }
            | UiEvent::DelegationBlock { .. }
            | UiEvent::CompactionNotice { .. }
    )
}

fn finish_prompt<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    result: Result<PromptTaskResult, CliError>,
    coding_session: &mut Option<CodingAgentSession>,
) -> Result<(), CliError> {
    if root_settings_show_progress(tui, root_id)? {
        set_terminal_progress(tui, false)?;
    }
    let root = root_mut(tui, root_id)?;
    match result {
        Ok(PromptTaskResult::Coding(result)) => {
            let completion_notice = result.completion_notice.clone();
            if result.hydrate_transcript {
                if let Ok(Some(hydration)) = result.session.hydrate_current() {
                    root.apply_hydrated_session(
                        hydrated_session_from_rust_native(hydration),
                        completion_notice,
                    );
                } else {
                    finish_coding_prompt(root, &result.session, result.outcome);
                    if let Some(notice) = completion_notice {
                        root.transcript.push(TranscriptItem::system(notice));
                    }
                }
            } else {
                finish_coding_prompt(root, &result.session, result.outcome);
                if let Some(notice) = completion_notice {
                    root.transcript.push(TranscriptItem::system(notice));
                }
            }
            *coding_session = Some(result.session);
        }
        Ok(PromptTaskResult::AgentInvocation(result)) => {
            root.set_default_agent_profile_id(
                result.session.view().default_agent_profile_id.clone(),
            );
            if let Ok(Some(hydration)) = result.session.hydrate_current() {
                let hydrated = hydrated_session_from_rust_native(hydration);
                let mut choice = hydrated.choice;
                if choice.active_leaf_id.is_none() {
                    choice.active_leaf_id = root.active_leaf_id.clone();
                }
                root.set_active_session_choice(choice);
            }
            *coding_session = Some(result.session);
        }
        Ok(PromptTaskResult::AgentTeam(result)) => {
            let _final_text = &result.outcome.final_text;
            root.set_default_agent_profile_id(
                result.session.view().default_agent_profile_id.clone(),
            );
            if let Ok(Some(hydration)) = result.session.hydrate_current() {
                let hydrated = hydrated_session_from_rust_native(hydration);
                let mut choice = hydrated.choice;
                if choice.active_leaf_id.is_none() {
                    choice.active_leaf_id = root.active_leaf_id.clone();
                }
                root.set_active_session_choice(choice);
            }
            *coding_session = Some(result.session);
        }
        Ok(PromptTaskResult::DelegationApproval(result)) => {
            root.set_default_agent_profile_id(
                result.session.view().default_agent_profile_id.clone(),
            );
            if let Ok(Some(hydration)) = result.session.hydrate_current() {
                let hydrated = hydrated_session_from_rust_native(hydration);
                let mut choice = hydrated.choice;
                if choice.active_leaf_id.is_none() {
                    choice.active_leaf_id = root.active_leaf_id.clone();
                }
                root.set_active_session_choice(choice);
            }
            *coding_session = Some(result.session);
        }
        Ok(PromptTaskResult::SetDefaultAgentProfile(result)) => {
            root.set_default_agent_profile_id(
                result.session.view().default_agent_profile_id.clone(),
            );
            if let Ok(Some(hydration)) = result.session.hydrate_current() {
                let hydrated = hydrated_session_from_rust_native(hydration);
                let mut choice = hydrated.choice;
                if choice.active_leaf_id.is_none() {
                    choice.active_leaf_id = root.active_leaf_id.clone();
                }
                root.set_active_session_choice(choice);
            }
            *coding_session = Some(result.session);
        }
        Ok(PromptTaskResult::DelegationRejection(result)) => {
            if let Some(notice) = result.fallback_notice {
                root.transcript.push(TranscriptItem::system(notice));
            }
            root.set_default_agent_profile_id(
                result.session.view().default_agent_profile_id.clone(),
            );
            if let Ok(Some(hydration)) = result.session.hydrate_current() {
                let hydrated = hydrated_session_from_rust_native(hydration);
                let mut choice = hydrated.choice;
                if choice.active_leaf_id.is_none() {
                    choice.active_leaf_id = root.active_leaf_id.clone();
                }
                root.set_active_session_choice(choice);
            }
            *coding_session = Some(result.session);
        }
        Ok(PromptTaskResult::SelfHealingEdit(result)) => {
            root.transcript
                .push(TranscriptItem::system(result.outcome.message.clone()));
            for diagnostic in &result.outcome.diagnostics {
                root.transcript
                    .push(TranscriptItem::system(diagnostic.message.clone()));
            }
            root.set_default_agent_profile_id(
                result.session.view().default_agent_profile_id.clone(),
            );
            if let Ok(Some(hydration)) = result.session.hydrate_current() {
                let hydrated = hydrated_session_from_rust_native(hydration);
                let mut choice = hydrated.choice;
                if choice.active_leaf_id.is_none() {
                    choice.active_leaf_id = root.active_leaf_id.clone();
                }
                root.set_active_session_choice(choice);
            }
            *coding_session = Some(result.session);
        }
        Ok(PromptTaskResult::PluginReload(result)) => {
            for notice in plugin_reload_notice_lines(&result.outcome) {
                root.transcript.push(TranscriptItem::system(notice));
            }
            root.set_plugin_commands(result.plugin_commands.clone());
            root.set_plugin_ui_extensions(
                result.plugin_ui_actions.clone(),
                result.plugin_keybindings.clone(),
                result.plugin_ui_dialogs.clone(),
            );
            *coding_session = Some(result.session);
        }
        Ok(PromptTaskResult::PluginCommand(result)) => {
            root.transcript.push(TranscriptItem::system(format!(
                "Plugin command {}: {}",
                result.command_id, result.output
            )));
            root.set_plugin_commands(result.plugin_commands.clone());
            root.set_plugin_ui_extensions(
                result.plugin_ui_actions.clone(),
                result.plugin_keybindings.clone(),
                result.plugin_ui_dialogs.clone(),
            );
            *coding_session = Some(result.session);
        }
        Ok(PromptTaskResult::ForkSession(result)) => {
            let completion_notice = result.completion_notice.clone();
            if result.hydrate_transcript {
                if let Ok(Some(hydration)) = result.session.hydrate_current() {
                    root.apply_hydrated_session(
                        hydrated_session_from_rust_native(hydration),
                        completion_notice,
                    );
                } else if let Some(notice) = completion_notice {
                    root.transcript.push(TranscriptItem::system(notice));
                }
            } else if let Some(notice) = completion_notice {
                root.transcript.push(TranscriptItem::system(notice));
            }
            root.set_default_agent_profile_id(
                result.session.view().default_agent_profile_id.clone(),
            );
            *coding_session = Some(result.session);
        }
        Err(error) => {
            root.apply_events(vec![UiEvent::AgentError {
                error: error.to_string(),
            }]);
        }
    }
    root.set_status(InteractiveStatus::Idle);
    Ok(())
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::coding_session::{
        CapabilityStatus, CodingAgentCapabilities, CodingAgentEvent, CodingAgentSession,
        CodingAgentSessionOptions, CodingAgentSessionView, ProductEvent, ProductEventSequence,
        ProfileId, UiSnapshot, UiSnapshotCursor,
    };
    use pi_ai::types::Usage;
    use pi_tui::VirtualTerminal;

    fn test_tui() -> (Tui<VirtualTerminal>, usize) {
        let mut tui = Tui::new(VirtualTerminal::new(80, 24));
        let root_id = tui.add_child_with_id(Box::new(InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "session".to_string(),
        )));
        (tui, root_id)
    }

    fn prompt_event(event: CodingAgentEvent) -> PromptTaskEvent {
        prompt_event_with_sequence(ProductEventSequence::new(1), event)
    }

    fn prompt_event_with_sequence(
        sequence: ProductEventSequence,
        event: CodingAgentEvent,
    ) -> PromptTaskEvent {
        PromptTaskEvent::Coding(ProductEvent::from_compat_event(sequence, event))
    }

    fn capabilities() -> CodingAgentCapabilities {
        CodingAgentCapabilities {
            prompt: CapabilityStatus::Available,
            abort: CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            },
            steer: CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            },
            follow_up: CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            },
            compact: CapabilityStatus::Available,
            fork: CapabilityStatus::Available,
            clone_session: CapabilityStatus::Available,
            branch_summary: CapabilityStatus::Available,
            switch_session: CapabilityStatus::Unsupported {
                reason: "session switching is not exposed on CodingAgentSession yet".into(),
            },
            export: CapabilityStatus::Available,
            plugin_reload: CapabilityStatus::Available,
            self_healing_edit: CapabilityStatus::Available,
            agent_profiles: CapabilityStatus::Available,
            team_profiles: CapabilityStatus::Available,
            delegation: CapabilityStatus::Available,
            tools: CapabilityStatus::Available,
            shell: CapabilityStatus::Available,
            plugins: CapabilityStatus::Available,
        }
    }

    async fn snapshot(last_event_sequence: ProductEventSequence, session_id: &str) -> UiSnapshot {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let base = session.ui_snapshot(Vec::new());
        UiSnapshot::new(
            UiSnapshotCursor {
                last_event_sequence,
                capability_generation: base.cursor.capability_generation,
            },
            base.version,
            CodingAgentSessionView {
                session_id: session_id.into(),
                default_agent_profile_id: ProfileId::from("default"),
            },
            capabilities(),
            None,
            Vec::new(),
        )
    }

    fn base_assistant_delta() -> CodingAgentEvent {
        CodingAgentEvent::AssistantMessageDelta {
            operation_id: "op_1".to_string(),
            turn_id: "turn_1".to_string(),
            message_id: Some("msg_1".to_string()),
            text: "hello".to_string(),
        }
    }

    fn tool_started() -> CodingAgentEvent {
        CodingAgentEvent::ToolCallStarted {
            operation_id: "op_1".to_string(),
            turn_id: "turn_1".to_string(),
            tool_call_id: "tool_1".to_string(),
            name: "read".to_string(),
            arguments_json: "{}".to_string(),
        }
    }

    fn assert_event_forces_render(setup: Vec<CodingAgentEvent>, event: CodingAgentEvent) {
        let mut projection = UiProjection::new();
        let (mut tui, root_id) = test_tui();
        let mut sequence = ProductEventSequence::new(1);
        for setup_event in setup {
            apply_prompt_task_event(
                &mut tui,
                root_id,
                &mut projection,
                prompt_event_with_sequence(sequence, setup_event),
            )
            .unwrap();
            sequence = sequence.next();
        }

        let request = apply_prompt_task_event(
            &mut tui,
            root_id,
            &mut projection,
            prompt_event_with_sequence(sequence, event),
        )
        .unwrap();

        assert_eq!(
            request,
            RenderRequest::FORCE,
            "transcript block events should flush the footer immediately"
        );
    }

    #[test]
    fn prompt_block_events_request_forced_render() {
        let cases = vec![
            (Vec::new(), base_assistant_delta()),
            (
                Vec::new(),
                CodingAgentEvent::AssistantThinkingDelta {
                    operation_id: "op_1".to_string(),
                    turn_id: "turn_1".to_string(),
                    message_id: Some("msg_1".to_string()),
                    text: "thinking".to_string(),
                },
            ),
            (
                vec![base_assistant_delta()],
                CodingAgentEvent::AssistantMessageCompleted {
                    operation_id: "op_1".to_string(),
                    turn_id: "turn_1".to_string(),
                    message_id: Some("msg_1".to_string()),
                    final_text: "hello".to_string(),
                    usage: Usage::default(),
                },
            ),
            (Vec::new(), tool_started()),
            (
                vec![tool_started()],
                CodingAgentEvent::ToolCallUpdated {
                    operation_id: "op_1".to_string(),
                    turn_id: "turn_1".to_string(),
                    tool_call_id: "tool_1".to_string(),
                    name: "read".to_string(),
                    message: "partial".to_string(),
                },
            ),
            (
                vec![tool_started()],
                CodingAgentEvent::ToolCallCompleted {
                    operation_id: "op_1".to_string(),
                    turn_id: "turn_1".to_string(),
                    tool_call_id: "tool_1".to_string(),
                    name: "read".to_string(),
                    summary: "done".to_string(),
                },
            ),
            (
                vec![tool_started()],
                CodingAgentEvent::ToolCallFailed {
                    operation_id: "op_1".to_string(),
                    turn_id: "turn_1".to_string(),
                    tool_call_id: "tool_1".to_string(),
                    name: "read".to_string(),
                    message: "failed".to_string(),
                },
            ),
            (
                Vec::new(),
                CodingAgentEvent::DelegationRequested {
                    operation_id: "op_1".to_string(),
                    turn_id: "turn_1".to_string(),
                    tool_call_id: "tool_1".to_string(),
                    requesting_profile_id: crate::coding_session::ProfileId::from("planner"),
                    target_kind: crate::coding_session::ProfileKind::Agent,
                    target_id: crate::coding_session::ProfileId::from("coder"),
                    task: "help".to_string(),
                },
            ),
        ];

        for (setup, event) in cases {
            assert_event_forces_render(setup, event);
        }
    }

    #[test]
    fn prompt_events_without_visible_ui_do_not_request_render() {
        let mut projection = UiProjection::new();
        let (mut tui, root_id) = test_tui();

        let request = apply_prompt_task_event(
            &mut tui,
            root_id,
            &mut projection,
            prompt_event(CodingAgentEvent::AssistantMessageStarted {
                operation_id: "op_1".to_string(),
                turn_id: "turn_1".to_string(),
                message_id: Some("msg_1".to_string()),
            }),
        )
        .unwrap();

        assert_eq!(request, RenderRequest::NONE);
    }

    #[tokio::test]
    async fn prompt_task_snapshots_do_not_append_visible_restore_notices() {
        let mut projection = UiProjection::new();
        let (mut tui, root_id) = test_tui();
        let before_items = root_ref(&tui, root_id).unwrap().transcript.items().len();

        let first = apply_prompt_task_event(
            &mut tui,
            root_id,
            &mut projection,
            PromptTaskEvent::Snapshot(snapshot(ProductEventSequence::new(7), "sess_loop").await),
        )
        .unwrap();
        let second = apply_prompt_task_event(
            &mut tui,
            root_id,
            &mut projection,
            PromptTaskEvent::Snapshot(snapshot(ProductEventSequence::new(7), "sess_loop").await),
        )
        .unwrap();

        let root = root_ref(&tui, root_id).unwrap();
        assert_eq!(first, RenderRequest::NONE);
        assert_eq!(second, RenderRequest::NONE);
        assert_eq!(root.transcript.items().len(), before_items);
        assert!(
            !root
                .transcript
                .items()
                .iter()
                .any(|item| matches!(item, TranscriptItem::System { text } if text.contains("Restored session")))
        );
    }

    #[test]
    fn interactive_loop_sync_delegation_rejection_uses_product_event_stream_boundary() {
        let source = include_str!("loop.rs");
        let product_subscription = [".", "subscribe_product_events()"].concat();
        let compatibility_subscription = [".", "subscribe()"].concat();
        let product_projection = "UiProjection::new()";

        assert!(source.contains(&product_subscription));
        assert!(!source.contains(&compatibility_subscription));
        assert!(source.contains(product_projection));
    }
}

fn plugin_reload_notice_lines(outcome: &CodingAgentPluginLoadOutcome) -> Vec<String> {
    let loaded = outcome.loaded_plugin_ids.len();
    let diagnostics = outcome.diagnostics.len();
    let mut lines = vec![format!(
        "Reloaded plugins: {loaded} loaded, {diagnostics} diagnostics"
    )];
    for diagnostic in &outcome.diagnostics {
        let message = match diagnostic.plugin_id.as_deref() {
            Some(plugin_id) => format!("{plugin_id}: {}", diagnostic.message),
            None => diagnostic.message.clone(),
        };
        lines.push(message);
    }
    lines
}

fn finish_coding_prompt(
    root: &mut InteractiveRoot,
    session: &CodingAgentSession,
    outcome: PromptTurnOutcome,
) {
    root.set_default_agent_profile_id(session.view().default_agent_profile_id.clone());
    root.clear_active_session();
    match outcome {
        PromptTurnOutcome::Success {
            session_id,
            leaf_id,
            ..
        } => {
            if let Some(session_id) = session_id {
                root.session_label = session_id;
                root.active_leaf_id = leaf_id;
            }
        }
        PromptTurnOutcome::Aborted { session_id, .. } => {
            if let Some(session_id) = session_id {
                root.session_label = session_id;
            }
        }
        PromptTurnOutcome::Failed { .. } => {}
    }
    if let Ok(Some(hydration)) = session.hydrate_current() {
        let hydrated = hydrated_session_from_rust_native(hydration);
        let mut choice = hydrated.choice;
        if choice.active_leaf_id.is_none() {
            choice.active_leaf_id = root.active_leaf_id.clone();
        }
        root.set_active_session_choice(choice);
    }
}

fn set_terminal_progress<T: Terminal>(tui: &mut Tui<T>, active: bool) -> Result<(), CliError> {
    tui.terminal_mut()
        .set_progress(active)
        .map_err(to_cli_error)
}

fn root_settings_show_progress<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
) -> Result<bool, CliError> {
    Ok(root_mut(tui, root_id)?.settings.terminal.show_progress)
}

fn render_tui<T: Terminal>(tui: &mut Tui<T>) -> Result<(), CliError> {
    tui.render_once().map(drop).map_err(tui_error)
}

fn root_mut<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
) -> Result<&mut InteractiveRoot, CliError> {
    tui.component_as_mut::<InteractiveRoot>(root_id)
        .ok_or_else(|| CliError::AgentFailure("interactive root component missing".to_string()))
}

fn root_ref<T: Terminal>(tui: &Tui<T>, root_id: usize) -> Result<&InteractiveRoot, CliError> {
    tui.component_as::<InteractiveRoot>(root_id)
        .ok_or_else(|| CliError::AgentFailure("interactive root component missing".to_string()))
}

fn tui_error(error: TuiError) -> CliError {
    CliError::AgentFailure(error.to_string())
}

/// Apply a hot-reloaded theme to the root component, mirroring TS
/// `setGlobalTheme(reloadedTheme)` + `onThemeChange` (UI invalidate).
fn apply_theme_reload<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    reload: crate::theme::ThemeReloadSignal,
) {
    if let Some(root) = tui.component_as_mut::<InteractiveRoot>(root_id) {
        root.apply_theme_reload(reload.name, reload.theme);
    }
}

fn to_cli_error(error: std::io::Error) -> CliError {
    CliError::AgentFailure(error.to_string())
}
