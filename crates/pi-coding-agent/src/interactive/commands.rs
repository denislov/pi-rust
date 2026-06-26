use std::path::PathBuf;

use pi_agent_core::session::JsonlSessionStorage;
use pi_tui::{KeybindingsManager, TUI_KEYBINDINGS};

use crate::config;
use crate::interactive::app::welcome_line;
use crate::interactive::key_hints::{app_key_hint, key_hint};
use crate::interactive::render::{abbreviate_cwd, format_tokens};
use crate::interactive::root::{InteractiveAction, InteractiveRoot, InteractiveStatus};
use crate::interactive::session_actions::{
    clone_session_to_sibling, export_path_arg, export_transcript as export_session_transcript,
    resolve_command_path, session_choice_from_metadata,
};
use crate::interactive::slash::{ParsedSlashCommand, help_text, parse_model_selector_arg};
use crate::interactive::{Transcript, TranscriptItem};

/// Expand a /skill:name command into its XML skill block.
pub(super) fn expand_skill_command(text: &str, _skills: &[pi_agent_core::Skill]) -> String {
    // Stub — full implementation in Task 4
    text.to_string()
}

/// Expand a /templatename command with arg substitution.
pub(super) fn expand_prompt_template(text: &str, _templates: &[pi_agent_core::PromptTemplate]) -> String {
    // Stub — full implementation in Task 4
    text.to_string()
}

pub(super) fn handle_slash_command(root: &mut InteractiveRoot, command: ParsedSlashCommand) {
    match command.name.as_str() {
        "quit" | "exit" | "q" => match root.status {
            InteractiveStatus::Idle => root.action = InteractiveAction::Exit,
            InteractiveStatus::Running => root.action = InteractiveAction::AbortRunning,
        },
        "help" | "h" | "?" => {
            root.transcript.push(TranscriptItem::system(help_text()));
        }
        "model" => handle_model_command(root, &command.args),
        "resume" => handle_resume_command(root, &command.args),
        "export" => handle_export_command(root, &command.args),
        "import" => handle_import_command(root, &command.args),
        "copy" => handle_copy_command(root),
        "new" => handle_new_command(root),
        "clone" => handle_clone_command(root),
        "reload" => handle_reload_command(root),
        "settings" => handle_settings_command(root),
        "name" => handle_name_command(root, &command.args),
        "session" => handle_session_command(root),
        "hotkeys" => handle_hotkeys_command(root),
        "changelog" => handle_changelog_command(root),
        "login" => handle_login_command(root, &command.args),
        "logout" => handle_logout_command(root, &command.args),
        "fork" => handle_fork_command(root, &command.args),
        "compact" => handle_compact_command(root, &command.args),
        "scoped-models" | "share" | "tree" => handle_pending_slash_command(root, &command),
        _ => {
            root.transcript.push(TranscriptItem::system(format!(
                "unknown command: {} - type /help for available commands",
                command.original
            )));
        }
    }
}

fn handle_pending_slash_command(root: &mut InteractiveRoot, command: &ParsedSlashCommand) {
    root.transcript.push(TranscriptItem::system(format!(
        "/{} is recognized but not implemented in the Rust interactive UI yet.",
        command.name
    )));
}

fn handle_compact_command(root: &mut InteractiveRoot, args: &str) {
    if root.status == InteractiveStatus::Running {
        root.transcript.push(TranscriptItem::system(
            "Wait for the current run to finish before compacting.",
        ));
        return;
    }
    if root.active_session_path.is_none() || root.active_leaf_id.is_none() {
        root.transcript.push(TranscriptItem::system(
            "Nothing to compact (no messages yet)",
        ));
        return;
    }

    let instructions = args.trim();
    root.pending_compact_instructions = if instructions.is_empty() {
        None
    } else {
        Some(instructions.to_string())
    };
    root.action = InteractiveAction::CompactSession;
}

fn handle_export_command(root: &mut InteractiveRoot, args: &str) {
    match export_transcript(root, args) {
        Ok(path) => root.transcript.push(TranscriptItem::system(format!(
            "Session exported to: {}",
            path.display()
        ))),
        Err(error) => root.transcript.push(TranscriptItem::system(format!(
            "Failed to export session: {error}"
        ))),
    }
}

fn handle_import_command(root: &mut InteractiveRoot, args: &str) {
    let Some(input_path) = export_path_arg(args) else {
        root.transcript
            .push(TranscriptItem::system("Usage: /import <path.jsonl>"));
        return;
    };
    let path = resolve_command_path(&root.cwd, &input_path);

    match JsonlSessionStorage::open(&path) {
        Ok(storage) => {
            let leaf_id = storage.get_leaf_id().unwrap_or(None);
            let choice = session_choice_from_metadata(storage.metadata());
            root.session_label = choice.display_name().to_string();
            root.selected_session = Some(choice);
            root.active_session_path = Some(path.clone());
            root.active_leaf_id = leaf_id;
            root.selecting_session = false;
            root.session_selection_selected = 0;
            root.editor.set_text("");
            root.transcript.push(TranscriptItem::system(format!(
                "Session imported from: {}",
                path.display()
            )));
        }
        Err(error) => {
            root.transcript.push(TranscriptItem::system(format!(
                "Failed to import session: {}",
                error.message
            )));
        }
    }
}

fn handle_copy_command(root: &mut InteractiveRoot) {
    let Some(text) = last_assistant_text(root) else {
        root.transcript
            .push(TranscriptItem::system("No agent messages to copy yet."));
        return;
    };

    match root.clipboard.copy_text(&text) {
        Ok(()) => root.transcript.push(TranscriptItem::system(
            "Copied last agent message to clipboard",
        )),
        Err(error) => root.transcript.push(TranscriptItem::system(error)),
    }
}

fn handle_new_command(root: &mut InteractiveRoot) {
    root.transcript = Transcript::new();
    root.transcript
        .push(TranscriptItem::system(welcome_line(&root.keybindings)));
    root.transcript
        .push(TranscriptItem::system("New session started"));
    root.editor.set_text("");
    root.selecting_model = false;
    root.selecting_session = false;
    root.selecting_settings = false;
    root.model_selection_selected = 0;
    root.session_selection_selected = 0;
    root.stats = Default::default();
    root.session_label = "session".to_string();
    root.active_session_path = None;
    root.active_leaf_id = None;
    root.action = InteractiveAction::NewSession;
}

fn handle_clone_command(root: &mut InteractiveRoot) {
    let Some(source_path) = root.active_session_path.clone() else {
        root.transcript
            .push(TranscriptItem::system("Nothing to clone yet"));
        return;
    };
    let Some(leaf_id) = root.active_leaf_id.clone() else {
        root.transcript
            .push(TranscriptItem::system("Nothing to clone yet"));
        return;
    };

    match clone_session_to_sibling(&source_path, &root.cwd, &leaf_id) {
        Ok(storage) => {
            let leaf_id = storage.get_leaf_id().unwrap_or(None);
            let choice = session_choice_from_metadata(storage.metadata());
            root.session_label = choice.display_name().to_string();
            root.selected_session = Some(choice.clone());
            root.active_session_path = Some(choice.path);
            root.active_leaf_id = leaf_id;
            root.editor.set_text("");
            root.transcript
                .push(TranscriptItem::system("Cloned to new session"));
        }
        Err(error) => {
            root.transcript.push(TranscriptItem::system(error));
        }
    }
}

fn handle_fork_command(root: &mut InteractiveRoot, args: &str) {
    let Some(source_path) = root.active_session_path.clone() else {
        root.transcript
            .push(TranscriptItem::system("Nothing to fork yet"));
        return;
    };
    let target_entry_id = if args.is_empty() {
        let Some(leaf_id) = root.active_leaf_id.clone() else {
            root.transcript
                .push(TranscriptItem::system("Nothing to fork yet"));
            return;
        };
        leaf_id
    } else {
        let mut parts = args.split_whitespace();
        let entry_id = parts.next().unwrap_or_default();
        if parts.next().is_some() {
            root.transcript
                .push(TranscriptItem::system("Usage: /fork [entry-id]"));
            return;
        }
        entry_id.to_string()
    };

    match clone_session_to_sibling(&source_path, &root.cwd, &target_entry_id) {
        Ok(storage) => {
            let leaf_id = storage.get_leaf_id().unwrap_or(None);
            let choice = session_choice_from_metadata(storage.metadata());
            root.session_label = choice.display_name().to_string();
            root.selected_session = Some(choice.clone());
            root.active_session_path = Some(choice.path);
            root.active_leaf_id = leaf_id;
            root.editor.set_text("");
            root.transcript
                .push(TranscriptItem::system("Forked to new session"));
        }
        Err(error) => {
            root.transcript.push(TranscriptItem::system(error));
        }
    }
}

fn handle_reload_command(root: &mut InteractiveRoot) {
    root.transcript.push(TranscriptItem::system(
        "Reloading keybindings and resources...",
    ));
    root.action = InteractiveAction::ReloadResources;
}

fn last_assistant_text(root: &InteractiveRoot) -> Option<String> {
    root.transcript.items().iter().rev().find_map(|item| {
        if let TranscriptItem::Assistant { markdown, .. } = item {
            let text = markdown.trim();
            if !text.is_empty() {
                return Some(markdown.clone());
            }
        }
        None
    })
}

fn export_transcript(root: &InteractiveRoot, args: &str) -> Result<PathBuf, String> {
    export_session_transcript(
        &root.cwd,
        &root.session_label,
        &root.model_id,
        root.transcript.items(),
        args,
    )
}

fn handle_settings_command(root: &mut InteractiveRoot) {
    root.selecting_settings = true;
    root.selecting_model = false;
    root.selecting_session = false;
    root.editor.set_text("");
}

fn handle_model_command(root: &mut InteractiveRoot, args: &str) {
    if args.is_empty() {
        root.selecting_model = true;
        root.selecting_settings = false;
        root.selecting_session = false;
        root.model_selection_selected = 0;
        root.editor.set_text("");
        return;
    }

    let (model_id, thinking_level) = match parse_model_selector_arg(args) {
        Ok(parsed) => parsed,
        Err(error) => {
            root.transcript.push(TranscriptItem::system(error));
            return;
        }
    };

    match pi_ai::lookup_model(&model_id) {
        Some(model) => root.set_selected_model_with_thinking(model, thinking_level),
        None => {
            root.transcript
                .push(TranscriptItem::system(format!("Unknown model: {model_id}")));
        }
    }
}

fn handle_resume_command(root: &mut InteractiveRoot, args: &str) {
    if root.session_choices.is_empty() {
        root.transcript.push(TranscriptItem::system(
            "No sessions found for the current workspace.".to_string(),
        ));
        return;
    }

    if !args.is_empty() {
        if let Some(choice) = root
            .session_choices
            .iter()
            .find(|choice| choice.matches_target(args))
            .cloned()
        {
            root.set_selected_session(choice);
        } else {
            root.transcript
                .push(TranscriptItem::system(format!("Unknown session: {args}")));
        }
        return;
    }

    root.selecting_session = true;
    root.selecting_model = false;
    root.selecting_settings = false;
    root.session_selection_selected = 0;
    root.editor.set_text("");
}

fn handle_name_command(root: &mut InteractiveRoot, args: &str) {
    if args.is_empty() {
        root.transcript.push(TranscriptItem::system(format!(
            "Session name: {}",
            root.session_label
        )));
        return;
    }

    root.session_label = args.to_string();
    root.transcript.push(TranscriptItem::system(format!(
        "Session name set: {}",
        root.session_label
    )));
}

fn handle_session_command(root: &mut InteractiveRoot) {
    let cwd = abbreviate_cwd(&root.cwd);
    root.transcript.push(TranscriptItem::system(format!(
        "Session Info\n\nName: {}\nModel: {}\nCwd: {}\nTokens\nInput: {}\nOutput: {}",
        root.session_label,
        root.model_id,
        cwd,
        format_tokens(root.stats.input),
        format_tokens(root.stats.output)
    )));
}

fn handle_hotkeys_command(root: &mut InteractiveRoot) {
    let keybindings = KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default());
    let submit = key_hint(&keybindings, "tui.input.submit", "submit");
    let newline = key_hint(&keybindings, "tui.input.newLine", "newline");
    let interrupt = app_key_hint(&keybindings, "app.interrupt", "interrupt/exit");
    let expand = app_key_hint(&keybindings, "app.tools.expand", "expand tools");
    let page_up = key_hint(&keybindings, "tui.editor.pageUp", "scroll up");
    let page_down = key_hint(&keybindings, "tui.editor.pageDown", "scroll down");
    root.transcript.push(TranscriptItem::system(format!(
        "Hotkeys\n\nNavigation\n- {page_up}\n- {page_down}\n\nEditing\n- {submit}\n- {newline}\n\nApp\n- {interrupt}\n- {expand}"
    )));
}

fn handle_changelog_command(root: &mut InteractiveRoot) {
    root.transcript.push(TranscriptItem::system(
        "Changelog display is not implemented in the Rust interactive UI yet.".to_string(),
    ));
}

fn handle_login_command(root: &mut InteractiveRoot, args: &str) {
    let mut parts = args.split_whitespace();
    let Some(provider) = parts.next() else {
        root.transcript
            .push(TranscriptItem::system("Usage: /login <provider> <api-key>"));
        return;
    };
    let Some(key) = parts.next() else {
        root.transcript
            .push(TranscriptItem::system("Usage: /login <provider> <api-key>"));
        return;
    };
    if parts.next().is_some() {
        root.transcript.push(TranscriptItem::system(
            "Usage: /login <provider> <api-key> (API keys cannot contain whitespace)",
        ));
        return;
    }

    root.auth.set_api_key(provider, key);
    let auth_path = config::resolve_paths(&root.cwd).global_auth();
    match root.auth.save(&auth_path) {
        Ok(()) => {
            root.mark_auth_updated();
            root.transcript.push(TranscriptItem::system(format!(
                "Saved API key for {provider} to {}",
                auth_path.display()
            )));
        }
        Err(error) => {
            root.transcript.push(TranscriptItem::system(format!(
                "Failed to save auth for {provider}: {error}"
            )));
        }
    }
}

fn handle_logout_command(root: &mut InteractiveRoot, args: &str) {
    let mut parts = args.split_whitespace();
    let Some(provider) = parts.next() else {
        root.transcript
            .push(TranscriptItem::system("Usage: /logout <provider>"));
        return;
    };
    if parts.next().is_some() {
        root.transcript
            .push(TranscriptItem::system("Usage: /logout <provider>"));
        return;
    }

    let removed = root.auth.remove_entry(provider);
    let auth_path = config::resolve_paths(&root.cwd).global_auth();
    match root.auth.save(&auth_path) {
        Ok(()) => {
            root.mark_auth_updated();
            if removed {
                root.transcript.push(TranscriptItem::system(format!(
                    "Removed stored auth for {provider}"
                )));
            } else {
                root.transcript.push(TranscriptItem::system(format!(
                    "No stored auth found for {provider}"
                )));
            }
        }
        Err(error) => {
            root.transcript.push(TranscriptItem::system(format!(
                "Failed to save auth after logout for {provider}: {error}"
            )));
        }
    }
}
