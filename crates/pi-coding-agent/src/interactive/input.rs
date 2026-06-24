use std::io::Read;

use pi_tui::{Component, InputEvent, matches_key};

use crate::interactive::root::{InteractiveAction, InteractiveRoot, InteractiveStatus};
use crate::interactive::slash::parse_slash_command;
use crate::interactive::{TranscriptItem, root::TranscriptScrollCommand};

pub(super) struct InputPump {
    rx: tokio::sync::mpsc::UnboundedReceiver<String>,
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
            _reader: Some(reader),
        }
    }

    pub(super) fn from_chunks(chunks: Vec<String>) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        for chunk in chunks {
            let _ = tx.send(chunk);
        }
        drop(tx);
        Self { rx, _reader: None }
    }

    pub(super) fn from_receiver(rx: tokio::sync::mpsc::UnboundedReceiver<String>) -> Self {
        Self { rx, _reader: None }
    }

    pub(super) async fn recv(&mut self) -> Option<String> {
        self.rx.recv().await
    }
}

pub(super) fn handle_root_input(root: &mut InteractiveRoot, event: &InputEvent) {
    if matches_key(event, "ctrl+c") {
        match root.status {
            InteractiveStatus::Running => {
                root.action = InteractiveAction::AbortRunning;
                return;
            }
            InteractiveStatus::Idle => {
                if root.editor.text().is_empty() {
                    root.action = InteractiveAction::Exit;
                } else {
                    root.editor.set_text("");
                }
                return;
            }
        }
    }

    if matches_key(event, "ctrl+o") {
        root.tool_output_expanded = !root.tool_output_expanded;
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
        if root.selecting_model {
            root.handle_model_selection_input(event);
            return;
        }
        if root.selecting_session {
            root.handle_session_selection_input(event);
            return;
        }
        if root.selecting_settings {
            return;
        }
        let before_text = root.editor.text().to_string();
        if root.handle_slash_suggestion_input(event) {
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
