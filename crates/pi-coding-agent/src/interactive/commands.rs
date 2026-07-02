use std::path::PathBuf;

use pi_agent_core::resources::{parse_command_args, substitute_args};
use pi_agent_core::{PromptTemplate, Skill};
use pi_tui::{KeybindingsManager, TUI_KEYBINDINGS};

use crate::config;
use crate::interactive::app::welcome_line;
use crate::interactive::key_hints::{app_key_hint, key_hint};
use crate::interactive::render::{abbreviate_cwd, format_tokens};
use crate::interactive::root::{
    InteractiveAction, InteractiveRoot, InteractiveStatus, PendingBranchSummaryRequest,
    PendingPluginCommandRequest, PendingPluginUiDialog, PluginUiDialogField,
};
use crate::interactive::session_actions::{
    SessionChoiceKind, clone_rust_native_choice, export_rust_native_choice,
    export_transcript as export_session_transcript, fork_rust_native_choice,
    rust_native_tree_for_choice,
};
use crate::interactive::slash::{ParsedSlashCommand, help_text, parse_model_selector_arg};
use crate::interactive::{Transcript, TranscriptItem};

/// Expand a /skill:name command into its XML skill block.
///
/// Mirrors TS `_expandSkillCommand` in `agent-session.ts`.
pub(super) fn expand_skill_command(text: &str, skills: &[Skill]) -> String {
    if !text.starts_with("/skill:") {
        return text.to_string();
    }

    let space_index = text.find(' ');
    let skill_name = match space_index {
        Some(i) => &text[7..i],
        None => &text[7..],
    };
    let args = match space_index {
        Some(i) => text[i + 1..].trim().to_string(),
        None => String::new(),
    };

    let Some(skill) = skills.iter().find(|s| s.name == skill_name) else {
        return text.to_string();
    };

    let skill_block = pi_agent_core::resources::format_skill_invocation(
        &skill.name,
        &skill.location,
        &skill.content,
        if args.is_empty() { None } else { Some(&args) },
    );
    skill_block
}

/// Expand a /templatename command with arg substitution.
///
/// Mirrors TS `expandPromptTemplate` in `prompt-templates.ts`.
pub(super) fn expand_prompt_template(text: &str, templates: &[PromptTemplate]) -> String {
    if !text.starts_with('/') {
        return text.to_string();
    }

    // Match /name followed by optional args (may include newlines)
    let Some(rest) = text.strip_prefix('/') else {
        return text.to_string();
    };
    let space_index = rest.find(|c: char| c.is_whitespace());
    let template_name = match space_index {
        Some(i) => &rest[..i],
        None => rest,
    };
    let args_string = match space_index {
        Some(i) => rest[i + 1..].to_string(),
        None => String::new(),
    };

    let Some(template) = templates.iter().find(|t| t.name == template_name) else {
        return text.to_string();
    };

    let args = parse_command_args(&args_string);
    substitute_args(&template.content, &args)
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
        "branch-summary" => handle_branch_summary_command(root, &command.args),
        "plugin-command" => handle_plugin_command(root, &command.args),
        "tree" => handle_tree_command(root),
        "scoped-models" | "share" => handle_pending_slash_command(root, &command),
        _ if root.has_plugin_command(&command.name) => {
            handle_plugin_slash_command(root, &command.name, &command.args)
        }
        _ => {
            let expanded = root.expand_prompt_text(&command.original);
            if expanded != command.original {
                root.editor.add_to_history(&expanded);
                root.pending_submit = Some(expanded);
                root.action = InteractiveAction::Submit;
            } else {
                root.transcript.push(TranscriptItem::system(format!(
                    "unknown command: {} - type /help for available commands",
                    command.original
                )));
            }
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
    let active_rust_native = matches!(
        root.active_session.as_ref().map(|choice| choice.kind),
        Some(SessionChoiceKind::RustNative)
    );
    if !active_rust_native {
        root.transcript.push(TranscriptItem::system(
            "Nothing to compact (no active Rust-native session)",
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

fn handle_branch_summary_command(root: &mut InteractiveRoot, args: &str) {
    if root.status == InteractiveStatus::Running {
        root.transcript.push(TranscriptItem::system(
            "Wait for the current run to finish before summarizing a branch.",
        ));
        return;
    }
    let active_rust_native = matches!(
        root.active_session.as_ref().map(|choice| choice.kind),
        Some(SessionChoiceKind::RustNative)
    );
    if !active_rust_native {
        root.transcript.push(TranscriptItem::system(
            "Nothing to summarize (no active Rust-native session)",
        ));
        return;
    }

    let mut parts = args.split_whitespace();
    let Some(source_leaf_id) = parts.next() else {
        root.transcript.push(TranscriptItem::system(
            "Usage: /branch-summary <source-leaf-id> <target-leaf-id> [instructions]",
        ));
        return;
    };
    let Some(target_leaf_id) = parts.next() else {
        root.transcript.push(TranscriptItem::system(
            "Usage: /branch-summary <source-leaf-id> <target-leaf-id> [instructions]",
        ));
        return;
    };
    let custom_instructions = {
        let instructions = parts.collect::<Vec<_>>().join(" ");
        if instructions.is_empty() {
            None
        } else {
            Some(instructions)
        }
    };
    root.pending_branch_summary_request = Some(PendingBranchSummaryRequest {
        source_leaf_id: source_leaf_id.to_owned(),
        target_leaf_id: target_leaf_id.to_owned(),
        custom_instructions,
    });
    root.action = InteractiveAction::BranchSummary;
}

fn handle_plugin_command(root: &mut InteractiveRoot, args: &str) {
    let mut parts = args.splitn(2, char::is_whitespace);
    let command_id = parts.next().unwrap_or_default().trim();
    if command_id.is_empty() {
        root.transcript.push(TranscriptItem::system(
            "Usage: /plugin-command <command-id> [json-args]",
        ));
        return;
    }
    let raw_args = parts.next().unwrap_or_default().trim();
    queue_plugin_command(root, command_id, raw_args);
}

fn handle_plugin_slash_command(root: &mut InteractiveRoot, command_id: &str, args: &str) {
    queue_plugin_command(root, command_id, args.trim());
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PluginDialogValidationError {
    message: String,
    field_id: Option<String>,
}

impl PluginDialogValidationError {
    fn dialog(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            field_id: None,
        }
    }

    fn field(field: &PluginUiDialogField, message: String) -> Self {
        Self {
            message,
            field_id: Some(field.id.clone()),
        }
    }
}

pub(super) fn queue_plugin_command(root: &mut InteractiveRoot, command_id: &str, raw_args: &str) {
    if root.status == InteractiveStatus::Running {
        root.transcript.push(TranscriptItem::system(
            "Wait for the current run to finish before running a plugin command.",
        ));
        return;
    }

    let parsed_args = if raw_args.is_empty() {
        serde_json::json!({})
    } else {
        match serde_json::from_str(raw_args) {
            Ok(args) => args,
            Err(error) => {
                root.transcript.push(TranscriptItem::system(format!(
                    "Invalid plugin command args: {error}"
                )));
                return;
            }
        }
    };

    if let Some(active_dialog) = root.active_plugin_ui_dialog.clone() {
        let dialog = active_dialog.dialog;
        if dialog.action_id == command_id {
            if let Err(error) = validate_plugin_dialog_args(&dialog, &parsed_args) {
                if let Some(field_id) = error.field_id.as_deref() {
                    root.set_active_plugin_dialog_field_error(field_id, error.message.clone());
                }
                root.transcript.push(TranscriptItem::system(error.message));
                return;
            }
            root.active_plugin_ui_dialog = None;
        } else {
            root.active_plugin_ui_dialog = None;
        }
    }

    root.pending_plugin_command_request = Some(PendingPluginCommandRequest {
        command_id: command_id.to_string(),
        args: parsed_args,
    });
    root.action = InteractiveAction::PluginCommand;
}

fn validate_plugin_dialog_args(
    dialog: &PendingPluginUiDialog,
    args: &serde_json::Value,
) -> Result<(), PluginDialogValidationError> {
    let Some(object) = args.as_object() else {
        return Err(PluginDialogValidationError::dialog(
            "Plugin dialog args must be a JSON object",
        ));
    };
    for field in &dialog.fields {
        let value = object.get(&field.id).unwrap_or(&serde_json::Value::Null);
        validate_plugin_dialog_field(field, value)
            .map_err(|message| PluginDialogValidationError::field(field, message))?;
    }
    Ok(())
}

fn validate_plugin_dialog_field(
    field: &PluginUiDialogField,
    value: &serde_json::Value,
) -> Result<(), String> {
    if field.required && dialog_field_value_missing(value) {
        return Err(format!("Plugin dialog field {} is required", field.label));
    }
    if value.is_null() || dialog_field_value_missing(value) {
        return Ok(());
    }
    let kind = normalized_dialog_field_kind(&field.kind);
    let valid_type = match kind.as_str() {
        "text" | "string" => value.is_string(),
        "boolean" | "bool" => value.is_boolean(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        _ => true,
    };
    if valid_type {
        Ok(())
    } else {
        Err(format!(
            "Plugin dialog field {} must be {}",
            field.label, field.kind
        ))
    }
}

fn dialog_field_value_missing(value: &serde_json::Value) -> bool {
    matches!(value, serde_json::Value::Null)
        || value.as_str().is_some_and(|value| value.trim().is_empty())
}

fn normalized_dialog_field_kind(kind: &str) -> String {
    kind.trim().replace('-', "_").to_ascii_lowercase()
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
    let _ = args;
    root.transcript.push(TranscriptItem::system(
        "JSONL session import is no longer supported.".to_string(),
    ));
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
    root.clear_active_session();
    root.action = InteractiveAction::NewSession;
}

fn handle_clone_command(root: &mut InteractiveRoot) {
    if let Some(choice) = root
        .active_session
        .as_ref()
        .filter(|choice| choice.kind == SessionChoiceKind::RustNative)
    {
        match clone_rust_native_choice(choice) {
            Ok(hydrated) => {
                root.apply_hydrated_session(hydrated, Some("Cloned to new session".into()));
            }
            Err(error) => root.transcript.push(TranscriptItem::system(format!(
                "Failed to clone session: {error}"
            ))),
        }
        return;
    }

    root.transcript
        .push(TranscriptItem::system("Nothing to clone yet"));
}

fn handle_fork_command(root: &mut InteractiveRoot, args: &str) {
    if let Some(choice) = root
        .active_session
        .as_ref()
        .filter(|choice| choice.kind == SessionChoiceKind::RustNative)
    {
        let target_leaf_id = if args.is_empty() {
            None
        } else {
            let mut parts = args.split_whitespace();
            let leaf_id = parts.next().unwrap_or_default();
            if parts.next().is_some() {
                root.transcript
                    .push(TranscriptItem::system("Usage: /fork [leaf-id]"));
                return;
            }
            Some(leaf_id)
        };
        match fork_rust_native_choice(choice, target_leaf_id) {
            Ok(hydrated) => {
                root.apply_hydrated_session(hydrated, Some("Forked to new session".into()));
            }
            Err(error) => root.transcript.push(TranscriptItem::system(format!(
                "Failed to fork session: {error}"
            ))),
        }
        return;
    }

    root.transcript
        .push(TranscriptItem::system("Nothing to fork yet"));
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

fn handle_tree_command(root: &mut InteractiveRoot) {
    if root.status == InteractiveStatus::Running {
        root.transcript.push(TranscriptItem::system(
            "Wait for the current run to finish before navigating the session tree.",
        ));
        return;
    }

    let Some(choice) = root
        .active_session
        .as_ref()
        .filter(|choice| choice.kind == SessionChoiceKind::RustNative)
    else {
        root.transcript
            .push(TranscriptItem::system("No entries in session"));
        return;
    };

    match rust_native_tree_for_choice(choice) {
        Ok((tree, leaf_id)) => {
            if tree.is_empty() {
                root.transcript
                    .push(TranscriptItem::system("No entries in session"));
                return;
            }
            let filter_mode =
                pi_agent_core::session::TreeFilterMode::from_str(&root.settings.tree_filter_mode);
            let selector = crate::interactive::tree_selector::TreeSelectorState::new(
                tree,
                leaf_id,
                filter_mode,
                root.viewport_width,
            );
            root.selecting_tree = true;
            root.tree_selector = Some(selector);
            root.selected_tree_entry_id = None;
            root.editor.set_text("");
        }
        Err(error) => root.transcript.push(TranscriptItem::system(format!(
            "Failed to open session: {error}"
        ))),
    }
}

fn export_transcript(root: &InteractiveRoot, args: &str) -> Result<PathBuf, String> {
    if let Some(choice) = root
        .active_session
        .as_ref()
        .filter(|choice| choice.kind == SessionChoiceKind::RustNative)
    {
        return export_rust_native_choice(choice, &root.cwd, args);
    }

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
    let mut details = format!(
        "Session Info\n\nName: {}\nModel: {}\nCwd: {}\nTokens\nInput: {}\nOutput: {}",
        root.session_label,
        root.model_id,
        cwd,
        format_tokens(root.stats.input),
        format_tokens(root.stats.output)
    );
    if let Some(choice) = &root.active_session {
        details.push_str(&format!(
            "\nStorage: rust-native\nSession ID: {}\nEntries: {}\nPath: {}",
            choice.id,
            choice.entry_count,
            choice.path.display()
        ));
        if let Some(leaf_id) = root.active_leaf_id.as_deref() {
            details.push_str(&format!("\nActive leaf: {leaf_id}"));
        }
    }
    root.transcript.push(TranscriptItem::system(details));
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
