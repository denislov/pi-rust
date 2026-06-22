use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pi_agent_core::{
    AgentResources,
    session::{
        JsonlSessionMetadata, JsonlSessionRepo, JsonlSessionStorage, SessionEntry, SessionHeader,
        StoredAgentMessage, StoredUsage, create_session_id, create_timestamp, generate_entry_id,
    },
};
use pi_ai::types::Model;
use pi_ai::types::{ContentBlock, StopReason};
use pi_tui::{
    Component, ERROR, Editor, InputEvent, KeybindingsManager, Loader, Markdown, PATH,
    ProcessTerminal, RenderScheduler, STATUS_IDLE, STATUS_RUNNING, SYSTEM, StdinBuffer, Style,
    TOOL_ERROR, TOOL_NAME, TUI_KEYBINDINGS, Terminal, Tui, TuiError, TuiTheme, USER, color_enabled,
    dark_theme, fuzzy_filter_indices, is_key_release, light_theme, matches_key, paint_with,
    truncate_to_width, visible_width,
};

use crate::interactive::key_hints::{app_key_hint, key_hint};
use crate::interactive::{InteractiveEventBridge, Transcript, TranscriptItem, UiEvent};
use crate::protocol::session_runner::{
    SessionPromptAbortHandle, SessionPromptOptions, SessionPromptResult, SpawnedSessionPrompt,
    spawn_session_prompt,
};
use crate::runtime::{PromptInvocation, SessionMode, SessionRunOptions};
use crate::session::ResolvedSessionTarget;
use crate::{
    CliArgs, CliError, CliOutput, CliRunOptions, config, effective_no_context_files,
    effective_session_dir, resources, select_model,
};

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
    match run_interactive_loop_with_input(parsed, options, terminal, InputPump::from_stdin).await {
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
const MAX_SLASH_SUGGESTIONS: usize = 5;
const MAX_MODEL_CHOICES: usize = 12;
const MAX_SESSION_CHOICES: usize = 12;
const SPINNER_INTERVAL: Duration = Duration::from_millis(120);

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
    cli_api_key: Option<String>,
    auth: crate::config::AuthStore,
    system_prompt: Option<String>,
    max_turns: Option<u32>,
    tools: Vec<pi_agent_core::AgentTool>,
    register_builtins: bool,
    session: Option<SessionRunOptions>,
    session_target: Option<ResolvedSessionTarget>,
    session_name: Option<String>,
    thinking_level: Option<pi_agent_core::ThinkingLevel>,
    tool_execution: Option<pi_agent_core::ToolExecutionMode>,
    resources: AgentResources,
    settings: crate::config::Settings,
    theme: TuiTheme,
    model_choices: Vec<Model>,
    model_rotation: Vec<Model>,
    session_choices: Vec<SessionChoice>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InteractiveAction {
    None,
    Submit,
    AbortRunning,
    NewSession,
    ReloadResources,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BuiltinSlashCommand {
    name: &'static str,
    description: &'static str,
}

const BUILTIN_SLASH_COMMANDS: &[BuiltinSlashCommand] = &[
    BuiltinSlashCommand {
        name: "help",
        description: "Show help",
    },
    BuiltinSlashCommand {
        name: "settings",
        description: "Open settings menu",
    },
    BuiltinSlashCommand {
        name: "model",
        description: "Select model",
    },
    BuiltinSlashCommand {
        name: "scoped-models",
        description: "Enable or disable models for cycling",
    },
    BuiltinSlashCommand {
        name: "export",
        description: "Export session",
    },
    BuiltinSlashCommand {
        name: "import",
        description: "Import and resume a session from JSONL",
    },
    BuiltinSlashCommand {
        name: "share",
        description: "Share session as a secret GitHub gist",
    },
    BuiltinSlashCommand {
        name: "copy",
        description: "Copy last assistant message to clipboard",
    },
    BuiltinSlashCommand {
        name: "name",
        description: "Show or set the session display name",
    },
    BuiltinSlashCommand {
        name: "session",
        description: "Show session info and stats",
    },
    BuiltinSlashCommand {
        name: "changelog",
        description: "Show changelog entries",
    },
    BuiltinSlashCommand {
        name: "hotkeys",
        description: "Show keyboard shortcuts",
    },
    BuiltinSlashCommand {
        name: "fork",
        description: "Create a new fork from a previous user message",
    },
    BuiltinSlashCommand {
        name: "clone",
        description: "Duplicate the current session at the current position",
    },
    BuiltinSlashCommand {
        name: "tree",
        description: "Navigate session tree",
    },
    BuiltinSlashCommand {
        name: "login",
        description: "Configure provider authentication",
    },
    BuiltinSlashCommand {
        name: "logout",
        description: "Remove provider authentication",
    },
    BuiltinSlashCommand {
        name: "new",
        description: "Start a new session",
    },
    BuiltinSlashCommand {
        name: "compact",
        description: "Manually compact the session context",
    },
    BuiltinSlashCommand {
        name: "resume",
        description: "Resume a different session",
    },
    BuiltinSlashCommand {
        name: "reload",
        description: "Reload keybindings and resources",
    },
    BuiltinSlashCommand {
        name: "quit",
        description: "Quit pi",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedSlashCommand {
    name: String,
    args: String,
    original: String,
}

trait ClipboardSink: Send + Sync {
    fn copy_text(&self, text: &str) -> Result<(), String>;
}

#[derive(Debug)]
struct SystemClipboard;

impl ClipboardSink for SystemClipboard {
    fn copy_text(&self, text: &str) -> Result<(), String> {
        system_copy_to_clipboard(text)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionChoice {
    id: String,
    cwd: String,
    path: PathBuf,
    created_at: String,
    name: Option<String>,
    entry_count: usize,
}

impl SessionChoice {
    fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }

    fn searchable_text(&self) -> String {
        format!(
            "{} {} {} {} {}",
            self.id,
            self.name.as_deref().unwrap_or_default(),
            self.cwd,
            self.path.display(),
            self.created_at
        )
    }

    fn matches_target(&self, target: &str) -> bool {
        self.id == target
            || self.id.starts_with(target)
            || self.path.display().to_string() == target
            || self.name.as_deref() == Some(target)
    }
}

fn parse_slash_command(text: &str) -> Option<ParsedSlashCommand> {
    if !text.starts_with('/') {
        return None;
    }
    let without_slash = &text[1..];
    let mut parts = without_slash.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or("").to_lowercase();
    let args = parts.next().unwrap_or("").trim().to_string();
    Some(ParsedSlashCommand {
        name,
        args,
        original: text.to_string(),
    })
}

fn parse_model_selector_arg(
    arg: &str,
) -> Result<(String, Option<pi_agent_core::ThinkingLevel>), String> {
    match arg.rsplit_once(':') {
        Some((model_id, level)) if !model_id.is_empty() && !level.is_empty() => {
            let thinking = level.parse().map_err(|error| format!("{error}"))?;
            Ok((model_id.to_string(), Some(thinking)))
        }
        _ => Ok((arg.to_string(), None)),
    }
}

fn export_path_arg(args: &str) -> Option<String> {
    let args = args.trim_start();
    if args.is_empty() {
        return None;
    }

    let first = args.chars().next()?;
    if first == '"' || first == '\'' {
        let closing = args[1..].find(first)?;
        return Some(args[1..1 + closing].to_string());
    }

    let end = args.find(char::is_whitespace).unwrap_or(args.len());
    Some(args[..end].to_string())
}

fn default_export_path(cwd: &Path) -> PathBuf {
    let stamp = create_timestamp()
        .replace(':', "-")
        .replace('.', "-")
        .replace('Z', "");
    cwd.join(format!("session-{stamp}.html"))
}

fn resolve_command_path(cwd: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn timestamp_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn html_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn system_copy_to_clipboard(text: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        copy_with_command("pbcopy", &[], text)
    }

    #[cfg(target_os = "windows")]
    {
        copy_with_command(
            "powershell",
            &["-NoProfile", "-Command", "Set-Clipboard"],
            text,
        )
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let attempts: &[(&str, &[&str])] = &[
            ("wl-copy", &[]),
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["--clipboard", "--input"]),
            ("termux-clipboard-set", &[]),
        ];
        let mut errors = Vec::new();
        for (program, args) in attempts {
            match copy_with_command(program, args, text) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(error),
            }
        }
        Err(format!(
            "Failed to copy to clipboard: {}",
            errors.join("; ")
        ))
    }
}

fn copy_with_command(program: &str, args: &[&str], text: &str) -> Result<(), String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("{program}: {error}"))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|error| format!("{program}: {error}"))?;
    }

    let status = child
        .wait()
        .map_err(|error| format!("{program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program}: exited with status {status}"))
    }
}

fn clone_session_to_sibling(
    source_path: &Path,
    target_cwd: &Path,
    leaf_id: &str,
) -> Result<JsonlSessionStorage, String> {
    let source = JsonlSessionStorage::open(source_path).map_err(|error| error.message)?;
    let entries = source.get_entries();
    let by_id: HashMap<&str, &SessionEntry> = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect();
    if !by_id.contains_key(leaf_id) {
        return Err(format!("entry id not found in source session: {leaf_id}"));
    }

    let parent = source_path
        .parent()
        .ok_or_else(|| "source session has no parent directory".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let session_id = create_session_id();
    let timestamp = create_timestamp();
    let filename = format!(
        "{}_{}.jsonl",
        timestamp.replace(':', "_").replace('.', "_"),
        session_id
    );
    let clone_path = parent.join(filename);
    let mut target = JsonlSessionStorage::create(
        &clone_path,
        target_cwd.display().to_string(),
        &session_id,
        timestamp,
        Some(source_path.to_path_buf()),
    )
    .map_err(|error| error.message)?;

    let mut branch = Vec::new();
    let mut current = by_id.get(leaf_id).copied();
    while let Some(entry) = current {
        branch.push(entry.clone());
        current = entry
            .parent_id
            .as_deref()
            .and_then(|parent_id| by_id.get(parent_id).copied());
    }
    branch.reverse();
    for entry in branch {
        target.append_entry(entry).map_err(|error| error.message)?;
    }

    Ok(target)
}

struct InteractiveRoot {
    transcript: Transcript,
    editor: Editor,
    keybindings: KeybindingsManager,
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
    selected_model: Option<Model>,
    selected_thinking_level: Option<pi_agent_core::ThinkingLevel>,
    available_models: Vec<Model>,
    model_rotation: Vec<Model>,
    selecting_model: bool,
    model_selection_selected: usize,
    session_choices: Vec<SessionChoice>,
    selected_session: Option<SessionChoice>,
    active_session_path: Option<PathBuf>,
    active_leaf_id: Option<String>,
    selecting_session: bool,
    session_selection_selected: usize,
    selecting_settings: bool,
    usage: (u32, u32),
    tool_output_expanded: bool,
    spinner_frame: usize,
    slash_suggestion_selected: usize,
    slash_suggestions_dismissed_for: Option<String>,
    theme: TuiTheme,
    clipboard: Arc<dyn ClipboardSink>,
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
    slash_suggestion_selected: usize,
    slash_suggestions_dismissed_for: Option<String>,
    selecting_settings: bool,
    selecting_model: bool,
    model_selection_selected: usize,
    selecting_session: bool,
    session_selection_selected: usize,
}

impl InteractiveRoot {
    #[cfg(test)]
    fn new(cwd: PathBuf, model_id: String, session_label: String) -> Self {
        Self::new_with_theme(cwd, model_id, session_label, dark_theme())
    }

    #[cfg(test)]
    fn new_with_theme(
        cwd: PathBuf,
        model_id: String,
        session_label: String,
        theme: TuiTheme,
    ) -> Self {
        Self::new_with_theme_and_models(cwd, model_id, session_label, theme, Vec::new())
    }

    fn new_with_theme_and_models(
        cwd: PathBuf,
        model_id: String,
        session_label: String,
        theme: TuiTheme,
        available_models: Vec<Model>,
    ) -> Self {
        let submitted = Arc::new(Mutex::new(None));
        let submitted_for_callback = Arc::clone(&submitted);
        let scroll_command = Arc::new(Mutex::new(None));
        let page_up_command = Arc::clone(&scroll_command);
        let page_down_command = Arc::clone(&scroll_command);
        let keybindings = KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default());
        let mut editor = Editor::new(keybindings.clone());
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
        transcript.push(TranscriptItem::system(welcome_line(&keybindings)));

        Self {
            transcript,
            editor,
            keybindings,
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
            selected_model: None,
            selected_thinking_level: None,
            available_models,
            model_rotation: Vec::new(),
            selecting_model: false,
            model_selection_selected: 0,
            session_choices: Vec::new(),
            selected_session: None,
            active_session_path: None,
            active_leaf_id: None,
            selecting_session: false,
            session_selection_selected: 0,
            selecting_settings: false,
            usage: (0, 0),
            tool_output_expanded: false,
            spinner_frame: 0,
            slash_suggestion_selected: 0,
            slash_suggestions_dismissed_for: None,
            theme,
            clipboard: Arc::new(SystemClipboard),
        }
    }

    #[cfg(test)]
    fn with_theme(mut self, theme: TuiTheme) -> Self {
        self.theme = theme;
        self
    }

    #[cfg(test)]
    fn with_clipboard(mut self, clipboard: Arc<dyn ClipboardSink>) -> Self {
        self.clipboard = clipboard;
        self
    }

    fn take_action(&mut self) -> InteractiveAction {
        std::mem::replace(&mut self.action, InteractiveAction::None)
    }

    fn take_selected_model(&mut self) -> Option<Model> {
        self.selected_model.take()
    }

    fn take_selected_thinking_level(&mut self) -> Option<pi_agent_core::ThinkingLevel> {
        self.selected_thinking_level.take()
    }

    fn take_selected_session(&mut self) -> Option<SessionChoice> {
        self.selected_session.take()
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

    fn apply_prompt_context(&mut self, prompt_context: &PromptContext) {
        self.cwd = prompt_context
            .session
            .as_ref()
            .map(|session| session.cwd.clone())
            .unwrap_or_else(|| PathBuf::from("."));
        self.model_id = prompt_context.model.id.clone();
        self.available_models = prompt_context.model_choices.clone();
        self.model_rotation = prompt_context.model_rotation.clone();
        self.session_choices = prompt_context.session_choices.clone();
        self.theme = prompt_context.theme.clone();
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

    fn handle_slash_command(&mut self, command: ParsedSlashCommand) {
        match command.name.as_str() {
            "quit" | "exit" | "q" => match self.status {
                InteractiveStatus::Idle => self.action = InteractiveAction::Exit,
                InteractiveStatus::Running => self.action = InteractiveAction::AbortRunning,
            },
            "help" | "h" | "?" => {
                self.transcript.push(TranscriptItem::system(help_text()));
            }
            "model" => self.handle_model_command(&command.args),
            "resume" => self.handle_resume_command(&command.args),
            "export" => self.handle_export_command(&command.args),
            "import" => self.handle_import_command(&command.args),
            "copy" => self.handle_copy_command(),
            "new" => self.handle_new_command(),
            "clone" => self.handle_clone_command(),
            "reload" => self.handle_reload_command(),
            "settings" => self.handle_settings_command(),
            "name" => self.handle_name_command(&command.args),
            "session" => self.handle_session_command(),
            "hotkeys" => self.handle_hotkeys_command(),
            "changelog" => self.handle_changelog_command(),
            "scoped-models" | "share" | "fork" | "tree" | "login" | "logout" | "compact" => {
                self.handle_pending_slash_command(&command)
            }
            _ => {
                self.transcript.push(TranscriptItem::system(format!(
                    "unknown command: {} - type /help for available commands",
                    command.original
                )));
            }
        }
    }

    fn handle_pending_slash_command(&mut self, command: &ParsedSlashCommand) {
        self.transcript.push(TranscriptItem::system(format!(
            "/{} is recognized but not implemented in the Rust interactive UI yet.",
            command.name
        )));
    }

    fn handle_export_command(&mut self, args: &str) {
        match self.export_transcript(args) {
            Ok(path) => self.transcript.push(TranscriptItem::system(format!(
                "Session exported to: {}",
                path.display()
            ))),
            Err(error) => self.transcript.push(TranscriptItem::system(format!(
                "Failed to export session: {error}"
            ))),
        }
    }

    fn handle_import_command(&mut self, args: &str) {
        let Some(input_path) = export_path_arg(args) else {
            self.transcript
                .push(TranscriptItem::system("Usage: /import <path.jsonl>"));
            return;
        };
        let path = resolve_command_path(&self.cwd, &input_path);

        match JsonlSessionStorage::open(&path) {
            Ok(storage) => {
                let leaf_id = storage.get_leaf_id().unwrap_or(None);
                let choice = session_choice_from_metadata(storage.metadata());
                self.session_label = choice.display_name().to_string();
                self.selected_session = Some(choice);
                self.active_session_path = Some(path.clone());
                self.active_leaf_id = leaf_id;
                self.selecting_session = false;
                self.session_selection_selected = 0;
                self.editor.set_text("");
                self.transcript.push(TranscriptItem::system(format!(
                    "Session imported from: {}",
                    path.display()
                )));
            }
            Err(error) => {
                self.transcript.push(TranscriptItem::system(format!(
                    "Failed to import session: {}",
                    error.message
                )));
            }
        }
    }

    fn handle_copy_command(&mut self) {
        let Some(text) = self.last_assistant_text() else {
            self.transcript
                .push(TranscriptItem::system("No agent messages to copy yet."));
            return;
        };

        match self.clipboard.copy_text(&text) {
            Ok(()) => self.transcript.push(TranscriptItem::system(
                "Copied last agent message to clipboard",
            )),
            Err(error) => self.transcript.push(TranscriptItem::system(error)),
        }
    }

    fn handle_new_command(&mut self) {
        self.transcript = Transcript::new();
        self.transcript
            .push(TranscriptItem::system(welcome_line(&self.keybindings)));
        self.transcript
            .push(TranscriptItem::system("New session started"));
        self.editor.set_text("");
        self.selecting_model = false;
        self.selecting_session = false;
        self.selecting_settings = false;
        self.model_selection_selected = 0;
        self.session_selection_selected = 0;
        self.usage = (0, 0);
        self.session_label = "session".to_string();
        self.active_session_path = None;
        self.active_leaf_id = None;
        self.action = InteractiveAction::NewSession;
    }

    fn handle_clone_command(&mut self) {
        let Some(source_path) = self.active_session_path.clone() else {
            self.transcript
                .push(TranscriptItem::system("Nothing to clone yet"));
            return;
        };
        let Some(leaf_id) = self.active_leaf_id.clone() else {
            self.transcript
                .push(TranscriptItem::system("Nothing to clone yet"));
            return;
        };

        match clone_session_to_sibling(&source_path, &self.cwd, &leaf_id) {
            Ok(storage) => {
                let leaf_id = storage.get_leaf_id().unwrap_or(None);
                let choice = session_choice_from_metadata(storage.metadata());
                self.session_label = choice.display_name().to_string();
                self.selected_session = Some(choice.clone());
                self.active_session_path = Some(choice.path);
                self.active_leaf_id = leaf_id;
                self.editor.set_text("");
                self.transcript
                    .push(TranscriptItem::system("Cloned to new session"));
            }
            Err(error) => {
                self.transcript.push(TranscriptItem::system(error));
            }
        }
    }

    fn handle_reload_command(&mut self) {
        self.transcript.push(TranscriptItem::system(
            "Reloading keybindings and resources...",
        ));
        self.action = InteractiveAction::ReloadResources;
    }

    fn last_assistant_text(&self) -> Option<String> {
        self.transcript.items().iter().rev().find_map(|item| {
            if let TranscriptItem::Assistant { markdown, .. } = item {
                let text = markdown.trim();
                if !text.is_empty() {
                    return Some(markdown.clone());
                }
            }
            None
        })
    }

    fn export_transcript(&self, args: &str) -> Result<PathBuf, String> {
        let path = export_path_arg(args)
            .map(PathBuf::from)
            .unwrap_or_else(|| default_export_path(&self.cwd));
        let path = if path.is_absolute() {
            path
        } else {
            self.cwd.join(path)
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }

        if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
            self.export_transcript_jsonl(&path)?;
        } else {
            self.export_transcript_html(&path)?;
        }
        Ok(path)
    }

    fn export_transcript_jsonl(&self, path: &Path) -> Result<(), String> {
        let timestamp = create_timestamp();
        let header = SessionHeader {
            entry_type: "session".to_string(),
            version: 3,
            id: create_session_id(),
            timestamp: timestamp.clone(),
            cwd: self.cwd.display().to_string(),
            parent_session: None,
        };
        let mut lines = vec![serde_json::to_string(&header).map_err(|error| error.to_string())?];
        let mut existing = HashSet::new();
        let mut parent_id = None;

        for message in self.exportable_messages() {
            let id = generate_entry_id(&existing);
            existing.insert(id.clone());
            let entry =
                SessionEntry::message(id.clone(), parent_id.clone(), timestamp.clone(), message);
            lines.push(serde_json::to_string(&entry).map_err(|error| error.to_string())?);
            parent_id = Some(id);
        }

        let mut text = lines.join("\n");
        text.push('\n');
        std::fs::write(path, text).map_err(|error| error.to_string())
    }

    fn export_transcript_html(&self, path: &Path) -> Result<(), String> {
        let mut body = String::new();
        for item in self.transcript.items() {
            match item {
                TranscriptItem::User { text } => body.push_str(&format!(
                    "<section class=\"message user\"><h2>User</h2><pre>{}</pre></section>",
                    html_escape(text)
                )),
                TranscriptItem::Assistant { markdown, .. } => body.push_str(&format!(
                    "<section class=\"message assistant\"><h2>Assistant</h2><pre>{}</pre></section>",
                    html_escape(markdown)
                )),
                TranscriptItem::Tool {
                    name,
                    result,
                    is_error,
                    ..
                } => body.push_str(&format!(
                    "<section class=\"message tool{}\"><h2>Tool: {}</h2><pre>{}</pre></section>",
                    if *is_error { " error" } else { "" },
                    html_escape(name),
                    html_escape(result.as_deref().unwrap_or(""))
                )),
                TranscriptItem::Error { text } => body.push_str(&format!(
                    "<section class=\"message error\"><h2>Error</h2><pre>{}</pre></section>",
                    html_escape(text)
                )),
                TranscriptItem::System { .. } => {}
            }
        }

        let html = format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title><style>{}</style></head><body><main><h1>{}</h1>{}</main></body></html>",
            html_escape(&self.session_label),
            "body{font-family:system-ui,sans-serif;margin:2rem;background:#101010;color:#f4f4f4}main{max-width:900px;margin:auto}.message{border:1px solid #444;padding:1rem;margin:1rem 0;border-radius:6px}pre{white-space:pre-wrap;font-family:ui-monospace,monospace}.user{border-color:#3b82f6}.assistant{border-color:#10b981}.tool{border-color:#a78bfa}.error{border-color:#ef4444;color:#fecaca}",
            html_escape(&self.session_label),
            body
        );
        std::fs::write(path, html).map_err(|error| error.to_string())
    }

    fn exportable_messages(&self) -> Vec<StoredAgentMessage> {
        let timestamp_ms = timestamp_millis();
        let mut messages = Vec::new();
        for item in self.transcript.items() {
            match item {
                TranscriptItem::User { text } => messages.push(StoredAgentMessage::User {
                    content: vec![ContentBlock::Text {
                        text: text.clone(),
                        text_signature: None,
                    }],
                    timestamp: timestamp_ms,
                }),
                TranscriptItem::Assistant { markdown, .. } => {
                    if !markdown.trim().is_empty() {
                        messages.push(StoredAgentMessage::Assistant {
                            content: vec![ContentBlock::Text {
                                text: markdown.clone(),
                                text_signature: None,
                            }],
                            api: "interactive".to_string(),
                            provider: "interactive".to_string(),
                            model: self.model_id.clone(),
                            response_model: None,
                            response_id: None,
                            usage: StoredUsage::default(),
                            stop_reason: StopReason::Stop,
                            error_message: None,
                            timestamp: timestamp_ms,
                        });
                    }
                }
                TranscriptItem::Tool {
                    call_id,
                    name,
                    result,
                    is_error,
                    ..
                } => messages.push(StoredAgentMessage::ToolResult {
                    tool_call_id: call_id.clone(),
                    tool_name: name.clone(),
                    content: vec![ContentBlock::Text {
                        text: result.clone().unwrap_or_default(),
                        text_signature: None,
                    }],
                    is_error: *is_error,
                    timestamp: timestamp_ms,
                }),
                TranscriptItem::Error { text } => messages.push(StoredAgentMessage::Custom {
                    custom_type: "error".to_string(),
                    content: vec![ContentBlock::Text {
                        text: text.clone(),
                        text_signature: None,
                    }],
                    display: true,
                    details: None,
                    timestamp: timestamp_ms,
                }),
                TranscriptItem::System { .. } => {}
            }
        }
        messages
    }

    fn handle_settings_command(&mut self) {
        self.selecting_settings = true;
        self.selecting_model = false;
        self.selecting_session = false;
        self.editor.set_text("");
    }

    fn handle_model_command(&mut self, args: &str) {
        if args.is_empty() {
            self.selecting_model = true;
            self.selecting_settings = false;
            self.selecting_session = false;
            self.model_selection_selected = 0;
            self.editor.set_text("");
            return;
        }

        let (model_id, thinking_level) = match parse_model_selector_arg(args) {
            Ok(parsed) => parsed,
            Err(error) => {
                self.transcript.push(TranscriptItem::system(error));
                return;
            }
        };

        match pi_ai::lookup_model(&model_id) {
            Some(model) => self.set_selected_model_with_thinking(model, thinking_level),
            None => {
                self.transcript
                    .push(TranscriptItem::system(format!("Unknown model: {model_id}")));
            }
        }
    }

    fn set_selected_model(&mut self, model: Model) {
        self.set_selected_model_with_thinking(model, None);
    }

    fn set_selected_model_with_thinking(
        &mut self,
        model: Model,
        thinking_level: Option<pi_agent_core::ThinkingLevel>,
    ) {
        self.model_id = model.id.clone();
        self.selected_model = Some(model);
        self.selected_thinking_level = thinking_level;
        self.selecting_model = false;
        self.model_selection_selected = 0;
        self.editor.set_text("");
        let suffix = thinking_level
            .map(|level| format!(" (thinking: {level})"))
            .unwrap_or_default();
        self.transcript.push(TranscriptItem::system(format!(
            "Model set: {}{}",
            self.model_id, suffix
        )));
    }

    fn cycle_model_rotation(&mut self, reverse: bool) {
        if self.model_rotation.is_empty() {
            return;
        }
        let len = self.model_rotation.len();
        let next_index = match self
            .model_rotation
            .iter()
            .position(|model| model.id == self.model_id)
        {
            Some(index) if reverse => (index + len - 1) % len,
            Some(index) => (index + 1) % len,
            None if reverse => len - 1,
            None => 0,
        };
        let model = self.model_rotation[next_index].clone();
        self.set_selected_model(model);
    }

    fn handle_resume_command(&mut self, args: &str) {
        if self.session_choices.is_empty() {
            self.transcript.push(TranscriptItem::system(
                "No sessions found for the current workspace.".to_string(),
            ));
            return;
        }

        if !args.is_empty() {
            if let Some(choice) = self
                .session_choices
                .iter()
                .find(|choice| choice.matches_target(args))
                .cloned()
            {
                self.set_selected_session(choice);
            } else {
                self.transcript
                    .push(TranscriptItem::system(format!("Unknown session: {args}")));
            }
            return;
        }

        self.selecting_session = true;
        self.selecting_model = false;
        self.selecting_settings = false;
        self.session_selection_selected = 0;
        self.editor.set_text("");
    }

    fn set_selected_session(&mut self, choice: SessionChoice) {
        self.session_label = choice.display_name().to_string();
        self.selected_session = Some(choice.clone());
        self.active_session_path = Some(choice.path.clone());
        self.active_leaf_id = JsonlSessionStorage::open(&choice.path)
            .ok()
            .and_then(|storage| storage.get_leaf_id().ok())
            .flatten();
        self.selecting_session = false;
        self.session_selection_selected = 0;
        self.editor.set_text("");
        self.transcript.push(TranscriptItem::system(format!(
            "Session selected: {}",
            choice.display_name()
        )));
    }

    fn handle_name_command(&mut self, args: &str) {
        if args.is_empty() {
            self.transcript.push(TranscriptItem::system(format!(
                "Session name: {}",
                self.session_label
            )));
            return;
        }

        self.session_label = args.to_string();
        self.transcript.push(TranscriptItem::system(format!(
            "Session name set: {}",
            self.session_label
        )));
    }

    fn handle_session_command(&mut self) {
        let cwd = abbreviate_cwd(&self.cwd);
        self.transcript.push(TranscriptItem::system(format!(
            "Session Info\n\nName: {}\nModel: {}\nCwd: {}\nTokens\nInput: {}\nOutput: {}",
            self.session_label,
            self.model_id,
            cwd,
            format_tokens(self.usage.0),
            format_tokens(self.usage.1)
        )));
    }

    fn handle_hotkeys_command(&mut self) {
        let keybindings = KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default());
        let submit = key_hint(&keybindings, "tui.input.submit", "submit");
        let newline = key_hint(&keybindings, "tui.input.newLine", "newline");
        let interrupt = app_key_hint(&keybindings, "app.interrupt", "interrupt/exit");
        let expand = app_key_hint(&keybindings, "app.tools.expand", "expand tools");
        let page_up = key_hint(&keybindings, "tui.editor.pageUp", "scroll up");
        let page_down = key_hint(&keybindings, "tui.editor.pageDown", "scroll down");
        self.transcript.push(TranscriptItem::system(format!(
            "Hotkeys\n\nNavigation\n- {page_up}\n- {page_down}\n\nEditing\n- {submit}\n- {newline}\n\nApp\n- {interrupt}\n- {expand}"
        )));
    }

    fn handle_changelog_command(&mut self) {
        self.transcript.push(TranscriptItem::system(
            "Changelog display is not implemented in the Rust interactive UI yet.".to_string(),
        ));
    }

    fn footer(&self) -> String {
        let color = color_enabled();
        let status_str = match self.status {
            InteractiveStatus::Idle => "idle".to_string(),
            InteractiveStatus::Running => running_status_text(self.spinner_frame),
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
            slash_suggestion_selected: self.slash_suggestion_selected,
            slash_suggestions_dismissed_for: self.slash_suggestions_dismissed_for.clone(),
            selecting_settings: self.selecting_settings,
            selecting_model: self.selecting_model,
            model_selection_selected: self.model_selection_selected,
            selecting_session: self.selecting_session,
            session_selection_selected: self.session_selection_selected,
        }
    }

    fn editor_border_style(&self) -> Style {
        if self.selecting_model || self.selecting_settings || self.selecting_session {
            self.theme.editor.menu_border
        } else {
            self.theme.editor.active_border
        }
    }

    fn slash_suggestion_indices(&self) -> Option<Vec<usize>> {
        if self.selecting_model || self.selecting_settings || self.selecting_session {
            return None;
        }
        let text = self.editor.text();
        if self
            .slash_suggestions_dismissed_for
            .as_deref()
            .is_some_and(|dismissed| dismissed == text)
        {
            return None;
        }
        let query = slash_completion_query(text, self.editor.cursor())?;
        let indices = fuzzy_filter_indices(BUILTIN_SLASH_COMMANDS, query, |command| {
            command.name.to_string()
        });
        (!indices.is_empty()).then_some(indices)
    }

    fn render_slash_suggestions(&mut self, width: usize) -> Vec<String> {
        let Some(indices) = self.slash_suggestion_indices() else {
            return Vec::new();
        };
        self.slash_suggestion_selected = self
            .slash_suggestion_selected
            .min(indices.len().saturating_sub(1));

        let color = color_enabled();
        let window_start = self
            .slash_suggestion_selected
            .saturating_add(1)
            .saturating_sub(MAX_SLASH_SUGGESTIONS);
        let mut lines = Vec::new();
        for (visible_offset, command_index) in indices
            .iter()
            .copied()
            .skip(window_start)
            .take(MAX_SLASH_SUGGESTIONS)
            .enumerate()
        {
            let absolute_index = window_start + visible_offset;
            let command = &BUILTIN_SLASH_COMMANDS[command_index];
            let label = format!("/{}", command.name);
            let marker = if absolute_index == self.slash_suggestion_selected {
                "->"
            } else {
                "  "
            };
            let line = format!(
                "{marker} {label:<17} {}",
                paint_with(command.description, &SYSTEM, color)
            );
            if absolute_index == self.slash_suggestion_selected {
                lines.push(fit_line(&paint_with(&line, &USER, color), width));
            } else {
                lines.push(fit_line(&line, width));
            }
        }
        lines.push(fit_line(
            &paint_with(
                &format!("({}/{})", self.slash_suggestion_selected + 1, indices.len()),
                &SYSTEM,
                color,
            ),
            width,
        ));
        lines
    }

    fn render_settings_menu(&self, width: usize) -> Vec<String> {
        if !self.selecting_settings {
            return Vec::new();
        }
        [
            "Settings".to_string(),
            format!("  Theme: {}", self.theme.name),
            format!("  Model: {}", self.model_id),
            format!("  Session: {}", self.session_label),
            "  Esc close".to_string(),
        ]
        .into_iter()
        .map(|line| fit_line(&line, width))
        .collect()
    }

    fn model_selection_indices(&self) -> Vec<usize> {
        fuzzy_filter_indices(&self.available_models, self.editor.text(), |model| {
            format!("{} {} {}", model.id, model.name, model.provider)
        })
    }

    fn session_selection_indices(&self) -> Vec<usize> {
        fuzzy_filter_indices(&self.session_choices, self.editor.text(), |choice| {
            choice.searchable_text()
        })
    }

    fn render_model_selector(&mut self, width: usize) -> Vec<String> {
        if !self.selecting_model {
            return Vec::new();
        }

        let indices = self.model_selection_indices();
        self.model_selection_selected = self
            .model_selection_selected
            .min(indices.len().saturating_sub(1));

        let color = color_enabled();
        let mut lines = vec![fit_line("Select model", width)];
        if indices.is_empty() {
            lines.push(fit_line(
                &paint_with(
                    "  No models for configured providers. Add keys in auth.toml or env.",
                    &SYSTEM,
                    color,
                ),
                width,
            ));
            lines.push(fit_line(&paint_with("  Esc close", &SYSTEM, color), width));
            return lines;
        }

        let window_start = self
            .model_selection_selected
            .saturating_add(1)
            .saturating_sub(MAX_MODEL_CHOICES);
        let mut previous_provider: Option<&str> = None;
        for (visible_offset, model_index) in indices
            .iter()
            .copied()
            .skip(window_start)
            .take(MAX_MODEL_CHOICES)
            .enumerate()
        {
            let absolute_index = window_start + visible_offset;
            let model = &self.available_models[model_index];
            if previous_provider != Some(model.provider.as_str()) {
                lines.push(fit_line(
                    &paint_with(&format!("  {}", model.provider), &SYSTEM, color),
                    width,
                ));
                previous_provider = Some(model.provider.as_str());
            }
            let marker = if absolute_index == self.model_selection_selected {
                "->"
            } else {
                "  "
            };
            let line = format!(
                "{marker} {:<24} {} · {}",
                model.id,
                paint_with(&model.provider, &SYSTEM, color),
                paint_with(&model.name, &SYSTEM, color)
            );
            if absolute_index == self.model_selection_selected {
                lines.push(fit_line(&paint_with(&line, &USER, color), width));
            } else {
                lines.push(fit_line(&line, width));
            }
        }
        lines.push(fit_line(
            &paint_with(
                &format!(
                    "({}/{}) Enter select · Esc close",
                    self.model_selection_selected + 1,
                    indices.len()
                ),
                &SYSTEM,
                color,
            ),
            width,
        ));
        lines
    }

    fn render_session_selector(&mut self, width: usize) -> Vec<String> {
        if !self.selecting_session {
            return Vec::new();
        }

        let indices = self.session_selection_indices();
        self.session_selection_selected = self
            .session_selection_selected
            .min(indices.len().saturating_sub(1));

        let color = color_enabled();
        let mut lines = vec![fit_line("Select session", width)];
        if indices.is_empty() {
            lines.push(fit_line(
                &paint_with("  No matching sessions", &SYSTEM, color),
                width,
            ));
            lines.push(fit_line(&paint_with("  Esc close", &SYSTEM, color), width));
            return lines;
        }

        let window_start = self
            .session_selection_selected
            .saturating_add(1)
            .saturating_sub(MAX_SESSION_CHOICES);
        for (visible_offset, session_index) in indices
            .iter()
            .copied()
            .skip(window_start)
            .take(MAX_SESSION_CHOICES)
            .enumerate()
        {
            let absolute_index = window_start + visible_offset;
            let choice = &self.session_choices[session_index];
            let marker = if absolute_index == self.session_selection_selected {
                "->"
            } else {
                "  "
            };
            let cwd = abbreviate_cwd(Path::new(&choice.cwd));
            let line = format!(
                "{marker} {:<24} {} · {} · {} entries",
                choice.display_name(),
                paint_with(&choice.id, &SYSTEM, color),
                paint_with(&cwd, &SYSTEM, color),
                choice.entry_count
            );
            if absolute_index == self.session_selection_selected {
                lines.push(fit_line(&paint_with(&line, &USER, color), width));
            } else {
                lines.push(fit_line(&line, width));
            }
        }
        lines.push(fit_line(
            &paint_with(
                &format!(
                    "({}/{}) Enter resume · Esc close",
                    self.session_selection_selected + 1,
                    indices.len()
                ),
                &SYSTEM,
                color,
            ),
            width,
        ));
        lines
    }

    fn render_editor_box(&mut self, width: usize) -> Vec<String> {
        let editor_lines = self.editor.render(width.saturating_sub(2));
        let border = editor_border_line(width, &self.editor_border_style(), color_enabled());
        let mut lines = Vec::with_capacity(editor_lines.len() + 2);
        lines.push(border.clone());
        for line in editor_lines {
            lines.push(fit_line(&format!("> {line}"), width));
        }
        lines.push(border);
        lines
    }

    fn handle_slash_suggestion_input(&mut self, event: &InputEvent) -> bool {
        let Some(indices) = self.slash_suggestion_indices() else {
            return false;
        };

        if self.keybindings.matches(event, "tui.select.up") {
            self.slash_suggestion_selected =
                (self.slash_suggestion_selected + indices.len() - 1) % indices.len();
            return true;
        }
        if self.keybindings.matches(event, "tui.select.down") {
            self.slash_suggestion_selected = (self.slash_suggestion_selected + 1) % indices.len();
            return true;
        }
        if self.keybindings.matches(event, "tui.select.pageUp") {
            self.slash_suggestion_selected = self
                .slash_suggestion_selected
                .saturating_sub(MAX_SLASH_SUGGESTIONS);
            return true;
        }
        if self.keybindings.matches(event, "tui.select.pageDown") {
            self.slash_suggestion_selected = (self.slash_suggestion_selected
                + MAX_SLASH_SUGGESTIONS)
                .min(indices.len().saturating_sub(1));
            return true;
        }
        let exact_query_matches_command =
            slash_completion_query(self.editor.text(), self.editor.cursor()).is_some_and(|query| {
                indices
                    .iter()
                    .any(|index| BUILTIN_SLASH_COMMANDS[*index].name == query)
            });
        if self.keybindings.matches(event, "tui.select.confirm") && exact_query_matches_command {
            return false;
        }
        if self.keybindings.matches(event, "tui.select.confirm")
            || self.keybindings.matches(event, "tui.input.tab")
        {
            let command_index = indices[self.slash_suggestion_selected.min(indices.len() - 1)];
            let command = &BUILTIN_SLASH_COMMANDS[command_index];
            self.editor.set_text(format!("/{} ", command.name));
            self.slash_suggestion_selected = 0;
            self.slash_suggestions_dismissed_for = None;
            return true;
        }
        if self.keybindings.matches(event, "tui.select.cancel") {
            self.slash_suggestions_dismissed_for = Some(self.editor.text().to_string());
            return true;
        }

        false
    }

    fn handle_model_selection_input(&mut self, event: &InputEvent) -> bool {
        if !self.selecting_model {
            return false;
        }

        let indices = self.model_selection_indices();
        if self.keybindings.matches(event, "tui.select.up") {
            if !indices.is_empty() {
                self.model_selection_selected =
                    (self.model_selection_selected + indices.len() - 1) % indices.len();
            }
            return true;
        }
        if self.keybindings.matches(event, "tui.select.down") {
            if !indices.is_empty() {
                self.model_selection_selected = (self.model_selection_selected + 1) % indices.len();
            }
            return true;
        }
        if self.keybindings.matches(event, "tui.select.pageUp") {
            self.model_selection_selected = self
                .model_selection_selected
                .saturating_sub(MAX_MODEL_CHOICES);
            return true;
        }
        if self.keybindings.matches(event, "tui.select.pageDown") {
            self.model_selection_selected = (self.model_selection_selected + MAX_MODEL_CHOICES)
                .min(indices.len().saturating_sub(1));
            return true;
        }
        if self.keybindings.matches(event, "tui.select.cancel") {
            self.selecting_model = false;
            self.model_selection_selected = 0;
            self.editor.set_text("");
            self.transcript.push(TranscriptItem::system(
                "Model selection canceled".to_string(),
            ));
            return true;
        }
        if self.keybindings.matches(event, "tui.select.confirm") {
            if let Some(model_index) = indices.get(self.model_selection_selected).copied() {
                let model = self.available_models[model_index].clone();
                self.set_selected_model(model);
            }
            return true;
        }

        let before_text = self.editor.text().to_string();
        self.editor.handle_input(event);
        if self.editor.text() != before_text {
            self.model_selection_selected = 0;
        }
        true
    }

    fn handle_session_selection_input(&mut self, event: &InputEvent) -> bool {
        if !self.selecting_session {
            return false;
        }

        let indices = self.session_selection_indices();
        if self.keybindings.matches(event, "tui.select.up") {
            if !indices.is_empty() {
                self.session_selection_selected =
                    (self.session_selection_selected + indices.len() - 1) % indices.len();
            }
            return true;
        }
        if self.keybindings.matches(event, "tui.select.down") {
            if !indices.is_empty() {
                self.session_selection_selected =
                    (self.session_selection_selected + 1) % indices.len();
            }
            return true;
        }
        if self.keybindings.matches(event, "tui.select.pageUp") {
            self.session_selection_selected = self
                .session_selection_selected
                .saturating_sub(MAX_SESSION_CHOICES);
            return true;
        }
        if self.keybindings.matches(event, "tui.select.pageDown") {
            self.session_selection_selected = (self.session_selection_selected
                + MAX_SESSION_CHOICES)
                .min(indices.len().saturating_sub(1));
            return true;
        }
        if self.keybindings.matches(event, "tui.select.cancel") {
            self.selecting_session = false;
            self.session_selection_selected = 0;
            self.editor.set_text("");
            self.transcript.push(TranscriptItem::system(
                "Session selection canceled".to_string(),
            ));
            return true;
        }
        if self.keybindings.matches(event, "tui.select.confirm") {
            if let Some(session_index) = indices.get(self.session_selection_selected).copied() {
                let choice = self.session_choices[session_index].clone();
                self.set_selected_session(choice);
            }
            return true;
        }

        let before_text = self.editor.text().to_string();
        self.editor.handle_input(event);
        if self.editor.text() != before_text {
            self.session_selection_selected = 0;
        }
        true
    }
}

impl Component for InteractiveRoot {
    fn render(&mut self, width: usize) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }

        let footer = fit_line(&self.footer(), width);
        let max_tool_result_lines = if self.tool_output_expanded {
            EXPANDED_TOOL_RESULT_LINES
        } else {
            MAX_TOOL_RESULT_LINES
        };
        let mut lines = render_transcript_lines(
            &self.transcript,
            width,
            max_tool_result_lines,
            color_enabled(),
        );
        lines.extend(self.render_editor_box(width));
        if self.selecting_model {
            lines.extend(self.render_model_selector(width));
        } else if self.selecting_session {
            lines.extend(self.render_session_selector(width));
        } else if self.selecting_settings {
            lines.extend(self.render_settings_menu(width));
        } else {
            lines.extend(self.render_slash_suggestions(width));
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

        if self.status == InteractiveStatus::Idle
            && !self.selecting_model
            && !self.selecting_session
            && !self.selecting_settings
        {
            if self.keybindings.matches(event, "app.model.next") {
                self.cycle_model_rotation(false);
                return;
            }
            if self.keybindings.matches(event, "app.model.previous") {
                self.cycle_model_rotation(true);
                return;
            }
        }

        if self.selecting_model && matches_key(event, "escape") {
            self.selecting_model = false;
            self.editor.set_text("");
            self.transcript.push(TranscriptItem::system(
                "Model selection canceled".to_string(),
            ));
            return;
        }

        if self.selecting_session && matches_key(event, "escape") {
            self.selecting_session = false;
            self.editor.set_text("");
            self.transcript.push(TranscriptItem::system(
                "Session selection canceled".to_string(),
            ));
            return;
        }

        if self.selecting_settings && matches_key(event, "escape") {
            self.selecting_settings = false;
            self.editor.set_text("");
            return;
        }

        if self.status == InteractiveStatus::Idle {
            if self.selecting_model {
                self.handle_model_selection_input(event);
                return;
            }
            if self.selecting_session {
                self.handle_session_selection_input(event);
                return;
            }
            if self.selecting_settings {
                return;
            }
            let before_text = self.editor.text().to_string();
            if self.handle_slash_suggestion_input(event) {
                return;
            }
            self.editor.handle_input(event);
            if self.editor.text() != before_text {
                self.slash_suggestion_selected = 0;
                self.slash_suggestions_dismissed_for = None;
            }
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
                    self.editor.add_to_history(&text);
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

async fn run_interactive_loop_with_input<T, F>(
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
        prompt_context.api_key = resolve_prompt_api_key(
            &model.provider,
            prompt_context.cli_api_key.as_deref(),
            &prompt_context.auth,
        );
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
                .is_some_and(|session| matches!(session.mode, SessionMode::Enabled))
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

fn build_prompt_context(
    parsed: &CliArgs,
    options: CliRunOptions,
) -> Result<PromptContext, CliError> {
    let cwd = options.session.cwd.clone();
    let (config, config_diags) = config::load_config(&cwd);
    let diag_text = config::drain_diagnostics(&config_diags);
    if !diag_text.is_empty() {
        eprint!("{diag_text}");
    }
    let model = select_model(
        parsed,
        config.settings.default_provider.as_deref(),
        config.settings.default_model.as_deref(),
        options.model_override,
    )?;
    let model_rotation = rotation_model_choices(
        parsed.models.as_deref(),
        parsed
            .provider
            .as_deref()
            .or(config.settings.default_provider.as_deref()),
    )?;
    let provider = model.provider.clone();
    let api_key = resolve_prompt_api_key(&provider, parsed.api_key.as_deref(), &config.auth);
    let model_choices = configured_model_choices(&model, parsed.api_key.as_deref(), &config.auth);
    let config_paths = config::resolve_paths(&cwd);
    let loaded = resources::load_cli_resources_with_options(
        &parsed.skills,
        &parsed.prompt_templates,
        &cwd,
        &config_paths.global_dir,
        resources::ResourceLoadOptions {
            no_skills: parsed.no_skills,
            no_prompt_templates: parsed.no_prompt_templates,
            no_themes: parsed.no_themes,
            skill_paths: config.settings.skills.clone(),
            prompt_paths: config.settings.prompts.clone(),
            theme_paths: config.settings.themes.clone(),
            theme: config.settings.theme.clone(),
        },
    )?;
    let theme = resolve_tui_theme(
        config.settings.theme.as_deref(),
        loaded.selected_theme.as_ref(),
    );
    let (skills, templates, diagnostics) =
        (loaded.skills, loaded.prompt_templates, loaded.diagnostics);
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

    let context_files = resources::discover_context_files(
        &cwd,
        &config_paths.global_dir,
        effective_no_context_files(parsed, &config.settings),
    );
    let mut system_prompt = parsed.system_prompt.clone();
    if !context_files.is_empty() || !parsed.append_system_prompt.is_empty() {
        let mut parts = Vec::new();
        if let Some(base) = system_prompt.take() {
            parts.push(base);
        }
        for file in context_files {
            parts.push(format!(
                "# Context file: {}\n{}",
                file.path.display(),
                file.content
            ));
        }
        parts.extend(parsed.append_system_prompt.clone());
        system_prompt = Some(parts.join("\n\n"));
    }

    let session = if parsed.no_session {
        None
    } else {
        let mut session_opts = options.session;
        if let Some(dir) = effective_session_dir(parsed, &config.settings) {
            session_opts.session_dir = Some(dir);
        }
        Some(session_opts)
    };

    let session_target = match (&session, resolve_session_target(parsed)) {
        (Some(session), None) if matches!(session.mode, SessionMode::Enabled) => {
            Some(ResolvedSessionTarget::OpenOrCreateId(create_session_id()))
        }
        (_, target) => target,
    };
    let session_choices = collect_session_choices(&session);

    Ok(PromptContext {
        model,
        api_key,
        cli_api_key: parsed.api_key.clone(),
        auth: config.auth,
        system_prompt,
        max_turns: parsed.max_turns,
        tools: options.tools,
        register_builtins: options.register_builtins,
        session,
        session_target,
        session_name: parsed.name.clone(),
        thinking_level: parsed.thinking,
        tool_execution: parsed.tool_execution,
        resources: resources::build_agent_resources(skills, templates),
        settings: config.settings,
        theme,
        model_choices,
        model_rotation,
        session_choices,
    })
}

fn collect_session_choices(session: &Option<SessionRunOptions>) -> Vec<SessionChoice> {
    let Some(session) = session else {
        return Vec::new();
    };
    if !matches!(session.mode, SessionMode::Enabled) {
        return Vec::new();
    }

    let root = match &session.session_dir {
        Some(dir) => dir.clone(),
        None => match crate::session::resolve_session_dir(&session.cwd, None, None) {
            Ok(dir) => dir,
            Err(_) => return Vec::new(),
        },
    };
    let repo = JsonlSessionRepo::new(root);
    let cwd = session.cwd.display().to_string();
    repo.list(Some(&cwd))
        .unwrap_or_default()
        .into_iter()
        .map(session_choice_from_metadata)
        .collect()
}

fn session_choice_from_metadata(metadata: JsonlSessionMetadata) -> SessionChoice {
    let (name, entry_count) = JsonlSessionStorage::open(&metadata.path)
        .map(|storage| {
            let entries = storage.get_entries();
            let name = entries
                .iter()
                .rev()
                .find(|entry| entry.entry_type == "session_info")
                .and_then(|entry| entry.field("name"))
                .and_then(|value| value.as_str())
                .map(str::to_string);
            (name, entries.len())
        })
        .unwrap_or((None, 0));

    SessionChoice {
        id: metadata.id,
        cwd: metadata.cwd,
        path: metadata.path,
        created_at: metadata.created_at,
        name,
        entry_count,
    }
}

fn resolve_tui_theme(
    theme_name: Option<&str>,
    selected: Option<&resources::ThemeResource>,
) -> TuiTheme {
    if let Some(theme) = selected {
        return resources::tui_theme_from_resource(theme);
    }
    match theme_name {
        Some("light") => light_theme(),
        _ => dark_theme(),
    }
}

fn resolve_prompt_api_key(
    provider: &str,
    cli_api_key: Option<&str>,
    auth: &crate::config::AuthStore,
) -> Option<String> {
    let mut key_diags = Vec::new();
    let resolved = config::auth::resolve_api_key(provider, cli_api_key, auth, &mut key_diags);
    let key_text = config::drain_diagnostics(&key_diags);
    if !key_text.is_empty() {
        eprint!("{key_text}");
    }
    resolved.map(|r| r.value)
}

fn configured_model_choices(
    current_model: &Model,
    cli_api_key: Option<&str>,
    auth: &crate::config::AuthStore,
) -> Vec<Model> {
    let mut configured_providers = BTreeSet::new();
    for provider in pi_ai::get_providers() {
        if provider_has_configured_key(&provider, &current_model.provider, cli_api_key, auth) {
            configured_providers.insert(provider);
        }
    }

    let mut models = pi_ai::all_models()
        .iter()
        .filter(|model| configured_providers.contains(&model.provider))
        .cloned()
        .collect::<Vec<_>>();
    models.sort_by(|left, right| {
        left.provider
            .cmp(&right.provider)
            .then_with(|| left.id.cmp(&right.id))
    });
    if let Some(current_index) = models
        .iter()
        .position(|model| model.provider == current_model.provider && model.id == current_model.id)
    {
        let current = models.remove(current_index);
        models.insert(0, current);
    }
    models
}

fn rotation_model_choices(
    models_arg: Option<&str>,
    provider: Option<&str>,
) -> Result<Vec<Model>, CliError> {
    let Some(models_arg) = models_arg else {
        return Ok(Vec::new());
    };
    let rotation = crate::models::parse_model_rotation(models_arg)?;
    let mut candidates = pi_ai::all_models().to_vec();
    candidates.sort_by(|left, right| left.id.cmp(&right.id));
    if let Some(provider) = provider {
        candidates.retain(|model| model.provider == provider);
    }
    Ok(candidates
        .into_iter()
        .filter(|model| rotation.matches(&model.id) || rotation.matches(&model.name))
        .collect())
}

fn provider_has_configured_key(
    provider: &str,
    current_provider: &str,
    cli_api_key: Option<&str>,
    auth: &crate::config::AuthStore,
) -> bool {
    if provider == current_provider && cli_api_key.is_some_and(|key| !key.is_empty()) {
        return true;
    }
    let mut diags = Vec::new();
    config::auth::resolve_api_key(provider, None, auth, &mut diags).is_some()
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

fn slash_completion_query(text: &str, cursor: usize) -> Option<&str> {
    if cursor != text.len() || !text.starts_with('/') || text.contains('\n') {
        return None;
    }
    let query = &text[1..cursor];
    if query.chars().any(char::is_whitespace) {
        return None;
    }
    Some(query)
}

fn editor_border_line(width: usize, style: &Style, color: bool) -> String {
    if width == 0 {
        return String::new();
    }
    fit_line(&paint_with(&"─".repeat(width), style, color), width)
}

fn fit_line(line: &str, width: usize) -> String {
    if visible_width(line) <= width {
        line.to_string()
    } else {
        truncate_to_width(line, width)
    }
}

fn help_text() -> String {
    let mut lines = vec![
        "commands:".to_string(),
        "  /help, /h, /? - show this help".to_string(),
    ];
    for command in BUILTIN_SLASH_COMMANDS {
        if command.name == "help" {
            continue;
        }
        lines.push(format!("  /{:<13} - {}", command.name, command.description));
    }
    lines.push("  /q, /exit      - aliases for /quit".to_string());
    lines.join("\n")
}

fn welcome_line(keybindings: &KeybindingsManager) -> String {
    format!(
        "pi-rust {}\n{} · {} · /help\n{} · {}",
        env!("CARGO_PKG_VERSION"),
        key_hint(keybindings, "tui.input.submit", "submit"),
        key_hint(keybindings, "tui.input.newLine", "newline"),
        app_key_hint(keybindings, "app.interrupt", "interrupt/exit"),
        app_key_hint(keybindings, "app.tools.expand", "expand tools"),
    )
}

fn running_status_text(frame: usize) -> String {
    let mut loader = Loader::new("running");
    for _ in 0..frame {
        loader.tick();
    }
    loader.render_text()
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

    fn key_event(data: &str) -> InputEvent {
        let mut buffer = StdinBuffer::new();
        let mut events = buffer.process(data);
        events.extend(buffer.flush());
        assert_eq!(events.len(), 1, "expected exactly one input event");
        events.remove(0)
    }

    fn ctrl_p_event(shift: bool) -> InputEvent {
        let mut modifiers = pi_tui::KeyModifiers::CTRL;
        if shift {
            modifiers.insert(pi_tui::KeyModifiers::SHIFT);
        }
        InputEvent::Key(pi_tui::KeyEvent {
            key: pi_tui::Key::Char(if shift { "P".into() } else { "p".into() }),
            modifiers,
            kind: pi_tui::KeyEventKind::Press,
        })
    }

    #[test]
    fn build_prompt_context_uses_config_defaults_and_auth() {
        let _guard = crate::test_support::env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.toml"),
            "default_model = \"claude-haiku-4-5\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("auth.toml"),
            "[anthropic]\ntype = \"api_key\"\nkey = \"from-auth\"\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                dir.path().join("auth.toml"),
                std::fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }
        unsafe {
            std::env::set_var("PI_RUST_DIR", dir.path().to_str().unwrap());
        }

        let ctx = build_prompt_context(&CliArgs::default(), CliRunOptions::default()).unwrap();

        assert_eq!(ctx.model.id, "claude-haiku-4-5");
        assert_eq!(ctx.api_key.as_deref(), Some("from-auth"));

        unsafe {
            std::env::remove_var("PI_RUST_DIR");
        }
    }

    #[test]
    fn build_prompt_context_applies_selected_theme_to_editor_borders() {
        let _guard = crate::test_support::env_lock();
        let dir = tempfile::tempdir().unwrap();
        let themes_dir = dir.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        std::fs::write(dir.path().join("settings.toml"), "theme = \"violet\"\n").unwrap();
        std::fs::write(
            themes_dir.join("violet.json"),
            r##"{
                "name": "violet",
                "palette": {
                    "input_border": "#211144",
                    "menu_border": "#1234aa"
                }
            }"##,
        )
        .unwrap();
        unsafe {
            std::env::set_var("PI_RUST_DIR", dir.path().to_str().unwrap());
        }

        let ctx = build_prompt_context(&CliArgs::default(), CliRunOptions::default()).unwrap();

        assert_eq!(
            ctx.theme.editor.active_border.fg,
            pi_tui::Color::Rgb(0x21, 0x11, 0x44)
        );
        assert_eq!(
            ctx.theme.editor.menu_border.fg,
            pi_tui::Color::Rgb(0x12, 0x34, 0xaa)
        );

        unsafe {
            std::env::remove_var("PI_RUST_DIR");
        }
    }

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
    fn running_status_text_uses_loader_sequence() {
        assert_eq!(running_status_text(0), "⠋ running");
        assert_eq!(running_status_text(1), "⠙ running");
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
    fn slash_registry_contains_typescript_builtin_commands() {
        let names: Vec<&str> = BUILTIN_SLASH_COMMANDS
            .iter()
            .map(|command| command.name)
            .collect();
        assert_eq!(
            names,
            vec![
                "help",
                "settings",
                "model",
                "scoped-models",
                "export",
                "import",
                "share",
                "copy",
                "name",
                "session",
                "changelog",
                "hotkeys",
                "fork",
                "clone",
                "tree",
                "login",
                "logout",
                "new",
                "compact",
                "resume",
                "reload",
                "quit",
            ]
        );
    }

    #[test]
    fn slash_suggestions_render_when_editor_starts_with_slash() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.editor.set_text("/");

        let rendered = root.render(80).join("\n");

        assert!(rendered.contains("/help"), "{rendered}");
        assert!(rendered.contains("Show help"), "{rendered}");
        assert!(rendered.contains("/settings"), "{rendered}");
        assert!(rendered.contains("Open settings menu"), "{rendered}");
        assert!(rendered.contains("/model"), "{rendered}");
        assert!(rendered.contains("(1/22)"), "{rendered}");
    }

    #[test]
    fn editor_border_uses_active_theme_style_in_normal_input_state() {
        let theme = pi_tui::TuiTheme::custom(
            "custom",
            pi_tui::ThemePalette {
                accent: pi_tui::Color::Cyan,
                muted: pi_tui::Color::Ansi256(244),
                text: pi_tui::Color::White,
                background: pi_tui::Color::Default,
                error: pi_tui::Color::Red,
                success: pi_tui::Color::Green,
                warning: pi_tui::Color::Yellow,
                path: pi_tui::Color::Cyan,
                input_border: pi_tui::Color::Rgb(10, 20, 30),
                menu_border: pi_tui::Color::Rgb(40, 50, 60),
            },
        );
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        )
        .with_theme(theme);

        assert_eq!(
            root.editor_border_style().fg,
            pi_tui::Color::Rgb(10, 20, 30)
        );

        let rendered = root.render(40);
        let editor_row = rendered
            .iter()
            .position(|line| line.contains("> "))
            .expect("editor row should render");
        assert!(rendered[editor_row - 1].contains("─"), "{rendered:?}");
        assert!(rendered[editor_row + 1].contains("─"), "{rendered:?}");
    }

    #[test]
    fn settings_menu_uses_menu_theme_border_style() {
        let theme = pi_tui::TuiTheme::custom(
            "custom",
            pi_tui::ThemePalette {
                accent: pi_tui::Color::Cyan,
                muted: pi_tui::Color::Ansi256(244),
                text: pi_tui::Color::White,
                background: pi_tui::Color::Default,
                error: pi_tui::Color::Red,
                success: pi_tui::Color::Green,
                warning: pi_tui::Color::Yellow,
                path: pi_tui::Color::Cyan,
                input_border: pi_tui::Color::Rgb(10, 20, 30),
                menu_border: pi_tui::Color::Rgb(40, 50, 60),
            },
        );
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        )
        .with_theme(theme);

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });

        assert!(root.selecting_settings);
        assert_eq!(
            root.editor_border_style().fg,
            pi_tui::Color::Rgb(40, 50, 60)
        );
        let rendered = root.render(60).join("\n");
        assert!(rendered.contains("Settings"), "{rendered}");
        assert!(!rendered.contains("not implemented"), "{rendered}");
    }

    #[test]
    fn slash_suggestions_filter_and_hide_after_arguments() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.editor.set_text("/mo");
        let filtered = root.render(80).join("\n");
        assert!(filtered.contains("model"), "{filtered}");
        assert!(!filtered.contains("settings"), "{filtered}");

        root.editor.set_text("/model ");
        let with_argument_space = root.render(80).join("\n");
        assert!(
            !with_argument_space.contains("Select model"),
            "{with_argument_space}"
        );
        assert!(
            !with_argument_space.contains("(1/"),
            "{with_argument_space}"
        );
    }

    #[test]
    fn slash_suggestions_can_be_selected_and_accepted() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.editor.set_text("/");
        root.handle_input(&key_event("\x1b[B"));
        root.handle_input(&key_event("\x1b[B"));
        let moved = root.render(80).join("\n");
        assert!(moved.contains("(3/22)"), "{moved}");

        root.handle_input(&key_event("\t"));

        assert_eq!(root.editor.text(), "/model ");
        assert_eq!(root.take_action(), InteractiveAction::None);
        let rendered = root.render(80).join("\n");
        assert!(!rendered.contains("(2/21)"), "{rendered}");
    }

    #[test]
    fn slash_suggestions_can_be_cancelled_for_current_query() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.editor.set_text("/");

        root.handle_input(&key_event("\x1b"));
        let cancelled = root.render(80).join("\n");
        assert!(!cancelled.contains("Open settings menu"), "{cancelled}");

        root.handle_input(&key_event("m"));
        let changed = root.render(80).join("\n");
        assert!(changed.contains("model"), "{changed}");
    }

    #[test]
    fn ctrl_p_cycles_models_from_rotation() {
        let rotation = vec![
            pi_ai::lookup_model("claude-haiku-4-5").unwrap(),
            pi_ai::lookup_model("gpt-5").unwrap(),
            pi_ai::lookup_model("gpt-5-mini").unwrap(),
        ];
        let mut root = InteractiveRoot::new_with_theme_and_models(
            PathBuf::from("."),
            "claude-haiku-4-5".to_string(),
            "no-session".to_string(),
            dark_theme(),
            rotation.clone(),
        );
        root.model_rotation = rotation;

        root.handle_input(&ctrl_p_event(false));
        assert_eq!(root.model_id, "gpt-5");
        assert_eq!(root.take_selected_model().unwrap().id, "gpt-5");

        root.handle_input(&ctrl_p_event(false));
        assert_eq!(root.model_id, "gpt-5-mini");

        root.handle_input(&ctrl_p_event(true));
        assert_eq!(root.model_id, "gpt-5");
    }

    #[test]
    fn resume_command_opens_session_selector_and_selects_session() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("/tmp/project"),
            "faux-model".to_string(),
            "session".to_string(),
        );
        root.session_choices = vec![SessionChoice {
            id: "session-alpha".to_string(),
            cwd: "/tmp/project".to_string(),
            path: PathBuf::from("/tmp/sessions/session-alpha.jsonl"),
            created_at: "2026-06-20T00:00:00Z".to_string(),
            name: Some("Project Alpha".to_string()),
            entry_count: 3,
        }];

        root.handle_slash_command(ParsedSlashCommand {
            name: "resume".to_string(),
            args: String::new(),
            original: "/resume".to_string(),
        });

        assert!(root.selecting_session);
        let rendered = root.render(80).join("\n");
        assert!(rendered.contains("Select session"), "{rendered}");
        assert!(rendered.contains("Project Alpha"), "{rendered}");
        assert!(rendered.contains("session-alpha"), "{rendered}");

        root.handle_input(&key_event("\r"));

        let selected = root
            .take_selected_session()
            .expect("session selection should be returned to loop");
        assert_eq!(selected.id, "session-alpha");
        assert_eq!(
            selected.path,
            PathBuf::from("/tmp/sessions/session-alpha.jsonl")
        );
        assert!(!root.selecting_session);
    }

    #[test]
    fn session_selector_filters_by_name_and_cwd() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("/tmp/project"),
            "faux-model".to_string(),
            "session".to_string(),
        );
        root.session_choices = vec![
            SessionChoice {
                id: "session-alpha".to_string(),
                cwd: "/tmp/project".to_string(),
                path: PathBuf::from("/tmp/sessions/session-alpha.jsonl"),
                created_at: "2026-06-20T00:00:00Z".to_string(),
                name: Some("Project Alpha".to_string()),
                entry_count: 3,
            },
            SessionChoice {
                id: "session-beta".to_string(),
                cwd: "/tmp/other".to_string(),
                path: PathBuf::from("/tmp/sessions/session-beta.jsonl"),
                created_at: "2026-06-21T00:00:00Z".to_string(),
                name: Some("Beta Tools".to_string()),
                entry_count: 8,
            },
        ];
        root.handle_slash_command(ParsedSlashCommand {
            name: "resume".to_string(),
            args: String::new(),
            original: "/resume".to_string(),
        });

        root.handle_input(&key_event("B"));

        let rendered = root.render(80).join("\n");
        assert!(rendered.contains("Beta Tools"), "{rendered}");
        assert!(rendered.contains("/tmp/other"), "{rendered}");
        assert!(!rendered.contains("Project Alpha"), "{rendered}");
    }

    #[test]
    fn model_command_accepts_thinking_suffix() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "claude-haiku-4-5".to_string(),
            "no-session".to_string(),
        );

        root.handle_slash_command(ParsedSlashCommand {
            name: "model".to_string(),
            args: "gpt-5:high".to_string(),
            original: "/model gpt-5:high".to_string(),
        });

        assert_eq!(root.model_id, "gpt-5");
        assert_eq!(root.take_selected_model().unwrap().id, "gpt-5");
        assert_eq!(
            root.take_selected_thinking_level(),
            Some(pi_agent_core::ThinkingLevel::High)
        );
    }

    #[test]
    fn render_state_changes_when_slash_suggestion_selection_changes() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.editor.set_text("/");

        let before = root.render_state();
        root.handle_input(&key_event("\x1b[B"));

        assert_ne!(root.render_state(), before);
    }

    #[test]
    fn exact_slash_command_enter_submits_instead_of_accepting_suggestion() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.editor.set_text("/quit");

        root.handle_input(&key_event("\r"));

        assert_eq!(root.take_action(), InteractiveAction::Exit);
    }

    #[test]
    fn submitted_prompt_is_added_to_editor_history() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.editor.set_text("hello history");

        root.handle_input(&key_event("\r"));
        assert_eq!(root.take_action(), InteractiveAction::Submit);
        assert_eq!(root.take_pending_submit().as_deref(), Some("hello history"));

        root.handle_input(&key_event("\x1b[A"));

        assert_eq!(root.editor.text(), "hello history");
    }

    #[test]
    fn parse_slash_command_returns_command_name_and_arguments() {
        assert_eq!(
            parse_slash_command("/model gpt-5"),
            Some(ParsedSlashCommand {
                name: "model".to_string(),
                args: "gpt-5".to_string(),
                original: "/model gpt-5".to_string(),
            })
        );
        assert_eq!(
            parse_slash_command("/NAME Project Phoenix"),
            Some(ParsedSlashCommand {
                name: "name".to_string(),
                args: "Project Phoenix".to_string(),
                original: "/NAME Project Phoenix".to_string(),
            })
        );
    }

    #[test]
    fn parse_slash_command_preserves_non_slash_prompt_path() {
        assert_eq!(parse_slash_command("hello"), None);
        assert_eq!(parse_slash_command("  /quit"), None);
    }

    #[test]
    fn help_text_lists_all_builtin_commands() {
        let help = help_text();
        for command in BUILTIN_SLASH_COMMANDS {
            assert!(
                help.contains(&format!("/{}", command.name)),
                "help text should list /{}: {help}",
                command.name
            );
        }
    }

    #[test]
    fn handle_slash_command_quit_sets_exit_when_idle() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(ParsedSlashCommand {
            name: "quit".to_string(),
            args: String::new(),
            original: "/quit".to_string(),
        });
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
        root.handle_slash_command(ParsedSlashCommand {
            name: "quit".to_string(),
            args: String::new(),
            original: "/quit".to_string(),
        });
        assert_eq!(root.action, InteractiveAction::AbortRunning);
    }

    #[test]
    fn handle_slash_command_help_pushes_system_item() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(ParsedSlashCommand {
            name: "help".to_string(),
            args: String::new(),
            original: "/help".to_string(),
        });
        let text = last_system_text(&root);
        assert!(text.contains("/model"), "{text}");
        assert!(text.contains("/reload"), "{text}");
        assert_ne!(root.action, InteractiveAction::Submit);
        assert!(root.pending_submit.is_none());
    }

    #[test]
    fn handle_known_pending_command_reports_not_implemented_without_submit() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(ParsedSlashCommand {
            name: "scoped-models".to_string(),
            args: String::new(),
            original: "/scoped-models".to_string(),
        });
        let text = last_system_text(&root);
        assert!(text.contains("/scoped-models"), "{text}");
        assert!(text.contains("not implemented"), "{text}");
        assert_ne!(root.action, InteractiveAction::Submit);
        assert!(root.pending_submit.is_none());
    }

    #[test]
    fn copy_command_copies_last_assistant_message_to_clipboard() {
        let clipboard = Arc::new(TestClipboard::default());
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        )
        .with_clipboard(clipboard.clone());
        root.apply_events(vec![
            UiEvent::AssistantDelta {
                text: "first answer".to_string(),
            },
            UiEvent::AssistantDone,
            UiEvent::AssistantDelta {
                text: "second answer".to_string(),
            },
            UiEvent::AssistantDone,
        ]);

        root.handle_slash_command(ParsedSlashCommand {
            name: "copy".to_string(),
            args: String::new(),
            original: "/copy".to_string(),
        });

        assert_eq!(clipboard.last_text(), Some("second answer".to_string()));
        let text = last_system_text(&root);
        assert!(
            text.contains("Copied last agent message to clipboard"),
            "{text}"
        );
        assert!(!text.contains("not implemented"), "{text}");
        assert_ne!(root.action, InteractiveAction::Submit);
    }

    #[test]
    fn copy_command_reports_error_when_no_assistant_message_exists() {
        let clipboard = Arc::new(TestClipboard::default());
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        )
        .with_clipboard(clipboard.clone());

        root.handle_slash_command(ParsedSlashCommand {
            name: "copy".to_string(),
            args: String::new(),
            original: "/copy".to_string(),
        });

        assert_eq!(clipboard.last_text(), None);
        let text = last_system_text(&root);
        assert!(text.contains("No agent messages to copy yet."), "{text}");
        assert!(!text.contains("not implemented"), "{text}");
    }

    #[test]
    fn export_command_writes_current_transcript_to_jsonl_path() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("session-export.jsonl");
        let mut root = InteractiveRoot::new(
            dir.path().to_path_buf(),
            "faux-model".to_string(),
            "Project Phoenix".to_string(),
        );
        root.push_user("hello".to_string());
        root.apply_events(vec![
            UiEvent::AssistantDelta {
                text: "world".to_string(),
            },
            UiEvent::AssistantDone,
        ]);

        root.handle_slash_command(ParsedSlashCommand {
            name: "export".to_string(),
            args: output.display().to_string(),
            original: format!("/export {}", output.display()),
        });

        let text = std::fs::read_to_string(&output).unwrap();
        let lines = text.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 3, "{text}");
        assert!(lines[0].contains(r#""type":"session""#), "{text}");
        assert!(lines[0].contains(r#""version":3"#), "{text}");
        assert!(lines[1].contains(r#""role":"user""#), "{text}");
        assert!(lines[1].contains("hello"), "{text}");
        assert!(lines[2].contains(r#""role":"assistant""#), "{text}");
        assert!(lines[2].contains("world"), "{text}");
        let status = last_system_text(&root);
        assert!(status.contains("Session exported to:"), "{status}");
        assert!(!status.contains("not implemented"), "{status}");
    }

    #[test]
    fn export_command_writes_html_when_path_ends_with_html() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("session-export.html");
        let mut root = InteractiveRoot::new(
            dir.path().to_path_buf(),
            "faux-model".to_string(),
            "Project Phoenix".to_string(),
        );
        root.push_user("hello <user>".to_string());
        root.apply_events(vec![
            UiEvent::AssistantDelta {
                text: "world <assistant>".to_string(),
            },
            UiEvent::AssistantDone,
        ]);

        root.handle_slash_command(ParsedSlashCommand {
            name: "export".to_string(),
            args: output.display().to_string(),
            original: format!("/export {}", output.display()),
        });

        let text = std::fs::read_to_string(&output).unwrap();
        assert!(text.contains("<!doctype html>"), "{text}");
        assert!(text.contains("hello &lt;user&gt;"), "{text}");
        assert!(text.contains("world &lt;assistant&gt;"), "{text}");
        let status = last_system_text(&root);
        assert!(status.contains("Session exported to:"), "{status}");
    }

    #[test]
    fn new_command_clears_ui_state_and_requests_new_session() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "Project Phoenix".to_string(),
        );
        root.usage = (123, 456);
        root.push_user("old prompt".to_string());
        root.apply_events(vec![
            UiEvent::AssistantDelta {
                text: "old response".to_string(),
            },
            UiEvent::AssistantDone,
        ]);

        root.handle_slash_command(ParsedSlashCommand {
            name: "new".to_string(),
            args: String::new(),
            original: "/new".to_string(),
        });

        assert_eq!(root.action, InteractiveAction::NewSession);
        assert_eq!(root.usage, (0, 0));
        let rendered = root.render(80).join("\n");
        assert!(rendered.contains("New session started"), "{rendered}");
        assert!(!rendered.contains("old prompt"), "{rendered}");
        assert!(!rendered.contains("old response"), "{rendered}");
    }

    #[test]
    fn reload_command_requests_resource_reload() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );

        root.handle_slash_command(ParsedSlashCommand {
            name: "reload".to_string(),
            args: String::new(),
            original: "/reload".to_string(),
        });

        assert_eq!(root.action, InteractiveAction::ReloadResources);
        let text = last_system_text(&root);
        assert!(
            text.contains("Reloading keybindings and resources"),
            "{text}"
        );
        assert!(!text.contains("not implemented"), "{text}");
    }

    #[test]
    fn import_command_opens_jsonl_and_selects_session() {
        let dir = tempfile::tempdir().unwrap();
        let source = write_test_session(dir.path(), dir.path(), "hello import");
        let mut root = InteractiveRoot::new(
            dir.path().to_path_buf(),
            "faux-model".to_string(),
            "session".to_string(),
        );

        root.handle_slash_command(ParsedSlashCommand {
            name: "import".to_string(),
            args: format!("\"{}\"", source.display()),
            original: format!("/import \"{}\"", source.display()),
        });

        let selected = root
            .take_selected_session()
            .expect("/import should select imported session");
        assert_eq!(selected.path, source);
        assert_eq!(
            root.active_session_path.as_deref(),
            Some(selected.path.as_path())
        );
        assert!(root.active_leaf_id.is_some());
        let text = last_system_text(&root);
        assert!(text.contains("Session imported from:"), "{text}");
        assert!(!text.contains("not implemented"), "{text}");
    }

    #[test]
    fn import_command_reports_usage_without_path() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "session".to_string(),
        );

        root.handle_slash_command(ParsedSlashCommand {
            name: "import".to_string(),
            args: String::new(),
            original: "/import".to_string(),
        });

        assert!(root.take_selected_session().is_none());
        let text = last_system_text(&root);
        assert!(text.contains("Usage: /import <path.jsonl>"), "{text}");
        assert!(!text.contains("not implemented"), "{text}");
    }

    #[test]
    fn clone_command_forks_active_session_and_selects_clone() {
        let dir = tempfile::tempdir().unwrap();
        let source = write_test_session(dir.path(), dir.path(), "hello clone");
        let mut root = InteractiveRoot::new(
            dir.path().to_path_buf(),
            "faux-model".to_string(),
            "session".to_string(),
        );
        root.active_session_path = Some(source.clone());
        root.active_leaf_id = JsonlSessionStorage::open(&source)
            .unwrap()
            .get_leaf_id()
            .unwrap();

        root.handle_slash_command(ParsedSlashCommand {
            name: "clone".to_string(),
            args: String::new(),
            original: "/clone".to_string(),
        });

        let selected = root
            .take_selected_session()
            .expect("/clone should select cloned session");
        assert_ne!(selected.path, source);
        assert!(selected.path.exists(), "clone should create a session file");
        let cloned = std::fs::read_to_string(&selected.path).unwrap();
        assert!(cloned.contains("hello clone"), "{cloned}");
        assert!(cloned.contains("parentSession"), "{cloned}");
        assert_eq!(
            root.active_session_path.as_deref(),
            Some(selected.path.as_path())
        );
        let text = last_system_text(&root);
        assert!(text.contains("Cloned to new session"), "{text}");
        assert!(!text.contains("not implemented"), "{text}");
    }

    #[test]
    fn clone_command_reports_status_when_no_active_session_exists() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "session".to_string(),
        );

        root.handle_slash_command(ParsedSlashCommand {
            name: "clone".to_string(),
            args: String::new(),
            original: "/clone".to_string(),
        });

        assert!(root.take_selected_session().is_none());
        let text = last_system_text(&root);
        assert!(text.contains("Nothing to clone yet"), "{text}");
        assert!(!text.contains("not implemented"), "{text}");
    }

    #[test]
    fn handle_unknown_slash_command_reports_error_without_submit() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(ParsedSlashCommand {
            name: "does-not-exist".to_string(),
            args: String::new(),
            original: "/does-not-exist".to_string(),
        });
        let text = last_system_text(&root);
        assert!(text.contains("unknown command: /does-not-exist"), "{text}");
        assert_ne!(root.action, InteractiveAction::Submit);
        assert!(root.pending_submit.is_none());
    }

    #[test]
    fn name_command_without_args_shows_current_session_label() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "session-123".to_string(),
        );
        root.handle_slash_command(ParsedSlashCommand {
            name: "name".to_string(),
            args: String::new(),
            original: "/name".to_string(),
        });
        let text = last_system_text(&root);
        assert!(text.contains("session-123"), "{text}");
    }

    #[test]
    fn name_command_with_args_updates_session_label() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(ParsedSlashCommand {
            name: "name".to_string(),
            args: "Project Phoenix".to_string(),
            original: "/name Project Phoenix".to_string(),
        });
        assert_eq!(root.session_label, "Project Phoenix");
        let text = last_system_text(&root);
        assert!(text.contains("Session name set: Project Phoenix"), "{text}");
    }

    #[test]
    fn session_command_reports_current_footer_state() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("/tmp/project"),
            "faux-model".to_string(),
            "Project Phoenix".to_string(),
        );
        root.usage = (1234, 5678);
        root.handle_slash_command(ParsedSlashCommand {
            name: "session".to_string(),
            args: String::new(),
            original: "/session".to_string(),
        });
        let text = last_system_text(&root);
        assert!(text.contains("Session Info"), "{text}");
        assert!(text.contains("Project Phoenix"), "{text}");
        assert!(text.contains("faux-model"), "{text}");
        assert!(text.contains("1k"), "{text}");
        assert!(text.contains("5k"), "{text}");
    }

    #[test]
    fn hotkeys_command_mentions_core_interactive_bindings() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(ParsedSlashCommand {
            name: "hotkeys".to_string(),
            args: String::new(),
            original: "/hotkeys".to_string(),
        });
        let text = last_system_text(&root);
        assert!(text.contains("Navigation"), "{text}");
        assert!(text.contains("Ctrl+C"), "{text}");
        assert!(text.contains("Ctrl+O"), "{text}");
    }

    #[tokio::test]
    async fn real_stdin_reader_is_created_after_terminal_start() {
        #[derive(Default)]
        struct OrderingTerminal {
            events: Arc<Mutex<Vec<&'static str>>>,
        }

        impl Terminal for OrderingTerminal {
            fn size(&self) -> pi_tui::TerminalSize {
                pi_tui::TerminalSize {
                    columns: 80,
                    rows: 24,
                }
            }

            fn write(&mut self, _data: &str) -> std::io::Result<()> {
                Ok(())
            }

            fn move_by(&mut self, _rows: i16) -> std::io::Result<()> {
                Ok(())
            }

            fn move_to_column(&mut self, _column: usize) -> std::io::Result<()> {
                Ok(())
            }

            fn hide_cursor(&mut self) -> std::io::Result<()> {
                Ok(())
            }

            fn show_cursor(&mut self) -> std::io::Result<()> {
                Ok(())
            }

            fn clear_line(&mut self) -> std::io::Result<()> {
                Ok(())
            }

            fn clear_from_cursor(&mut self) -> std::io::Result<()> {
                Ok(())
            }

            fn clear_screen(&mut self) -> std::io::Result<()> {
                Ok(())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }

            fn start(&mut self) -> std::io::Result<()> {
                self.events.lock().unwrap().push("start");
                Ok(())
            }
        }

        let events = Arc::new(Mutex::new(Vec::new()));
        let terminal = OrderingTerminal {
            events: Arc::clone(&events),
        };
        let parsed = CliArgs::default();
        let options = CliRunOptions {
            register_builtins: false,
            ..CliRunOptions::default()
        };

        let result = run_interactive_loop_with_input(parsed, options, terminal, || {
            events.lock().unwrap().push("input");
            InputPump::from_chunks(Vec::new())
        })
        .await;

        if let Err(error) = result {
            panic!("interactive loop should complete: {error}");
        }
        assert_eq!(&*events.lock().unwrap(), &["start", "input"]);
    }

    fn last_system_text(root: &InteractiveRoot) -> String {
        match root.transcript.items().last() {
            Some(TranscriptItem::System { text }) => text.clone(),
            other => panic!("expected last transcript item to be System, got {other:?}"),
        }
    }

    fn write_test_session(root: &Path, cwd: &Path, text: &str) -> PathBuf {
        let path = root.join(format!("{}.jsonl", create_session_id()));
        let timestamp = create_timestamp();
        let mut storage = JsonlSessionStorage::create(
            &path,
            cwd.display().to_string(),
            "test-session",
            timestamp.clone(),
            None,
        )
        .unwrap();
        storage
            .append_entry(SessionEntry::message(
                "entry-user".to_string(),
                None,
                timestamp.clone(),
                StoredAgentMessage::User {
                    content: vec![ContentBlock::Text {
                        text: text.to_string(),
                        text_signature: None,
                    }],
                    timestamp: 0,
                },
            ))
            .unwrap();
        storage
            .append_entry(SessionEntry::message(
                "entry-assistant".to_string(),
                Some("entry-user".to_string()),
                timestamp,
                StoredAgentMessage::Assistant {
                    content: vec![ContentBlock::Text {
                        text: format!("response to {text}"),
                        text_signature: None,
                    }],
                    api: "test".to_string(),
                    provider: "test".to_string(),
                    model: "faux-model".to_string(),
                    response_model: None,
                    response_id: None,
                    usage: StoredUsage::default(),
                    stop_reason: StopReason::Stop,
                    error_message: None,
                    timestamp: 0,
                },
            ))
            .unwrap();
        path
    }

    #[derive(Default)]
    struct TestClipboard {
        text: Mutex<Option<String>>,
    }

    impl ClipboardSink for TestClipboard {
        fn copy_text(&self, text: &str) -> Result<(), String> {
            *self.text.lock().unwrap() = Some(text.to_string());
            Ok(())
        }
    }

    impl TestClipboard {
        fn last_text(&self) -> Option<String> {
            self.text.lock().unwrap().clone()
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

    /// Drive the interactive loop with a sequence of `(chunk, post_delay)`
    /// steps. After each chunk is sent, the harness sleeps `post_delay`
    /// before sending the next chunk (or, on the final step, before closing
    /// stdin and letting the loop terminate). This allows tests to exercise
    /// the [`StdinBuffer`] idle-flush timer for stuck escape sequences.
    pub async fn run_scripted_idle_interactive_with_delays(
        steps: Vec<(&str, Duration)>,
        columns: usize,
        rows: usize,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut input = InputPump { rx, _reader: None };
        let parsed = CliArgs::default();
        let options = CliRunOptions {
            register_builtins: false,
            ..CliRunOptions::default()
        };

        let owned_steps = steps
            .into_iter()
            .map(|(chunk, delay)| (chunk.to_string(), delay))
            .collect::<Vec<_>>();
        let driver = async move {
            for (chunk, delay) in owned_steps {
                if tx.send(chunk).is_err() {
                    return;
                }
                if delay > Duration::ZERO {
                    tokio::time::sleep(delay).await;
                }
            }
            drop(tx);
        };

        let run = run_interactive_loop(
            parsed,
            options,
            VirtualTerminal::new(columns, rows),
            &mut input,
        );
        let (result, ()) = tokio::join!(run, driver);
        Ok(scripted_output(result?, None))
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
