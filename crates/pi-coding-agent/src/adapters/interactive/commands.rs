use std::path::PathBuf;

use pi_agent_core::api::resources::{PromptTemplate, Skill, parse_command_args, substitute_args};
use pi_tui::api::input::KeybindingsManager;

use crate::adapters::interactive::app::welcome_line;
use crate::adapters::interactive::key_hints::{app_key_hint, key_hint};
use crate::adapters::interactive::keybindings;
use crate::adapters::interactive::render::{abbreviate_cwd, format_tokens};
use crate::adapters::interactive::root::{
    InteractiveAction, InteractiveRoot, InteractiveStatus, PendingAgentInvocationRequest,
    PendingAgentTeamRequest, PendingBranchSummaryRequest, PendingDelegationConfirmationCommand,
    PendingDelegationConfirmationSelection, PendingForkRequest, PendingInteractiveCommand,
    PendingSelfHealingEditModelRepair, PendingSelfHealingEditRequest,
};
use crate::adapters::interactive::session_actions::{
    SessionChoiceKind, clone_rust_native_choice, export_rust_native_choice,
    export_transcript as export_session_transcript, rust_native_tree_for_choice,
};
use crate::adapters::interactive::slash::{
    ParsedSlashCommand, help_text, parse_model_selector_arg,
};
use crate::adapters::interactive::{Transcript, TranscriptItem};
use crate::app::configuration::{remove_provider_auth, save_provider_api_key};
use crate::runtime::facade::SelfHealingEditReplacement;

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

    pi_agent_core::api::resources::format_skill_invocation(
        &skill.name,
        &skill.location,
        &skill.content,
        if args.is_empty() { None } else { Some(&args) },
    )
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
        "agents" => handle_agents_command(root),
        "agent" => handle_agent_command(root, &command.args),
        _ if command.name.starts_with("agent:") => {
            handle_agent_colon_command(root, &command.name, &command.args)
        }
        "teams" => handle_teams_command(root),
        "team" => handle_team_command(root, &command.args),
        _ if command.name.starts_with("team:") => {
            handle_team_colon_command(root, &command.name, &command.args)
        }
        "delegations" => handle_delegations_command(root, &command.args),
        "delegation" => handle_delegation_command(root, &command.args),
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
        "self-healing-edit" => handle_self_healing_edit_command(root, &command.args),
        "tree" => handle_tree_command(root),
        "scoped-models" | "share" => handle_pending_slash_command(root, &command),
        _ => {
            let expanded = root.expand_prompt_text(&command.original);
            if expanded != command.original {
                root.local.editor.add_to_history(&expanded);
                root.queue_command(PendingInteractiveCommand::Submit(expanded));
            } else {
                root.transcript.push(TranscriptItem::system(format!(
                    "unknown command: {} - type /help for available commands",
                    command.original
                )));
            }
        }
    }
}

fn handle_agents_command(root: &mut InteractiveRoot) {
    let mut lines = vec!["Agent profiles:".to_string()];
    for profile in root.profile_registry.agents() {
        let current = if profile.id == *root.display_default_agent_profile_id() {
            " (current)"
        } else {
            ""
        };
        let description = profile
            .description
            .as_deref()
            .unwrap_or(profile.display_name.as_str());
        lines.push(format!("  {}{current} - {description}", profile.id));
    }
    if root.profile_registry.agents().next().is_none() {
        lines.push("  (none)".into());
    }
    root.transcript
        .push(TranscriptItem::system(lines.join("\n")));
}

fn handle_teams_command(root: &mut InteractiveRoot) {
    let mut lines = vec!["Team profiles:".to_string()];
    for profile in root.profile_registry.teams() {
        lines.push(format!(
            "  {} - {} ({:?})",
            profile.id, profile.display_name, profile.supervisor
        ));
    }
    if root.profile_registry.teams().next().is_none() {
        lines.push("  (none)".into());
    }
    root.transcript
        .push(TranscriptItem::system(lines.join("\n")));
}

fn handle_agent_command(root: &mut InteractiveRoot, args: &str) {
    let args = args.trim();
    if args.is_empty() {
        root.open_agent_menu();
        return;
    }
    root.transcript
        .push(TranscriptItem::system(agent_usage_text()));
}

fn handle_agent_colon_command(root: &mut InteractiveRoot, command_name: &str, args: &str) {
    let Some(profile_id) = command_name.strip_prefix("agent:") else {
        root.transcript
            .push(TranscriptItem::system(agent_usage_text()));
        return;
    };
    let task = args.trim();
    if profile_id.is_empty() || task.is_empty() {
        root.transcript
            .push(TranscriptItem::system(agent_usage_text()));
        return;
    }
    let Some(profile) = root.profile_registry.agent(profile_id) else {
        root.transcript.push(TranscriptItem::system(format!(
            "Unknown agent profile: {profile_id}"
        )));
        return;
    };
    root.queue_command(PendingInteractiveCommand::AgentInvocation(
        PendingAgentInvocationRequest {
            profile_id: profile.id.clone(),
            task: task.to_string(),
        },
    ));
}

fn handle_team_command(root: &mut InteractiveRoot, args: &str) {
    let args = args.trim();
    if args.is_empty() {
        root.open_team_menu();
        return;
    }
    root.transcript
        .push(TranscriptItem::system(team_usage_text()));
}

fn handle_team_colon_command(root: &mut InteractiveRoot, command_name: &str, args: &str) {
    let Some(team_id) = command_name.strip_prefix("team:") else {
        root.transcript
            .push(TranscriptItem::system(team_usage_text()));
        return;
    };
    let task = args.trim();
    if team_id.is_empty() || task.is_empty() {
        root.transcript
            .push(TranscriptItem::system(team_usage_text()));
        return;
    }
    if root.profile_registry.team(team_id).is_none() {
        root.transcript.push(TranscriptItem::system(format!(
            "Unknown team profile: {team_id}"
        )));
        return;
    }
    root.queue_command(PendingInteractiveCommand::AgentTeam(
        PendingAgentTeamRequest {
            team_id: team_id.into(),
            task: task.to_string(),
        },
    ));
}

fn agent_usage_text() -> &'static str {
    "Usage: /agent or /agent:<agent-id> <task>"
}

fn team_usage_text() -> &'static str {
    "Usage: /team or /team:<team-id> <task>"
}

fn handle_delegations_command(root: &mut InteractiveRoot, args: &str) {
    if root.status == InteractiveStatus::Running {
        root.transcript.push(TranscriptItem::system(
            "Wait for the current run to finish before listing delegation confirmations.",
        ));
        return;
    }
    if !args.trim().is_empty() {
        root.transcript
            .push(TranscriptItem::system("Usage: /delegations"));
        return;
    }
    root.pending_delegation_confirmation_command = Some(PendingDelegationConfirmationCommand::List);
    root.action = InteractiveAction::DelegationConfirmation;
}

fn handle_delegation_command(root: &mut InteractiveRoot, args: &str) {
    if root.status == InteractiveStatus::Running {
        root.transcript.push(TranscriptItem::system(
            "Wait for the current run to finish before handling delegation confirmations.",
        ));
        return;
    }

    let args = args.trim();
    if args.is_empty() || args == "list" {
        root.pending_delegation_confirmation_command =
            Some(PendingDelegationConfirmationCommand::List);
        root.action = InteractiveAction::DelegationConfirmation;
        return;
    }

    let mut parts = args.splitn(2, char::is_whitespace);
    let verb = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or_default().trim();
    match verb {
        "approve" => match parse_delegation_selection(rest) {
            Ok(selection) => {
                root.pending_delegation_confirmation_command =
                    Some(PendingDelegationConfirmationCommand::Approve { selection });
                root.action = InteractiveAction::DelegationConfirmation;
            }
            Err(usage) => root.transcript.push(TranscriptItem::system(usage)),
        },
        "reject" => match parse_delegation_rejection(rest) {
            Ok((selection, reason)) => {
                root.pending_delegation_confirmation_command =
                    Some(PendingDelegationConfirmationCommand::Reject { selection, reason });
                root.action = InteractiveAction::DelegationConfirmation;
            }
            Err(usage) => root.transcript.push(TranscriptItem::system(usage)),
        },
        _ => root.transcript.push(TranscriptItem::system(
            "Usage: /delegation list | approve <tool-call-id> | approve <operation-id> <tool-call-id> | reject <tool-call-id> [reason]",
        )),
    }
}

fn parse_delegation_selection(
    args: &str,
) -> Result<PendingDelegationConfirmationSelection, String> {
    let mut parts = args.split_whitespace();
    let Some(first) = parts.next() else {
        return Err(
            "Usage: /delegation approve <tool-call-id> or /delegation approve <operation-id> <tool-call-id>"
                .to_string(),
        );
    };
    let second = parts.next();
    if parts.next().is_some() {
        return Err(
            "Usage: /delegation approve <tool-call-id> or /delegation approve <operation-id> <tool-call-id>"
                .to_string(),
        );
    }
    Ok(match second {
        Some(tool_call_id) => PendingDelegationConfirmationSelection {
            operation_id: Some(first.to_string()),
            tool_call_id: tool_call_id.to_string(),
        },
        None => PendingDelegationConfirmationSelection {
            operation_id: None,
            tool_call_id: first.to_string(),
        },
    })
}

fn parse_delegation_rejection(
    args: &str,
) -> Result<(PendingDelegationConfirmationSelection, Option<String>), String> {
    let mut parts = args.split_whitespace();
    let Some(first) = parts.next() else {
        return Err(
            "Usage: /delegation reject <tool-call-id> [reason] or /delegation reject <operation-id> <tool-call-id> [reason]"
                .to_string(),
        );
    };
    let Some(second) = parts.next() else {
        return Ok((
            PendingDelegationConfirmationSelection {
                operation_id: None,
                tool_call_id: first.to_string(),
            },
            None,
        ));
    };
    let rest = parts.collect::<Vec<_>>().join(" ");
    if first.starts_with("op_") {
        let reason = (!rest.is_empty()).then_some(rest);
        return Ok((
            PendingDelegationConfirmationSelection {
                operation_id: Some(first.to_string()),
                tool_call_id: second.to_string(),
            },
            reason,
        ));
    }

    let reason = std::iter::once(second)
        .chain(rest.split_whitespace())
        .collect::<Vec<_>>()
        .join(" ");
    Ok((
        PendingDelegationConfirmationSelection {
            operation_id: None,
            tool_call_id: first.to_string(),
        },
        (!reason.is_empty()).then_some(reason),
    ))
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
    root.queue_command(PendingInteractiveCommand::Compact {
        instructions: (!instructions.is_empty()).then(|| instructions.to_string()),
    });
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
    root.queue_command(PendingInteractiveCommand::BranchSummary(
        PendingBranchSummaryRequest {
            source_leaf_id: source_leaf_id.to_owned(),
            target_leaf_id: target_leaf_id.to_owned(),
            custom_instructions,
        },
    ));
}

fn handle_self_healing_edit_command(root: &mut InteractiveRoot, args: &str) {
    if root.status == InteractiveStatus::Running {
        root.transcript.push(TranscriptItem::system(
            "Wait for the current run to finish before applying a self-healing edit.",
        ));
        return;
    }

    match parse_self_healing_edit_args(args) {
        Ok(request) => {
            root.queue_command(PendingInteractiveCommand::SelfHealingEdit(request));
        }
        Err(usage) => root.transcript.push(TranscriptItem::system(usage)),
    }
}

fn parse_self_healing_edit_args(args: &str) -> Result<PendingSelfHealingEditRequest, String> {
    let usage = "Usage: /self-healing-edit <path> <oldText> => <newText> [--model-repair] [--model-repair-attempts N] [--check <command>]";
    let args = args.trim();
    let mut parts = args.splitn(2, char::is_whitespace);
    let path = parts.next().unwrap_or_default().trim();
    let rest = parts.next().unwrap_or_default().trim();
    if path.is_empty() || rest.is_empty() {
        return Err(usage.to_string());
    }
    let Some((old_text, new_text)) = rest.split_once("=>") else {
        return Err(usage.to_string());
    };
    let old_text = old_text.trim();
    let (new_text, model_repair_after_check) =
        parse_self_healing_edit_model_repair_suffix(new_text.trim(), usage)?;
    let (new_text, check_command) = parse_self_healing_edit_check_suffix(&new_text, usage)?;
    let (new_text, model_repair_before_check) =
        parse_self_healing_edit_model_repair_suffix(&new_text, usage)?;
    let model_repair = model_repair_after_check.or(model_repair_before_check);
    if old_text.is_empty() || new_text.is_empty() {
        return Err(usage.to_string());
    }
    Ok(PendingSelfHealingEditRequest {
        path: path.to_string(),
        replacements: vec![SelfHealingEditReplacement::new(old_text, new_text)],
        check_command,
        model_repair,
    })
}

fn parse_self_healing_edit_check_suffix(
    new_text: &str,
    usage: &str,
) -> Result<(String, Option<String>), String> {
    if let Some((text, command)) = new_text.rsplit_once(" --check ") {
        let text = text.trim();
        let command = command.trim();
        if text.is_empty() || command.is_empty() {
            return Err(usage.to_string());
        }
        return Ok((text.to_string(), Some(command.to_string())));
    }
    if new_text == "--check" || new_text.ends_with(" --check") {
        return Err(usage.to_string());
    }
    Ok((new_text.to_string(), None))
}

fn parse_self_healing_edit_model_repair_suffix(
    value: &str,
    usage: &str,
) -> Result<(String, Option<PendingSelfHealingEditModelRepair>), String> {
    let mut value = value.trim().to_string();
    let mut max_attempts = None;
    let mut enabled = false;
    loop {
        if let Some(before) = value.strip_suffix(" --model-repair") {
            enabled = true;
            value = before.trim_end().to_string();
            continue;
        }
        if let Some((before, attempts)) = value.rsplit_once(" --model-repair-attempts ") {
            let attempts = attempts.trim();
            if attempts.is_empty() {
                return Err(usage.to_string());
            }
            if attempts.chars().any(char::is_whitespace) {
                break;
            }
            let attempts = attempts
                .parse::<usize>()
                .ok()
                .filter(|attempts| *attempts > 0)
                .ok_or_else(|| usage.to_string())?;
            enabled = true;
            max_attempts = Some(attempts);
            value = before.trim_end().to_string();
            continue;
        }
        break;
    }
    if value == "--model-repair-attempts"
        || value.ends_with(" --model-repair-attempts")
        || value.ends_with(" --model-repair-attempts ")
    {
        return Err(usage.to_string());
    }
    let policy = enabled.then_some(PendingSelfHealingEditModelRepair {
        max_attempts: max_attempts.unwrap_or(1),
    });
    Ok((value, policy))
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
    root.transcript.push(TranscriptItem::system(welcome_line(
        &root.local.keybindings,
    )));
    root.transcript
        .push(TranscriptItem::system("New session started"));
    root.local.editor.set_text("");
    root.local.selecting_model = false;
    root.local.selecting_session = false;
    root.local.selecting_settings = false;
    root.local.model_selection_selected = 0;
    root.local.session_selection_selected = 0;
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
    if root.status == InteractiveStatus::Running {
        root.transcript.push(TranscriptItem::system(
            "Wait for the current run to finish before forking.",
        ));
        return;
    }
    if !matches!(
        root.active_session.as_ref().map(|choice| choice.kind),
        Some(SessionChoiceKind::RustNative)
    ) {
        root.transcript
            .push(TranscriptItem::system("Nothing to fork yet"));
        return;
    }

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
        Some(leaf_id.to_owned())
    };
    root.queue_command(PendingInteractiveCommand::Fork(PendingForkRequest {
        target_leaf_id,
    }));
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
                crate::adapters::interactive::tree_selector::TreeFilterMode::from_str_name(
                    &root.settings.tree_filter_mode,
                );
            let selector = crate::adapters::interactive::tree_selector::TreeSelectorState::new(
                tree,
                leaf_id,
                filter_mode,
                root.viewport_width,
            );
            root.local.selecting_tree = true;
            root.local.tree_selector = Some(selector);
            root.local.selected_tree_entry_id = None;
            root.local.editor.set_text("");
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
    root.local.selecting_settings = true;
    root.local.selecting_model = false;
    root.local.selecting_session = false;
    root.local.editor.set_text("");
}

fn handle_model_command(root: &mut InteractiveRoot, args: &str) {
    if args.is_empty() {
        root.local.selecting_model = true;
        root.local.selecting_settings = false;
        root.local.selecting_session = false;
        root.local.model_selection_selected = 0;
        root.local.editor.set_text("");
        return;
    }

    let (model_id, thinking_level) = match parse_model_selector_arg(args) {
        Ok(parsed) => parsed,
        Err(error) => {
            root.transcript.push(TranscriptItem::system(error));
            return;
        }
    };

    match pi_ai::api::model::lookup_model(&model_id) {
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

    root.local.selecting_session = true;
    root.local.selecting_model = false;
    root.local.selecting_settings = false;
    root.local.session_selection_selected = 0;
    root.local.editor.set_text("");
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
    let keybindings =
        KeybindingsManager::new(keybindings::default_keybindings(), Default::default());
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

    match save_provider_api_key(&mut root.auth, &root.cwd, provider, key) {
        Ok(auth_path) => {
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

    match remove_provider_auth(&mut root.auth, &root.cwd, provider) {
        Ok(removed) => {
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
