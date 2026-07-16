use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pi_ai::api::model::Model;
use pi_tui::api::component::OverlayHandle;
use pi_tui::api::component::{Component, Editor, SettingItem, SettingsList, SettingsListOptions};
use pi_tui::api::input::{
    InputEvent, Key, KeyEventKind, KeyModifiers, KeybindingsManager, matches_key,
};
use pi_tui::api::render::{
    Constraint, ERROR, FocusRing, Frame, Layout, Rect, STATUS_IDLE, STATUS_RUNNING, SYSTEM, Style,
    USER, color_enabled, paint_with, truncate_to_width, truncate_to_width_with_ellipsis,
    visible_width,
};
use pi_tui::api::theme::{MarkdownTheme, TuiTheme, dark_theme, light_theme};

use crate::adapters::interactive::app::{PromptContext, welcome_line};
use crate::adapters::interactive::clipboard::{ClipboardSink, SystemClipboard};
use crate::adapters::interactive::commands;
use crate::adapters::interactive::delegation_confirmation_menu::{
    DelegationConfirmationMenuOutcome, DelegationConfirmationMenuRenderState,
    DelegationConfirmationMenuState,
};
use crate::adapters::interactive::git_branch::GitBranchProvider;
use crate::adapters::interactive::input;
use crate::adapters::interactive::keybindings;
use crate::adapters::interactive::model_selector;
use crate::adapters::interactive::profile_menu::{
    PendingProfileTask, ProfileMenuOutcome, ProfileMenuRenderState, ProfileMenuState,
};
use crate::adapters::interactive::render::{
    TranscriptRenderCache, TranscriptRenderOptions, TranscriptRowSnapshot, TranscriptStyles,
    WARNING, abbreviate_cwd, editor_border_line, fit_line, format_tokens,
    markdown_theme_from_resolved, running_status_text,
};
use crate::adapters::interactive::session_actions::{HydratedSession, SessionChoice};
use crate::adapters::interactive::session_selector;
use crate::adapters::interactive::slash::{self, ParsedSlashCommand};
use crate::adapters::interactive::transcript::TranscriptMutation;
use crate::adapters::interactive::transient_overlay::TransientOverlayBridge;
use crate::adapters::interactive::tree_selector::{TreeSelectorInput, TreeSelectorState};
use crate::adapters::interactive::{Transcript, TranscriptItem, UiEvent};
use crate::app::cli::request::profile_registry_for_cwd;
use crate::authorization::{
    ToolAuthorizationDecision, ToolAuthorizationRequest, ToolAuthorizationRisk,
};
use crate::config::{AuthStore, Settings};
use crate::runtime::facade::{
    PendingDelegationConfirmation, ProfileId, ProfileRegistry, SelfHealingEditReplacement,
};
use crate::theme::{ResolvedTheme, ThemeColor};

const MAX_TOOL_RESULT_LINES: usize = 3;
const EXPANDED_TOOL_RESULT_LINES: usize = 20;
const WIDE_LAYOUT_MIN_WIDTH: usize = 100;
const MEDIUM_LAYOUT_MIN_WIDTH: usize = 64;
const TIPS_MIN_HEIGHT: usize = 18;
const MAX_COMPOSER_HEIGHT: usize = 8;
pub(super) const DOUBLE_ESCAPE_WINDOW: Duration = Duration::from_millis(500);

const HTTP_IDLE_TIMEOUT_CHOICES: [(&str, u64); 5] = [
    ("30 sec", 30_000),
    ("1 min", 60_000),
    ("2 min", 120_000),
    ("5 min", 300_000),
    ("disabled", 0),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InteractiveAction {
    None,
    Submit,
    FollowUp,
    CompactSession,
    BranchSummary,
    SelfHealingEdit,
    PluginCommand,
    PluginUiDialog,
    DelegationConfirmation,
    ToolAuthorization,
    AgentProfileUse,
    AgentInvocation,
    AgentTeam,
    AbortRunning,
    NewSession,
    ReloadResources,
    Fork,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InteractiveRegion {
    Conversation,
    Context,
    Composer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextTab {
    Ops,
    Changes,
    Agents,
    Usage,
}

impl ContextTab {
    const ALL: [Self; 4] = [Self::Ops, Self::Changes, Self::Agents, Self::Usage];

    fn label(self) -> &'static str {
        match self {
            Self::Ops => "ops",
            Self::Changes => "changes",
            Self::Agents => "agents",
            Self::Usage => "usage",
        }
    }

    fn next(self) -> Self {
        let index = Self::ALL.iter().position(|tab| *tab == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    fn previous(self) -> Self {
        let index = Self::ALL.iter().position(|tab| *tab == self).unwrap_or(0);
        Self::ALL[index.checked_sub(1).unwrap_or(Self::ALL.len() - 1)]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellLayoutMode {
    Wide,
    Medium,
    Narrow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TransientOverlayProjection {
    pub(super) modal_visible: bool,
    pub(super) support_visible: bool,
    pub(super) bottom_margin: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShellLayout {
    mode: ShellLayoutMode,
    conversation: Rect,
    context: Option<Rect>,
    tips: Option<Rect>,
    composer: Rect,
    status: Rect,
    work: Rect,
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
pub(super) struct PendingForkRequest {
    pub(super) target_leaf_id: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingSelfHealingEditModelRepair {
    pub(super) max_attempts: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingSelfHealingEditRequest {
    pub(super) path: String,
    pub(super) replacements: Vec<SelfHealingEditReplacement>,
    pub(super) check_command: Option<String>,
    pub(super) model_repair: Option<PendingSelfHealingEditModelRepair>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PendingPluginCommandRequest {
    pub(super) command_id: String,
    pub(super) args: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingDelegationConfirmationSelection {
    pub(super) operation_id: Option<String>,
    pub(super) tool_call_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PendingDelegationConfirmationCommand {
    List,
    Approve {
        selection: PendingDelegationConfirmationSelection,
    },
    Reject {
        selection: PendingDelegationConfirmationSelection,
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingDelegationRejectionReason {
    selection: PendingDelegationConfirmationSelection,
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
    pub(super) pending_fork_request: Option<PendingForkRequest>,
    pub(super) pending_agent_invocation_request: Option<PendingAgentInvocationRequest>,
    pub(super) pending_agent_team_request: Option<PendingAgentTeamRequest>,
    pub(super) pending_self_healing_edit_request: Option<PendingSelfHealingEditRequest>,
    pub(super) pending_plugin_command_request: Option<PendingPluginCommandRequest>,
    pub(super) pending_delegation_confirmation_command:
        Option<PendingDelegationConfirmationCommand>,
    delegation_confirmation_menu: Option<DelegationConfirmationMenuState>,
    pending_delegation_rejection_reason: Option<PendingDelegationRejectionReason>,
    tool_authorizations: VecDeque<ToolAuthorizationRequest>,
    tool_authorization_selected: usize,
    pending_tool_authorization_decision:
        Option<(ToolAuthorizationRequest, ToolAuthorizationDecision)>,
    pub(super) pending_plugin_ui_dialog: Option<PendingPluginUiDialog>,
    pub(super) active_plugin_ui_dialog: Option<ActivePluginUiDialog>,
    profile_menu: Option<ProfileMenuState>,
    pending_profile_task: Option<PendingProfileTask>,
    pub(super) selected_agent_profile_id: Option<ProfileId>,
    pub(super) action: InteractiveAction,
    pub(super) status: InteractiveStatus,
    pub(super) viewport_width: usize,
    pub(super) viewport_height: usize,
    fullscreen_viewport: bool,
    focus_ring: FocusRing<InteractiveRegion>,
    context_tab: ContextTab,
    context_open: bool,
    context_restore_focus: InteractiveRegion,
    conversation_viewport_height: usize,
    modal_overlay: TransientOverlayBridge,
    support_overlay: TransientOverlayBridge,
    modal_overlay_handle: Option<OverlayHandle>,
    support_overlay_handle: Option<OverlayHandle>,
    pub(super) cwd: PathBuf,
    pub(super) model_id: String,
    pub(super) session_label: String,
    pub(super) selected_model: Option<Model>,
    pub(super) selected_thinking_level: Option<pi_agent_core::api::agent::ThinkingLevel>,
    /// Currently active model for footer display. Distinct from
    /// `selected_model`, which is consumed to apply pending changes.
    pub(super) model: Option<Model>,
    /// Currently active thinking level (never consumed by `take_*`).
    pub(super) thinking_level: pi_agent_core::api::agent::ThinkingLevel,
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
    pub(super) prompt_templates: Vec<pi_agent_core::api::resources::PromptTemplate>,
    pub(super) skills: Vec<pi_agent_core::api::resources::Skill>,
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
    focused_region: Option<InteractiveRegion>,
    context_tab: ContextTab,
    context_open: bool,
    status: InteractiveStatus,
    stats: FooterStats,
    tool_output_expanded: bool,
    spinner_frame: usize,
    slash_suggestion_selected: usize,
    slash_suggestions_dismissed_for: Option<String>,
    selecting_settings: bool,
    selecting_tree: bool,
    tree_selector_state:
        Option<crate::adapters::interactive::tree_selector::TreeSelectorRenderState>,
    settings: Settings,
    auth: AuthStore,
    theme_name: String,
    settings_selected_item_id: Option<String>,
    selecting_model: bool,
    model_selection_selected: usize,
    selecting_session: bool,
    session_selection_selected: usize,
    active_plugin_ui_dialog: Option<ActivePluginUiDialog>,
    delegation_confirmation_menu_state: Option<DelegationConfirmationMenuRenderState>,
    pending_delegation_rejection_reason: Option<PendingDelegationRejectionReason>,
    tool_authorization_ids: Vec<String>,
    tool_authorization_selected: usize,
    profile_menu_state: Option<ProfileMenuRenderState>,
    pending_profile_task: Option<PendingProfileTask>,
}

impl InteractiveRoot {
    #[cfg(test)]
    pub(super) fn new(cwd: PathBuf, model_id: String, session_label: String) -> Self {
        Self::new_with_theme(
            cwd,
            model_id,
            session_label,
            pi_tui::api::theme::dark_theme(),
        )
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
        let keybindings =
            KeybindingsManager::new(keybindings::default_keybindings(), Default::default());
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
        let mut focus_ring = FocusRing::new([
            InteractiveRegion::Conversation,
            InteractiveRegion::Context,
            InteractiveRegion::Composer,
        ]);
        focus_ring.focus(InteractiveRegion::Composer);
        let modal_overlay = TransientOverlayBridge::default();
        let support_overlay = TransientOverlayBridge::default();

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
            pending_fork_request: None,
            pending_agent_invocation_request: None,
            pending_agent_team_request: None,
            pending_self_healing_edit_request: None,
            pending_plugin_command_request: None,
            pending_delegation_confirmation_command: None,
            delegation_confirmation_menu: None,
            pending_delegation_rejection_reason: None,
            tool_authorizations: VecDeque::new(),
            tool_authorization_selected: 0,
            pending_tool_authorization_decision: None,
            pending_plugin_ui_dialog: None,
            active_plugin_ui_dialog: None,
            profile_menu: None,
            pending_profile_task: None,
            selected_agent_profile_id: None,
            action: InteractiveAction::None,
            status: InteractiveStatus::Idle,
            viewport_width: 80,
            viewport_height: 24,
            fullscreen_viewport: false,
            focus_ring,
            context_tab: ContextTab::Ops,
            context_open: false,
            context_restore_focus: InteractiveRegion::Composer,
            conversation_viewport_height: 1,
            modal_overlay,
            support_overlay,
            modal_overlay_handle: None,
            support_overlay_handle: None,
            git_branch: GitBranchProvider::new(&cwd),
            cwd,
            model_id,
            session_label,
            selected_model: None,
            selected_thinking_level: None,
            model: None,
            thinking_level: pi_agent_core::api::agent::ThinkingLevel::default(),
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

    pub(super) fn transient_overlay_components(
        &self,
    ) -> (
        crate::adapters::interactive::transient_overlay::TransientOverlay,
        crate::adapters::interactive::transient_overlay::TransientOverlay,
    ) {
        (
            self.support_overlay.component(),
            self.modal_overlay.component(),
        )
    }

    pub(super) fn install_transient_overlay_handles(
        &mut self,
        support: OverlayHandle,
        modal: OverlayHandle,
    ) {
        self.support_overlay_handle = Some(support);
        self.modal_overlay_handle = Some(modal);
    }

    pub(super) fn transient_overlay_handles(&self) -> Option<(OverlayHandle, OverlayHandle)> {
        Some((self.support_overlay_handle?, self.modal_overlay_handle?))
    }

    pub(super) fn prepare_transient_overlays(
        &mut self,
        terminal_width: usize,
    ) -> TransientOverlayProjection {
        let modal_lines = self.render_modal_surface(terminal_width.max(1));
        let support_width = terminal_width.saturating_sub(4).clamp(1, 72);
        let mut support_lines = self.render_transient_prompts(support_width);
        if modal_lines.is_empty() {
            support_lines.extend(self.render_completion_surface(support_width));
        }
        let modal_visible = !modal_lines.is_empty();
        let support_visible = !support_lines.is_empty();
        self.modal_overlay.set_lines(modal_lines);
        self.support_overlay.set_lines(support_lines);

        let composer_height = self
            .render_editor_box(terminal_width.max(1))
            .len()
            .clamp(1, MAX_COMPOSER_HEIGHT);
        TransientOverlayProjection {
            modal_visible,
            support_visible,
            bottom_margin: composer_height.saturating_add(1),
        }
    }

    pub(super) fn drain_modal_overlay_input(&mut self) {
        for event in self.modal_overlay.take_pending_input() {
            input::handle_root_input(self, &event);
        }
    }

    #[cfg(test)]
    pub(super) fn modal_overlay_focused(&self) -> bool {
        self.modal_overlay.focused()
    }

    pub(super) fn take_pending_tool_authorization_decision(
        &mut self,
    ) -> Option<(ToolAuthorizationRequest, ToolAuthorizationDecision)> {
        self.pending_tool_authorization_decision.take()
    }

    pub(super) fn restore_tool_authorization(&mut self, request: ToolAuthorizationRequest) {
        if self
            .tool_authorizations
            .iter()
            .all(|pending| pending.authorization_id != request.authorization_id)
        {
            self.tool_authorizations.push_front(request);
        }
        self.tool_authorization_selected = 0;
    }

    pub(super) fn take_selected_model(&mut self) -> Option<Model> {
        self.selected_model.take()
    }

    pub(super) fn take_selected_thinking_level(
        &mut self,
    ) -> Option<pi_agent_core::api::agent::ThinkingLevel> {
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

    pub(super) fn apply_tree_label_update(
        &mut self,
        entry_id: &str,
        label: Option<String>,
        updated_at: String,
    ) {
        if let Some(selector) = self.tree_selector.as_mut() {
            let timestamp = label.as_ref().map(|_| updated_at);
            selector.update_node_label(entry_id, label, timestamp);
        }
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

    pub(super) fn take_pending_fork_request(&mut self) -> Option<PendingForkRequest> {
        self.pending_fork_request.take()
    }

    pub(super) fn take_pending_agent_invocation_request(
        &mut self,
    ) -> Option<PendingAgentInvocationRequest> {
        self.pending_agent_invocation_request.take()
    }

    pub(super) fn take_pending_agent_team_request(&mut self) -> Option<PendingAgentTeamRequest> {
        self.pending_agent_team_request.take()
    }

    pub(super) fn take_pending_self_healing_edit_request(
        &mut self,
    ) -> Option<PendingSelfHealingEditRequest> {
        self.pending_self_healing_edit_request.take()
    }

    pub(super) fn take_pending_plugin_command_request(
        &mut self,
    ) -> Option<PendingPluginCommandRequest> {
        self.pending_plugin_command_request.take()
    }

    pub(super) fn take_pending_delegation_confirmation_command(
        &mut self,
    ) -> Option<PendingDelegationConfirmationCommand> {
        self.pending_delegation_confirmation_command.take()
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

    pub(super) fn open_delegation_confirmation_menu(
        &mut self,
        pending: Vec<PendingDelegationConfirmation>,
    ) {
        self.delegation_confirmation_menu = Some(DelegationConfirmationMenuState::new(pending));
        self.pending_delegation_rejection_reason = None;
        self.profile_menu = None;
        self.pending_profile_task = None;
        self.editor.set_text("");
        self.slash_suggestion_selected = 0;
        self.slash_suggestions_dismissed_for = None;
    }

    fn enqueue_delegation_confirmation(&mut self, pending: PendingDelegationConfirmation) {
        if let Some(menu) = self.delegation_confirmation_menu.as_mut() {
            menu.upsert(pending);
        } else {
            self.delegation_confirmation_menu =
                Some(DelegationConfirmationMenuState::new(vec![pending]));
        }
        self.pending_delegation_rejection_reason = None;
        self.profile_menu = None;
        self.pending_profile_task = None;
    }

    fn resolve_delegation_confirmation(&mut self, operation_id: &str, tool_call_id: &str) {
        let Some(menu) = self.delegation_confirmation_menu.as_mut() else {
            return;
        };
        menu.remove(operation_id, tool_call_id);
        if menu.is_empty() {
            self.delegation_confirmation_menu = None;
        }
    }

    pub(super) fn has_active_delegation_confirmation_menu(&self) -> bool {
        self.delegation_confirmation_menu.is_some()
    }

    pub(super) fn handle_delegation_confirmation_menu_input(&mut self, event: &InputEvent) -> bool {
        let Some(menu) = self.delegation_confirmation_menu.as_mut() else {
            return false;
        };
        let outcome = menu.handle_input(&self.keybindings, event);
        match outcome {
            DelegationConfirmationMenuOutcome::None => {}
            DelegationConfirmationMenuOutcome::Close => {
                self.delegation_confirmation_menu = None;
                self.editor.set_text("");
            }
            DelegationConfirmationMenuOutcome::Approve {
                operation_id,
                tool_call_id,
            } => {
                self.delegation_confirmation_menu = None;
                self.pending_delegation_confirmation_command =
                    Some(PendingDelegationConfirmationCommand::Approve {
                        selection: PendingDelegationConfirmationSelection {
                            operation_id: Some(operation_id),
                            tool_call_id,
                        },
                    });
                self.action = InteractiveAction::DelegationConfirmation;
            }
            DelegationConfirmationMenuOutcome::Reject {
                operation_id,
                tool_call_id,
            } => {
                self.delegation_confirmation_menu = None;
                self.pending_delegation_confirmation_command =
                    Some(PendingDelegationConfirmationCommand::Reject {
                        selection: PendingDelegationConfirmationSelection {
                            operation_id: Some(operation_id),
                            tool_call_id,
                        },
                        reason: None,
                    });
                self.action = InteractiveAction::DelegationConfirmation;
            }
            DelegationConfirmationMenuOutcome::RejectWithReason {
                operation_id,
                tool_call_id,
            } => {
                self.delegation_confirmation_menu = None;
                self.pending_delegation_rejection_reason = Some(PendingDelegationRejectionReason {
                    selection: PendingDelegationConfirmationSelection {
                        operation_id: Some(operation_id),
                        tool_call_id,
                    },
                });
                self.editor.set_text("");
            }
        }
        true
    }

    fn render_delegation_confirmation_menu(&mut self, width: usize) -> Vec<String> {
        let Some(menu) = self.delegation_confirmation_menu.as_mut() else {
            return Vec::new();
        };
        menu.render(width)
    }

    pub(super) fn has_pending_tool_authorization(&self) -> bool {
        !self.tool_authorizations.is_empty()
    }

    pub(super) fn handle_tool_authorization_input(&mut self, event: &InputEvent) -> bool {
        if self.tool_authorizations.is_empty() || matches_key(event, "ctrl+c") {
            return false;
        }
        if matches_key(event, "escape") {
            self.resolve_current_tool_authorization(ToolAuthorizationDecision::Deny {
                reason: None,
            });
            return true;
        }
        if self.keybindings.matches(event, "tui.select.up") {
            self.tool_authorization_selected = (self.tool_authorization_selected + 2) % 3;
            return true;
        }
        if self.keybindings.matches(event, "tui.select.down") {
            self.tool_authorization_selected = (self.tool_authorization_selected + 1) % 3;
            return true;
        }
        let InputEvent::Key(key_event) = event else {
            return true;
        };
        if key_event.kind == KeyEventKind::Release {
            return true;
        }
        if matches!(key_event.key, Key::Tab) {
            if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                self.tool_authorization_selected = (self.tool_authorization_selected + 2) % 3;
            } else {
                self.tool_authorization_selected = (self.tool_authorization_selected + 1) % 3;
            }
            return true;
        }
        if self.keybindings.matches(event, "tui.select.confirm") {
            let decision = match self.tool_authorization_selected {
                0 => ToolAuthorizationDecision::AllowOnce,
                1 => ToolAuthorizationDecision::AllowForOperation,
                _ => ToolAuthorizationDecision::Deny { reason: None },
            };
            self.resolve_current_tool_authorization(decision);
        }
        true
    }

    fn resolve_current_tool_authorization(&mut self, decision: ToolAuthorizationDecision) {
        let Some(request) = self.tool_authorizations.pop_front() else {
            return;
        };
        self.tool_authorization_selected = 0;
        self.pending_tool_authorization_decision = Some((request, decision));
        self.action = InteractiveAction::ToolAuthorization;
    }

    fn render_tool_authorization(&self, width: usize) -> Vec<String> {
        let Some(request) = self.tool_authorizations.front() else {
            return Vec::new();
        };
        let color = color_enabled();
        let mut lines = vec![fit_line(
            &paint_with(
                &format!("Tool authorization (1/{})", self.tool_authorizations.len()),
                &WARNING,
                color,
            ),
            width,
        )];
        lines.push(fit_line(
            &format!(
                "  tool: {}  risk: {}  operation: {}",
                request.tool_name,
                tool_authorization_risk_label(request.risk),
                request.operation_id
            ),
            width,
        ));
        lines.push(fit_line(&format!("  {}", request.preview.summary), width));
        if let Some(path) = request.preview.path.as_deref() {
            lines.push(fit_line(&format!("  path: {path}"), width));
        }
        if let Some(cwd) = request.preview.cwd.as_deref() {
            lines.push(fit_line(&format!("  cwd: {cwd}"), width));
        }
        if let Some(command) = request.preview.command.as_deref() {
            for (index, command_line) in command.lines().take(3).enumerate() {
                let label = if index == 0 { "command" } else { "       " };
                lines.push(fit_line(&format!("  {label}: {command_line}"), width));
            }
        }
        if let Some(content) = request.preview.content_preview.as_deref() {
            lines.push(fit_line("  preview:", width));
            for content_line in content.lines().take(6) {
                lines.push(fit_line(&format!("    {content_line}"), width));
            }
        }
        for (index, label) in ["Allow once", "Allow for operation", "Deny"]
            .into_iter()
            .enumerate()
        {
            let marker = if index == self.tool_authorization_selected {
                "->"
            } else {
                "  "
            };
            let line = format!("{marker} {label}");
            if index == self.tool_authorization_selected {
                lines.push(fit_line(&paint_with(&line, &USER, color), width));
            } else {
                lines.push(fit_line(&line, width));
            }
        }
        lines.push(fit_line(
            &paint_with(
                "Up/Down or Tab choose · Enter confirm · Esc deny · Ctrl+C abort operation",
                &SYSTEM,
                color,
            ),
            width,
        ));
        lines
    }

    pub(super) fn has_active_profile_menu(&self) -> bool {
        self.profile_menu.is_some()
    }

    pub(super) fn has_pending_delegation_rejection_reason(&self) -> bool {
        self.pending_delegation_rejection_reason.is_some()
    }

    pub(super) fn handle_pending_delegation_rejection_reason_input(
        &mut self,
        event: &InputEvent,
    ) -> bool {
        let Some(pending_reason) = self.pending_delegation_rejection_reason.clone() else {
            return false;
        };
        if matches_key(event, "escape") || matches_key(event, "ctrl+c") {
            self.pending_delegation_rejection_reason = None;
            self.editor.set_text("");
            self.transcript
                .push(TranscriptItem::system("Delegation rejection canceled"));
            return true;
        }

        let before_text = self.editor.text().to_string();
        self.editor.handle_input(event);
        if self.editor.text() != before_text {
            self.slash_suggestion_selected = 0;
            self.slash_suggestions_dismissed_for = None;
        }
        if let Some(command) = self.take_scroll_command() {
            let page_rows = self.viewport_height.saturating_sub(2).max(1);
            match command {
                TranscriptScrollCommand::PageUp => self.transcript.scroll_page_up(page_rows),
                TranscriptScrollCommand::PageDown => self.transcript.scroll_page_down(page_rows),
            }
        }
        let Some(text) = self.take_submitted() else {
            return true;
        };
        let reason = text.trim().to_string();
        self.pending_delegation_confirmation_command =
            Some(PendingDelegationConfirmationCommand::Reject {
                selection: pending_reason.selection,
                reason: (!reason.is_empty()).then_some(reason),
            });
        self.pending_delegation_rejection_reason = None;
        self.editor.set_text("");
        self.action = InteractiveAction::DelegationConfirmation;
        true
    }

    pub(super) fn has_pending_profile_task(&self) -> bool {
        self.pending_profile_task.is_some()
    }

    pub(super) fn open_agent_menu(&mut self) {
        self.delegation_confirmation_menu = None;
        self.pending_delegation_rejection_reason = None;
        self.profile_menu = Some(ProfileMenuState::agent());
        self.pending_profile_task = None;
        self.editor.set_text("");
        self.slash_suggestion_selected = 0;
        self.slash_suggestions_dismissed_for = None;
    }

    pub(super) fn open_team_menu(&mut self) {
        self.delegation_confirmation_menu = None;
        self.pending_delegation_rejection_reason = None;
        self.profile_menu = Some(ProfileMenuState::team());
        self.pending_profile_task = None;
        self.editor.set_text("");
        self.slash_suggestion_selected = 0;
        self.slash_suggestions_dismissed_for = None;
    }

    pub(super) fn handle_profile_menu_input(&mut self, event: &InputEvent) -> bool {
        let Some(menu) = self.profile_menu.as_mut() else {
            return false;
        };
        let outcome = menu.handle_input(
            &self.keybindings,
            event,
            &self.profile_registry,
            &self.default_agent_profile_id,
        );
        match outcome {
            ProfileMenuOutcome::None => {}
            ProfileMenuOutcome::Close => {
                self.profile_menu = None;
                self.editor.set_text("");
            }
            ProfileMenuOutcome::SetDefaultAgent(profile_id) => {
                self.profile_menu = None;
                self.set_default_agent_profile_id(profile_id.clone());
                self.selected_agent_profile_id = Some(profile_id.clone());
                self.action = InteractiveAction::AgentProfileUse;
                self.transcript.push(TranscriptItem::system(format!(
                    "Default agent profile: {profile_id}"
                )));
            }
            ProfileMenuOutcome::BeginAgentTask(profile_id) => {
                self.profile_menu = None;
                self.pending_profile_task = Some(PendingProfileTask::Agent { profile_id });
                self.editor.set_text("");
            }
            ProfileMenuOutcome::BeginTeamTask(team_id) => {
                self.profile_menu = None;
                self.pending_profile_task = Some(PendingProfileTask::Team { team_id });
                self.editor.set_text("");
            }
        }
        true
    }

    pub(super) fn handle_pending_profile_task_input(&mut self, event: &InputEvent) -> bool {
        let Some(pending_task) = self.pending_profile_task.clone() else {
            return false;
        };
        if matches_key(event, "escape") || matches_key(event, "ctrl+c") {
            self.pending_profile_task = None;
            self.editor.set_text("");
            self.transcript
                .push(TranscriptItem::system("Profile task canceled"));
            return true;
        }

        let before_text = self.editor.text().to_string();
        self.editor.handle_input(event);
        if self.editor.text() != before_text {
            self.slash_suggestion_selected = 0;
            self.slash_suggestions_dismissed_for = None;
        }
        if let Some(command) = self.take_scroll_command() {
            let page_rows = self.viewport_height.saturating_sub(2).max(1);
            match command {
                TranscriptScrollCommand::PageUp => self.transcript.scroll_page_up(page_rows),
                TranscriptScrollCommand::PageDown => self.transcript.scroll_page_down(page_rows),
            }
        }
        let Some(text) = self.take_submitted() else {
            return true;
        };
        let task = text.trim().to_string();
        if task.is_empty() {
            self.transcript
                .push(TranscriptItem::system("Profile task requires text"));
            return true;
        }
        self.editor.add_to_history(&task);
        match pending_task {
            PendingProfileTask::Agent { profile_id } => {
                self.pending_agent_invocation_request =
                    Some(PendingAgentInvocationRequest { profile_id, task });
                self.action = InteractiveAction::AgentInvocation;
            }
            PendingProfileTask::Team { team_id } => {
                self.pending_agent_team_request = Some(PendingAgentTeamRequest { team_id, task });
                self.action = InteractiveAction::AgentTeam;
            }
        }
        self.pending_profile_task = None;
        true
    }

    fn render_profile_menu(&mut self, width: usize) -> Vec<String> {
        let Some(menu) = self.profile_menu.as_mut() else {
            return Vec::new();
        };
        menu.render(
            &self.profile_registry,
            &self.default_agent_profile_id,
            width,
        )
    }

    fn render_pending_delegation_rejection_reason(&self, width: usize) -> Vec<String> {
        let Some(pending_reason) = &self.pending_delegation_rejection_reason else {
            return Vec::new();
        };
        let operation_id = pending_reason
            .selection
            .operation_id
            .as_deref()
            .unwrap_or("unknown-operation");
        let text = format!(
            "Delegation rejection reason for {operation_id} {}: enter reason, then press Enter",
            pending_reason.selection.tool_call_id
        );
        vec![fit_line(
            &paint_with(&text, &SYSTEM, color_enabled()),
            width,
        )]
    }

    fn render_pending_profile_task(&self, width: usize) -> Vec<String> {
        let Some(pending_task) = &self.pending_profile_task else {
            return Vec::new();
        };
        let text = match pending_task {
            PendingProfileTask::Agent { profile_id } => {
                format!("Agent {profile_id}: enter task, then press Enter")
            }
            PendingProfileTask::Team { team_id } => {
                format!("Team {team_id}: enter task, then press Enter")
            }
        };
        vec![fit_line(
            &paint_with(&text, &SYSTEM, color_enabled()),
            width,
        )]
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
            crate::adapters::interactive::commands::expand_skill_command(text, &self.skills)
        } else {
            text.to_string()
        };
        crate::adapters::interactive::commands::expand_prompt_template(
            &text,
            &self.prompt_templates,
        )
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
            || self.delegation_confirmation_menu.is_some()
            || self.pending_delegation_rejection_reason.is_some()
            || self.profile_menu.is_some()
            || self.pending_profile_task.is_some()
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
            .cloned()
        else {
            self.transcript.push(TranscriptItem::system(format!(
                "Plugin keybinding {} has no registered UI action",
                keybinding.id
            )));
            return true;
        };
        self.activate_plugin_ui_action(&action.id)
    }

    pub(super) fn activate_plugin_ui_action(&mut self, action_id: &str) -> bool {
        let Some(action) = self
            .plugin_ui_actions
            .iter()
            .find(|action| action.id == action_id)
            .cloned()
        else {
            self.transcript.push(TranscriptItem::system(format!(
                "Plugin UI action not found: {action_id}"
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
        self.transcript.push(TranscriptItem::system(format!(
            "Plugin UI action {} has unavailable target {}",
            action.id, action.action_id
        )));
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
                UiEvent::ToolAuthorizationRequired { request } => {
                    if self
                        .tool_authorizations
                        .iter()
                        .all(|pending| pending.authorization_id != request.authorization_id)
                        && self
                            .pending_tool_authorization_decision
                            .as_ref()
                            .is_none_or(|(pending, _)| {
                                pending.authorization_id != request.authorization_id
                            })
                    {
                        self.tool_authorizations.push_back(request);
                    }
                }
                UiEvent::ToolAuthorizationResolved { authorization_id } => {
                    self.tool_authorizations
                        .retain(|request| request.authorization_id != authorization_id);
                    if self
                        .pending_tool_authorization_decision
                        .as_ref()
                        .is_some_and(|(request, _)| request.authorization_id == authorization_id)
                    {
                        self.pending_tool_authorization_decision = None;
                    }
                    self.tool_authorization_selected = self.tool_authorization_selected.min(2);
                }
                UiEvent::DelegationConfirmationRequired { pending } => {
                    self.enqueue_delegation_confirmation(pending);
                }
                UiEvent::DelegationConfirmationResolved {
                    operation_id,
                    tool_call_id,
                } => {
                    self.resolve_delegation_confirmation(&operation_id, &tool_call_id);
                }
                UiEvent::UsageUpdate {
                    input,
                    output,
                    cache_read,
                    cache_write,
                    cost,
                    context_tokens,
                } => {
                    // Accumulate delta values from the stateless bridge.
                    // This ensures hydration-seeded stats are preserved:
                    //   root.stats starts at 0 (fresh) or at the hydrated
                    //   cumulative value, and each UsageUpdate adds to it.
                    self.stats.input = self.stats.input.saturating_add(input);
                    self.stats.output = self.stats.output.saturating_add(output);
                    self.stats.cache_read = self.stats.cache_read.saturating_add(cache_read);
                    self.stats.cache_write = self.stats.cache_write.saturating_add(cache_write);
                    self.stats.cost += cost;
                    self.stats.context_tokens = context_tokens;
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
        thinking_level: Option<pi_agent_core::api::agent::ThinkingLevel>,
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
        let mut status_text = format!("status: {status_str}");
        if self.fullscreen_viewport && self.transcript.has_new_output_below() {
            status_text.push_str(" | new output below");
        }
        let status_line = fit_line(&paint_with(&status_text, &status_style, color), width);

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
            if level == pi_agent_core::api::agent::ThinkingLevel::Off {
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
            focused_region: self.focus_ring.current(),
            context_tab: self.context_tab,
            context_open: self.context_open,
            status: self.status,
            stats: self.stats,
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
            delegation_confirmation_menu_state: self
                .delegation_confirmation_menu
                .as_ref()
                .map(|menu| menu.render_state()),
            pending_delegation_rejection_reason: self.pending_delegation_rejection_reason.clone(),
            tool_authorization_ids: self
                .tool_authorizations
                .iter()
                .map(|request| request.authorization_id.clone())
                .collect(),
            tool_authorization_selected: self.tool_authorization_selected,
            profile_menu_state: self.profile_menu.as_ref().map(|menu| menu.render_state()),
            pending_profile_task: self.pending_profile_task.clone(),
        }
    }

    pub(super) fn editor_border_style(&self) -> Style {
        if self.selecting_model
            || self.selecting_settings
            || self.selecting_session
            || self.delegation_confirmation_menu.is_some()
            || !self.tool_authorizations.is_empty()
            || self.pending_delegation_rejection_reason.is_some()
            || self.profile_menu.is_some()
            || self.pending_profile_task.is_some()
        {
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
    fn thinking_border_token(level: pi_agent_core::api::agent::ThinkingLevel) -> ThemeColor {
        use pi_agent_core::api::agent::ThinkingLevel;
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
        if self.selecting_model
            || self.selecting_settings
            || self.selecting_session
            || self.delegation_confirmation_menu.is_some()
            || self.profile_menu.is_some()
            || self.pending_profile_task.is_some()
        {
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
                    .get_or_insert_with(crate::config::settings::PartialCompaction::default)
                    .enabled = Some(value == "on");
            }
            "steering_mode" => {
                self.settings.steering_mode = value.to_string();
                self.settings_delta.steering_mode = Some(value.to_string());
            }
            "follow_up_mode" => {
                self.settings.follow_up_mode = value.to_string();
                self.settings_delta.follow_up_mode = Some(value.to_string());
            }
            "show_progress" => {
                self.settings.terminal.show_progress = value == "on";
                self.settings_delta
                    .terminal
                    .get_or_insert_with(crate::config::settings::PartialTerminal::default)
                    .show_progress = Some(value == "on");
            }
            "auto_resize_images" => {
                self.settings.terminal.auto_resize_images = value == "on";
                self.settings_delta
                    .terminal
                    .get_or_insert_with(crate::config::settings::PartialTerminal::default)
                    .auto_resize_images = Some(value == "on");
            }
            "block_images" => {
                self.settings.terminal.block_images = value == "on";
                self.settings_delta
                    .terminal
                    .get_or_insert_with(crate::config::settings::PartialTerminal::default)
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
            "quiet_startup" => {
                self.settings.quiet_startup = value == "on";
                self.settings_delta.quiet_startup = Some(value == "on");
            }
            "clear_on_shrink" => {
                self.settings.terminal.clear_on_shrink = value == "on";
                self.settings_delta
                    .terminal
                    .get_or_insert_with(crate::config::settings::PartialTerminal::default)
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
            "default_thinking_level" => {
                self.settings.default_thinking_level = Some(value.to_string());
                self.settings_delta.default_thinking_level = Some(value.to_string());
                // Also update the active thinking level so the editor border reflects it
                if let Ok(level) = value.parse::<pi_agent_core::api::agent::ThinkingLevel>() {
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
        let mut md = match &self.resolved_theme {
            Some(resolved) => markdown_theme_from_resolved(resolved),
            None => self.theme.markdown.clone(),
        };
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
        width: usize,
        max_tool_result_lines: usize,
    ) -> TranscriptRenderOptions<'static> {
        TranscriptRenderOptions {
            width,
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
        let opts = self.transcript_render_options(self.viewport_width, max_tool_result_lines);
        self.render_cache.render_lines(&self.transcript, &opts)
    }

    fn transcript_lines_at(&mut self, width: usize, max_tool_result_lines: usize) -> Vec<String> {
        let opts = self.transcript_render_options(width, max_tool_result_lines);
        self.render_cache.render_lines(&self.transcript, &opts)
    }

    pub(super) fn set_fullscreen_viewport(&mut self, enabled: bool) {
        self.fullscreen_viewport = enabled;
        self.refresh_shell_focus();
    }

    pub(super) fn handle_shell_input(&mut self, event: &InputEvent) -> bool {
        if !self.fullscreen_viewport {
            return false;
        }
        if self.selecting_model || self.selecting_session || self.selecting_settings {
            return false;
        }

        let mode = shell_layout_mode(self.viewport_width);
        if self.context_open && mode != ShellLayoutMode::Wide && matches_key(event, "escape") {
            self.close_context_overlay();
            return true;
        }
        if self.keybindings.matches(event, "app.context.toggle") {
            self.toggle_context(mode);
            return true;
        }

        let editor_accepts_tab = self.focus_ring.current() == Some(InteractiveRegion::Composer)
            && !self.editor.text().is_empty();
        if self.keybindings.matches(event, "app.focus.next") && !editor_accepts_tab {
            self.focus_ring.focus_next();
            self.apply_region_focus();
            return true;
        }
        if self.keybindings.matches(event, "app.focus.previous") {
            self.focus_ring.focus_previous();
            self.apply_region_focus();
            return true;
        }

        match self.focus_ring.current() {
            Some(InteractiveRegion::Conversation) => {
                if matches_key(event, "pageup") {
                    self.transcript
                        .scroll_page_up(self.conversation_viewport_height.max(1));
                    return true;
                }
                if matches_key(event, "pagedown") {
                    self.transcript
                        .scroll_page_down(self.conversation_viewport_height.max(1));
                    return true;
                }
            }
            Some(InteractiveRegion::Context) => {
                if matches_key(event, "left") {
                    self.context_tab = self.context_tab.previous();
                    return true;
                }
                if matches_key(event, "right") {
                    self.context_tab = self.context_tab.next();
                    return true;
                }
            }
            Some(InteractiveRegion::Composer) | None => return false,
        }

        if matches_key(event, "ctrl+c")
            || matches_key(event, "ctrl+o")
            || self.keybindings.matches(event, "app.model.next")
            || self.keybindings.matches(event, "app.model.previous")
        {
            return false;
        }
        true
    }

    fn toggle_context(&mut self, mode: ShellLayoutMode) {
        if mode == ShellLayoutMode::Wide {
            self.focus_ring.focus(InteractiveRegion::Context);
            self.apply_region_focus();
            return;
        }
        if self.context_open {
            self.close_context_overlay();
        } else {
            self.context_restore_focus = self
                .focus_ring
                .current()
                .unwrap_or(InteractiveRegion::Composer);
            self.context_open = true;
            self.refresh_shell_focus();
        }
    }

    fn close_context_overlay(&mut self) {
        self.context_open = false;
        self.refresh_shell_focus();
        self.focus_ring.focus(self.context_restore_focus);
        self.apply_region_focus();
    }

    fn refresh_shell_focus(&mut self) {
        if !self.fullscreen_viewport {
            self.focus_ring.set_items([InteractiveRegion::Composer]);
            self.focus_ring.focus(InteractiveRegion::Composer);
            self.apply_region_focus();
            return;
        }
        match shell_layout_mode(self.viewport_width) {
            ShellLayoutMode::Wide => {
                self.context_open = false;
                self.focus_ring.set_items([
                    InteractiveRegion::Conversation,
                    InteractiveRegion::Context,
                    InteractiveRegion::Composer,
                ]);
            }
            ShellLayoutMode::Medium | ShellLayoutMode::Narrow if self.context_open => {
                self.focus_ring.set_items([InteractiveRegion::Context]);
                self.focus_ring.focus(InteractiveRegion::Context);
            }
            ShellLayoutMode::Medium | ShellLayoutMode::Narrow => {
                self.focus_ring
                    .set_items([InteractiveRegion::Conversation, InteractiveRegion::Composer]);
            }
        }
        self.apply_region_focus();
    }

    fn apply_region_focus(&mut self) {
        self.editor
            .set_focused(self.focus_ring.current() == Some(InteractiveRegion::Composer));
    }

    fn shell_layout(&self, composer_height: usize) -> ShellLayout {
        let width = self.viewport_width.max(1);
        let height = self.viewport_height.max(1);
        let mode = shell_layout_mode(width);
        let status_height = usize::from(height >= 2);
        let maximum_composer = height.saturating_sub(status_height + 1).max(1);
        let composer_height = composer_height.clamp(1, maximum_composer);
        let rows = Layout::vertical(
            Rect::new(0, 0, width, height),
            &[
                Constraint::Fill(1),
                Constraint::Length(composer_height),
                Constraint::Length(status_height),
            ],
        );
        let work = rows[0];
        let composer = rows[1];
        let status = rows[2];

        match mode {
            ShellLayoutMode::Wide => {
                let context_width = (width / 3).clamp(26, 38).min(width.saturating_sub(2));
                let columns = Layout::horizontal(
                    work,
                    &[
                        Constraint::Fill(1),
                        Constraint::Length(1),
                        Constraint::Length(context_width),
                    ],
                );
                let side_rows = if work.height >= TIPS_MIN_HEIGHT {
                    Layout::vertical(
                        columns[2],
                        &[
                            Constraint::Fill(1),
                            Constraint::Length(1),
                            Constraint::Length(4),
                        ],
                    )
                } else {
                    Layout::vertical(columns[2], &[Constraint::Fill(1)])
                };
                ShellLayout {
                    mode,
                    conversation: columns[0],
                    context: Some(side_rows[0]),
                    tips: (side_rows.len() == 3).then(|| side_rows[2]),
                    composer,
                    status,
                    work,
                }
            }
            ShellLayoutMode::Medium => {
                let context = self.context_open.then(|| {
                    let overlay_width = (width * 2 / 5).clamp(26, 38).min(width);
                    Rect::new(
                        width.saturating_sub(overlay_width),
                        0,
                        overlay_width,
                        work.height,
                    )
                });
                ShellLayout {
                    mode,
                    conversation: work,
                    context,
                    tips: None,
                    composer,
                    status,
                    work,
                }
            }
            ShellLayoutMode::Narrow => ShellLayout {
                mode,
                conversation: work,
                context: self.context_open.then_some(work),
                tips: None,
                composer,
                status,
                work,
            },
        }
    }

    fn render_fullscreen_shell(&mut self, width: usize) -> Vec<String> {
        let editor_lines = self.render_editor_box(width);
        let composer_height = editor_lines.len().clamp(1, MAX_COMPOSER_HEIGHT);
        let layout = self.shell_layout(composer_height);
        let mut frame = Frame::new(self.viewport_width, self.viewport_height);

        let conversation_body = panel_body(layout.conversation);
        self.conversation_viewport_height = conversation_body.height.max(1);
        let max_tool_result_lines = if self.tool_output_expanded {
            EXPANDED_TOOL_RESULT_LINES
        } else {
            MAX_TOOL_RESULT_LINES
        };
        let transcript_lines =
            self.transcript_lines_at(conversation_body.width.max(1), max_tool_result_lines);
        let transcript_lines = transcript_viewport(
            &transcript_lines,
            conversation_body.height,
            self.transcript.scroll_offset(),
        );
        frame.draw(
            Rect::new(
                layout.conversation.x,
                layout.conversation.y,
                layout.conversation.width,
                1.min(layout.conversation.height),
            ),
            &[self.panel_header(
                "Conversation",
                InteractiveRegion::Conversation,
                layout.conversation.width,
            )],
        );
        frame.draw(conversation_body, &transcript_lines);

        if layout.mode == ShellLayoutMode::Wide {
            let separator = Rect::new(
                layout.conversation.right(),
                layout.work.y,
                1,
                layout.work.height,
            );
            frame.fill(separator, "│");
        }
        if let Some(context) = layout.context {
            let context_lines = self.render_context_region(context.width, context.height);
            if layout.mode != ShellLayoutMode::Wide {
                frame.fill(context, "");
            }
            frame.draw(context, &context_lines);
        }
        if let Some(tips) = layout.tips {
            frame.draw(tips, &self.render_tips_region(tips.width, tips.height));
        }

        let composer_lines = tail_lines(&editor_lines, layout.composer.height);
        frame.draw(layout.composer, &composer_lines);
        if !layout.status.is_empty() {
            frame.draw(
                layout.status,
                &[self.render_status_bar(layout.status.width)],
            );
        }

        frame.into_lines()
    }

    fn panel_header(&self, title: &str, region: InteractiveRegion, width: usize) -> String {
        let prefix = if self.focus_ring.current() == Some(region) {
            "> "
        } else {
            "  "
        };
        fit_line(&format!("{prefix}{title}"), width)
    }

    fn render_context_region(&self, width: usize, height: usize) -> Vec<String> {
        if width == 0 || height == 0 {
            return Vec::new();
        }
        let tabs = ContextTab::ALL
            .iter()
            .map(|tab| {
                if *tab == self.context_tab {
                    format!("[{}]", tab.label())
                } else {
                    tab.label().to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        let mut lines = vec![self.panel_header(
            &format!("Context {tabs}"),
            InteractiveRegion::Context,
            width,
        )];
        match self.context_tab {
            ContextTab::Ops => {
                lines.push(format!(
                    "state     {}",
                    match self.status {
                        InteractiveStatus::Idle => "idle",
                        InteractiveStatus::Running => "running",
                    }
                ));
                lines.push("operation unavailable".into());
            }
            ContextTab::Changes => {
                lines.push("change projection unavailable".into());
            }
            ContextTab::Agents => {
                let active = self
                    .profile_registry
                    .agent(self.default_agent_profile_id.as_str());
                lines.push(format!(
                    "active    {}",
                    active
                        .map(|profile| profile.display_name.as_str())
                        .unwrap_or(self.default_agent_profile_id.as_str())
                ));
                lines.push(format!(
                    "profiles  {} agents / {} teams",
                    self.profile_registry.agents().count(),
                    self.profile_registry.teams().count()
                ));
            }
            ContextTab::Usage => {
                lines.push(format!("input     {}", format_tokens(self.stats.input)));
                lines.push(format!("output    {}", format_tokens(self.stats.output)));
                lines.push(format!(
                    "cache     {}",
                    format_tokens(self.stats.cache_read)
                ));
                if self.stats.cost > 0.0 {
                    lines.push(format!("cost      ${:.3}", self.stats.cost));
                } else {
                    lines.push("cost      unavailable".into());
                }
            }
        }
        lines.truncate(height);
        lines
            .into_iter()
            .map(|line| fit_line(&line, width))
            .collect()
    }

    fn render_tips_region(&self, width: usize, height: usize) -> Vec<String> {
        let key = |id: &str| {
            self.keybindings
                .get_keys(id)
                .into_iter()
                .next()
                .unwrap_or_else(|| "?".into())
        };
        let mut lines = vec![fit_line("  Tips", width)];
        lines.push(fit_line(
            &format!(
                "{} / {}  focus",
                key("app.focus.next"),
                key("app.focus.previous")
            ),
            width,
        ));
        lines.push(fit_line(
            &format!("{}  context", key("app.context.toggle")),
            width,
        ));
        let focused = match self.focus_ring.current() {
            Some(InteractiveRegion::Conversation) => "PageUp/PageDown  scroll",
            Some(InteractiveRegion::Context) => "Left/Right  tabs",
            Some(InteractiveRegion::Composer) => "Enter  submit",
            None => "",
        };
        lines.push(fit_line(focused, width));
        lines.truncate(height);
        lines
    }

    fn render_status_bar(&self, width: usize) -> String {
        let status = match self.status {
            InteractiveStatus::Idle => "idle".to_string(),
            InteractiveStatus::Running => running_status_text(self.spinner_frame),
        };
        let pending = if self.transcript.has_new_output_below() {
            " | new output below"
        } else {
            ""
        };
        let model = self
            .current_model()
            .map_or("no-model", |model| model.id.as_str());
        fit_line(
            &format!(
                " {status}{pending} | {} | {model} | {}",
                self.session_label,
                abbreviate_cwd(&self.cwd)
            ),
            width,
        )
    }

    fn render_transient_prompts(&self, width: usize) -> Vec<String> {
        let mut lines = self.render_pending_delegation_rejection_reason(width);
        lines.extend(self.render_pending_profile_task(width));
        lines
    }

    fn render_modal_surface(&mut self, width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        if self.selecting_tree {
            if let Some(ref selector) = self.tree_selector {
                lines.extend(selector.render(width));
            }
        } else if !self.tool_authorizations.is_empty() {
            lines.extend(self.render_tool_authorization(width));
        } else if self.active_plugin_ui_dialog.is_some() {
            lines.extend(self.render_plugin_dialog_form(width));
        } else if self.delegation_confirmation_menu.is_some() {
            lines.extend(self.render_delegation_confirmation_menu(width));
        } else if self.profile_menu.is_some() {
            lines.extend(self.render_profile_menu(width));
        } else if self.selecting_model {
            lines.extend(self.render_model_selector(width));
        } else if self.selecting_session {
            lines.extend(self.render_session_selector(width));
        } else if self.selecting_settings {
            lines.extend(self.render_settings_menu(width));
        }
        lines
    }

    fn render_completion_surface(&mut self, width: usize) -> Vec<String> {
        self.render_slash_suggestions(width)
    }

    fn render_transient_surface(&mut self, width: usize) -> Vec<String> {
        let mut lines = self.render_transient_prompts(width);
        let modal = self.render_modal_surface(width);
        if modal.is_empty() {
            lines.extend(self.render_completion_surface(width));
        } else {
            lines.extend(modal);
        }
        lines
    }

    fn transcript_row_snapshot(&mut self, max_tool_result_lines: usize) -> TranscriptRowSnapshot {
        let opts = self.transcript_render_options(self.viewport_width, max_tool_result_lines);
        self.render_cache.row_snapshot(&self.transcript, &opts)
    }

    fn transcript_row_delta_since(
        &mut self,
        snapshot: TranscriptRowSnapshot,
        changed_indices: &[usize],
        max_tool_result_lines: usize,
    ) -> usize {
        let opts = self.transcript_render_options(self.viewport_width, max_tool_result_lines);
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
    ) -> crate::adapters::interactive::render::TranscriptRenderCacheStats {
        self.render_cache.stats()
    }

    pub(super) fn handle_slash_suggestion_input(&mut self, event: &InputEvent) -> bool {
        if self.selecting_model
            || self.selecting_settings
            || self.selecting_session
            || self.delegation_confirmation_menu.is_some()
            || self.profile_menu.is_some()
            || self.pending_profile_task.is_some()
        {
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
        if self.fullscreen_viewport {
            return self.render_fullscreen_shell(width);
        }

        let max_tool_result_lines = if self.tool_output_expanded {
            EXPANDED_TOOL_RESULT_LINES
        } else {
            MAX_TOOL_RESULT_LINES
        };
        let mut lines = self.transcript_lines(max_tool_result_lines);
        lines.extend(self.render_editor_box(width));
        lines.extend(self.render_transient_surface(width));
        lines.extend(self.footer(width));
        lines
    }

    fn handle_input(&mut self, event: &InputEvent) {
        input::handle_root_input(self, event);
    }

    fn set_viewport_size(&mut self, width: usize, height: usize) {
        self.viewport_width = width.max(1);
        self.viewport_height = height.max(1);
        self.refresh_shell_focus();
    }

    fn set_focused(&mut self, focused: bool) {
        if focused {
            self.apply_region_focus();
        } else {
            self.editor.set_focused(false);
        }
    }

    fn focused(&self) -> bool {
        self.editor.focused()
    }
}

fn transcript_viewport(lines: &[String], height: usize, scroll_offset: usize) -> Vec<String> {
    if height == 0 || lines.is_empty() {
        return Vec::new();
    }
    let max_offset = lines.len().saturating_sub(height);
    let offset = scroll_offset.min(max_offset);
    let end = lines.len().saturating_sub(offset);
    let start = end.saturating_sub(height);
    lines[start..end].to_vec()
}

fn shell_layout_mode(width: usize) -> ShellLayoutMode {
    if width >= WIDE_LAYOUT_MIN_WIDTH {
        ShellLayoutMode::Wide
    } else if width >= MEDIUM_LAYOUT_MIN_WIDTH {
        ShellLayoutMode::Medium
    } else {
        ShellLayoutMode::Narrow
    }
}

fn panel_body(panel: Rect) -> Rect {
    Rect::new(
        panel.x,
        panel.y.saturating_add(usize::from(panel.height > 0)),
        panel.width,
        panel.height.saturating_sub(1),
    )
}

fn tail_lines(lines: &[String], height: usize) -> Vec<String> {
    if height == 0 {
        return Vec::new();
    }
    lines[lines.len().saturating_sub(height)..].to_vec()
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

fn tool_authorization_risk_label(risk: ToolAuthorizationRisk) -> &'static str {
    match risk {
        ToolAuthorizationRisk::ExternalRead => "external read",
        ToolAuthorizationRisk::FilesystemMutation => "filesystem mutation",
        ToolAuthorizationRisk::ShellExecution => "shell execution",
        ToolAuthorizationRisk::PluginSideEffect => "plugin side effect",
        ToolAuthorizationRisk::Unknown => "unknown",
    }
}

fn format_http_idle_timeout_ms(timeout_ms: u64) -> String {
    HTTP_IDLE_TIMEOUT_CHOICES
        .iter()
        .find(|(_, value)| *value == timeout_ms)
        .map(|(label, _)| (*label).to_string())
        .unwrap_or_else(|| format!("{} sec", timeout_ms as f64 / 1000.0))
}

#[cfg(test)]
mod transcript_viewport_tests {
    use std::path::PathBuf;

    use pi_tui::api::component::Component;
    use pi_tui::api::input::{InputEvent, parse_key};

    use super::{ContextTab, InteractiveRegion, InteractiveRoot, transcript_viewport};

    fn lines() -> Vec<String> {
        (1..=6).map(|line| line.to_string()).collect()
    }

    #[test]
    fn follows_the_bottom_when_not_scrolled() {
        assert_eq!(transcript_viewport(&lines(), 3, 0), ["4", "5", "6"]);
    }

    #[test]
    fn preserves_an_offset_from_the_bottom() {
        assert_eq!(transcript_viewport(&lines(), 3, 2), ["2", "3", "4"]);
    }

    #[test]
    fn clamps_offsets_and_empty_viewports() {
        assert_eq!(
            transcript_viewport(&lines(), 3, usize::MAX),
            ["1", "2", "3"]
        );
        assert!(transcript_viewport(&lines(), 0, 0).is_empty());
        assert!(transcript_viewport(&[], 3, 0).is_empty());
    }

    fn key(data: &str) -> InputEvent {
        InputEvent::Key(parse_key(data).expect("test key should parse"))
    }

    #[test]
    fn fullscreen_focus_ring_tracks_visible_regions_and_restores_overlay_focus() {
        let mut root = InteractiveRoot::new(PathBuf::from("."), "model".into(), "session".into());
        root.set_fullscreen_viewport(true);
        root.set_viewport_size(120, 24);
        assert_eq!(root.focus_ring.current(), Some(InteractiveRegion::Composer));

        assert!(root.handle_shell_input(&key("\t")));
        assert_eq!(
            root.focus_ring.current(),
            Some(InteractiveRegion::Conversation)
        );
        assert!(root.handle_shell_input(&key("\t")));
        assert_eq!(root.focus_ring.current(), Some(InteractiveRegion::Context));

        root.set_viewport_size(80, 24);
        assert_eq!(
            root.focus_ring.current(),
            Some(InteractiveRegion::Conversation)
        );
        assert!(root.handle_shell_input(&key("\x07")));
        assert_eq!(root.focus_ring.current(), Some(InteractiveRegion::Context));
        assert!(root.context_open);
        assert!(root.handle_shell_input(&key("\x1b")));
        assert_eq!(
            root.focus_ring.current(),
            Some(InteractiveRegion::Conversation)
        );
        assert!(!root.context_open);
    }

    #[test]
    fn context_focus_cycles_tabs_without_editing_composer() {
        let mut root = InteractiveRoot::new(PathBuf::from("."), "model".into(), "session".into());
        root.set_fullscreen_viewport(true);
        root.set_viewport_size(120, 24);
        root.focus_ring.focus(InteractiveRegion::Context);
        root.apply_region_focus();

        assert!(root.handle_shell_input(&key("\x1b[C")));
        assert_eq!(root.context_tab, ContextTab::Changes);
        assert!(root.editor.text().is_empty());
        assert!(root.handle_shell_input(&key("\x1b[D")));
        assert_eq!(root.context_tab, ContextTab::Ops);
    }
}
