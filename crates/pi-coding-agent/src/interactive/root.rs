use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pi_agent_core::session::JsonlSessionStorage;
use pi_ai::types::Model;
use pi_tui::{
    Component, ERROR, Editor, InputEvent, KeybindingsManager, MarkdownTheme, STATUS_IDLE,
    STATUS_RUNNING, SYSTEM, SettingItem, SettingsList, SettingsListOptions, Style, TUI_KEYBINDINGS,
    TuiTheme, color_enabled, dark_theme, light_theme, paint_with, truncate_to_width,
    truncate_to_width_with_ellipsis, visible_width,
};

use crate::config::{AuthStore, Settings};
use crate::interactive::app::{PromptContext, welcome_line};
use crate::interactive::clipboard::{ClipboardSink, SystemClipboard};
use crate::interactive::commands;
use crate::interactive::git_branch::GitBranchProvider;
use crate::interactive::input;
use crate::interactive::model_selector;
use crate::interactive::render::{
    WARNING, abbreviate_cwd, editor_border_line, fit_line, format_tokens, render_transcript_lines,
    running_status_text,
};
use crate::interactive::session_actions::{HydratedSession, SessionChoice};
use crate::interactive::session_selector;
use crate::interactive::slash::{self, ParsedSlashCommand};
use crate::interactive::{Transcript, TranscriptItem, UiEvent};
use crate::theme::{ResolvedTheme, ThemeColor};

const MAX_TOOL_RESULT_LINES: usize = 3;
const EXPANDED_TOOL_RESULT_LINES: usize = 20;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InteractiveAction {
    None,
    Submit,
    CompactSession,
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

pub(super) struct InteractiveRoot {
    pub(super) transcript: Transcript,
    pub(super) editor: Editor,
    pub(super) keybindings: KeybindingsManager,
    pub(super) submitted: Arc<Mutex<Option<String>>>,
    pub(super) scroll_command: Arc<Mutex<Option<TranscriptScrollCommand>>>,
    pub(super) pending_submit: Option<String>,
    pub(super) pending_compact_instructions: Option<String>,
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
    pub(super) active_session_path: Option<PathBuf>,
    pub(super) active_leaf_id: Option<String>,
    pub(super) selecting_session: bool,
    pub(super) session_selection_selected: usize,
    pub(super) selecting_settings: bool,
    pub(super) settings: Settings,
    settings_list: SettingsList,
    settings_update: Option<Settings>,
    pub(super) auth: AuthStore,
    auth_update: Option<AuthStore>,
    pub(super) git_branch: GitBranchProvider,
    pub(super) stats: FooterStats,
    pub(super) tool_output_expanded: bool,
    pub(super) spinner_frame: usize,
    pub(super) slash_suggestion_selected: usize,
    pub(super) slash_suggestions_dismissed_for: Option<String>,
    pub(super) theme: TuiTheme,
    pub(super) resolved_theme: Option<ResolvedTheme>,
    pub(super) prompt_templates: Vec<pi_agent_core::PromptTemplate>,
    pub(super) skills: Vec<pi_agent_core::Skill>,
    pub(super) clipboard: Arc<dyn ClipboardSink>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct InteractiveRenderState {
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
    settings: Settings,
    auth: AuthStore,
    theme_name: String,
    settings_selected_item_id: Option<String>,
    selecting_model: bool,
    model_selection_selected: usize,
    selecting_session: bool,
    session_selection_selected: usize,
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

        Self {
            transcript,
            editor,
            keybindings,
            submitted,
            scroll_command,
            pending_submit: None,
            pending_compact_instructions: None,
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
            active_session_path: None,
            active_leaf_id: None,
            selecting_session: false,
            session_selection_selected: 0,
            selecting_settings: false,
            settings,
            settings_list,
            settings_update: None,
            auth,
            auth_update: None,
            stats: FooterStats::default(),
            tool_output_expanded: false,
            spinner_frame: 0,
            slash_suggestion_selected: 0,
            slash_suggestions_dismissed_for: None,
            theme,
            resolved_theme: None,
            prompt_templates: Vec::new(),
            skills: Vec::new(),
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

    pub(super) fn take_selected_session(&mut self) -> Option<SessionChoice> {
        self.selected_session.take()
    }

    pub(super) fn take_selected_session_hydrate(&mut self) -> bool {
        std::mem::take(&mut self.selected_session_hydrate)
    }

    pub(super) fn take_settings_update(&mut self) -> Option<Settings> {
        self.settings_update.take()
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

    pub(super) fn take_scroll_command(&mut self) -> Option<TranscriptScrollCommand> {
        self.scroll_command.lock().unwrap().take()
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
        self.auth = prompt_context.auth.clone();
        self.git_branch.set_cwd(&self.cwd);
        self.prompt_templates = prompt_context.resources.prompt_templates.clone();
        self.skills = prompt_context.resources.skills.clone();
    }

    pub(super) fn expand_prompt_text(&self, text: &str) -> String {
        let text = crate::interactive::commands::expand_skill_command(text, &self.skills);
        crate::interactive::commands::expand_prompt_template(&text, &self.prompt_templates)
    }

    pub(super) fn all_slash_commands(&self) -> Vec<slash::BuiltinSlashCommand> {
        let mut commands = slash::builtin_slash_commands();
        for t in &self.prompt_templates {
            commands.push(slash::BuiltinSlashCommand {
                name: t.name.clone(),
                description: t.description.clone(),
            });
        }
        for s in &self.skills {
            commands.push(slash::BuiltinSlashCommand {
                name: format!("skill:{}", s.name),
                description: s.description.clone(),
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
            render_transcript_lines(
                &self.transcript,
                self.viewport_width,
                MAX_TOOL_RESULT_LINES,
                color_enabled(),
                &self.markdown_theme(),
            )
            .len()
        } else {
            0
        };
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
                other => self.transcript.apply_event(other),
            }
        }
        if previous_scroll_offset > 0 {
            let current_rows = render_transcript_lines(
                &self.transcript,
                self.viewport_width,
                MAX_TOOL_RESULT_LINES,
                color_enabled(),
                &self.markdown_theme(),
            )
            .len();
            self.transcript.preserve_scrolled_view_after_hidden_change(
                previous_scroll_offset,
                current_rows.saturating_sub(previous_rows),
            );
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

    pub(super) fn apply_hydrated_session(
        &mut self,
        hydrated: HydratedSession,
        notice: Option<String>,
    ) {
        self.session_label = hydrated.choice.display_name().to_string();
        self.active_session_path = Some(hydrated.choice.path.clone());
        self.active_leaf_id = hydrated.leaf_id;

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
            transcript: self.transcript.items().to_vec(),
            transcript_scroll_offset: self.transcript.scroll_offset(),
            transcript_has_new_output_below: self.transcript.has_new_output_below(),
            status: self.status,
            tool_output_expanded: self.tool_output_expanded,
            spinner_frame: self.spinner_frame,
            slash_suggestion_selected: self.slash_suggestion_selected,
            slash_suggestions_dismissed_for: self.slash_suggestions_dismissed_for.clone(),
            selecting_settings: self.selecting_settings,
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
                self.apply_builtin_theme(value);
            }
            "auto_compaction" => {
                self.settings.compaction.enabled = value == "on";
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
        let mut lines = render_transcript_lines(
            &self.transcript,
            width,
            max_tool_result_lines,
            color_enabled(),
            &self.markdown_theme(),
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
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
        ],
        6,
        keybindings,
        SettingsListOptions {
            enable_search: false,
        },
    )
}
