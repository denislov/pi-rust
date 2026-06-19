use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pi_agent_core::{AgentResources, session::create_session_id};
use pi_ai::types::Model;
use pi_tui::{
    Component, ERROR, Editor, InputEvent, KeybindingsManager, Markdown, PATH, ProcessTerminal,
    RenderScheduler, STATUS_IDLE, STATUS_RUNNING, SYSTEM, StdinBuffer, Style, TOOL_ERROR,
    TOOL_NAME, TUI_KEYBINDINGS, Terminal, Tui, TuiError, USER, color_enabled, is_key_release,
    matches_key, paint_with, truncate_to_width, visible_width,
};

use crate::interactive::key_hints::{app_key_hint, key_hint};
use crate::interactive::{InteractiveEventBridge, Transcript, TranscriptItem, UiEvent};
use crate::protocol::session_runner::{
    SessionPromptAbortHandle, SessionPromptOptions, SessionPromptResult, SpawnedSessionPrompt,
    spawn_session_prompt,
};
use crate::runtime::{PromptInvocation, SessionMode, SessionRunOptions};
use crate::session::ResolvedSessionTarget;
use crate::{CliArgs, CliError, CliOutput, CliRunOptions, resources, select_model};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteractiveModeOptions {
    pub terminal_required: bool,
}

impl Default for InteractiveModeOptions {
    fn default() -> Self {
        Self {
            terminal_required: true,
        }
    }
}

pub async fn run_interactive_mode(parsed: CliArgs, options: CliRunOptions) -> CliOutput {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return CliOutput {
            exit_code: 1,
            stdout: String::new(),
            stderr: "interactive mode requires a TTY\n".to_string(),
        };
    }

    let terminal = ProcessTerminal::new();
    let mut input = InputPump::from_stdin();
    match run_interactive_loop(parsed, options, terminal, &mut input).await {
        Ok(result) => CliOutput {
            exit_code: result.exit_code,
            stdout: String::new(),
            stderr: String::new(),
        },
        Err(error) => CliOutput {
            exit_code: 1,
            stdout: String::new(),
            stderr: format!("{error}\n"),
        },
    }
}

static INTERACTIVE_ID: AtomicUsize = AtomicUsize::new(1);
const NORMAL_RENDER_INTERVAL: Duration = Duration::from_millis(16);
const MAX_TOOL_RESULT_LINES: usize = 3;
const EXPANDED_TOOL_RESULT_LINES: usize = 20;
const SPINNER_INTERVAL: Duration = Duration::from_millis(120);
const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

struct InputPump {
    rx: tokio::sync::mpsc::UnboundedReceiver<String>,
    _reader: Option<std::thread::JoinHandle<()>>,
}

impl InputPump {
    fn from_stdin() -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let reader = std::thread::spawn(move || {
            let mut stdin = std::io::stdin();
            loop {
                let mut buffer = [0_u8; 1024];
                match stdin.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => {
                        let chunk = String::from_utf8_lossy(&buffer[..count]).to_string();
                        if tx.send(chunk).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            rx,
            _reader: Some(reader),
        }
    }

    fn from_chunks(chunks: Vec<String>) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        for chunk in chunks {
            let _ = tx.send(chunk);
        }
        drop(tx);
        Self { rx, _reader: None }
    }

    async fn recv(&mut self) -> Option<String> {
        self.rx.recv().await
    }
}

#[derive(Clone)]
struct PromptContext {
    model: Model,
    api_key: Option<String>,
    system_prompt: Option<String>,
    max_turns: u32,
    tools: Vec<pi_agent_core::AgentTool>,
    register_builtins: bool,
    session: Option<SessionRunOptions>,
    session_target: Option<ResolvedSessionTarget>,
    session_name: Option<String>,
    thinking_level: Option<pi_agent_core::ThinkingLevel>,
    tool_execution: Option<pi_agent_core::ToolExecutionMode>,
    resources: AgentResources,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InteractiveAction {
    None,
    Submit,
    AbortRunning,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InteractiveStatus {
    Idle,
    Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TranscriptScrollCommand {
    PageUp,
    PageDown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SlashCommand {
    Quit,
    Help,
    Unknown(String),
}

fn parse_slash_command(text: &str) -> Option<SlashCommand> {
    if !text.starts_with('/') {
        return None;
    }
    let command = text[1..].to_lowercase();
    let command_name = command.split_whitespace().next().unwrap_or("");
    Some(match command_name {
        "quit" | "exit" | "q" => SlashCommand::Quit,
        "help" | "h" | "?" => SlashCommand::Help,
        _ => SlashCommand::Unknown(text.to_string()),
    })
}

struct InteractiveRoot {
    transcript: Transcript,
    editor: Editor,
    submitted: Arc<Mutex<Option<String>>>,
    scroll_command: Arc<Mutex<Option<TranscriptScrollCommand>>>,
    pending_submit: Option<String>,
    action: InteractiveAction,
    status: InteractiveStatus,
    viewport_width: usize,
    viewport_height: usize,
    cwd: PathBuf,
    model_id: String,
    session_label: String,
    usage: (u32, u32),
    tool_output_expanded: bool,
    spinner_frame: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct InteractiveRenderState {
    editor_text: String,
    editor_cursor: usize,
    transcript: Vec<TranscriptItem>,
    transcript_scroll_offset: usize,
    transcript_has_new_output_below: bool,
    status: InteractiveStatus,
    tool_output_expanded: bool,
    spinner_frame: usize,
}

impl InteractiveRoot {
    fn new(cwd: PathBuf, model_id: String, session_label: String) -> Self {
        let submitted = Arc::new(Mutex::new(None));
        let submitted_for_callback = Arc::clone(&submitted);
        let scroll_command = Arc::new(Mutex::new(None));
        let page_up_command = Arc::clone(&scroll_command);
        let page_down_command = Arc::clone(&scroll_command);
        let mut editor = Editor::new(KeybindingsManager::new(
            TUI_KEYBINDINGS.clone(),
            Default::default(),
        ));
        editor.set_on_submit(Box::new(move |text| {
            *submitted_for_callback.lock().unwrap() = Some(text.to_string());
        }));
        editor.set_on_scroll_page_up(Box::new(move || {
            *page_up_command.lock().unwrap() = Some(TranscriptScrollCommand::PageUp);
        }));
        editor.set_on_scroll_page_down(Box::new(move || {
            *page_down_command.lock().unwrap() = Some(TranscriptScrollCommand::PageDown);
        }));
        editor.set_focused(true);

        let mut transcript = Transcript::new();
        let keybindings = KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default());
        transcript.push(TranscriptItem::system(welcome_line(&keybindings)));

        Self {
            transcript,
            editor,
            submitted,
            scroll_command,
            pending_submit: None,
            action: InteractiveAction::None,
            status: InteractiveStatus::Idle,
            viewport_width: 80,
            viewport_height: 24,
            cwd,
            model_id,
            session_label,
            usage: (0, 0),
            tool_output_expanded: false,
            spinner_frame: 0,
        }
    }

    fn take_action(&mut self) -> InteractiveAction {
        std::mem::replace(&mut self.action, InteractiveAction::None)
    }

    fn take_submitted(&mut self) -> Option<String> {
        self.submitted.lock().unwrap().take()
    }

    fn take_pending_submit(&mut self) -> Option<String> {
        self.pending_submit.take()
    }

    fn take_scroll_command(&mut self) -> Option<TranscriptScrollCommand> {
        self.scroll_command.lock().unwrap().take()
    }

    fn push_user(&mut self, prompt: String) {
        self.transcript.push(TranscriptItem::user(prompt));
    }

    fn apply_events(&mut self, events: Vec<UiEvent>) {
        let previous_scroll_offset = self.transcript.scroll_offset();
        let previous_rows = if previous_scroll_offset > 0 {
            render_transcript_lines(
                &self.transcript,
                self.viewport_width,
                MAX_TOOL_RESULT_LINES,
                color_enabled(),
            )
            .len()
        } else {
            0
        };
        for event in events {
            match event {
                UiEvent::UsageUpdate { input, output } => {
                    self.usage = (input, output);
                }
                other => self.transcript.apply_event(other),
            }
        }
        if previous_scroll_offset > 0 {
            let current_rows = render_transcript_lines(
                &self.transcript,
                self.viewport_width,
                MAX_TOOL_RESULT_LINES,
                color_enabled(),
            )
            .len();
            self.transcript.preserve_scrolled_view_after_hidden_change(
                previous_scroll_offset,
                current_rows.saturating_sub(previous_rows),
            );
        }
    }

    fn set_status(&mut self, status: InteractiveStatus) {
        if status == InteractiveStatus::Idle {
            self.spinner_frame = 0;
        }
        self.status = status;
    }

    fn handle_slash_command(&mut self, command: SlashCommand) {
        match command {
            SlashCommand::Quit => match self.status {
                InteractiveStatus::Idle => self.action = InteractiveAction::Exit,
                InteractiveStatus::Running => self.action = InteractiveAction::AbortRunning,
            },
            SlashCommand::Help => {
                self.transcript.push(TranscriptItem::system(help_text()));
            }
            SlashCommand::Unknown(cmd) => {
                self.transcript.push(TranscriptItem::system(format!(
                    "unknown command: {cmd} — type /help for available commands"
                )));
            }
        }
    }

    fn footer(&self) -> String {
        let color = color_enabled();
        let status_str = match self.status {
            InteractiveStatus::Idle => "idle".to_string(),
            InteractiveStatus::Running => {
                let spinner = SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()];
                format!("{spinner} running")
            }
        };
        let status_style = match self.status {
            InteractiveStatus::Idle => STATUS_IDLE,
            InteractiveStatus::Running => STATUS_RUNNING,
        };
        let cwd = abbreviate_cwd(&self.cwd);
        let mut parts = vec![
            paint_with(&format!("status: {status_str}"), &status_style, color),
            format!("cwd: {}", paint_with(&cwd, &PATH, color)),
            format!("model: {}", self.model_id),
            format!("session: {}", self.session_label),
        ];
        if self.usage != (0, 0) {
            parts.push(paint_with(
                &format!(
                    "↑{} ↓{}",
                    format_tokens(self.usage.0),
                    format_tokens(self.usage.1)
                ),
                &SYSTEM,
                color,
            ));
        }
        parts.join(" | ")
    }

    fn render_state(&self) -> InteractiveRenderState {
        InteractiveRenderState {
            editor_text: self.editor.text().to_string(),
            editor_cursor: self.editor.cursor(),
            transcript: self.transcript.items().to_vec(),
            transcript_scroll_offset: self.transcript.scroll_offset(),
            transcript_has_new_output_below: self.transcript.has_new_output_below(),
            status: self.status,
            tool_output_expanded: self.tool_output_expanded,
            spinner_frame: self.spinner_frame,
        }
    }
}

impl Component for InteractiveRoot {
    fn render(&mut self, width: usize) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }

        let editor_lines = self.editor.render(width.saturating_sub(2));
        let footer = fit_line(&self.footer(), width);
        let reserved_rows = editor_lines.len().saturating_add(1);
        let transcript_rows = self.viewport_height.saturating_sub(reserved_rows).max(1);
        let max_tool_result_lines = if self.tool_output_expanded {
            EXPANDED_TOOL_RESULT_LINES
        } else {
            MAX_TOOL_RESULT_LINES
        };
        let mut lines = render_transcript_viewport(
            &self.transcript,
            width,
            transcript_rows,
            max_tool_result_lines,
            color_enabled(),
        );
        for line in editor_lines {
            lines.push(fit_line(&format!("> {line}"), width));
        }
        lines.push(footer);
        lines
    }

    fn handle_input(&mut self, event: &InputEvent) {
        if matches_key(event, "ctrl+c") {
            match self.status {
                InteractiveStatus::Running => {
                    self.action = InteractiveAction::AbortRunning;
                    return;
                }
                InteractiveStatus::Idle => {
                    if self.editor.text().is_empty() {
                        self.action = InteractiveAction::Exit;
                    } else {
                        self.editor.set_text("");
                    }
                    return;
                }
            }
        }

        if matches_key(event, "ctrl+o") {
            self.tool_output_expanded = !self.tool_output_expanded;
            return;
        }

        if self.status == InteractiveStatus::Idle {
            self.editor.handle_input(event);
            if let Some(command) = self.take_scroll_command() {
                let page_rows = self.viewport_height.saturating_sub(2).max(1);
                match command {
                    TranscriptScrollCommand::PageUp => self.transcript.scroll_page_up(page_rows),
                    TranscriptScrollCommand::PageDown => {
                        self.transcript.scroll_page_down(page_rows)
                    }
                }
            }
            if let Some(text) = self.take_submitted() {
                if let Some(command) = parse_slash_command(&text) {
                    self.handle_slash_command(command);
                } else {
                    self.pending_submit = Some(text);
                    self.action = InteractiveAction::Submit;
                }
            }
        }
    }

    fn set_viewport_size(&mut self, width: usize, height: usize) {
        self.viewport_width = width.max(1);
        self.viewport_height = height.max(1);
    }

    fn set_focused(&mut self, focused: bool) {
        self.editor.set_focused(focused);
    }

    fn focused(&self) -> bool {
        self.editor.focused()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

struct LoopResult<T: Terminal> {
    tui: Tui<T>,
    exit_code: i32,
}

struct PromptTask {
    abort: SessionPromptAbortHandle,
    events: tokio::sync::mpsc::UnboundedReceiver<pi_agent_core::AgentEvent>,
    done: tokio::sync::oneshot::Receiver<Result<SessionPromptResult, CliError>>,
    abort_requested: bool,
    events_closed: bool,
}

impl PromptTask {
    fn new(spawned: SpawnedSessionPrompt) -> Self {
        Self {
            abort: spawned.abort,
            events: spawned.events,
            done: spawned.done,
            abort_requested: false,
            events_closed: false,
        }
    }

    fn abort_once(&mut self) {
        if !self.abort_requested {
            self.abort.abort();
            self.abort_requested = true;
        }
    }
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
    const NORMAL: Self = Self {
        requested: true,
        force: false,
    };
    const FORCE: Self = Self {
        requested: true,
        force: true,
    };

    fn changed(changed: bool) -> Self {
        if changed { Self::NORMAL } else { Self::NONE }
    }
}

enum LoopControl {
    Continue(RenderRequest),
    Exit,
}

async fn run_interactive_loop<T: Terminal>(
    parsed: CliArgs,
    options: CliRunOptions,
    mut terminal: T,
    input: &mut InputPump,
) -> Result<LoopResult<T>, CliError> {
    let prompt_context = build_prompt_context(&parsed, options)?;
    let cwd = prompt_context
        .session
        .as_ref()
        .map(|session| session.cwd.clone())
        .unwrap_or_else(|| PathBuf::from("."));
    let session_label = session_label(&prompt_context.session);

    terminal.start().map_err(to_cli_error)?;
    let mut tui = Tui::new(terminal);
    let root_id = tui.add_child_with_id(Box::new(InteractiveRoot::new(
        cwd,
        prompt_context.model.id.clone(),
        session_label,
    )));
    tui.set_focus(Some(root_id));

    let loop_result = run_started_interactive_loop(&mut tui, root_id, input, prompt_context).await;
    let stop_result = tui.terminal_mut().stop().map_err(to_cli_error);
    match (loop_result, stop_result) {
        (Ok(exit_code), Ok(())) => Ok(LoopResult { tui, exit_code }),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

async fn run_started_interactive_loop<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    input: &mut InputPump,
    prompt_context: PromptContext,
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
            tokio::select! {
                _ = sleep_render_delay(render_delay), if render_delay.is_some() => {
                    flush_render_if_ready(tui, &mut render_scheduler)?;
                    running = Some(task);
                }
                chunk = input.recv(), if input_open => {
                    match chunk {
                        Some(chunk) => {
                            running = Some(task);
                            match process_input_events(
                                tui,
                                root_id,
                                stdin_buffer.process(&chunk),
                                &prompt_context,
                                &mut running,
                                &mut render_scheduler,
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
                        root.spinner_frame =
                            (root.spinner_frame + 1) % SPINNER_FRAMES.len();
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
            tokio::select! {
                _ = sleep_render_delay(render_delay), if render_delay.is_some() => {
                    flush_render_if_ready(tui, &mut render_scheduler)?;
                }
                chunk = input.recv() => {
                    let Some(chunk) = chunk else {
                        input_open = false;
                        match process_input_events(
                            tui,
                            root_id,
                            stdin_buffer.flush(),
                            &prompt_context,
                            &mut running,
                            &mut render_scheduler,
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
                        &prompt_context,
                        &mut running,
                        &mut render_scheduler,
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
    prompt_context: &PromptContext,
    running: &mut Option<PromptTask>,
    render_scheduler: &mut RenderScheduler,
) -> Result<LoopControl, CliError> {
    for event in events {
        match handle_input_event(tui, root_id, event, prompt_context, running)? {
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
    prompt_context: &PromptContext,
    running: &mut Option<PromptTask>,
) -> Result<LoopControl, CliError> {
    if is_key_release(&event) {
        return Ok(LoopControl::Continue(RenderRequest::NONE));
    }

    let (action, prompt, render_request) = {
        let root = root_mut(tui, root_id)?;
        let before = root.render_state();
        root.handle_input(&event);
        let action = root.take_action();
        let prompt = if action == InteractiveAction::Submit {
            root.take_pending_submit()
        } else {
            None
        };
        let after = root.render_state();
        (action, prompt, RenderRequest::changed(before != after))
    };

    match action {
        InteractiveAction::None => Ok(LoopControl::Continue(render_request)),
        InteractiveAction::Exit => Ok(LoopControl::Exit),
        InteractiveAction::AbortRunning => {
            if let Some(task) = running.as_mut() {
                task.abort_once();
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
    if let Err(error) = result {
        root.apply_events(vec![UiEvent::AgentError {
            error: error.to_string(),
        }]);
    }
    root.set_status(InteractiveStatus::Idle);
    Ok(())
}

fn build_prompt_context(
    parsed: &CliArgs,
    options: CliRunOptions,
) -> Result<PromptContext, CliError> {
    let model = select_model(parsed, options.model_override)?;
    let cwd = options.session.cwd.clone();
    let (skills, templates, diagnostics) =
        resources::load_cli_resources(&parsed.skills, &parsed.prompt_templates, &cwd)?;
    resources::print_diagnostics(&diagnostics);

    if let Some(ref skill_name) = parsed.skill {
        if resources::find_skill(&skills, skill_name).is_none() {
            return Err(CliError::InvalidInput(format!(
                "skill '{skill_name}' not found in loaded skills"
            )));
        }
    }
    if let Some(ref template_name) = parsed.prompt_template {
        if resources::find_template(&templates, template_name).is_none() {
            return Err(CliError::InvalidInput(format!(
                "prompt template '{template_name}' not found in loaded templates"
            )));
        }
    }

    let session = if parsed.no_session {
        None
    } else {
        let mut session_opts = options.session;
        if let Some(ref dir) = parsed.session_dir {
            session_opts.session_dir = Some(PathBuf::from(dir));
        }
        Some(session_opts)
    };

    let session_target = match (&session, resolve_session_target(parsed)) {
        (Some(session), None) if matches!(session.mode, SessionMode::Enabled) => {
            Some(ResolvedSessionTarget::OpenOrCreateId(create_session_id()))
        }
        (_, target) => target,
    };

    Ok(PromptContext {
        model,
        api_key: parsed.api_key.clone(),
        system_prompt: parsed.system_prompt.clone(),
        max_turns: parsed.max_turns,
        tools: options.tools,
        register_builtins: options.register_builtins,
        session,
        session_target,
        session_name: parsed.name.clone(),
        thinking_level: parsed.thinking,
        tool_execution: parsed.tool_execution,
        resources: resources::build_agent_resources(skills, templates),
    })
}

fn resolve_session_target(parsed: &CliArgs) -> Option<ResolvedSessionTarget> {
    if parsed.no_session {
        None
    } else if let Some(ref fork_target) = parsed.fork {
        Some(ResolvedSessionTarget::ForkTarget(fork_target.clone()))
    } else if let Some(ref session_target) = parsed.session {
        Some(ResolvedSessionTarget::OpenTarget(session_target.clone()))
    } else if let Some(ref session_id) = parsed.session_id {
        Some(ResolvedSessionTarget::OpenOrCreateId(session_id.clone()))
    } else if parsed.continue_session || parsed.resume {
        Some(ResolvedSessionTarget::ContinueMostRecent)
    } else {
        None
    }
}

fn session_label(session: &Option<SessionRunOptions>) -> String {
    match session {
        Some(session) if matches!(session.mode, SessionMode::Enabled) => "session".to_string(),
        _ => "no-session".to_string(),
    }
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

fn render_transcript_lines(
    transcript: &Transcript,
    width: usize,
    max_tool_result_lines: usize,
    color: bool,
) -> Vec<String> {
    transcript
        .items()
        .iter()
        .flat_map(|item| match item {
            TranscriptItem::User { text } => {
                vec![fit_line(
                    &format!("{}: {}", paint_with("user", &USER, color), text),
                    width,
                )]
            }
            TranscriptItem::System { text } => text
                .split('\n')
                .map(|line| fit_line(&paint_with(line, &SYSTEM, color), width))
                .collect(),
            TranscriptItem::Assistant { markdown, .. } => {
                let mut markdown = Markdown::new(markdown);
                markdown
                    .render(width)
                    .into_iter()
                    .map(|line| fit_line(&line, width))
                    .collect::<Vec<_>>()
            }
            TranscriptItem::Tool {
                call_id,
                name,
                result,
                is_error,
                ..
            } => render_tool_lines(
                call_id,
                name,
                result.as_deref(),
                *is_error,
                width,
                max_tool_result_lines,
                color,
            ),
            TranscriptItem::Error { text } => {
                vec![fit_line(
                    &format!(
                        "{}: {}",
                        paint_with("error", &ERROR, color),
                        paint_with(text, &ERROR, color)
                    ),
                    width,
                )]
            }
        })
        .collect()
}

fn render_tool_lines(
    call_id: &str,
    name: &str,
    result: Option<&str>,
    is_error: bool,
    width: usize,
    max_tool_result_lines: usize,
    color: bool,
) -> Vec<String> {
    let status = match (result, is_error) {
        (None, _) => "running",
        (Some(_), true) => "error",
        (Some(_), false) => "done",
    };
    let status_style = match status {
        "running" => STATUS_RUNNING,
        "error" => TOOL_ERROR,
        "done" => STATUS_IDLE,
        _ => Style::default(),
    };
    let header = format!(
        "{} {} {} {}",
        paint_with("tool", &TOOL_NAME, color),
        paint_with(name, &TOOL_NAME, color),
        call_id,
        paint_with(status, &status_style, color),
    );
    let mut lines = vec![fit_line(&header, width)];
    let Some(result) = result else {
        return lines;
    };

    let result_lines = result.lines().collect::<Vec<_>>();
    lines.extend(result_lines.iter().take(max_tool_result_lines).map(|line| {
        if is_error {
            fit_line(&paint_with(line, &TOOL_ERROR, color), width)
        } else {
            fit_line(line, width)
        }
    }));
    let omitted = result_lines.len().saturating_sub(max_tool_result_lines);
    if omitted > 0 {
        lines.push(fit_line(
            &paint_with(&format!("... truncated {omitted} lines"), &SYSTEM, color),
            width,
        ));
    }
    lines
}

fn render_transcript_viewport(
    transcript: &Transcript,
    width: usize,
    viewport_rows: usize,
    max_tool_result_lines: usize,
    color: bool,
) -> Vec<String> {
    let lines = render_transcript_lines(transcript, width, max_tool_result_lines, color);
    if lines.len() <= viewport_rows {
        let mut padded = lines;
        while padded.len() < viewport_rows {
            padded.push(String::new());
        }
        return padded;
    }

    let max_scroll_offset = lines.len().saturating_sub(1);
    let scroll_offset = transcript.scroll_offset().min(max_scroll_offset);
    let bottom = lines.len().saturating_sub(scroll_offset);
    let top = bottom.saturating_sub(viewport_rows);
    let mut visible = lines[top..bottom].to_vec();
    while visible.len() < viewport_rows {
        visible.insert(0, String::new());
    }
    if transcript.has_new_output_below() && !visible.is_empty() {
        let indicator = fit_line(&paint_with("... new output below", &SYSTEM, color), width);
        let last = visible.len() - 1;
        visible[last] = indicator;
    }
    visible
}

fn fit_line(line: &str, width: usize) -> String {
    if visible_width(line) <= width {
        line.to_string()
    } else {
        truncate_to_width(line, width)
    }
}

fn help_text() -> String {
    "commands:\n  /help, /h, /?  — show this help\n  /quit, /q, /exit — exit interactive mode"
        .to_string()
}

fn welcome_line(keybindings: &KeybindingsManager) -> String {
    let parts = [
        key_hint(keybindings, "tui.input.submit", "submit"),
        key_hint(keybindings, "tui.input.newLine", "newline"),
        "/help commands".to_string(),
        app_key_hint(keybindings, "app.interrupt", "interrupt/exit"),
        app_key_hint(keybindings, "app.tools.expand", "expand tools"),
        key_hint(keybindings, "tui.editor.pageUp", "scroll up"),
        key_hint(keybindings, "tui.editor.pageDown", "scroll down"),
    ];
    format!("pi · {}", parts.join(" · "))
}

fn format_tokens(count: u32) -> String {
    if count < 1000 {
        count.to_string()
    } else if count < 1000000 {
        format!("{}k", count / 1000)
    } else {
        format!("{}M", count / 1000000)
    }
}

fn abbreviate_cwd(cwd: &Path) -> String {
    let display = cwd.display().to_string();
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() && display.starts_with(&home) {
            return format!("~{}", &display[home.len()..]);
        }
    }
    display
}

fn tui_error(error: TuiError) -> CliError {
    CliError::AgentFailure(error.to_string())
}

fn to_cli_error(error: std::io::Error) -> CliError {
    CliError::AgentFailure(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_transcript_lines_compacts_tool_rows_and_truncates_noisy_output() {
        use pi_tui::{STATUS_IDLE, STATUS_RUNNING, SYSTEM, TOOL_NAME, paint_with};
        let yellow = |s: &str| paint_with(s, &TOOL_NAME, true);
        let dim = |s: &str| paint_with(s, &SYSTEM, true);
        let running = |s: &str| paint_with(s, &STATUS_RUNNING, true);
        let idle = |s: &str| paint_with(s, &STATUS_IDLE, true);

        let mut transcript = Transcript::new();
        transcript.apply_event(UiEvent::ToolStarted {
            call_id: "tool_1".to_string(),
            name: "read".to_string(),
            args: serde_json::Value::Null,
        });

        assert_eq!(
            render_transcript_lines(&transcript, 80, 3, true),
            vec![format!(
                "{} {} tool_1 {}",
                yellow("tool"),
                yellow("read"),
                running("running")
            )]
        );
        assert_eq!(
            render_transcript_lines(&transcript, 80, 3, false),
            vec!["tool read tool_1 running"]
        );

        transcript.apply_event(UiEvent::ToolFinished {
            call_id: "tool_1".to_string(),
            result: "line 1\nline 2\nline 3\nline 4\nline 5".to_string(),
            is_error: false,
        });

        assert_eq!(
            render_transcript_lines(&transcript, 80, 3, true),
            vec![
                format!(
                    "{} {} tool_1 {}",
                    yellow("tool"),
                    yellow("read"),
                    idle("done")
                ),
                "line 1".to_string(),
                "line 2".to_string(),
                "line 3".to_string(),
                dim("... truncated 2 lines"),
            ]
        );
        assert_eq!(
            render_transcript_lines(&transcript, 80, 3, false),
            vec![
                "tool read tool_1 done",
                "line 1",
                "line 2",
                "line 3",
                "... truncated 2 lines",
            ]
        );

        assert_eq!(
            render_transcript_lines(&transcript, 80, 20, true),
            vec![
                format!(
                    "{} {} tool_1 {}",
                    yellow("tool"),
                    yellow("read"),
                    idle("done")
                ),
                "line 1".to_string(),
                "line 2".to_string(),
                "line 3".to_string(),
                "line 4".to_string(),
                "line 5".to_string(),
            ]
        );
    }

    #[test]
    fn render_transcript_lines_colors_error_item_red_bold() {
        use pi_tui::{ERROR, paint_with};
        let red_bold = |s: &str| paint_with(s, &ERROR, true);
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::Error {
            text: "boom".to_string(),
        });
        assert_eq!(
            render_transcript_lines(&transcript, 80, 3, true),
            vec![format!("{}: {}", red_bold("error"), red_bold("boom"))]
        );
        assert_eq!(
            render_transcript_lines(&transcript, 80, 3, false),
            vec!["error: boom"]
        );
    }

    #[test]
    fn ctrl_o_toggles_tool_output_expansion_in_root() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_viewport_size(40, 24);
        root.transcript.push(TranscriptItem::Tool {
            call_id: "tool_1".to_string(),
            name: "read".to_string(),
            args: serde_json::Value::Null,
            result: Some("l1\nl2\nl3\nl4\nl5\nl6".to_string()),
            is_error: false,
        });

        let collapsed = root.render(40).join("\n");
        assert!(
            collapsed.contains("... truncated"),
            "collapsed tool output should show truncation: {collapsed}"
        );

        // Ctrl+O is the single byte 0x0f, which parse_control_char maps to
        // Key::Char("o") + CTRL. Feed it through StdinBuffer like the real loop.
        let mut buffer = StdinBuffer::new();
        let events = buffer.process("\x0f");
        assert_eq!(events.len(), 1, "ctrl+o should produce one input event");
        root.handle_input(&events[0]);
        assert!(
            root.tool_output_expanded,
            "ctrl+o should flip the expand flag"
        );

        let expanded = root.render(40).join("\n");
        assert!(
            !expanded.contains("... truncated"),
            "expanded tool output should not show truncation: {expanded}"
        );
        assert!(
            expanded.contains("l6"),
            "expanded tool output should show the last line: {expanded}"
        );
    }

    #[test]
    fn footer_shows_spinner_when_running() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_status(InteractiveStatus::Running);
        let footer = root.footer();
        assert!(
            footer.contains("running"),
            "footer should contain 'running' when status is Running: {footer}"
        );
        let has_spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
            .iter()
            .any(|frame| footer.contains(frame));
        assert!(
            has_spinner,
            "footer should contain a braille spinner char when Running: {footer}"
        );
    }

    #[test]
    fn footer_no_spinner_when_idle() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_status(InteractiveStatus::Idle);
        let footer = root.footer();
        assert!(
            footer.contains("status: idle"),
            "footer should contain 'status: idle' when Idle: {footer}"
        );
        let has_spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
            .iter()
            .any(|frame| footer.contains(frame));
        assert!(
            !has_spinner,
            "footer should NOT contain a braille spinner char when Idle: {footer}"
        );
    }

    #[test]
    fn spinner_frame_advances_through_sequence() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_status(InteractiveStatus::Running);

        root.spinner_frame = 3;
        let footer_at_3 = root.footer();
        assert!(
            footer_at_3.contains("⠸"),
            "footer at frame 3 should contain '⠸': {footer_at_3}"
        );

        root.spinner_frame = 4;
        let footer_at_4 = root.footer();
        assert!(
            footer_at_4.contains("⠼"),
            "footer at frame 4 should contain '⠼': {footer_at_4}"
        );
    }

    #[test]
    fn set_status_idle_resets_spinner_frame() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.spinner_frame = 5;
        root.set_status(InteractiveStatus::Idle);
        assert_eq!(
            root.spinner_frame, 0,
            "set_status(Idle) should reset spinner_frame to 0"
        );
    }

    #[test]
    fn render_state_changes_with_spinner_frame() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_status(InteractiveStatus::Running);
        root.spinner_frame = 0;
        let state_at_0 = root.render_state();
        root.spinner_frame = 1;
        let state_at_1 = root.render_state();
        assert_ne!(
            state_at_0, state_at_1,
            "render_state should differ when spinner_frame changes"
        );
    }

    #[test]
    fn parse_slash_command_recognizes_quit_variants() {
        assert_eq!(parse_slash_command("/quit"), Some(SlashCommand::Quit));
        assert_eq!(parse_slash_command("/QUIT"), Some(SlashCommand::Quit));
        assert_eq!(parse_slash_command("/Quit"), Some(SlashCommand::Quit));
        assert_eq!(parse_slash_command("/q"), Some(SlashCommand::Quit));
        assert_eq!(parse_slash_command("/exit"), Some(SlashCommand::Quit));
    }

    #[test]
    fn parse_slash_command_recognizes_help_variants() {
        assert_eq!(parse_slash_command("/help"), Some(SlashCommand::Help));
        assert_eq!(parse_slash_command("/h"), Some(SlashCommand::Help));
        assert_eq!(parse_slash_command("/?"), Some(SlashCommand::Help));
        assert_eq!(parse_slash_command("/HELP"), Some(SlashCommand::Help));
    }

    #[test]
    fn parse_slash_command_rejects_non_slash() {
        assert_eq!(parse_slash_command("hello"), None);
        assert_eq!(parse_slash_command("  /quit"), None);
    }

    #[test]
    fn parse_slash_command_unknown_command() {
        assert_eq!(
            parse_slash_command("/foo"),
            Some(SlashCommand::Unknown("/foo".to_string()))
        );
    }

    #[test]
    fn handle_slash_command_quit_sets_exit_when_idle() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(SlashCommand::Quit);
        assert_eq!(root.action, InteractiveAction::Exit);
    }

    #[test]
    fn handle_slash_command_quit_sets_abort_when_running() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_status(InteractiveStatus::Running);
        root.handle_slash_command(SlashCommand::Quit);
        assert_eq!(root.action, InteractiveAction::AbortRunning);
    }

    #[test]
    fn handle_slash_command_help_pushs_system_item() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(SlashCommand::Help);
        let items = root.transcript.items();
        let last = items.last().expect("transcript should have an item");
        match last {
            TranscriptItem::System { text } => {
                assert!(
                    text.contains("/quit"),
                    "help text should mention /quit: {text}"
                );
            }
            _ => panic!("expected System item, got {last:?}"),
        }
        assert_ne!(root.action, InteractiveAction::Submit);
        assert!(root.pending_submit.is_none());
    }

    #[test]
    fn handle_slash_command_unknown_pushs_error() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(SlashCommand::Unknown("/foo".to_string()));
        let items = root.transcript.items();
        let last = items.last().expect("transcript should have an item");
        match last {
            TranscriptItem::System { text } => {
                assert!(
                    text.contains("unknown command"),
                    "error should mention 'unknown command': {text}"
                );
            }
            _ => panic!("expected System item, got {last:?}"),
        }
    }
}

pub mod test_harness {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::Ordering;

    use pi_ai::providers::faux::FauxProvider;
    use pi_ai::registry;
    use pi_ai::types::{Model, ModelCost, ModelInput};
    use pi_tui::{TerminalOp, VirtualTerminal};

    use super::*;

    #[derive(Debug)]
    pub struct ScriptedInteractiveOutput {
        pub rendered: String,
        pub exit_code: i32,
        pub terminal_restored: bool,
        pub cursor_row: usize,
        pub cursor_col: usize,
        pub ops: Vec<TerminalOp>,
        pub rendered_lines: Vec<String>,
        pub session_file: PathBuf,
    }

    impl ScriptedInteractiveOutput {
        pub fn contains(&self, needle: &str) -> bool {
            self.rendered.contains(needle)
        }
    }

    pub async fn run_scripted_interactive(
        provider: FauxProvider,
        input: &str,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted(provider, input, None).await
    }

    pub async fn run_scripted_interactive_with_size(
        provider: FauxProvider,
        input: &str,
        columns: usize,
        rows: usize,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted_with_provider_and_size(Arc::new(provider), vec![input], None, columns, rows)
            .await
    }

    pub async fn run_scripted_interactive_with_session_dir(
        provider: FauxProvider,
        session_dir: &Path,
        input: &str,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted(provider, input, Some(session_dir)).await
    }

    pub async fn run_scripted_interactive_with_session_dir_and_waits(
        provider: FauxProvider,
        session_dir: &Path,
        input_steps: Vec<(&str, &str)>,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted_interactive_with_session_dir_size_and_waits(
            provider,
            session_dir,
            input_steps,
            80,
            24,
        )
        .await
    }

    pub async fn run_scripted_interactive_with_session_dir_size_and_waits(
        provider: FauxProvider,
        session_dir: &Path,
        input_steps: Vec<(&str, &str)>,
        columns: usize,
        rows: usize,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted_with_provider_and_waits(
            Arc::new(provider),
            session_dir,
            input_steps,
            columns,
            rows,
        )
        .await
    }

    pub async fn run_scripted_interactive_with_provider_chunks(
        provider: Arc<dyn pi_ai::registry::ApiProvider>,
        input_chunks: Vec<&str>,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted_with_provider(provider, input_chunks, None).await
    }

    pub async fn run_scripted_idle_interactive(
        input: &str,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted_idle_interactive_with_size(input, 80, 24).await
    }

    pub async fn run_scripted_idle_interactive_with_size(
        input: &str,
        columns: usize,
        rows: usize,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        let mut input = InputPump::from_chunks(vec![input.to_string()]);
        let parsed = CliArgs::default();
        let options = CliRunOptions {
            register_builtins: false,
            ..CliRunOptions::default()
        };
        let result = run_interactive_loop(
            parsed,
            options,
            VirtualTerminal::new(columns, rows),
            &mut input,
        )
        .await?;
        Ok(scripted_output(result, None))
    }

    async fn run_scripted_with_provider(
        provider: Arc<dyn pi_ai::registry::ApiProvider>,
        input_chunks: Vec<&str>,
        session_dir: Option<&Path>,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted_with_provider_and_size(provider, input_chunks, session_dir, 80, 24).await
    }

    async fn run_scripted_with_provider_and_size(
        provider: Arc<dyn pi_ai::registry::ApiProvider>,
        input_chunks: Vec<&str>,
        session_dir: Option<&Path>,
        columns: usize,
        rows: usize,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        let api = format!(
            "interactive-harness-{}",
            INTERACTIVE_ID.fetch_add(1, Ordering::SeqCst)
        );
        registry::register(&api, provider);

        let chunks = input_chunks
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let mut input = InputPump::from_chunks(chunks);
        let parsed = CliArgs::default();
        let session = session_dir
            .map(|dir| SessionRunOptions {
                mode: SessionMode::Enabled,
                cwd: dir.to_path_buf(),
                session_dir: Some(dir.to_path_buf()),
            })
            .unwrap_or_else(|| SessionRunOptions::disabled(PathBuf::from(".")));
        let options = CliRunOptions {
            model_override: Some(faux_model(&api)),
            tools: Vec::new(),
            register_builtins: false,
            session,
        };

        let result = run_interactive_loop(
            parsed,
            options,
            VirtualTerminal::new(columns, rows),
            &mut input,
        )
        .await;
        registry::unregister(&api);

        Ok(scripted_output(result?, session_dir))
    }

    async fn run_scripted_with_provider_and_waits(
        provider: Arc<dyn pi_ai::registry::ApiProvider>,
        session_dir: &Path,
        input_steps: Vec<(&str, &str)>,
        columns: usize,
        rows: usize,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        let api = format!(
            "interactive-harness-{}",
            INTERACTIVE_ID.fetch_add(1, Ordering::SeqCst)
        );
        registry::register(&api, provider);

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut input = InputPump { rx, _reader: None };
        let parsed = CliArgs::default();
        let session = SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: session_dir.to_path_buf(),
            session_dir: Some(session_dir.to_path_buf()),
        };
        let options = CliRunOptions {
            model_override: Some(faux_model(&api)),
            tools: Vec::new(),
            register_builtins: false,
            session,
        };

        let session_dir_for_input = session_dir.to_path_buf();
        let input_steps = input_steps
            .into_iter()
            .map(|(chunk, wait_for)| (chunk.to_string(), wait_for.to_string()))
            .collect::<Vec<_>>();
        let input_driver = async move {
            for (chunk, wait_for) in input_steps {
                if tx.send(chunk).is_err() {
                    return Ok::<(), CliError>(());
                }
                wait_for_session_text(&session_dir_for_input, &wait_for).await?;
            }
            Ok(())
        };

        let run = run_interactive_loop(
            parsed,
            options,
            VirtualTerminal::new(columns, rows),
            &mut input,
        );
        let (result, input_result) = tokio::join!(run, input_driver);
        registry::unregister(&api);
        input_result?;

        Ok(scripted_output(result?, Some(session_dir)))
    }

    async fn run_scripted(
        provider: FauxProvider,
        input: &str,
        session_dir: Option<&Path>,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted_with_provider(Arc::new(provider), vec![input], session_dir).await
    }

    fn scripted_output(
        result: LoopResult<VirtualTerminal>,
        session_dir: Option<&Path>,
    ) -> ScriptedInteractiveOutput {
        let terminal_restored = result.tui.terminal().ops().contains(&TerminalOp::Stop);
        let rendered = result.tui.terminal().written_output();
        let cursor_row = result.tui.terminal().cursor_row();
        let cursor_col = result.tui.terminal().cursor_col();
        let ops = result.tui.terminal().ops().to_vec();
        let rendered_lines = result.tui.rendered_lines().to_vec();
        ScriptedInteractiveOutput {
            rendered,
            exit_code: result.exit_code,
            terminal_restored,
            cursor_row,
            cursor_col,
            ops,
            rendered_lines,
            session_file: session_dir
                .and_then(|dir| first_jsonl_file(dir).ok())
                .unwrap_or_default(),
        }
    }

    fn faux_model(api: &str) -> Model {
        Model {
            id: "faux-model".into(),
            name: "Faux Model".into(),
            api: api.into(),
            provider: "faux".into(),
            base_url: String::new(),
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 0,
            max_tokens: 0,
            headers: None,
            compat: None,
        }
    }

    fn first_jsonl_file(root: &Path) -> Result<PathBuf, std::io::Error> {
        let mut files = Vec::new();
        collect_jsonl_files(root, &mut files)?;
        files.sort();
        files.into_iter().next().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "no jsonl session file")
        })
    }

    async fn wait_for_session_text(root: &Path, needle: &str) -> Result<(), CliError> {
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(2);
        loop {
            if session_files_contain(root, needle) {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(CliError::AgentFailure(format!(
                    "timed out waiting for session text: {needle}"
                )));
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }

    fn session_files_contain(root: &Path, needle: &str) -> bool {
        let mut files = Vec::new();
        if collect_jsonl_files(root, &mut files).is_err() {
            return false;
        }
        files.iter().any(|path| {
            std::fs::read_to_string(path)
                .map(|text| text.contains(needle))
                .unwrap_or(false)
        })
    }

    fn collect_jsonl_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
        if !root.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(root)? {
            let path = entry?.path();
            if path.is_dir() {
                collect_jsonl_files(&path, out)?;
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                out.push(path);
            }
        }
        Ok(())
    }
}
