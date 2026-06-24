use std::path::PathBuf;
use std::time::{Duration, Instant};

use pi_agent_core::session::create_session_id;
use pi_tui::{
    Component, InputEvent, RenderScheduler, StdinBuffer, Terminal, Tui, TuiError, is_key_release,
};

use crate::interactive::app::{
    PromptContext, build_prompt_context, resolve_prompt_api_key, session_label,
};
use crate::interactive::input::InputPump;
use crate::interactive::prompt_task::PromptTask;
use crate::interactive::root::{InteractiveAction, InteractiveRoot, InteractiveStatus};
use crate::interactive::{InteractiveEventBridge, TranscriptItem, UiEvent};
use crate::protocol::session_runner::{
    SessionPromptOptions, SessionPromptResult, spawn_session_prompt,
};
use crate::runtime::PromptInvocation;
use crate::session::ResolvedSessionTarget;
use crate::{CliArgs, CliError, CliRunOptions};

const NORMAL_RENDER_INTERVAL: Duration = Duration::from_millis(16);
const SPINNER_INTERVAL: Duration = Duration::from_millis(120);

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

pub(super) async fn run_interactive_loop<T: Terminal>(
    parsed: CliArgs,
    options: CliRunOptions,
    mut terminal: T,
    input: &mut InputPump,
) -> Result<LoopResult<T>, CliError> {
    let prompt_context = build_prompt_context(&parsed, options.clone())?;

    terminal.start().map_err(to_cli_error)?;
    let (mut tui, root_id) = initialize_started_tui(terminal, &prompt_context)?;

    let loop_result =
        run_started_interactive_loop(&mut tui, root_id, input, prompt_context, &parsed, &options)
            .await;
    let stop_result = tui.terminal_mut().stop().map_err(to_cli_error);
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

    terminal.start().map_err(to_cli_error)?;
    let mut input = make_input();
    let (mut tui, root_id) = initialize_started_tui(terminal, &prompt_context)?;

    let loop_result = run_started_interactive_loop(
        &mut tui,
        root_id,
        &mut input,
        prompt_context,
        &parsed,
        &options,
    )
    .await;
    let stop_result = tui.terminal_mut().stop().map_err(to_cli_error);
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
    let root_id = tui.add_child_with_id(Box::new(InteractiveRoot::new_with_theme_and_models(
        cwd,
        prompt_context.model.id.clone(),
        session_label,
        prompt_context.theme.clone(),
        prompt_context.model_choices.clone(),
    )));
    {
        let root = root_mut(&mut tui, root_id)?;
        root.model_rotation = prompt_context.model_rotation.clone();
        root.session_choices = prompt_context.session_choices.clone();
    }
    tui.set_focus(Some(root_id));
    Ok((tui, root_id))
}

async fn run_started_interactive_loop<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    input: &mut InputPump,
    mut prompt_context: PromptContext,
    parsed: &CliArgs,
    options: &CliRunOptions,
) -> Result<i32, CliError> {
    let mut stdin_buffer = StdinBuffer::new();
    let mut running: Option<PromptTask> = None;
    let mut bridge = InteractiveEventBridge::new();
    let mut input_open = true;
    let mut render_scheduler = RenderScheduler::new(NORMAL_RENDER_INTERVAL);
    render_scheduler.request(true);
    flush_render_if_ready(tui, &mut render_scheduler)?;

    loop {
        flush_render_if_ready(tui, &mut render_scheduler)?;
        if let Some(mut task) = running.take() {
            let render_delay = pending_render_delay(&render_scheduler);
            let stdin_delay = stdin_pending_delay(&stdin_buffer);
            tokio::select! {
                _ = sleep_render_delay(render_delay), if render_delay.is_some() => {
                    flush_render_if_ready(tui, &mut render_scheduler)?;
                    running = Some(task);
                }
                _ = sleep_stdin_pending(stdin_delay), if stdin_delay.is_some() => {
                    running = Some(task);
                    let events = stdin_buffer.tick(Instant::now());
                    if !events.is_empty() {
                        match process_input_events(
                            tui,
                            root_id,
                            events,
                            &mut prompt_context,
                            &mut running,
                            &mut render_scheduler,
                            parsed,
                            options,
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
                                stdin_buffer.process(&chunk),
                                &mut prompt_context,
                                &mut running,
                                &mut render_scheduler,
                                parsed,
                                options,
                            )? {
                                LoopControl::Continue(_) => {}
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
                                apply_agent_event(tui, root_id, &mut bridge, event)?,
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
                            apply_agent_event(tui, root_id, &mut bridge, event)?,
                        );
                    }
                    finish_prompt(tui, root_id, result)?;
                    schedule_render(&mut render_scheduler, RenderRequest::FORCE);
                    flush_render_if_ready(tui, &mut render_scheduler)?;
                    running = None;
                }
            }
        } else {
            if !input_open {
                flush_pending_render(tui, &mut render_scheduler)?;
                return Ok(0);
            }

            let render_delay = pending_render_delay(&render_scheduler);
            let stdin_delay = stdin_pending_delay(&stdin_buffer);
            tokio::select! {
                _ = sleep_render_delay(render_delay), if render_delay.is_some() => {
                    flush_render_if_ready(tui, &mut render_scheduler)?;
                }
                _ = sleep_stdin_pending(stdin_delay), if stdin_delay.is_some() => {
                    let events = stdin_buffer.tick(Instant::now());
                    if !events.is_empty() {
                        match process_input_events(
                            tui,
                            root_id,
                            events,
                            &mut prompt_context,
                            &mut running,
                            &mut render_scheduler,
                            parsed,
                            options,
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
                            &mut render_scheduler,
                            parsed,
                            options,
                        )? {
                            LoopControl::Continue(_) => {}
                            LoopControl::Exit => return Ok(0),
                        }
                        if running.is_none() {
                            flush_pending_render(tui, &mut render_scheduler)?;
                            return Ok(0);
                        }
                        tokio::task::yield_now().await;
                        continue;
                    };

                    match process_input_events(
                        tui,
                        root_id,
                        stdin_buffer.process(&chunk),
                        &mut prompt_context,
                        &mut running,
                        &mut render_scheduler,
                        parsed,
                        options,
                    )? {
                        LoopControl::Continue(_) => {}
                        LoopControl::Exit => return Ok(0),
                    }
                    if running.is_some() {
                        tokio::task::yield_now().await;
                    }
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
    render_scheduler: &mut RenderScheduler,
    parsed: &CliArgs,
    options: &CliRunOptions,
) -> Result<LoopControl, CliError> {
    for event in events {
        match handle_input_event(
            tui,
            root_id,
            event,
            prompt_context,
            running,
            parsed,
            options,
        )? {
            LoopControl::Continue(request) => {
                schedule_render(render_scheduler, request);
                flush_render_if_ready(tui, render_scheduler)?;
            }
            LoopControl::Exit => return Ok(LoopControl::Exit),
        }
        if running.is_some() {
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

fn pending_render_delay(render_scheduler: &RenderScheduler) -> Option<Duration> {
    let now = Instant::now();
    render_scheduler
        .next_render_at(now)
        .map(|deadline| deadline.saturating_duration_since(now))
}

async fn sleep_render_delay(delay: Option<Duration>) {
    if let Some(delay) = delay {
        tokio::time::sleep(delay).await;
    }
}

fn stdin_pending_delay(stdin_buffer: &StdinBuffer) -> Option<Duration> {
    stdin_buffer.pending_timeout_at(Instant::now())
}

async fn sleep_stdin_pending(delay: Option<Duration>) {
    if let Some(delay) = delay {
        tokio::time::sleep(delay).await;
    }
}

fn flush_render_if_ready<T: Terminal>(
    tui: &mut Tui<T>,
    render_scheduler: &mut RenderScheduler,
) -> Result<(), CliError> {
    let now = Instant::now();
    if render_scheduler.should_render_now(now) {
        render_tui(tui)?;
        render_scheduler.mark_rendered(now);
    }
    Ok(())
}

fn flush_pending_render<T: Terminal>(
    tui: &mut Tui<T>,
    render_scheduler: &mut RenderScheduler,
) -> Result<(), CliError> {
    if render_scheduler.has_pending() {
        render_scheduler.request(true);
        flush_render_if_ready(tui, render_scheduler)?;
    }
    Ok(())
}

fn handle_input_event<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    event: InputEvent,
    prompt_context: &mut PromptContext,
    running: &mut Option<PromptTask>,
    parsed: &CliArgs,
    options: &CliRunOptions,
) -> Result<LoopControl, CliError> {
    if is_key_release(&event) {
        return Ok(LoopControl::Continue(RenderRequest::NONE));
    }

    let (action, prompt, selected_model, selected_thinking_level, selected_session, render_request) = {
        let root = root_mut(tui, root_id)?;
        let before = root.render_state();
        root.handle_input(&event);
        let action = root.take_action();
        let prompt = if action == InteractiveAction::Submit {
            root.take_pending_submit()
        } else {
            None
        };
        let selected_model = root.take_selected_model();
        let selected_thinking_level = root.take_selected_thinking_level();
        let selected_session = root.take_selected_session();
        let after = root.render_state();
        (
            action,
            prompt,
            selected_model,
            selected_thinking_level,
            selected_session,
            RenderRequest::changed(before != after),
        )
    };

    if let Some(model) = selected_model {
        let (api_key, diagnostics) = resolve_prompt_api_key(
            &model.provider,
            prompt_context.cli_api_key.as_deref(),
            &prompt_context.auth,
        );
        let diagnostic_text = crate::request::render_diagnostics(&diagnostics);
        if !diagnostic_text.is_empty() {
            eprint!("{diagnostic_text}");
        }
        prompt_context.api_key = api_key;
        prompt_context.model = model;
    }
    if let Some(thinking_level) = selected_thinking_level {
        prompt_context.thinking_level = Some(thinking_level);
    }
    if let Some(session) = selected_session {
        prompt_context.session_target = Some(ResolvedSessionTarget::OpenTarget(
            session.path.display().to_string(),
        ));
        prompt_context.session_name = session.name.clone();
    }

    match action {
        InteractiveAction::None => Ok(LoopControl::Continue(render_request)),
        InteractiveAction::Exit => Ok(LoopControl::Exit),
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
                prompt_context.session_target =
                    Some(ResolvedSessionTarget::OpenOrCreateId(create_session_id()));
                prompt_context.session_name = None;
            }
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
        InteractiveAction::ReloadResources => {
            match build_prompt_context(parsed, options.clone()) {
                Ok(reloaded) => {
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
                }
            }
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
        InteractiveAction::Submit => {
            if running.is_some() {
                return Ok(LoopControl::Continue(render_request));
            }
            let Some(prompt) = prompt else {
                return Ok(LoopControl::Continue(render_request));
            };
            if prompt.trim().is_empty() {
                return Ok(LoopControl::Continue(render_request));
            }
            *running = Some(start_prompt_task(tui, root_id, prompt, prompt_context)?);
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
    }
}

fn start_prompt_task<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    prompt: String,
    prompt_context: &PromptContext,
) -> Result<PromptTask, CliError> {
    {
        let root = root_mut(tui, root_id)?;
        root.push_user(prompt.clone());
        root.set_status(InteractiveStatus::Running);
    }

    let options = SessionPromptOptions {
        prompt: prompt.clone(),
        model: prompt_context.model.clone(),
        api_key: prompt_context.api_key.clone(),
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
        invocation: PromptInvocation::Text(prompt),
    };

    spawn_session_prompt(options).map(PromptTask::new)
}

fn apply_agent_event<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    bridge: &mut InteractiveEventBridge,
    event: pi_agent_core::AgentEvent,
) -> Result<RenderRequest, CliError> {
    let ui_events = bridge.handle(&event);
    let root = root_mut(tui, root_id)?;
    let before = root.render_state();
    root.apply_events(ui_events);
    let after = root.render_state();
    Ok(RenderRequest::changed(before != after))
}

fn finish_prompt<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    result: Result<SessionPromptResult, CliError>,
) -> Result<(), CliError> {
    let root = root_mut(tui, root_id)?;
    match result {
        Ok(result) => {
            root.active_session_path = result.session_path;
            root.active_leaf_id = result.leaf_id;
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

fn tui_error(error: TuiError) -> CliError {
    CliError::AgentFailure(error.to_string())
}

fn to_cli_error(error: std::io::Error) -> CliError {
    CliError::AgentFailure(error.to_string())
}
