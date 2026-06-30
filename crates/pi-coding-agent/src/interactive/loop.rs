use std::path::PathBuf;
use std::time::{Duration, Instant};

use pi_agent_core::session::{JsonlSessionStorage, create_session_id};
use pi_ai::types::Usage;
use pi_tui::{
    Component, InputEvent, RenderScheduler, StdinBuffer, Terminal, Tui, TuiError, is_key_release,
};

use crate::coding_session::{CodingAgentSession, PromptTurnOutcome};
use crate::input::{self, ProcessedPromptInput};
use crate::interactive::app::{
    PromptContext, build_prompt_context, resolve_prompt_api_key, session_label,
};
use crate::interactive::input::InputPump;
use crate::interactive::prompt_task::{PromptTask, PromptTaskEvent, PromptTaskResult};
use crate::interactive::root::{InteractiveAction, InteractiveRoot, InteractiveStatus};
use crate::interactive::session_actions::{
    hydrate_existing_session_target, session_choice_from_metadata,
};
use crate::interactive::{CodingEventBridge, InteractiveEventBridge, TranscriptItem, UiEvent};
use crate::protocol::session_runner::{SessionPromptOptions, SessionPromptResult};
use crate::runtime::PromptInvocation;
use crate::session::ResolvedSessionTarget;
use crate::{CliArgs, CliError, CliRunOptions};

const NORMAL_RENDER_INTERVAL: Duration = Duration::from_millis(16);
const SPINNER_INTERVAL: Duration = Duration::from_millis(120);

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
                // Session files are `{timestamp}_{uuid}.jsonl`;
                // the session id is the full stem.
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

pub(super) async fn run_interactive_loop<T: Terminal>(
    parsed: CliArgs,
    options: CliRunOptions,
    mut terminal: T,
    input: &mut InputPump,
) -> Result<LoopResult<T>, CliError> {
    let prompt_context = build_prompt_context(&parsed, options.clone())?;

    print_startup_banner(&prompt_context);

    terminal.start().map_err(to_cli_error)?;
    let (mut tui, root_id) = initialize_started_tui(terminal, &prompt_context)?;

    let loop_result =
        run_started_interactive_loop(&mut tui, root_id, input, prompt_context, &parsed, &options)
            .await;
    // Drain in-flight Kitty key release events before stopping, matching TS `drainInput(1000)`.
    let _ = tui
        .terminal_mut()
        .drain_input(Duration::from_millis(1000), Duration::from_millis(50));
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

    let loop_result = run_started_interactive_loop(
        &mut tui,
        root_id,
        &mut input,
        prompt_context,
        &parsed,
        &options,
    )
    .await;
    // Drain in-flight Kitty key release events before stopping.
    let _ = tui
        .terminal_mut()
        .drain_input(Duration::from_millis(1000), Duration::from_millis(50));
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
    let mut coding_session: Option<CodingAgentSession> = None;
    let mut agent_bridge = InteractiveEventBridge::new();
    let mut coding_bridge = CodingEventBridge::new();
    let mut input_open = true;
    let mut render_scheduler = RenderScheduler::new(NORMAL_RENDER_INTERVAL);
    render_scheduler.request(true);
    flush_render_if_ready(tui, &mut render_scheduler)?;

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
                            &mut coding_session,
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
                                &mut coding_session,
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
                                apply_prompt_task_event(
                                    tui,
                                    root_id,
                                    &mut agent_bridge,
                                    &mut coding_bridge,
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
                                &mut agent_bridge,
                                &mut coding_bridge,
                                event,
                            )?,
                        );
                    }
                    finish_prompt(tui, root_id, result, &mut coding_session)?;
                    schedule_render(&mut render_scheduler, RenderRequest::FORCE);
                    flush_render_if_ready(tui, &mut render_scheduler)?;
                    running = None;
                }
                Some(reload) = theme_reload.recv() => {
                    apply_theme_reload(tui, root_id, reload);
                    render_scheduler.request(true);
                    running = Some(task);
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
                            &mut coding_session,
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
                            &mut coding_session,
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
                        &mut coding_session,
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
) -> Result<LoopControl, CliError> {
    for event in events {
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
        selected_session,
        selected_session_hydrate,
        settings_update,
        auth_update,
        compact_instructions,
        render_request,
    ) = {
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
        let selected_session_hydrate = root.take_selected_session_hydrate();
        let settings_update = root.take_settings_update();
        let auth_update = root.take_auth_update();
        let compact_instructions = if action == InteractiveAction::CompactSession {
            root.take_pending_compact_instructions()
        } else {
            None
        };
        let after = root.render_state();
        (
            action,
            prompt,
            selected_model,
            selected_thinking_level,
            selected_session,
            selected_session_hydrate,
            settings_update,
            auth_update,
            compact_instructions,
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
        let (api_key, diagnostics) = resolve_prompt_api_key(
            &prompt_context.model.provider,
            prompt_context.cli_api_key.as_deref(),
            &prompt_context.auth,
        );
        let diagnostic_text = crate::request::render_diagnostics(&diagnostics);
        if !diagnostic_text.is_empty() {
            eprint!("{diagnostic_text}");
        }
        prompt_context.api_key = api_key;
    }

    // Process tree label changes (no navigation needed, just persist).
    {
        let root = root_mut(tui, root_id)?;
        if let Some((entry_id, label)) = root.take_pending_tree_label_change() {
            if let Some(ref session_path) = root.active_session_path.clone() {
                if let Ok(mut storage) = JsonlSessionStorage::open(session_path) {
                    let _ = storage.append_label_change(&entry_id, label.as_deref());
                    let choice = session_choice_from_metadata(storage.metadata());
                    root.session_label = choice.display_name().to_string();
                }
            }
        }
    }

    // Process tree navigation.
    {
        let root = root_mut(tui, root_id)?;
        if let Some(target_id) = root.take_selected_tree_entry_id() {
            if let Some(ref session_path) = root.active_session_path.clone() {
                match JsonlSessionStorage::open(session_path) {
                    Ok(mut storage) => {
                        // Check if already at this point.
                        let current_leaf = storage.get_leaf_id().ok().flatten();
                        if current_leaf.as_deref() == Some(&target_id) {
                            root.transcript
                                .push(TranscriptItem::system("Already at this point".to_string()));
                        } else {
                            // Navigate.
                            let target_entry = storage.get_entry(&target_id);
                            let is_user_message = target_entry.is_some()
                                && target_entry.unwrap().entry_type == "message"
                                && target_entry
                                    .unwrap()
                                    .field("message")
                                    .and_then(|m| m.get("role"))
                                    .and_then(|r| r.as_str())
                                    == Some("user");
                            let is_custom_message = target_entry.is_some()
                                && (target_entry.unwrap().entry_type == "custom_message"
                                    || target_entry.unwrap().entry_type == "custom");

                            if is_user_message || is_custom_message {
                                // Set leaf to parent and extract text for editor.
                                let parent_id = target_entry.unwrap().parent_id.clone();
                                let text = target_entry
                                    .unwrap()
                                    .field("message")
                                    .and_then(|m| m.get("content"))
                                    .and_then(|c| c.as_array())
                                    .and_then(|arr| arr.first())
                                    .and_then(|b| b.get("text"))
                                    .and_then(|t| t.as_str())
                                    .map(str::to_string)
                                    .unwrap_or_default();

                                // Branch to parent.
                                if let Some(ref pid) = parent_id {
                                    let _ = storage.branch(pid);
                                } else {
                                    let _ = storage.reset_leaf();
                                }

                                // Reload storage to get fresh leaf_id.
                                if let Ok(reloaded) = JsonlSessionStorage::open(session_path) {
                                    let leaf_id = reloaded.get_leaf_id().ok().flatten();
                                    root.active_leaf_id = leaf_id;
                                }
                                root.editor.set_text(text.clone());
                            } else {
                                // Navigate to this entry directly.
                                let _ = storage.branch(&target_id);
                                if let Ok(reloaded) = JsonlSessionStorage::open(session_path) {
                                    let leaf_id = reloaded.get_leaf_id().ok().flatten();
                                    root.active_leaf_id = leaf_id;
                                }
                            }

                            // Update session label.
                            let choice = session_choice_from_metadata(storage.metadata());
                            root.session_label = choice.display_name().to_string();

                            // Re-hydrate transcript.
                            if let Ok(reopened) = JsonlSessionStorage::open(session_path) {
                                if let Ok(hydrated) =
                                    crate::interactive::session_actions::hydrate_session_storage(
                                        reopened,
                                    )
                                {
                                    root.apply_hydrated_session(
                                        hydrated,
                                        Some("Navigated to selected point".to_string()),
                                    );
                                }
                            }

                            // Update session_target.
                            prompt_context.session_target =
                                Some(ResolvedSessionTarget::OpenTarget(
                                    session_path.display().to_string(),
                                ));
                        }
                    }
                    Err(error) => {
                        root.transcript.push(TranscriptItem::system(format!(
                            "Failed to navigate tree: {}",
                            error.message
                        )));
                    }
                }
            }
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
            *running = Some(start_prompt_task(
                tui,
                root_id,
                prompt,
                prompt_context,
                coding_session,
            )?);
            Ok(LoopControl::Continue(RenderRequest::FORCE))
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
            )?);
            Ok(LoopControl::Continue(RenderRequest::FORCE))
        }
    }
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

    let options = SessionPromptOptions {
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
        invocation,
        prompt: task_prompt,
    };

    let existing_session = coding_session.take();
    let task = PromptTask::spawn_prompt(options, existing_session)?;
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

fn start_compact_task<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    custom_instructions: Option<String>,
    prompt_context: &PromptContext,
) -> Result<PromptTask, CliError> {
    {
        let root = root_mut(tui, root_id)?;
        root.transcript
            .push(TranscriptItem::system("Compacting session..."));
        root.set_status(InteractiveStatus::Running);
    }

    let options = SessionPromptOptions {
        prompt: String::new(),
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
        invocation: PromptInvocation::Compact {
            custom_instructions,
        },
    };

    let task = PromptTask::spawn_legacy(options)?;
    if prompt_context.settings.terminal.show_progress {
        set_terminal_progress(tui, true)?;
    }
    Ok(task)
}

fn apply_prompt_task_event<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    agent_bridge: &mut InteractiveEventBridge,
    coding_bridge: &mut CodingEventBridge,
    event: PromptTaskEvent,
) -> Result<RenderRequest, CliError> {
    let ui_events = match event {
        PromptTaskEvent::Agent(event) => agent_bridge.handle(&event),
        PromptTaskEvent::Coding(event) => coding_bridge.handle(&event),
    };
    let root = root_mut(tui, root_id)?;
    let before = root.render_state();
    root.apply_events(ui_events);
    let after = root.render_state();
    Ok(RenderRequest::changed(before != after))
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
        Ok(PromptTaskResult::Legacy(result)) => finish_legacy_prompt(root, result),
        Ok(PromptTaskResult::Coding(result)) => {
            finish_coding_prompt(root, result.outcome);
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

fn finish_legacy_prompt(root: &mut InteractiveRoot, result: SessionPromptResult) {
    root.active_session_path = result.session_path;
    root.active_leaf_id = result.leaf_id;
    if let Some(path) = root.active_session_path.as_ref()
        && let Ok(storage) = pi_agent_core::session::JsonlSessionStorage::open(path)
    {
        let choice = session_choice_from_metadata(storage.metadata());
        root.session_label = choice.display_name().to_string();
    }
}

fn finish_coding_prompt(root: &mut InteractiveRoot, outcome: PromptTurnOutcome) {
    root.active_session_path = None;
    root.active_leaf_id = None;
    match outcome {
        PromptTurnOutcome::Success {
            session_id,
            leaf_id,
            final_message,
            ..
        } => {
            apply_success_usage(root, &final_message.usage);
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
}

fn apply_success_usage(root: &mut InteractiveRoot, usage: &Usage) {
    root.stats.input = root.stats.input.saturating_add(usage.input);
    root.stats.output = root.stats.output.saturating_add(usage.output);
    root.stats.cache_read = root.stats.cache_read.saturating_add(usage.cache_read);
    root.stats.cache_write = root.stats.cache_write.saturating_add(usage.cache_write);
    root.stats.cost +=
        usage.cost.input + usage.cost.output + usage.cost.cache_read + usage.cost.cache_write;
    root.stats.context_tokens = Some(calculate_context_tokens(usage));
}

fn calculate_context_tokens(usage: &Usage) -> u32 {
    if usage.total_tokens > 0 {
        usage.total_tokens
    } else {
        usage
            .input
            .saturating_add(usage.output)
            .saturating_add(usage.cache_read)
            .saturating_add(usage.cache_write)
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
