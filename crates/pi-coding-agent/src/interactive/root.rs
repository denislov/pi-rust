use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pi_ai::types::Model;
use pi_tui::{
    Component, ERROR, Editor, InputEvent, Key, KeyEventKind, KeyModifiers, KeybindingsManager,
    MarkdownTheme, STATUS_IDLE, STATUS_RUNNING, SYSTEM, SettingItem, SettingsList,
    SettingsListOptions, Style, TUI_KEYBINDINGS, TuiTheme, color_enabled, dark_theme, light_theme,
    matches_key, paint_with, truncate_to_width, truncate_to_width_with_ellipsis, visible_width,
};

use crate::coding_session::{ProfileId, ProfileRegistry, ProfileRegistryOptions};
use crate::config::{AuthStore, Settings};
use crate::interactive::app::{PromptContext, welcome_line};
use crate::interactive::clipboard::{ClipboardSink, SystemClipboard};
use crate::interactive::commands;
use crate::interactive::git_branch::GitBranchProvider;
use crate::interactive::input;
use crate::interactive::model_selector;
use crate::interactive::render::{
    TranscriptRenderCache, TranscriptRenderOptions, TranscriptRowSnapshot, TranscriptStyles,
    WARNING, abbreviate_cwd, editor_border_line, fit_line, format_tokens, running_status_text,
};
use crate::interactive::session_actions::{HydratedSession, SessionChoice};
use crate::interactive::session_selector;
use crate::interactive::slash::{self, ParsedSlashCommand};
use crate::interactive::transcript::TranscriptMutation;
use crate::interactive::tree_selector::{TreeSelectorInput, TreeSelectorState};
use crate::interactive::{Transcript, TranscriptItem, UiEvent};
use crate::theme::{ResolvedTheme, ThemeColor};

const MAX_TOOL_RESULT_LINES: usize = 3;
const EXPANDED_TOOL_RESULT_LINES: usize = 20;
pub(super) const DOUBLE_ESCAPE_WINDOW: Duration = Duration::from_millis(500);

const HTTP_IDLE_TIMEOUT_CHOICES: [(&str, u64); 5] = [
    ("30 sec", 30_000),
    ("1 min", 60_000),
    ("2 min", 120_000),
    ("5 min", 300_000),
    ("disabled", 0),
];

fn profile_registry_for_cwd(cwd: &Path) -> ProfileRegistry {
    let paths = crate::config::resolve_paths(cwd);
    ProfileRegistry::load(
        ProfileRegistryOptions::new()
            .with_user_root(paths.global_dir)
            .with_project_root(paths.project_dir),
    )
    .unwrap_or_else(|_| {
        ProfileRegistry::load(ProfileRegistryOptions::new())
            .expect("built-in default profile registry should load")
    })
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InteractiveAction {
    None,
    Submit,
    FollowUp,
    CompactSession,
    BranchSummary,
    PluginCommand,
    PluginUiAction,
    PluginUiDialog,
    AgentProfileUse,
    AgentInvocation,
    AgentTeam,
    AbortRunning,
    NewSession,
    ReloadResources,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InteractiveStatus {
    Idle,
    Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TranscriptScrollCommand {
    PageUp,
    PageDown,
}

/// Cumulative token/cost statistics and live context estimate for the footer.
///
/// Mirrors the values the TypeScript `FooterComponent.render` computes by
/// iterating session entries, plus the context estimate from `getContextUsage`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct FooterStats {
    pub input: u32,
    pub output: u32,
    pub cache_read: u32,
    pub cache_write: u32,
    pub cost: f64,
    /// Estimated context tokens from the last assistant usage. `None` means
    /// unknown (e.g. right after compaction, before the next LLM response).
    pub context_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingBranchSummaryRequest {
    pub(super) source_leaf_id: String,
    pub(super) target_leaf_id: String,
    pub(super) custom_instructions: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingAgentInvocationRequest {
    pub(super) profile_id: ProfileId,
    pub(super) task: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingAgentTeamRequest {
    pub(super) team_id: ProfileId,
    pub(super) task: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PendingPluginCommandRequest {
    pub(super) command_id: String,
    pub(super) args: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingPluginUiAction {
    pub(super) action_id: String,
    pub(super) label: String,
    pub(super) description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PluginUiDialogField {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) description: String,
    pub(super) kind: String,
    pub(super) default_value: serde_json::Value,
    pub(super) required: bool,
    pub(super) options: Vec<String>,
}

impl PluginUiDialogField {
    pub(super) fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
        kind: impl Into<String>,
        default_value: serde_json::Value,
        required: bool,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: description.into(),
            kind: kind.into(),
            default_value,
            required,
            options: Vec::new(),
        }
    }

    pub(super) fn with_options(mut self, options: Vec<String>) -> Self {
        self.options = options;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingPluginUiDialog {
    pub(super) dialog_id: String,
    pub(super) title: String,
    pub(super) description: String,
    pub(super) action_id: String,
    pub(super) fields: Vec<PluginUiDialogField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PluginDialogFormError {
    pub(super) field_id: String,
    pub(super) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActivePluginUiDialog {
    pub(super) dialog: PendingPluginUiDialog,
    pub(super) values: Vec<String>,
    pub(super) selected_field: usize,
    pub(super) validation_error: Option<PluginDialogFormError>,
}

impl ActivePluginUiDialog {
    pub(super) fn new(dialog: PendingPluginUiDialog) -> Self {
        let values = dialog
            .fields
            .iter()
            .map(plugin_dialog_initial_field_text)
            .collect();
        Self {
            dialog,
            values,
            selected_field: 0,
            validation_error: None,
        }
    }

    fn selected_field_mut(&mut self) -> Option<(&PluginUiDialogField, &mut String)> {
        let field = self.dialog.fields.get(self.selected_field)?;
        let value = self.values.get_mut(self.selected_field)?;
        Some((field, value))
    }

    fn selected_field_id(&self) -> Option<String> {
        self.dialog
            .fields
            .get(self.selected_field)
            .map(|field| field.id.clone())
    }

    fn clear_validation_error_for_selected_field(&mut self) {
        let Some(field_id) = self.selected_field_id() else {
            return;
        };
        if self
            .validation_error
            .as_ref()
            .is_some_and(|error| error.field_id == field_id)
        {
            self.validation_error = None;
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.dialog.fields.len();
        if len == 0 {
            self.selected_field = 0;
            return;
        }
        let current = self.selected_field.min(len - 1) as isize;
        let next = (current + delta).rem_euclid(len as isize) as usize;
        self.selected_field = next;
    }

    fn args_json(&self) -> serde_json::Value {
        let mut args = serde_json::Map::new();
        for (index, field) in self.dialog.fields.iter().enumerate() {
            let raw = self
                .values
                .get(index)
                .map(String::as_str)
                .unwrap_or_default();
            args.insert(field.id.clone(), plugin_dialog_form_value(field, raw));
        }
        serde_json::Value::Object(args)
    }
}

fn plugin_dialog_initial_field_text(field: &PluginUiDialogField) -> String {
    let value = match &field.default_value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(value) => value.clone(),
        other => other.to_string(),
    };
    if value.is_empty() && plugin_dialog_field_is_choice(field) {
        field.options.first().cloned().unwrap_or(value)
    } else {
        value
    }
}

fn plugin_dialog_form_value(field: &PluginUiDialogField, raw: &str) -> serde_json::Value {
    if !field.default_value.is_null() && raw == plugin_dialog_initial_field_text(field) {
        return field.default_value.clone();
    }

    let kind = normalized_dialog_field_kind(&field.kind);
    match kind.as_str() {
        "text" | "string" | "select" | "choice" | "enum" => {
            serde_json::Value::String(raw.to_string())
        }
        "boolean" | "bool" => plugin_dialog_bool_value(raw)
            .map(serde_json::Value::Bool)
            .unwrap_or_else(|| serde_json::Value::String(raw.to_string())),
        "integer" => plugin_dialog_integer_value(raw)
            .map(serde_json::Value::Number)
            .unwrap_or_else(|| serde_json::Value::String(raw.to_string())),
        "number" => plugin_dialog_number_value(raw)
            .map(serde_json::Value::Number)
            .unwrap_or_else(|| serde_json::Value::String(raw.to_string())),
        _ => {
            serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_string()))
        }
    }
}

fn plugin_dialog_field_is_bool(field: &PluginUiDialogField) -> bool {
    matches!(
        normalized_dialog_field_kind(&field.kind).as_str(),
        "boolean" | "bool"
    )
}

fn plugin_dialog_field_is_choice(field: &PluginUiDialogField) -> bool {
    matches!(
        normalized_dialog_field_kind(&field.kind).as_str(),
        "select" | "choice" | "enum"
    )
}

fn plugin_dialog_next_choice_value(field: &PluginUiDialogField, current: &str) -> Option<String> {
    if !plugin_dialog_field_is_choice(field) || field.options.is_empty() {
        return None;
    }
    let current = current.trim();
    let next_index = field
        .options
        .iter()
        .position(|option| option == current)
        .map(|index| (index + 1) % field.options.len())
        .unwrap_or(0);
    field.options.get(next_index).cloned()
}

fn plugin_dialog_filtered_insert(field: &PluginUiDialogField, current: &str, text: &str) -> String {
    match normalized_dialog_field_kind(&field.kind).as_str() {
        "integer" => plugin_dialog_numeric_filtered_insert(current, text, false),
        "number" => plugin_dialog_numeric_filtered_insert(current, text, true),
        "select" | "choice" | "enum" => String::new(),
        _ => text.to_string(),
    }
}

fn plugin_dialog_numeric_filtered_insert(current: &str, text: &str, allow_float: bool) -> String {
    let mut candidate = current.to_string();
    let mut accepted = String::new();
    for ch in text.chars() {
        if plugin_dialog_numeric_char_allowed(&candidate, ch, allow_float) {
            candidate.push(ch);
            accepted.push(ch);
        }
    }
    accepted
}

fn plugin_dialog_numeric_char_allowed(current: &str, ch: char, allow_float: bool) -> bool {
    if ch.is_ascii_digit() {
        return true;
    }
    match ch {
        '+' | '-' => {
            current.is_empty()
                || (allow_float && (current.ends_with('e') || current.ends_with('E')))
        }
        '.' => {
            allow_float
                && !current.contains('.')
                && !current.contains('e')
                && !current.contains('E')
        }
        'e' | 'E' => {
            allow_float
                && !current.contains('e')
                && !current.contains('E')
                && current.chars().any(|existing| existing.is_ascii_digit())
        }
        _ => false,
    }
}

fn plugin_dialog_bool_value(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn plugin_dialog_integer_value(raw: &str) -> Option<serde_json::Number> {
    let trimmed = raw.trim();
    if let Ok(value) = trimmed.parse::<i64>() {
        return Some(value.into());
    }
    trimmed.parse::<u64>().ok().map(Into::into)
}

fn plugin_dialog_number_value(raw: &str) -> Option<serde_json::Number> {
    let trimmed = raw.trim();
    if let Some(integer) = plugin_dialog_integer_value(trimmed) {
        return Some(integer);
    }
    trimmed
        .parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
}

fn plugin_dialog_field_kind_label(field: &PluginUiDialogField) -> String {
    if plugin_dialog_field_is_choice(field) && !field.options.is_empty() {
        format!("{}: {}", field.kind, field.options.join("/"))
    } else {
        field.kind.clone()
    }
}

fn normalized_dialog_field_kind(kind: &str) -> String {
    kind.trim().replace('-', "_").to_ascii_lowercase()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PluginUiAction {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) description: String,
    pub(super) action_id: String,
}

impl PluginUiAction {
    pub(super) fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
        action_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: description.into(),
            action_id: action_id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PluginUiDialog {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) description: String,
    pub(super) action_id: String,
    pub(super) fields: Vec<PluginUiDialogField>,
}

impl PluginUiDialog {
    pub(super) fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        action_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            action_id: action_id.into(),
            fields: Vec::new(),
        }
    }

    pub(super) fn with_fields(mut self, fields: Vec<PluginUiDialogField>) -> Self {
        self.fields = fields;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PluginKeybinding {
    pub(super) id: String,
    pub(super) key: String,
    pub(super) description: String,
    pub(super) action_id: String,
}

impl PluginKeybinding {
    pub(super) fn new(
        id: impl Into<String>,
        key: impl Into<String>,
        description: impl Into<String>,
        action_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            key: key.into(),
            description: description.into(),
            action_id: action_id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PluginSlashCommand {
    pub(super) command_id: String,
    pub(super) description: String,
}

impl PluginSlashCommand {
    pub(super) fn new(command_id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            command_id: command_id.into(),
            description: description.into(),
        }
    }
}

pub(super) struct InteractiveRoot {
    pub(super) selecting_tree: bool,
    pub(super) tree_selector: Option<TreeSelectorState>,
    pub(super) selected_tree_entry_id: Option<String>,
    pub(super) pending_tree_label_change: Option<(String, Option<String>)>,
    pub(super) transcript: Transcript,
    render_cache: TranscriptRenderCache,
    pub(super) editor: Editor,
    pub(super) keybindings: KeybindingsManager,
    pub(super) submitted: Arc<Mutex<Option<String>>>,
    pub(super) scroll_command: Arc<Mutex<Option<TranscriptScrollCommand>>>,
    pub(super) pending_submit: Option<String>,
    pub(super) pending_compact_instructions: Option<String>,
    pub(super) pending_branch_summary_request: Option<PendingBranchSummaryRequest>,
    pub(super) pending_agent_invocation_request: Option<PendingAgentInvocationRequest>,
    pub(super) pending_agent_team_request: Option<PendingAgentTeamRequest>,
    pub(super) pending_plugin_command_request: Option<PendingPluginCommandRequest>,
    pub(super) pending_plugin_ui_action: Option<PendingPluginUiAction>,
    pub(super) pending_plugin_ui_dialog: Option<PendingPluginUiDialog>,
    pub(super) active_plugin_ui_dialog: Option<ActivePluginUiDialog>,
    pub(super) selected_agent_profile_id: Option<ProfileId>,
    pub(super) action: InteractiveAction,
    pub(super) status: InteractiveStatus,
    pub(super) viewport_width: usize,
    pub(super) viewport_height: usize,
    pub(super) cwd: PathBuf,
    pub(super) model_id: String,
    pub(super) session_label: String,
    pub(super) selected_model: Option<Model>,
    pub(super) selected_thinking_level: Option<pi_agent_core::ThinkingLevel>,
    /// Currently active model for footer display. Distinct from
    /// `selected_model`, which is consumed to apply pending changes.
    pub(super) model: Option<Model>,
    /// Currently active thinking level (never consumed by `take_*`).
    pub(super) thinking_level: pi_agent_core::ThinkingLevel,
    pub(super) available_models: Vec<Model>,
    pub(super) model_rotation: Vec<Model>,
    pub(super) selecting_model: bool,
    pub(super) model_selection_selected: usize,
    pub(super) session_choices: Vec<SessionChoice>,
    pub(super) selected_session: Option<SessionChoice>,
    pub(super) selected_session_hydrate: bool,
    pub(super) active_session: Option<SessionChoice>,
    pub(super) active_session_path: Option<PathBuf>,
    pub(super) active_leaf_id: Option<String>,
    pub(super) selecting_session: bool,
    pub(super) session_selection_selected: usize,
    pub(super) selecting_settings: bool,
    pub(super) settings: Settings,
    settings_list: SettingsList,
    settings_update: Option<Settings>,
    settings_delta: crate::config::settings::PartialSettings,
    pub(super) auth: AuthStore,
    auth_update: Option<AuthStore>,
    pub(super) git_branch: GitBranchProvider,
    pub(super) stats: FooterStats,
    pub(super) tool_output_expanded: bool,
    pub(super) spinner_frame: usize,
    pub(super) slash_suggestion_selected: usize,
    pub(super) slash_suggestions_dismissed_for: Option<String>,
    last_empty_editor_escape_at: Option<Instant>,
    pub(super) theme: TuiTheme,
    pub(super) resolved_theme: Option<ResolvedTheme>,
    pub(super) prompt_templates: Vec<pi_agent_core::PromptTemplate>,
    pub(super) skills: Vec<pi_agent_core::Skill>,
    pub(super) profile_registry: ProfileRegistry,
    pub(super) default_agent_profile_id: ProfileId,
    plugin_commands: Vec<PluginSlashCommand>,
    plugin_ui_actions: Vec<PluginUiAction>,
    plugin_ui_dialogs: Vec<PluginUiDialog>,
    plugin_keybindings: Vec<PluginKeybinding>,
    pub(super) clipboard: Arc<dyn ClipboardSink>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct InteractiveRenderState {
    editor_text: String,
    editor_cursor: usize,
    transcript_revision: u64,
    transcript_scroll_offset: usize,
    transcript_has_new_output_below: bool,
    status: InteractiveStatus,
    tool_output_expanded: bool,
    spinner_frame: usize,
    slash_suggestion_selected: usize,
    slash_suggestions_dismissed_for: Option<String>,
    selecting_settings: bool,
    selecting_tree: bool,
    tree_selector_state: Option<crate::interactive::tree_selector::TreeSelectorRenderState>,
    settings: Settings,
    auth: AuthStore,
    theme_name: String,
    settings_selected_item_id: Option<String>,
    selecting_model: bool,
    model_selection_selected: usize,
    selecting_session: bool,
    session_selection_selected: usize,
    active_plugin_ui_dialog: Option<ActivePluginUiDialog>,
}

impl InteractiveRoot {
    #[cfg(test)]
    pub(super) fn new(cwd: PathBuf, model_id: String, session_label: String) -> Self {
        Self::new_with_theme(cwd, model_id, session_label, pi_tui::dark_theme())
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

    #[cfg(test)]
    pub(super) fn new_with_theme_and_models(
        cwd: PathBuf,
        model_id: String,
        session_label: String,
        theme: TuiTheme,
        available_models: Vec<Model>,
    ) -> Self {
        Self::new_with_theme_models_and_settings(
            cwd,
            model_id,
            session_label,
            theme,
            available_models,
            crate::config::settings::PartialSettings::default().resolve(),
            AuthStore::default(),
        )
    }

    pub(super) fn new_with_theme_models_and_settings(
        cwd: PathBuf,
        model_id: String,
        session_label: String,
        theme: TuiTheme,
        available_models: Vec<Model>,
        settings: Settings,
        auth: AuthStore,
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
        let settings_list = build_settings_list(settings.clone(), &theme, keybindings.clone());
        let profile_registry = profile_registry_for_cwd(&cwd);

        Self {
            selecting_tree: false,
            tree_selector: None,
            selected_tree_entry_id: None,
            pending_tree_label_change: None,
            transcript,
            render_cache: TranscriptRenderCache::new(),
            editor,
            keybindings,
            submitted,
            scroll_command,
            pending_submit: None,
            pending_compact_instructions: None,
            pending_branch_summary_request: None,
            pending_agent_invocation_request: None,
            pending_agent_team_request: None,
            pending_plugin_command_request: None,
            pending_plugin_ui_action: None,
            pending_plugin_ui_dialog: None,
            active_plugin_ui_dialog: None,
            selected_agent_profile_id: None,
            action: InteractiveAction::None,
            status: InteractiveStatus::Idle,
            viewport_width: 80,
            viewport_height: 24,
            git_branch: GitBranchProvider::new(&cwd),
            cwd,
            model_id,
            session_label,
            selected_model: None,
            selected_thinking_level: None,
            model: None,
            thinking_level: pi_agent_core::ThinkingLevel::default(),
            available_models,
            model_rotation: Vec::new(),
            selecting_model: false,
            model_selection_selected: 0,
            session_choices: Vec::new(),
            selected_session: None,
            selected_session_hydrate: false,
            active_session: None,
            active_session_path: None,
            active_leaf_id: None,
            selecting_session: false,
            session_selection_selected: 0,
            selecting_settings: false,
            settings,
            settings_list,
            settings_update: None,
            settings_delta: crate::config::settings::PartialSettings::default(),
            auth,
            auth_update: None,
            stats: FooterStats::default(),
            tool_output_expanded: false,
            spinner_frame: 0,
            slash_suggestion_selected: 0,
            slash_suggestions_dismissed_for: None,
            last_empty_editor_escape_at: None,
            theme,
            resolved_theme: None,
            prompt_templates: Vec::new(),
            skills: Vec::new(),
            profile_registry,
            default_agent_profile_id: ProfileId::from("default"),
            plugin_commands: Vec::new(),
            plugin_ui_actions: Vec::new(),
            plugin_ui_dialogs: Vec::new(),
            plugin_keybindings: Vec::new(),
            clipboard: Arc::new(SystemClipboard),
        }
    }

    #[cfg(test)]
    pub(super) fn with_theme(mut self, theme: TuiTheme) -> Self {
        self.theme = theme;
        self
    }

    pub(super) fn with_resolved_theme(mut self, resolved_theme: ResolvedTheme) -> Self {
        self.resolved_theme = Some(resolved_theme);
        self
    }

    #[cfg(test)]
    pub(super) fn with_clipboard(mut self, clipboard: Arc<dyn ClipboardSink>) -> Self {
        self.clipboard = clipboard;
        self
    }

    pub(super) fn take_action(&mut self) -> InteractiveAction {
        std::mem::replace(&mut self.action, InteractiveAction::None)
    }

    pub(super) fn take_selected_model(&mut self) -> Option<Model> {
        self.selected_model.take()
    }

    pub(super) fn take_selected_thinking_level(&mut self) -> Option<pi_agent_core::ThinkingLevel> {
        self.selected_thinking_level.take()
    }

    pub(super) fn take_selected_agent_profile_id(&mut self) -> Option<ProfileId> {
        self.selected_agent_profile_id.take()
    }

    pub(super) fn set_default_agent_profile_id(&mut self, profile_id: ProfileId) {
        self.default_agent_profile_id = profile_id;
        self.slash_suggestion_selected = 0;
        self.slash_suggestions_dismissed_for = None;
    }

    pub(super) fn take_selected_session(&mut self) -> Option<SessionChoice> {
        self.selected_session.take()
    }

    pub(super) fn take_selected_session_hydrate(&mut self) -> bool {
        std::mem::take(&mut self.selected_session_hydrate)
    }

    pub(super) fn take_selected_tree_entry_id(&mut self) -> Option<String> {
        self.selected_tree_entry_id.take()
    }

    pub(super) fn take_pending_tree_label_change(&mut self) -> Option<(String, Option<String>)> {
        self.pending_tree_label_change.take()
    }

    pub(super) fn take_settings_update(&mut self) -> Option<Settings> {
        self.settings_update.take()
    }

    pub(super) fn settings_delta(&self) -> &crate::config::settings::PartialSettings {
        &self.settings_delta
    }

    pub(super) fn take_auth_update(&mut self) -> Option<AuthStore> {
        self.auth_update.take()
    }

    pub(super) fn take_submitted(&mut self) -> Option<String> {
        self.submitted.lock().unwrap().take()
    }

    pub(super) fn take_pending_submit(&mut self) -> Option<String> {
        self.pending_submit.take()
    }

    pub(super) fn take_pending_compact_instructions(&mut self) -> Option<String> {
        self.pending_compact_instructions.take()
    }

    pub(super) fn take_pending_branch_summary_request(
        &mut self,
    ) -> Option<PendingBranchSummaryRequest> {
        self.pending_branch_summary_request.take()
    }

    pub(super) fn take_pending_agent_invocation_request(
        &mut self,
    ) -> Option<PendingAgentInvocationRequest> {
        self.pending_agent_invocation_request.take()
    }

    pub(super) fn take_pending_agent_team_request(&mut self) -> Option<PendingAgentTeamRequest> {
        self.pending_agent_team_request.take()
    }

    pub(super) fn take_pending_plugin_command_request(
        &mut self,
    ) -> Option<PendingPluginCommandRequest> {
        self.pending_plugin_command_request.take()
    }

    pub(super) fn take_pending_plugin_ui_action(&mut self) -> Option<PendingPluginUiAction> {
        self.pending_plugin_ui_action.take()
    }

    pub(super) fn take_pending_plugin_ui_dialog(&mut self) -> Option<PendingPluginUiDialog> {
        self.pending_plugin_ui_dialog.take()
    }

    pub(super) fn take_scroll_command(&mut self) -> Option<TranscriptScrollCommand> {
        self.scroll_command.lock().unwrap().take()
    }

    pub(super) fn has_active_plugin_ui_dialog(&self) -> bool {
        self.active_plugin_ui_dialog.is_some()
    }

    pub(super) fn focus_active_plugin_dialog_field(&mut self, field_id: &str) {
        if let Some(active_dialog) = self.active_plugin_ui_dialog.as_mut()
            && let Some(index) = active_dialog
                .dialog
                .fields
                .iter()
                .position(|field| field.id == field_id)
        {
            active_dialog.selected_field = index;
        }
    }

    pub(super) fn set_active_plugin_dialog_field_error(
        &mut self,
        field_id: impl Into<String>,
        message: impl Into<String>,
    ) {
        let field_id = field_id.into();
        self.focus_active_plugin_dialog_field(&field_id);
        if let Some(active_dialog) = self.active_plugin_ui_dialog.as_mut() {
            active_dialog.validation_error = Some(PluginDialogFormError {
                field_id,
                message: message.into(),
            });
        }
    }

    pub(super) fn handle_plugin_dialog_form_input(&mut self, event: &InputEvent) -> bool {
        if self.active_plugin_ui_dialog.is_none() {
            return false;
        }
        let InputEvent::Key(key_event) = event else {
            return true;
        };
        if key_event.kind == KeyEventKind::Release {
            return true;
        }

        if matches_key(event, "escape") || matches_key(event, "ctrl+c") {
            self.active_plugin_ui_dialog = None;
            self.editor.set_text("");
            self.transcript
                .push(TranscriptItem::system("Plugin dialog canceled"));
            return true;
        }
        if matches_key(event, "enter") {
            self.submit_active_plugin_dialog_form();
            return true;
        }

        let Some(active_dialog) = self.active_plugin_ui_dialog.as_mut() else {
            return true;
        };
        if active_dialog.dialog.fields.is_empty() {
            return true;
        }

        match &key_event.key {
            Key::Tab => {
                if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                    active_dialog.move_selection(-1);
                } else {
                    active_dialog.move_selection(1);
                }
            }
            Key::Down => active_dialog.move_selection(1),
            Key::Up => active_dialog.move_selection(-1),
            Key::Backspace => {
                if let Some((_, value)) = active_dialog.selected_field_mut() {
                    value.pop();
                    active_dialog.clear_validation_error_for_selected_field();
                }
            }
            Key::Delete => {
                if let Some((_, value)) = active_dialog.selected_field_mut() {
                    value.clear();
                    active_dialog.clear_validation_error_for_selected_field();
                }
            }
            Key::Space
                if !key_event
                    .modifiers
                    .intersects(KeyModifiers::CTRL | KeyModifiers::ALT | KeyModifiers::SUPER) =>
            {
                let mut field_changed = false;
                if let Some((field, value)) = active_dialog.selected_field_mut() {
                    if plugin_dialog_field_is_bool(field) {
                        *value = if value.trim().eq_ignore_ascii_case("true") {
                            "false".to_string()
                        } else {
                            "true".to_string()
                        };
                        field_changed = true;
                    } else if let Some(next_value) = plugin_dialog_next_choice_value(field, value) {
                        *value = next_value;
                        field_changed = true;
                    } else {
                        let inserted = plugin_dialog_filtered_insert(field, value, " ");
                        if !inserted.is_empty() {
                            value.push_str(&inserted);
                            field_changed = true;
                        }
                    }
                }
                if field_changed {
                    active_dialog.clear_validation_error_for_selected_field();
                }
            }
            Key::Char(text)
                if !key_event
                    .modifiers
                    .intersects(KeyModifiers::CTRL | KeyModifiers::ALT | KeyModifiers::SUPER) =>
            {
                let mut field_changed = false;
                if let Some((field, value)) = active_dialog.selected_field_mut() {
                    let inserted = plugin_dialog_filtered_insert(field, value, text);
                    if !inserted.is_empty() {
                        value.push_str(&inserted);
                        field_changed = true;
                    }
                }
                if field_changed {
                    active_dialog.clear_validation_error_for_selected_field();
                }
            }
            _ => {}
        }
        true
    }

    fn submit_active_plugin_dialog_form(&mut self) {
        let Some(active_dialog) = self.active_plugin_ui_dialog.as_ref() else {
            return;
        };
        let action_id = active_dialog.dialog.action_id.clone();
        let args = active_dialog.args_json();
        let raw_args = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
        self.editor
            .set_text(format!("/plugin-command {action_id} {raw_args}"));
        commands::queue_plugin_command(self, &action_id, &raw_args);
    }

    pub(super) fn render_plugin_dialog_form(&self, width: usize) -> Vec<String> {
        let Some(active_dialog) = &self.active_plugin_ui_dialog else {
            return Vec::new();
        };
        let mut lines = Vec::new();
        if active_dialog.dialog.fields.is_empty() {
            lines.push(fit_line("Plugin dialog has no fields", width));
            return lines;
        }
        for (index, field) in active_dialog.dialog.fields.iter().enumerate() {
            let prefix = if index == active_dialog.selected_field {
                ">"
            } else {
                " "
            };
            let required = if field.required { " *" } else { "" };
            let value = active_dialog
                .values
                .get(index)
                .map(|value| value.replace('\n', "\\n"))
                .unwrap_or_default();
            let kind = plugin_dialog_field_kind_label(field);
            lines.push(fit_line(
                &format!("{prefix} {}{} [{kind}]: {value}", field.label, required),
                width,
            ));
            if active_dialog
                .validation_error
                .as_ref()
                .is_some_and(|error| error.field_id == field.id)
                && let Some(error) = active_dialog.validation_error.as_ref()
            {
                lines.push(fit_line(&format!("  error: {}", error.message), width));
            }
        }
        lines
    }

    pub(super) fn apply_prompt_context(&mut self, prompt_context: &PromptContext) {
        self.cwd = prompt_context
            .session
            .as_ref()
            .map(|session| session.cwd.clone())
            .unwrap_or_else(|| PathBuf::from("."));
        self.model_id = prompt_context.model.id.clone();
        self.model = Some(prompt_context.model.clone());
        self.thinking_level = prompt_context.thinking_level.unwrap_or_default();
        self.available_models = prompt_context.model_choices.clone();
        self.model_rotation = prompt_context.model_rotation.clone();
        self.session_choices = prompt_context.session_choices.clone();
        self.theme = prompt_context.theme.clone();
        self.settings = prompt_context.settings.clone();
        self.settings_list =
            build_settings_list(self.settings.clone(), &self.theme, self.keybindings.clone());
        self.render_cache.clear();
        self.auth = prompt_context.auth.clone();
        self.git_branch.set_cwd(&self.cwd);
        self.prompt_templates = prompt_context.resources.prompt_templates.clone();
        self.skills = prompt_context.resources.skills.clone();
        self.profile_registry = prompt_context.profile_registry.clone();
        self.default_agent_profile_id = prompt_context.default_agent_profile_id.clone();
    }

    pub(super) fn expand_prompt_text(&self, text: &str) -> String {
        let text = if self.settings.enable_skill_commands {
            crate::interactive::commands::expand_skill_command(text, &self.skills)
        } else {
            text.to_string()
        };
        crate::interactive::commands::expand_prompt_template(&text, &self.prompt_templates)
    }

    pub(super) fn set_plugin_commands(&mut self, mut commands: Vec<PluginSlashCommand>) {
        commands.sort_by(|left, right| left.command_id.cmp(&right.command_id));
        commands.dedup_by(|left, right| left.command_id == right.command_id);
        self.plugin_commands = commands;
        self.slash_suggestion_selected = 0;
        self.slash_suggestions_dismissed_for = None;
    }

    pub(super) fn has_plugin_command(&self, command_id: &str) -> bool {
        self.plugin_commands
            .iter()
            .any(|command| command.command_id == command_id)
    }

    pub(super) fn set_plugin_ui_extensions(
        &mut self,
        mut actions: Vec<PluginUiAction>,
        mut keybindings: Vec<PluginKeybinding>,
        mut dialogs: Vec<PluginUiDialog>,
    ) {
        actions.sort_by(|left, right| left.id.cmp(&right.id));
        actions.dedup_by(|left, right| left.id == right.id);
        keybindings.sort_by(|left, right| left.id.cmp(&right.id));
        keybindings.dedup_by(|left, right| left.id == right.id);
        dialogs.sort_by(|left, right| left.id.cmp(&right.id));
        dialogs.dedup_by(|left, right| left.id == right.id);
        self.plugin_ui_actions = actions;
        self.plugin_keybindings = keybindings;
        self.plugin_ui_dialogs = dialogs;
    }

    pub(super) fn handle_plugin_keybinding_input(&mut self, event: &InputEvent) -> bool {
        if self.status != InteractiveStatus::Idle
            || self.selecting_model
            || self.selecting_session
            || self.selecting_settings
            || self.selecting_tree
        {
            return false;
        }
        let Some(keybinding) = self
            .plugin_keybindings
            .iter()
            .find(|keybinding| matches_key(event, &keybinding.key))
        else {
            return false;
        };
        let Some(action) = self
            .plugin_ui_actions
            .iter()
            .find(|action| action.action_id == keybinding.action_id)
        else {
            self.transcript.push(TranscriptItem::system(format!(
                "Plugin keybinding {} has no registered UI action",
                keybinding.id
            )));
            return true;
        };
        if self.has_plugin_command(&action.action_id) {
            self.pending_plugin_command_request = Some(PendingPluginCommandRequest {
                command_id: action.action_id.clone(),
                args: serde_json::json!({}),
            });
            self.action = InteractiveAction::PluginCommand;
            return true;
        }
        if let Some(dialog) = self
            .plugin_ui_dialogs
            .iter()
            .find(|dialog| dialog.id == action.action_id)
        {
            self.pending_plugin_ui_dialog = Some(PendingPluginUiDialog {
                dialog_id: dialog.id.clone(),
                title: dialog.title.clone(),
                description: dialog.description.clone(),
                action_id: dialog.action_id.clone(),
                fields: dialog.fields.clone(),
            });
            self.action = InteractiveAction::PluginUiDialog;
            return true;
        }
        self.pending_plugin_ui_action = Some(PendingPluginUiAction {
            action_id: action.action_id.clone(),
            label: action.label.clone(),
            description: action.description.clone(),
        });
        self.action = InteractiveAction::PluginUiAction;
        true
    }

    pub(super) fn all_slash_commands(&self) -> Vec<slash::BuiltinSlashCommand> {
        let mut commands = slash::builtin_slash_commands();
        for t in &self.prompt_templates {
            commands.push(slash::BuiltinSlashCommand {
                name: t.name.clone(),
                description: t.description.clone(),
            });
        }
        if self.settings.enable_skill_commands {
            for s in &self.skills {
                commands.push(slash::BuiltinSlashCommand {
                    name: format!("skill:{}", s.name),
                    description: s.description.clone(),
                });
            }
        }
        for command in &self.plugin_commands {
            commands.push(slash::BuiltinSlashCommand {
                name: command.command_id.clone(),
                description: command.description.clone(),
            });
        }
        commands
    }

    pub(super) fn push_user(&mut self, prompt: String) {
        self.transcript.push(TranscriptItem::user(prompt));
    }

    pub(super) fn apply_events(&mut self, events: Vec<UiEvent>) {
        let previous_scroll_offset = self.transcript.scroll_offset();
        let previous_rows = if previous_scroll_offset > 0 {
            Some(self.transcript_row_snapshot(MAX_TOOL_RESULT_LINES))
        } else {
            None
        };
        let mut mutation = TranscriptMutation::default();
        for event in events {
            match event {
                UiEvent::UsageUpdate {
                    input,
                    output,
                    cache_read,
                    cache_write,
                    cost,
                    context_tokens,
                } => {
                    self.stats = FooterStats {
                        input,
                        output,
                        cache_read,
                        cache_write,
                        cost,
                        context_tokens,
                    };
                }
                other => mutation.extend(self.transcript.apply_event_with_mutation(other)),
            }
        }
        if let Some(previous_rows) = previous_rows {
            let added_rows = self.transcript_row_delta_since(
                previous_rows,
                mutation.changed_indices(),
                MAX_TOOL_RESULT_LINES,
            );
            self.transcript
                .preserve_scrolled_view_after_hidden_change(previous_scroll_offset, added_rows);
        }
    }

    pub(super) fn set_status(&mut self, status: InteractiveStatus) {
        if status == InteractiveStatus::Idle {
            self.spinner_frame = 0;
        }
        self.status = status;
    }

    pub(super) fn handle_slash_command(&mut self, command: ParsedSlashCommand) {
        commands::handle_slash_command(self, command);
    }

    pub(super) fn handle_empty_editor_escape(&mut self) {
        let action = self.settings.double_escape_action.as_str();
        if action == "none" {
            self.last_empty_editor_escape_at = None;
            return;
        }

        let now = Instant::now();
        let is_double_escape = self
            .last_empty_editor_escape_at
            .is_some_and(|previous| now.duration_since(previous) < DOUBLE_ESCAPE_WINDOW);
        if !is_double_escape {
            self.last_empty_editor_escape_at = Some(now);
            return;
        }

        self.last_empty_editor_escape_at = None;
        match action {
            "fork" => self.handle_slash_command(ParsedSlashCommand {
                name: "fork".to_string(),
                args: String::new(),
                original: "/fork".to_string(),
            }),
            "tree" => self.handle_slash_command(ParsedSlashCommand {
                name: "tree".to_string(),
                args: String::new(),
                original: "/tree".to_string(),
            }),
            _ => {}
        }
    }

    pub(super) fn clear_empty_editor_escape(&mut self) {
        self.last_empty_editor_escape_at = None;
    }

    fn set_selected_model(&mut self, model: Model) {
        self.set_selected_model_with_thinking(model, None);
    }

    pub(super) fn set_selected_model_with_thinking(
        &mut self,
        model: Model,
        thinking_level: Option<pi_agent_core::ThinkingLevel>,
    ) {
        self.model_id = model.id.clone();
        self.model = Some(model.clone());
        self.thinking_level = thinking_level.unwrap_or_default();
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

    pub(super) fn cycle_model_rotation(&mut self, reverse: bool) {
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

    pub(super) fn set_selected_session(&mut self, choice: SessionChoice) {
        self.session_label = choice.display_name().to_string();
        self.selected_session = Some(choice.clone());
        self.selected_session_hydrate = true;
        self.set_active_session_choice(choice.clone());
        self.selecting_session = false;
        self.session_selection_selected = 0;
        self.editor.set_text("");
        self.transcript.push(TranscriptItem::system(format!(
            "Session selected: {}",
            choice.display_name()
        )));
    }

    pub(super) fn apply_hydrated_session(
        &mut self,
        hydrated: HydratedSession,
        notice: Option<String>,
    ) {
        self.session_label = hydrated.choice.display_name().to_string();
        let mut choice = hydrated.choice.clone();
        choice.active_leaf_id = hydrated.leaf_id.clone();
        self.set_active_session_choice(choice);
        // Restore cumulative token/cost stats so the footer reflects the
        // entire session immediately after resume, without waiting for the
        // next turn to emit a UsageUpdate event.
        self.stats = FooterStats {
            input: hydrated.cumulative_usage.input,
            output: hydrated.cumulative_usage.output,
            cache_read: hydrated.cumulative_usage.cache_read,
            cache_write: hydrated.cumulative_usage.cache_write,
            cost: hydrated.cumulative_usage.cost,
            context_tokens: hydrated.cumulative_usage.last_context_tokens,
        };

        let mut transcript = Transcript::new();
        if let Some(first) = self.transcript.items().first().cloned() {
            transcript.push(first);
        }
        for item in hydrated.transcript_items {
            transcript.push(item);
        }
        if let Some(notice) = notice {
            transcript.push(TranscriptItem::system(notice));
        }
        self.transcript = transcript;
        self.render_cache.clear();
    }

    pub(super) fn set_active_session_choice(&mut self, choice: SessionChoice) {
        self.active_session_path = None;
        self.active_leaf_id = choice.active_leaf_id.clone();
        self.active_session = Some(choice);
    }

    pub(super) fn clear_active_session(&mut self) {
        self.active_session = None;
        self.active_session_path = None;
        self.active_leaf_id = None;
    }

    pub(super) fn footer(&self, width: usize) -> Vec<String> {
        let color = color_enabled();
        let width = width.max(1);

        // Line 1: status (Rust-specific; preserves the spinner indicator that
        // the TypeScript footer surfaces via a separate status container).
        let (status_str, status_style) = match self.status {
            InteractiveStatus::Idle => ("idle".to_string(), STATUS_IDLE),
            InteractiveStatus::Running => (running_status_text(self.spinner_frame), STATUS_RUNNING),
        };
        let status_line = fit_line(
            &paint_with(&format!("status: {status_str}"), &status_style, color),
            width,
        );

        // Line 2: pwd line — `cwd (branch) • session-name`, dimmed. Mirrors the
        // TypeScript footer's first render line.
        let mut pwd = abbreviate_cwd(&self.cwd);
        if let Some(branch) = self.git_branch.branch() {
            pwd = format!("{pwd} ({branch})");
        }
        let session_name = self.session_label.trim();
        if !session_name.is_empty() && session_name != "session" {
            pwd = format!("{pwd} • {session_name}");
        }
        let pwd_line = paint_with(
            &truncate_to_width_with_ellipsis(&pwd, width),
            &SYSTEM,
            color,
        );

        // Line 3: cumulative stats (left) + model info (right-aligned).
        vec![status_line, pwd_line, self.render_stats_line(width, color)]
    }

    /// The currently active model for footer display (context window,
    /// reasoning, provider). Distinct from `selected_model`, which is consumed
    /// by `take_selected_model` to apply a pending change to the agent.
    fn current_model(&self) -> Option<&Model> {
        self.model.as_ref()
    }

    /// Number of distinct providers among the available models plus the active
    /// model's provider, mirroring the TypeScript `getAvailableProviderCount`.
    fn available_provider_count(&self) -> usize {
        let mut providers: Vec<&str> = self
            .available_models
            .iter()
            .map(|m| m.provider.as_str())
            .collect();
        if let Some(model) = &self.model {
            providers.push(model.provider.as_str());
        }
        providers.sort_unstable();
        providers.dedup();
        providers.len()
    }

    /// Whether the active model's provider is authenticated via an OAuth
    /// subscription token, mirroring `modelRegistry.isUsingOAuth(model)`.
    fn using_subscription(&self) -> bool {
        self.current_model()
            .map(|m| self.auth.oauth_access_entry(&m.provider).is_some())
            .unwrap_or(false)
    }

    /// `(context_window, auto_indicator)` for the active model.
    fn context_window_and_indicator(&self) -> (u32, &'static str) {
        let window = self.current_model().map(|m| m.context_window).unwrap_or(0);
        let auto = if self.settings.compaction.enabled {
            " (auto)"
        } else {
            ""
        };
        (window, auto)
    }

    /// Stats line: token/cost/context on the left, model info right-aligned.
    /// Mirrors the TypeScript footer's second render line, including the
    /// right-align padding and graceful right-side truncation on narrow widths.
    fn render_stats_line(&self, width: usize, color: bool) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.stats.input > 0 {
            parts.push(format!("↑{}", format_tokens(self.stats.input)));
        }
        if self.stats.output > 0 {
            parts.push(format!("↓{}", format_tokens(self.stats.output)));
        }
        if self.stats.cache_read > 0 {
            parts.push(format!("R{}", format_tokens(self.stats.cache_read)));
        }
        if self.stats.cache_write > 0 {
            parts.push(format!("W{}", format_tokens(self.stats.cache_write)));
        }

        let using_sub = self.using_subscription();
        if self.stats.cost > 0.0 || using_sub {
            parts.push(format!(
                "${:.3}{}",
                self.stats.cost,
                if using_sub { " (sub)" } else { "" }
            ));
        }

        // Context usage: `percent/contextWindow (auto)`, or `?` when unknown
        // (right after compaction, before the next LLM response).
        let (context_window, auto_indicator) = self.context_window_and_indicator();
        let (percent_value, context_display) = match (self.stats.context_tokens, context_window) {
            (Some(tokens), window) if window > 0 => {
                let pct = (tokens as f64 / window as f64) * 100.0;
                (
                    pct,
                    format!("{:.1}%/{}{}", pct, format_tokens(window), auto_indicator),
                )
            }
            _ => (
                0.0,
                format!("?/{}{}", format_tokens(context_window), auto_indicator),
            ),
        };
        let context_style = if percent_value > 90.0 {
            ERROR
        } else if percent_value > 70.0 {
            WARNING
        } else {
            Style::default()
        };
        parts.push(paint_with(&context_display, &context_style, color));

        let mut stats_left = paint_with(&parts.join(" "), &SYSTEM, color);
        if visible_width(&stats_left) > width {
            stats_left = truncate_to_width_with_ellipsis(&stats_left, width);
        }
        let stats_left_width = visible_width(&stats_left);

        // Right side: optional `(provider)` prefix + model name + thinking.
        let model_name = self
            .current_model()
            .map(|m| m.id.as_str())
            .unwrap_or("no-model")
            .to_string();
        let mut right_side = if self.current_model().map(|m| m.reasoning).unwrap_or(false) {
            let level = self.thinking_level;
            if level == pi_agent_core::ThinkingLevel::Off {
                format!("{model_name} • thinking off")
            } else {
                format!("{model_name} • {level}")
            }
        } else {
            model_name
        };
        let min_padding = 2;
        if self.available_provider_count() > 1 && self.current_model().is_some() {
            let prefixed = format!(
                "({}) {right_side}",
                self.current_model()
                    .map(|m| m.provider.as_str())
                    .unwrap_or(""),
            );
            if stats_left_width + min_padding + visible_width(&prefixed) <= width {
                right_side = prefixed;
            }
        }

        let right_width = visible_width(&right_side);
        if stats_left_width + min_padding + right_width <= width {
            let padding = " ".repeat(width - stats_left_width - right_width);
            let remainder = format!("{padding}{right_side}");
            format!("{}{}", stats_left, paint_with(&remainder, &SYSTEM, color))
        } else {
            let available_for_right = width.saturating_sub(stats_left_width + min_padding);
            if available_for_right > 0 {
                let truncated_right = truncate_to_width(&right_side, available_for_right);
                let padding = " ".repeat(
                    width.saturating_sub(stats_left_width + visible_width(&truncated_right)),
                );
                let remainder = format!("{padding}{truncated_right}");
                format!("{}{}", stats_left, paint_with(&remainder, &SYSTEM, color))
            } else {
                stats_left
            }
        }
    }

    pub(super) fn render_state(&self) -> InteractiveRenderState {
        InteractiveRenderState {
            editor_text: self.editor.text().to_string(),
            editor_cursor: self.editor.cursor(),
            transcript_revision: self.transcript.revision(),
            transcript_scroll_offset: self.transcript.scroll_offset(),
            transcript_has_new_output_below: self.transcript.has_new_output_below(),
            status: self.status,
            tool_output_expanded: self.tool_output_expanded,
            spinner_frame: self.spinner_frame,
            slash_suggestion_selected: self.slash_suggestion_selected,
            slash_suggestions_dismissed_for: self.slash_suggestions_dismissed_for.clone(),
            selecting_settings: self.selecting_settings,
            selecting_tree: self.selecting_tree,
            tree_selector_state: self
                .tree_selector
                .as_ref()
                .map(|selector| selector.render_state()),
            settings: self.settings.clone(),
            auth: self.auth.clone(),
            theme_name: self.theme.name.clone(),
            settings_selected_item_id: self
                .settings_list
                .selected_item()
                .map(|item| item.id.clone()),
            selecting_model: self.selecting_model,
            model_selection_selected: self.model_selection_selected,
            selecting_session: self.selecting_session,
            session_selection_selected: self.session_selection_selected,
            active_plugin_ui_dialog: self.active_plugin_ui_dialog.clone(),
        }
    }

    pub(super) fn editor_border_style(&self) -> Style {
        if self.selecting_model || self.selecting_settings || self.selecting_session {
            self.theme.editor.menu_border
        } else if let Some(resolved) = &self.resolved_theme {
            // Editor border reflects the active thinking level, mirroring TS
            // `getThinkingBorderColor`. Bash-mode border (TS
            // `getBashModeBorderColor`) is not yet wired: Rust has no
            // bash-mode input state.
            Style::fg(crate::resources::to_color(
                resolved.fg(Self::thinking_border_token(self.thinking_level)),
            ))
        } else {
            self.theme.editor.active_border
        }
    }

    /// Map a thinking level to its border color token, mirroring TS
    /// `getThinkingBorderColor`.
    fn thinking_border_token(level: pi_agent_core::ThinkingLevel) -> ThemeColor {
        use pi_agent_core::ThinkingLevel;
        match level {
            ThinkingLevel::Off => ThemeColor::ThinkingOff,
            ThinkingLevel::Minimal => ThemeColor::ThinkingMinimal,
            ThinkingLevel::Low => ThemeColor::ThinkingLow,
            ThinkingLevel::Medium => ThemeColor::ThinkingMedium,
            ThinkingLevel::High => ThemeColor::ThinkingHigh,
            ThinkingLevel::XHigh => ThemeColor::ThinkingXhigh,
        }
    }

    fn render_slash_suggestions(&mut self, width: usize) -> Vec<String> {
        if self.selecting_model || self.selecting_settings || self.selecting_session {
            return Vec::new();
        }

        let commands = self.all_slash_commands();
        slash::render_suggestions(
            self.editor.text(),
            self.editor.cursor(),
            self.slash_suggestions_dismissed_for.as_deref(),
            &mut self.slash_suggestion_selected,
            width,
            &commands,
        )
    }

    fn render_settings_menu(&mut self, width: usize) -> Vec<String> {
        if !self.selecting_settings {
            return Vec::new();
        }
        let mut lines = vec![fit_line("Settings", width)];
        lines.extend(self.settings_list.render(width));
        lines
    }

    fn apply_settings_value(&mut self, id: &str, value: &str) {
        match id {
            "theme" => {
                self.settings.theme = Some(value.to_string());
                self.settings_delta.theme = Some(value.to_string());
                self.apply_builtin_theme(value);
            }
            "auto_compaction" => {
                self.settings.compaction.enabled = value == "on";
                self.settings_delta
                    .compaction
                    .get_or_insert_with(|| crate::config::settings::PartialCompaction::default())
                    .enabled = Some(value == "on");
            }
            "transport" => {
                self.settings.transport = value.to_string();
                self.settings_delta.transport = Some(value.to_string());
            }
            "steering_mode" => {
                self.settings.steering_mode = value.to_string();
                self.settings_delta.steering_mode = Some(value.to_string());
            }
            "follow_up_mode" => {
                self.settings.follow_up_mode = value.to_string();
                self.settings_delta.follow_up_mode = Some(value.to_string());
            }
            "show_images" => {
                self.settings.terminal.show_images = value == "on";
                self.settings_delta
                    .terminal
                    .get_or_insert_with(|| crate::config::settings::PartialTerminal::default())
                    .show_images = Some(value == "on");
            }
            "show_progress" => {
                self.settings.terminal.show_progress = value == "on";
                self.settings_delta
                    .terminal
                    .get_or_insert_with(|| crate::config::settings::PartialTerminal::default())
                    .show_progress = Some(value == "on");
            }
            "image_width_cells" => {
                if let Ok(width) = value.parse::<u32>() {
                    self.settings.terminal.image_width_cells = width;
                    self.settings_delta
                        .terminal
                        .get_or_insert_with(|| crate::config::settings::PartialTerminal::default())
                        .image_width_cells = Some(width);
                }
            }
            "auto_resize_images" => {
                self.settings.terminal.auto_resize_images = value == "on";
                self.settings_delta
                    .terminal
                    .get_or_insert_with(|| crate::config::settings::PartialTerminal::default())
                    .auto_resize_images = Some(value == "on");
            }
            "block_images" => {
                self.settings.terminal.block_images = value == "on";
                self.settings_delta
                    .terminal
                    .get_or_insert_with(|| crate::config::settings::PartialTerminal::default())
                    .block_images = Some(value == "on");
            }
            "enable_skill_commands" => {
                self.settings.enable_skill_commands = value == "on";
                self.settings_delta.enable_skill_commands = Some(value == "on");
            }
            "hide_thinking_block" => {
                self.settings.hide_thinking_block = value == "on";
                self.settings_delta.hide_thinking_block = Some(value == "on");
            }
            "collapse_changelog" => {
                self.settings.collapse_changelog = value == "on";
                self.settings_delta.collapse_changelog = Some(value == "on");
            }
            "quiet_startup" => {
                self.settings.quiet_startup = value == "on";
                self.settings_delta.quiet_startup = Some(value == "on");
            }
            "clear_on_shrink" => {
                self.settings.terminal.clear_on_shrink = value == "on";
                self.settings_delta
                    .terminal
                    .get_or_insert_with(|| crate::config::settings::PartialTerminal::default())
                    .clear_on_shrink = Some(value == "on");
            }
            "double_escape_action" => {
                self.settings.double_escape_action = value.to_string();
                self.settings_delta.double_escape_action = Some(value.to_string());
            }
            "tree_filter_mode" => {
                self.settings.tree_filter_mode = value.to_string();
                self.settings_delta.tree_filter_mode = Some(value.to_string());
            }
            "warnings_anthropic_extra_usage" => {
                self.settings.warnings.anthropic_extra_usage = value == "on";
                self.settings_delta
                    .warnings
                    .get_or_insert_with(|| crate::config::settings::PartialWarnings::default())
                    .anthropic_extra_usage = Some(value == "on");
            }
            "default_thinking_level" => {
                self.settings.default_thinking_level = Some(value.to_string());
                self.settings_delta.default_thinking_level = Some(value.to_string());
                // Also update the active thinking level so the editor border reflects it
                if let Ok(level) = value.parse::<pi_agent_core::ThinkingLevel>() {
                    self.thinking_level = level;
                    self.selected_thinking_level = Some(level);
                }
            }
            "http_idle_timeout" => {
                if let Some((_, timeout_ms)) = HTTP_IDLE_TIMEOUT_CHOICES
                    .iter()
                    .find(|(label, _)| *label == value)
                {
                    self.settings.http_idle_timeout_ms = *timeout_ms;
                    self.settings_delta.http_idle_timeout_ms = Some(*timeout_ms);
                }
            }
            _ => return,
        }
        self.settings_update = Some(self.settings.clone());
    }

    /// Apply a built-in theme by name ("dark"/"light"), updating both the
    /// `pi-tui` palette theme and the resolved 51-token theme.
    fn apply_builtin_theme(&mut self, name: &str) {
        self.theme = match name {
            "light" => light_theme(),
            _ => dark_theme(),
        };
        let json = match name {
            "light" => crate::theme::builtin_light(),
            _ => crate::theme::builtin_dark(),
        };
        self.resolved_theme = Some(json.resolve_colors().expect("built-in theme resolves"));
        self.render_cache.clear();
    }

    /// Apply a hot-reloaded custom theme: update the resolved 51-token theme
    /// and the `pi-tui` palette bridge. Mirrors TS `startThemeWatcher`'s
    /// `setGlobalTheme(reloadedTheme)` + `onThemeChange` callback.
    pub(super) fn apply_theme_reload(&mut self, name: String, theme: crate::theme::ThemeJson) {
        if let Ok(resolved) = theme.resolve_colors() {
            self.resolved_theme = Some(resolved);
            // Refresh the palette bridge so non-thinking borders also track
            // the reloaded theme.
            self.theme = crate::resources::tui_theme_from_resolved_json(&name, &theme);
            self.render_cache.clear();
        }
    }

    /// Build a `MarkdownTheme` for the active resolved theme, wiring the
    /// syntax-highlight callback (TS `getMarkdownTheme` + `highlightCode`).
    /// Falls back to the palette theme's markdown styles when no resolved
    /// theme is set.
    fn markdown_theme(&self) -> MarkdownTheme {
        let mut md = self.theme.markdown.clone();
        if let Some(resolved) = &self.resolved_theme {
            let resolved = resolved.clone();
            md.highlight_code = Some(std::sync::Arc::new(
                move |code: &str, lang: Option<&str>| {
                    crate::theme::highlight_code(code, lang, &resolved)
                },
            ));
        }
        md
    }

    /// Build the [`TranscriptRenderOptions`] used by transcript block
    /// rendering. Resolves styles from the active [`ResolvedTheme`] when
    /// available, falling back to the built-in palette otherwise.
    fn transcript_render_options(
        &self,
        max_tool_result_lines: usize,
    ) -> TranscriptRenderOptions<'static> {
        TranscriptRenderOptions {
            width: self.viewport_width,
            max_tool_result_lines,
            color: color_enabled(),
            markdown_theme: self.markdown_theme(),
            hide_thinking_block: self.settings.hide_thinking_block,
            hidden_thinking_label: "Thinking...",
            styles: TranscriptStyles::from_theme(self.resolved_theme.as_ref()),
        }
    }

    pub(super) fn handle_settings_input(&mut self, event: &InputEvent) -> bool {
        if !self.selecting_settings {
            return false;
        }

        let before = self
            .settings_list
            .selected_item()
            .map(|item| (item.id.clone(), item.current_value.clone()));
        self.settings_list.handle_input(event);
        let after = self
            .settings_list
            .selected_item()
            .map(|item| (item.id.clone(), item.current_value.clone()));

        if let (Some((before_id, before_value)), Some((after_id, after_value))) = (before, after)
            && before_id == after_id
            && before_value != after_value
        {
            self.apply_settings_value(&after_id, &after_value);
        }
        true
    }

    pub(super) fn mark_auth_updated(&mut self) {
        self.auth_update = Some(self.auth.clone());
    }

    fn render_model_selector(&mut self, width: usize) -> Vec<String> {
        if !self.selecting_model {
            return Vec::new();
        }
        model_selector::render(
            &self.available_models,
            self.editor.text(),
            &mut self.model_selection_selected,
            width,
        )
    }

    fn render_session_selector(&mut self, width: usize) -> Vec<String> {
        if !self.selecting_session {
            return Vec::new();
        }
        session_selector::render(
            &self.session_choices,
            self.editor.text(),
            &mut self.session_selection_selected,
            width,
        )
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

    fn transcript_lines(&mut self, max_tool_result_lines: usize) -> Vec<String> {
        let opts = self.transcript_render_options(max_tool_result_lines);
        self.render_cache.render_lines(&self.transcript, &opts)
    }

    fn transcript_row_snapshot(&mut self, max_tool_result_lines: usize) -> TranscriptRowSnapshot {
        let opts = self.transcript_render_options(max_tool_result_lines);
        self.render_cache.row_snapshot(&self.transcript, &opts)
    }

    fn transcript_row_delta_since(
        &mut self,
        snapshot: TranscriptRowSnapshot,
        changed_indices: &[usize],
        max_tool_result_lines: usize,
    ) -> usize {
        let opts = self.transcript_render_options(max_tool_result_lines);
        self.render_cache
            .row_delta_since(&self.transcript, &opts, snapshot, changed_indices)
    }

    #[cfg(test)]
    pub(super) fn reset_render_cache_stats(&mut self) {
        self.render_cache.reset_stats();
    }

    #[cfg(test)]
    pub(super) fn render_cache_stats(
        &self,
    ) -> crate::interactive::render::TranscriptRenderCacheStats {
        self.render_cache.stats()
    }

    pub(super) fn handle_slash_suggestion_input(&mut self, event: &InputEvent) -> bool {
        if self.selecting_model || self.selecting_settings || self.selecting_session {
            return false;
        }
        let commands = self.all_slash_commands();
        slash::handle_suggestion_input(
            &self.keybindings,
            event,
            &mut self.editor,
            &mut self.slash_suggestion_selected,
            &mut self.slash_suggestions_dismissed_for,
            &commands,
        )
    }

    pub(super) fn handle_model_selection_input(&mut self, event: &InputEvent) -> bool {
        if !self.selecting_model {
            return false;
        }

        match model_selector::handle_input(
            &self.keybindings,
            event,
            &mut self.editor,
            &mut self.model_selection_selected,
            &self.available_models,
        ) {
            model_selector::SelectorInput::Handled => {}
            model_selector::SelectorInput::Cancel => {
                self.selecting_model = false;
                self.model_selection_selected = 0;
                self.editor.set_text("");
                self.transcript.push(TranscriptItem::system(
                    "Model selection canceled".to_string(),
                ));
            }
            model_selector::SelectorInput::Confirm(Some(model_index)) => {
                let model = self.available_models[model_index].clone();
                self.set_selected_model(model);
            }
            model_selector::SelectorInput::Confirm(None) => {}
        }
        true
    }

    pub(super) fn handle_tree_selection_input(&mut self, event: &InputEvent) -> bool {
        if !self.selecting_tree {
            return false;
        }

        let Some(selector) = self.tree_selector.as_mut() else {
            return false;
        };

        match selector.handle_input(&self.keybindings, event) {
            TreeSelectorInput::Cancel => {
                self.selecting_tree = false;
                self.tree_selector = None;
                self.selected_tree_entry_id = None;
                self.editor.set_text("");
            }
            TreeSelectorInput::Confirm(Some(entry_id)) => {
                self.selected_tree_entry_id = Some(entry_id);
                self.selecting_tree = false;
                self.tree_selector = None;
            }
            TreeSelectorInput::Confirm(None) => {}
            TreeSelectorInput::EditLabel { .. } => {
                // Label edit is handled inside the selector state
            }
            TreeSelectorInput::SaveLabel { entry_id, label } => {
                self.pending_tree_label_change = Some((entry_id, label));
            }
            TreeSelectorInput::Handled => {}
        }
        true
    }

    pub(super) fn handle_session_selection_input(&mut self, event: &InputEvent) -> bool {
        if !self.selecting_session {
            return false;
        }

        match session_selector::handle_input(
            &self.keybindings,
            event,
            &mut self.editor,
            &mut self.session_selection_selected,
            &self.session_choices,
        ) {
            session_selector::SelectorInput::Handled => {}
            session_selector::SelectorInput::Cancel => {
                self.selecting_session = false;
                self.session_selection_selected = 0;
                self.editor.set_text("");
                self.transcript.push(TranscriptItem::system(
                    "Session selection canceled".to_string(),
                ));
            }
            session_selector::SelectorInput::Confirm(Some(session_index)) => {
                let choice = self.session_choices[session_index].clone();
                self.set_selected_session(choice);
            }
            session_selector::SelectorInput::Confirm(None) => {}
        }
        true
    }
}

impl Component for InteractiveRoot {
    fn render(&mut self, width: usize) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }

        let max_tool_result_lines = if self.tool_output_expanded {
            EXPANDED_TOOL_RESULT_LINES
        } else {
            MAX_TOOL_RESULT_LINES
        };
        let mut lines = self.transcript_lines(max_tool_result_lines);
        lines.extend(self.render_editor_box(width));
        if self.selecting_tree {
            if let Some(ref selector) = self.tree_selector {
                lines.extend(selector.render(width));
            }
        } else if self.selecting_model {
            lines.extend(self.render_model_selector(width));
        } else if self.selecting_session {
            lines.extend(self.render_session_selector(width));
        } else if self.selecting_settings {
            lines.extend(self.render_settings_menu(width));
        } else if self.active_plugin_ui_dialog.is_some() {
            lines.extend(self.render_plugin_dialog_form(width));
        } else {
            lines.extend(self.render_slash_suggestions(width));
        }
        lines.extend(self.footer(width));
        lines
    }

    fn handle_input(&mut self, event: &InputEvent) {
        input::handle_root_input(self, event);
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
}

fn build_settings_list(
    settings: Settings,
    theme: &TuiTheme,
    keybindings: KeybindingsManager,
) -> SettingsList {
    SettingsList::with_options(
        vec![
            SettingItem::new("theme", "Theme", theme.name.clone())
                .values(["dark", "light"])
                .description("Change the active interface theme"),
            SettingItem::new(
                "auto_compaction",
                "Auto compact",
                if settings.compaction.enabled {
                    "on"
                } else {
                    "off"
                },
            )
            .values(["on", "off"])
            .description("Automatically compact context before it exceeds the model window"),
            SettingItem::new("transport", "Transport", &settings.transport)
                .values(["sse", "websocket", "websocket-cached", "auto"])
                .description("Preferred transport for provider connections"),
            SettingItem::new(
                "steering_mode",
                "Steering mode",
                &settings.steering_mode,
            )
            .values(["one-at-a-time", "all"])
            .description("Enter while streaming queues steering messages ('one-at-a-time' delivers one at a time)"),
            SettingItem::new(
                "follow_up_mode",
                "Follow-up mode",
                &settings.follow_up_mode,
            )
            .values(["one-at-a-time", "all"])
            .description("Queue follow-up messages until agent stops"),
            SettingItem::new(
                "show_images",
                "Show images",
                if settings.terminal.show_images {
                    "on"
                } else {
                    "off"
                },
            )
            .values(["on", "off"])
            .description("Render images inline in terminal"),
            SettingItem::new(
                "image_width_cells",
                "Image width",
                settings.terminal.image_width_cells.to_string(),
            )
            .values(["60", "80", "120"])
            .description("Preferred inline image width in terminal cells"),
            SettingItem::new(
                "show_progress",
                "Terminal progress",
                if settings.terminal.show_progress {
                    "on"
                } else {
                    "off"
                },
            )
            .values(["on", "off"])
            .description("Show progress indicators in terminal tab bar"),
            SettingItem::new(
                "auto_resize_images",
                "Auto-resize images",
                if settings.terminal.auto_resize_images {
                    "on"
                } else {
                    "off"
                },
            )
            .values(["on", "off"])
            .description("Resize large images to 2000\u{d7}2000 max for better model compatibility"),
            SettingItem::new(
                "block_images",
                "Block images",
                if settings.terminal.block_images {
                    "on"
                } else {
                    "off"
                },
            )
            .values(["on", "off"])
            .description("Prevent images from being sent to LLM providers"),
            SettingItem::new(
                "enable_skill_commands",
                "Skill commands",
                if settings.enable_skill_commands {
                    "on"
                } else {
                    "off"
                },
            )
            .values(["on", "off"])
            .description("Register skills as /skill:name commands"),
            SettingItem::new(
                "hide_thinking_block",
                "Hide thinking",
                if settings.hide_thinking_block {
                    "on"
                } else {
                    "off"
                },
            )
            .values(["on", "off"])
            .description("Hide thinking blocks in assistant responses"),
            SettingItem::new(
                "collapse_changelog",
                "Collapse changelog",
                if settings.collapse_changelog {
                    "on"
                } else {
                    "off"
                },
            )
            .values(["on", "off"])
            .description("Show condensed changelog after updates"),
            SettingItem::new(
                "quiet_startup",
                "Quiet startup",
                if settings.quiet_startup {
                    "on"
                } else {
                    "off"
                },
            )
            .values(["on", "off"])
            .description("Disable verbose printing at startup"),
            SettingItem::new(
                "clear_on_shrink",
                "Clear on shrink",
                if settings.terminal.clear_on_shrink {
                    "on"
                } else {
                    "off"
                },
            )
            .values(["on", "off"])
            .description("Clear empty rows when content shrinks (may cause flicker)"),
            SettingItem::new(
                "double_escape_action",
                "Double-escape action",
                &settings.double_escape_action,
            )
            .values(["tree", "fork", "none"])
            .description("Action when pressing Escape twice with empty editor"),
            SettingItem::new(
                "warnings_anthropic_extra_usage",
                "Warn: Anthropic extra usage",
                if settings.warnings.anthropic_extra_usage {
                    "on"
                } else {
                    "off"
                },
            )
            .values(["on", "off"])
            .description("Warn when Anthropic subscription auth may use paid extra usage"),
            SettingItem::new(
                "default_thinking_level",
                "Thinking level",
                settings
                    .default_thinking_level
                    .as_deref()
                    .unwrap_or("off"),
            )
            .values(["off", "minimal", "low", "medium", "high", "xhigh"])
            .description("Default reasoning depth for thinking-capable models"),
            SettingItem::new(
                "http_idle_timeout",
                "HTTP idle timeout",
                format_http_idle_timeout_ms(settings.http_idle_timeout_ms),
            )
            .values(HTTP_IDLE_TIMEOUT_CHOICES.map(|(label, _)| label))
            .description("Maximum idle gap while waiting for HTTP provider response data"),
        ],
        16,
        keybindings,
        SettingsListOptions {
            enable_search: false,
        },
    )
}

fn format_http_idle_timeout_ms(timeout_ms: u64) -> String {
    HTTP_IDLE_TIMEOUT_CHOICES
        .iter()
        .find(|(_, value)| *value == timeout_ms)
        .map(|(label, _)| (*label).to_string())
        .unwrap_or_else(|| format!("{} sec", timeout_ms as f64 / 1000.0))
}
