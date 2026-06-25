use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pi_agent_core::session::JsonlSessionStorage;
use pi_ai::types::Model;
use pi_tui::{
    Component, Editor, InputEvent, KeybindingsManager, PATH, STATUS_IDLE, STATUS_RUNNING, SYSTEM,
    SettingItem, SettingsList, SettingsListOptions, Style, TUI_KEYBINDINGS, TuiTheme,
    color_enabled, dark_theme, light_theme, paint_with,
};

use crate::config::{AuthStore, Settings};
use crate::interactive::app::{PromptContext, welcome_line};
use crate::interactive::clipboard::{ClipboardSink, SystemClipboard};
use crate::interactive::commands;
use crate::interactive::input;
use crate::interactive::model_selector;
use crate::interactive::render::{
    abbreviate_cwd, editor_border_line, fit_line, format_tokens, render_transcript_lines,
    running_status_text,
};
use crate::interactive::session_actions::{HydratedSession, SessionChoice};
use crate::interactive::session_selector;
use crate::interactive::slash::{self, ParsedSlashCommand};
use crate::interactive::{Transcript, TranscriptItem, UiEvent};

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
    pub(super) usage: (u32, u32),
    pub(super) tool_output_expanded: bool,
    pub(super) spinner_frame: usize,
    pub(super) slash_suggestion_selected: usize,
    pub(super) slash_suggestions_dismissed_for: Option<String>,
    pub(super) theme: TuiTheme,
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
    pub(super) fn with_theme(mut self, theme: TuiTheme) -> Self {
        self.theme = theme;
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
        self.available_models = prompt_context.model_choices.clone();
        self.model_rotation = prompt_context.model_rotation.clone();
        self.session_choices = prompt_context.session_choices.clone();
        self.theme = prompt_context.theme.clone();
        self.settings = prompt_context.settings.clone();
        self.settings_list =
            build_settings_list(self.settings.clone(), &self.theme, self.keybindings.clone());
        self.auth = prompt_context.auth.clone();
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

    pub(super) fn footer(&self) -> String {
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
        } else {
            self.theme.editor.active_border
        }
    }

    fn render_slash_suggestions(&mut self, width: usize) -> Vec<String> {
        if self.selecting_model || self.selecting_settings || self.selecting_session {
            return Vec::new();
        }

        slash::render_suggestions(
            self.editor.text(),
            self.editor.cursor(),
            self.slash_suggestions_dismissed_for.as_deref(),
            &mut self.slash_suggestion_selected,
            width,
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
                self.theme = match value {
                    "light" => light_theme(),
                    _ => dark_theme(),
                };
            }
            "auto_compaction" => {
                self.settings.compaction.enabled = value == "on";
            }
            _ => return,
        }
        self.settings_update = Some(self.settings.clone());
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
        slash::handle_suggestion_input(
            &self.keybindings,
            event,
            &mut self.editor,
            &mut self.slash_suggestion_selected,
            &mut self.slash_suggestions_dismissed_for,
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
