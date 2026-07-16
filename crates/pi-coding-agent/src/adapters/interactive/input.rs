use std::io::Read;

use pi_tui::api::component::Component;
use pi_tui::api::input::{InputEvent, matches_key};

use crate::adapters::interactive::root::{InteractiveAction, InteractiveRoot, InteractiveStatus};
use crate::adapters::interactive::slash::parse_slash_command;
use crate::adapters::interactive::{TranscriptItem, root::TranscriptScrollCommand};

pub(super) struct InputPump {
    rx: tokio::sync::mpsc::UnboundedReceiver<String>,
    consumed_tx: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    idle_tx: Option<tokio::sync::mpsc::UnboundedSender<()>>,
    _reader: Option<std::thread::JoinHandle<()>>,
}

impl InputPump {
    pub(super) fn from_stdin() -> Self {
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
            consumed_tx: None,
            idle_tx: None,
            _reader: Some(reader),
        }
    }

    #[cfg(test)]
    pub(super) fn from_chunks(chunks: Vec<String>) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        for chunk in chunks {
            let _ = tx.send(chunk);
        }
        drop(tx);
        Self {
            rx,
            consumed_tx: None,
            idle_tx: None,
            _reader: None,
        }
    }

    #[cfg(test)]
    pub(super) fn from_receiver(rx: tokio::sync::mpsc::UnboundedReceiver<String>) -> Self {
        Self {
            rx,
            consumed_tx: None,
            idle_tx: None,
            _reader: None,
        }
    }

    #[cfg(test)]
    pub(super) fn from_receiver_with_observers(
        rx: tokio::sync::mpsc::UnboundedReceiver<String>,
        consumed_tx: tokio::sync::mpsc::UnboundedSender<String>,
        idle_tx: tokio::sync::mpsc::UnboundedSender<()>,
    ) -> Self {
        Self {
            rx,
            consumed_tx: Some(consumed_tx),
            idle_tx: Some(idle_tx),
            _reader: None,
        }
    }

    pub(super) async fn recv(&mut self) -> Option<String> {
        self.rx.recv().await
    }

    pub(super) fn mark_processed(&self, chunk: &str) {
        if let Some(consumed_tx) = &self.consumed_tx {
            let _ = consumed_tx.send(chunk.to_string());
        }
    }

    pub(super) fn mark_idle(&self) {
        if let Some(idle_tx) = &self.idle_tx {
            let _ = idle_tx.send(());
        }
    }
}

pub(super) fn handle_root_input(root: &mut InteractiveRoot, event: &InputEvent) {
    // Tree selector has highest priority when active
    if root.selecting_tree {
        if matches_key(event, "ctrl+c") {
            root.selecting_tree = false;
            root.tree_selector = None;
            root.selected_tree_entry_id = None;
            root.editor.set_text("");
            return;
        }
        root.handle_tree_selection_input(event);
        return;
    }

    if root.has_pending_tool_authorization() && root.handle_tool_authorization_input(event) {
        return;
    }

    if root.status == InteractiveStatus::Idle
        && root.has_active_plugin_ui_dialog()
        && root.handle_plugin_dialog_form_input(event)
    {
        return;
    }

    if root.status == InteractiveStatus::Idle
        && root.has_active_delegation_confirmation_menu()
        && root.handle_delegation_confirmation_menu_input(event)
    {
        return;
    }

    if root.status == InteractiveStatus::Idle
        && root.has_active_profile_menu()
        && root.handle_profile_menu_input(event)
    {
        return;
    }

    if root.status == InteractiveStatus::Idle
        && root.has_pending_delegation_rejection_reason()
        && root.handle_pending_delegation_rejection_reason_input(event)
    {
        return;
    }

    if root.status == InteractiveStatus::Idle
        && root.has_pending_profile_task()
        && root.handle_pending_profile_task_input(event)
    {
        return;
    }

    if root.handle_shell_input(event) {
        return;
    }

    if matches_key(event, "ctrl+c") || matches_key(event, "escape") {
        match root.status {
            InteractiveStatus::Running => {
                root.action = InteractiveAction::AbortRunning;
                return;
            }
            InteractiveStatus::Idle => {
                if matches_key(event, "ctrl+c") {
                    if root.editor.text().is_empty() {
                        root.action = InteractiveAction::Exit;
                    } else {
                        root.editor.set_text("");
                    }
                    return;
                }
            }
        }
    }

    if matches_key(event, "ctrl+o") {
        root.tool_output_expanded = !root.tool_output_expanded;
        return;
    }

    if root.status == InteractiveStatus::Running {
        if matches_key(event, "shift+enter") {
            let text = root.editor.expanded_text().trim().to_string();
            root.editor.set_text("");
            if !text.is_empty() {
                root.editor.add_to_history(&text);
                root.pending_submit = Some(text);
                root.action = InteractiveAction::FollowUp;
            }
            return;
        }

        let before_text = root.editor.text().to_string();
        root.editor.handle_input(event);
        if root.editor.text() != before_text {
            root.slash_suggestion_selected = 0;
            root.slash_suggestions_dismissed_for = None;
        }
        if let Some(command) = root.take_scroll_command() {
            let page_rows = root.viewport_height.saturating_sub(2).max(1);
            match command {
                TranscriptScrollCommand::PageUp => root.transcript.scroll_page_up(page_rows),
                TranscriptScrollCommand::PageDown => root.transcript.scroll_page_down(page_rows),
            }
        }
        if let Some(text) = root.take_submitted() {
            root.editor.add_to_history(&text);
            root.pending_submit = Some(text);
            root.action = InteractiveAction::Submit;
        }
        return;
    }

    if root.status == InteractiveStatus::Idle
        && !root.selecting_model
        && !root.selecting_session
        && !root.selecting_settings
    {
        if root.keybindings.matches(event, "app.model.next") {
            root.cycle_model_rotation(false);
            return;
        }
        if root.keybindings.matches(event, "app.model.previous") {
            root.cycle_model_rotation(true);
            return;
        }
    }

    if root.selecting_model && matches_key(event, "escape") {
        root.selecting_model = false;
        root.editor.set_text("");
        root.transcript.push(TranscriptItem::system(
            "Model selection canceled".to_string(),
        ));
        return;
    }

    if root.selecting_session && matches_key(event, "escape") {
        root.selecting_session = false;
        root.editor.set_text("");
        root.transcript.push(TranscriptItem::system(
            "Session selection canceled".to_string(),
        ));
        return;
    }

    if root.selecting_settings && matches_key(event, "escape") {
        root.selecting_settings = false;
        root.editor.set_text("");
        return;
    }

    if root.status == InteractiveStatus::Idle {
        let before_text = root.editor.text().to_string();
        if root.handle_slash_suggestion_input(event) {
            root.clear_empty_editor_escape();
            return;
        }
        if !root.selecting_model
            && !root.selecting_session
            && !root.selecting_settings
            && matches_key(event, "escape")
        {
            if root.editor.text().trim().is_empty() {
                root.handle_empty_editor_escape();
            } else {
                root.clear_empty_editor_escape();
            }
            return;
        }
        if !matches_key(event, "escape") {
            root.clear_empty_editor_escape();
        }
        if root.selecting_model {
            root.handle_model_selection_input(event);
            return;
        }
        if root.selecting_session {
            root.handle_session_selection_input(event);
            return;
        }
        if root.selecting_settings {
            root.handle_settings_input(event);
            return;
        }
        if root.handle_plugin_keybinding_input(event) {
            return;
        }
        root.editor.handle_input(event);
        if root.editor.text() != before_text {
            root.slash_suggestion_selected = 0;
            root.slash_suggestions_dismissed_for = None;
        }
        if let Some(command) = root.take_scroll_command() {
            let page_rows = root.viewport_height.saturating_sub(2).max(1);
            match command {
                TranscriptScrollCommand::PageUp => root.transcript.scroll_page_up(page_rows),
                TranscriptScrollCommand::PageDown => root.transcript.scroll_page_down(page_rows),
            }
        }
        if let Some(text) = root.take_submitted() {
            if let Some(command) = parse_slash_command(&text) {
                root.handle_slash_command(command);
            } else {
                root.editor.add_to_history(&text);
                root.pending_submit = Some(text);
                root.action = InteractiveAction::Submit;
            }
        }
    }
}
