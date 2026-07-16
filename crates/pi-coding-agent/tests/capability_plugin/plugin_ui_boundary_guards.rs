use std::fs;
use std::path::{Path, PathBuf};

const INTERACTIVE_INPUT: &str = include_str!("../../src/adapters/interactive/input.rs");
const INTERACTIVE_COMMANDS: &str = include_str!("../../src/adapters/interactive/commands.rs");
const INTERACTIVE_ROOT: &str = include_str!("../../src/adapters/interactive/root.rs");
const PROMPT_TASK: &str = include_str!("../../src/adapters/interactive/prompt_task.rs");

#[test]
fn plugin_ui_routes_through_interactive_adapter_state() {
    for required in [
        "plugin_ui_actions(&session)",
        "plugin_keybindings(&session)",
        "plugin_ui_dialogs(&session)",
    ] {
        assert!(
            PROMPT_TASK.contains(required),
            "prompt task must map plugin UI capabilities from CodingAgentSession through `{required}`"
        );
    }

    for required in [
        "root.handle_plugin_keybinding_input(event)",
        "root.handle_plugin_dialog_form_input(event)",
    ] {
        assert!(
            INTERACTIVE_INPUT.contains(required),
            "interactive input must route plugin UI handling through InteractiveRoot::{required}"
        );
    }

    for required in ["queue_plugin_command(root", "validate_plugin_dialog_args"] {
        assert!(
            INTERACTIVE_COMMANDS.contains(required),
            "interactive commands must keep plugin command/dialog routing in the adapter via `{required}`"
        );
    }

    for required in [
        "pending_plugin_command_request",
        "pending_plugin_ui_action",
        "pending_plugin_ui_dialog",
        "active_plugin_ui_dialog",
        "plugin_ui_actions",
        "plugin_keybindings",
        "plugin_ui_dialogs",
    ] {
        assert!(
            INTERACTIVE_ROOT.contains(required),
            "InteractiveRoot must own plugin UI adapter state `{required}`"
        );
    }
}

#[test]
fn plugin_ui_routing_does_not_live_in_pi_tui() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/pi-coding-agent")
        .to_path_buf();
    let mut violations = Vec::new();

    collect_source_violations(
        &repo_root,
        &repo_root.join("crates/pi-tui/src"),
        &mut violations,
    );

    assert!(
        violations.is_empty(),
        "plugin UI routing/state must remain in pi-coding-agent interactive adapters, not pi-tui:\n{}",
        violations.join("\n")
    );
}

fn collect_source_violations(repo_root: &Path, path: &Path, violations: &mut Vec<String>) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };

    if metadata.is_dir() {
        let mut entries = fs::read_dir(path)
            .expect("read source directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read source entries");
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect_source_violations(repo_root, &entry.path(), violations);
        }
        return;
    }

    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
        return;
    }

    let relative = path
        .strip_prefix(repo_root)
        .expect("scanned file should be under repo root")
        .to_string_lossy()
        .replace('\\', "/");
    let content = fs::read_to_string(path).expect("read source file");
    for (line_index, line) in content.lines().enumerate() {
        for forbidden in PLUGIN_UI_FORBIDDEN_TERMS {
            if line.contains(forbidden) {
                violations.push(format!(
                    "{}:{}: forbidden `{}` in {}",
                    relative,
                    line_index + 1,
                    forbidden,
                    line.trim()
                ));
            }
        }
    }
}

const PLUGIN_UI_FORBIDDEN_TERMS: &[&str] = &[
    "PluginUi",
    "PluginUI",
    "PluginSlashCommand",
    "PluginCommand",
    "PendingPlugin",
    "active_plugin_ui_dialog",
    "pending_plugin_command_request",
    "pending_plugin_ui_action",
    "pending_plugin_ui_dialog",
    "plugin_ui_actions",
    "plugin_ui_dialogs",
    "plugin_keybindings",
];
