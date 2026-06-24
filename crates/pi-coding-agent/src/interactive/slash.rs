use pi_tui::{
    Editor, InputEvent, KeybindingsManager, SYSTEM, USER, color_enabled, fuzzy_filter_indices,
    paint_with,
};

use crate::interactive::render::fit_line;

const MAX_SLASH_SUGGESTIONS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BuiltinSlashCommand {
    pub(super) name: &'static str,
    pub(super) description: &'static str,
}

pub(super) const BUILTIN_SLASH_COMMANDS: &[BuiltinSlashCommand] = &[
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
pub(super) struct ParsedSlashCommand {
    pub(super) name: String,
    pub(super) args: String,
    pub(super) original: String,
}

pub(super) fn parse_slash_command(text: &str) -> Option<ParsedSlashCommand> {
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

pub(super) fn parse_model_selector_arg(
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

pub(super) fn slash_completion_query(text: &str, cursor: usize) -> Option<&str> {
    if cursor != text.len() || !text.starts_with('/') || text.contains('\n') {
        return None;
    }
    let query = &text[1..cursor];
    if query.chars().any(char::is_whitespace) {
        return None;
    }
    Some(query)
}

fn suggestion_indices(
    text: &str,
    cursor: usize,
    dismissed_for: Option<&str>,
) -> Option<Vec<usize>> {
    if dismissed_for.is_some_and(|dismissed| dismissed == text) {
        return None;
    }
    let query = slash_completion_query(text, cursor)?;
    let indices = fuzzy_filter_indices(BUILTIN_SLASH_COMMANDS, query, |command| {
        command.name.to_string()
    });
    (!indices.is_empty()).then_some(indices)
}

pub(super) fn render_suggestions(
    text: &str,
    cursor: usize,
    dismissed_for: Option<&str>,
    selected: &mut usize,
    width: usize,
) -> Vec<String> {
    let Some(indices) = suggestion_indices(text, cursor, dismissed_for) else {
        return Vec::new();
    };
    *selected = (*selected).min(indices.len().saturating_sub(1));

    let color = color_enabled();
    let window_start = (*selected)
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
        let marker = if absolute_index == *selected {
            "->"
        } else {
            "  "
        };
        let line = format!(
            "{marker} {label:<17} {}",
            paint_with(command.description, &SYSTEM, color)
        );
        if absolute_index == *selected {
            lines.push(fit_line(&paint_with(&line, &USER, color), width));
        } else {
            lines.push(fit_line(&line, width));
        }
    }
    lines.push(fit_line(
        &paint_with(
            &format!("({}/{})", *selected + 1, indices.len()),
            &SYSTEM,
            color,
        ),
        width,
    ));
    lines
}

pub(super) fn handle_suggestion_input(
    keybindings: &KeybindingsManager,
    event: &InputEvent,
    editor: &mut Editor,
    selected: &mut usize,
    dismissed_for: &mut Option<String>,
) -> bool {
    let Some(indices) =
        suggestion_indices(editor.text(), editor.cursor(), dismissed_for.as_deref())
    else {
        return false;
    };

    if keybindings.matches(event, "tui.select.up") {
        *selected = (*selected + indices.len() - 1) % indices.len();
        return true;
    }
    if keybindings.matches(event, "tui.select.down") {
        *selected = (*selected + 1) % indices.len();
        return true;
    }
    if keybindings.matches(event, "tui.select.pageUp") {
        *selected = selected.saturating_sub(MAX_SLASH_SUGGESTIONS);
        return true;
    }
    if keybindings.matches(event, "tui.select.pageDown") {
        *selected = (*selected + MAX_SLASH_SUGGESTIONS).min(indices.len().saturating_sub(1));
        return true;
    }
    let exact_query_matches_command = slash_completion_query(editor.text(), editor.cursor())
        .is_some_and(|query| {
            indices
                .iter()
                .any(|index| BUILTIN_SLASH_COMMANDS[*index].name == query)
        });
    if keybindings.matches(event, "tui.select.confirm") && exact_query_matches_command {
        return false;
    }
    if keybindings.matches(event, "tui.select.confirm")
        || keybindings.matches(event, "tui.input.tab")
    {
        let command_index = indices[(*selected).min(indices.len() - 1)];
        let command = &BUILTIN_SLASH_COMMANDS[command_index];
        editor.set_text(format!("/{} ", command.name));
        *selected = 0;
        *dismissed_for = None;
        return true;
    }
    if keybindings.matches(event, "tui.select.cancel") {
        *dismissed_for = Some(editor.text().to_string());
        return true;
    }

    false
}

pub(super) fn help_text() -> String {
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
