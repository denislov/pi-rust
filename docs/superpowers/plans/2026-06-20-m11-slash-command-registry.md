# M11 Slash Command Registry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand Rust interactive slash commands from the current `/quit` and `/help` slice to a registry containing the TypeScript built-in command set, with deterministic in-TUI behavior for commands the Rust backend can support now.

**Architecture:** Keep command parsing and execution inside `crates/pi-coding-agent/src/interactive/app.rs` for this slice, because the current interactive root owns transcript, status, footer state, and submit interception. Introduce static command metadata, parse the first slash token plus arguments, route implemented commands to small `InteractiveRoot` methods, and make not-yet-ported selector/peripheral commands produce explicit system messages instead of being sent to the model.

**Tech Stack:** Rust 2024, existing `pi-coding-agent` interactive TUI, existing `pi-tui` rendering primitives, offline async test harness in `crates/pi-coding-agent/tests/interactive_mode.rs`.

---

## File Structure

- Modify `crates/pi-coding-agent/src/interactive/app.rs`: replace the two-command enum with a metadata-backed slash registry, parser, help renderer, and command handlers for status-only commands.
- Modify `crates/pi-coding-agent/tests/interactive_mode.rs`: add scripted tests proving registry help, known placeholder commands, unknown commands, and non-slash model prompts behave correctly.
- Modify `docs/roadmap/M11-interactive-ux.md`: mark the slash command framework/registry progress accurately after tests pass.

## Task 1: Registry Metadata And Parser

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`

- [ ] **Step 1: Write failing parser tests**

Add tests inside the existing `#[cfg(test)] mod tests` in `app.rs`:

```rust
#[test]
fn slash_registry_contains_typescript_builtin_commands() {
    let names: Vec<&str> = BUILTIN_SLASH_COMMANDS
        .iter()
        .map(|command| command.name)
        .collect();
    assert_eq!(
        names,
        vec![
            "settings",
            "model",
            "scoped-models",
            "export",
            "import",
            "share",
            "copy",
            "name",
            "session",
            "changelog",
            "hotkeys",
            "fork",
            "clone",
            "tree",
            "login",
            "logout",
            "new",
            "compact",
            "resume",
            "reload",
            "quit",
        ]
    );
}

#[test]
fn parse_slash_command_returns_command_name_and_arguments() {
    assert_eq!(
        parse_slash_command("/model gpt-5"),
        Some(ParsedSlashCommand {
            name: "model".to_string(),
            args: "gpt-5".to_string(),
            original: "/model gpt-5".to_string(),
        })
    );
    assert_eq!(
        parse_slash_command("/NAME Project Phoenix"),
        Some(ParsedSlashCommand {
            name: "name".to_string(),
            args: "Project Phoenix".to_string(),
            original: "/NAME Project Phoenix".to_string(),
        })
    );
}

#[test]
fn parse_slash_command_preserves_non_slash_prompt_path() {
    assert_eq!(parse_slash_command("hello"), None);
    assert_eq!(parse_slash_command("  /quit"), None);
}
```

- [ ] **Step 2: Run parser tests and verify they fail**

Run:

```bash
cargo test -p pi-coding-agent slash_registry_contains_typescript_builtin_commands parse_slash_command_returns_command_name_and_arguments parse_slash_command_preserves_non_slash_prompt_path
```

Expected: compile failure because `BUILTIN_SLASH_COMMANDS` and `ParsedSlashCommand` are not defined yet, or assertion failure if only part of the parser exists.

- [ ] **Step 3: Implement registry metadata and parser**

Replace the current `SlashCommand` enum/parser with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BuiltinSlashCommand {
    name: &'static str,
    description: &'static str,
}

const BUILTIN_SLASH_COMMANDS: &[BuiltinSlashCommand] = &[
    BuiltinSlashCommand { name: "settings", description: "Open settings menu" },
    BuiltinSlashCommand { name: "model", description: "Select model" },
    BuiltinSlashCommand { name: "scoped-models", description: "Enable or disable models for cycling" },
    BuiltinSlashCommand { name: "export", description: "Export session" },
    BuiltinSlashCommand { name: "import", description: "Import and resume a session from JSONL" },
    BuiltinSlashCommand { name: "share", description: "Share session as a secret GitHub gist" },
    BuiltinSlashCommand { name: "copy", description: "Copy last assistant message to clipboard" },
    BuiltinSlashCommand { name: "name", description: "Show or set the session display name" },
    BuiltinSlashCommand { name: "session", description: "Show session info and stats" },
    BuiltinSlashCommand { name: "changelog", description: "Show changelog entries" },
    BuiltinSlashCommand { name: "hotkeys", description: "Show keyboard shortcuts" },
    BuiltinSlashCommand { name: "fork", description: "Create a new fork from a previous user message" },
    BuiltinSlashCommand { name: "clone", description: "Duplicate the current session at the current position" },
    BuiltinSlashCommand { name: "tree", description: "Navigate session tree" },
    BuiltinSlashCommand { name: "login", description: "Configure provider authentication" },
    BuiltinSlashCommand { name: "logout", description: "Remove provider authentication" },
    BuiltinSlashCommand { name: "new", description: "Start a new session" },
    BuiltinSlashCommand { name: "compact", description: "Manually compact the session context" },
    BuiltinSlashCommand { name: "resume", description: "Resume a different session" },
    BuiltinSlashCommand { name: "reload", description: "Reload keybindings and resources" },
    BuiltinSlashCommand { name: "quit", description: "Quit pi" },
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedSlashCommand {
    name: String,
    args: String,
    original: String,
}

fn parse_slash_command(text: &str) -> Option<ParsedSlashCommand> {
    if !text.starts_with('/') {
        return None;
    }
    let original = text.to_string();
    let without_slash = &text[1..];
    let mut parts = without_slash.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or("").to_lowercase();
    let args = parts.next().unwrap_or("").trim().to_string();
    Some(ParsedSlashCommand { name, args, original })
}
```

- [ ] **Step 4: Run parser tests and verify they pass**

Run:

```bash
cargo test -p pi-coding-agent slash_registry_contains_typescript_builtin_commands parse_slash_command_returns_command_name_and_arguments parse_slash_command_preserves_non_slash_prompt_path
```

Expected: parser tests pass.

## Task 2: Help And Command Dispatch

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`

- [ ] **Step 1: Write failing command behavior tests**

Replace the old slash command tests with:

```rust
#[test]
fn help_text_lists_all_builtin_commands() {
    let help = help_text();
    for command in BUILTIN_SLASH_COMMANDS {
        assert!(
            help.contains(&format!("/{}", command.name)),
            "help text should list /{}: {help}",
            command.name
        );
    }
}

#[test]
fn handle_slash_command_quit_sets_exit_when_idle() {
    let mut root = InteractiveRoot::new(
        PathBuf::from("."),
        "faux-model".to_string(),
        "no-session".to_string(),
    );
    root.handle_slash_command(ParsedSlashCommand {
        name: "quit".to_string(),
        args: String::new(),
        original: "/quit".to_string(),
    });
    assert_eq!(root.action, InteractiveAction::Exit);
}

#[test]
fn handle_slash_command_help_pushes_system_item() {
    let mut root = InteractiveRoot::new(
        PathBuf::from("."),
        "faux-model".to_string(),
        "no-session".to_string(),
    );
    root.handle_slash_command(ParsedSlashCommand {
        name: "help".to_string(),
        args: String::new(),
        original: "/help".to_string(),
    });
    let text = last_system_text(&root);
    assert!(text.contains("/model"), "{text}");
    assert!(text.contains("/reload"), "{text}");
    assert_ne!(root.action, InteractiveAction::Submit);
    assert!(root.pending_submit.is_none());
}

#[test]
fn handle_known_pending_command_reports_not_implemented_without_submit() {
    let mut root = InteractiveRoot::new(
        PathBuf::from("."),
        "faux-model".to_string(),
        "no-session".to_string(),
    );
    root.handle_slash_command(ParsedSlashCommand {
        name: "model".to_string(),
        args: "gpt-5".to_string(),
        original: "/model gpt-5".to_string(),
    });
    let text = last_system_text(&root);
    assert!(text.contains("/model"), "{text}");
    assert!(text.contains("not implemented"), "{text}");
    assert_ne!(root.action, InteractiveAction::Submit);
    assert!(root.pending_submit.is_none());
}

#[test]
fn handle_unknown_slash_command_reports_error_without_submit() {
    let mut root = InteractiveRoot::new(
        PathBuf::from("."),
        "faux-model".to_string(),
        "no-session".to_string(),
    );
    root.handle_slash_command(ParsedSlashCommand {
        name: "does-not-exist".to_string(),
        args: String::new(),
        original: "/does-not-exist".to_string(),
    });
    let text = last_system_text(&root);
    assert!(text.contains("unknown command: /does-not-exist"), "{text}");
    assert_ne!(root.action, InteractiveAction::Submit);
    assert!(root.pending_submit.is_none());
}

fn last_system_text(root: &InteractiveRoot) -> String {
    match root.transcript.items().last() {
        Some(TranscriptItem::System { text }) => text.clone(),
        other => panic!("expected last transcript item to be System, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run command tests and verify they fail**

Run:

```bash
cargo test -p pi-coding-agent help_text_lists_all_builtin_commands handle_known_pending_command_reports_not_implemented_without_submit handle_unknown_slash_command_reports_error_without_submit
```

Expected: failures because help text and dispatch still know only `/help` and `/quit`.

- [ ] **Step 3: Implement metadata-backed help and dispatch**

Update `InteractiveRoot::handle_slash_command` to accept `ParsedSlashCommand` and route:

```rust
fn handle_slash_command(&mut self, command: ParsedSlashCommand) {
    match command.name.as_str() {
        "quit" | "exit" | "q" => match self.status {
            InteractiveStatus::Idle => self.action = InteractiveAction::Exit,
            InteractiveStatus::Running => self.action = InteractiveAction::AbortRunning,
        },
        "help" | "h" | "?" => {
            self.transcript.push(TranscriptItem::system(help_text()));
        }
        "name" => self.handle_name_command(&command.args),
        "session" => self.handle_session_command(),
        "hotkeys" => self.handle_hotkeys_command(),
        "changelog" => self.handle_changelog_command(),
        "new" | "reload" | "compact" | "clone" | "fork" | "tree" | "resume" | "settings"
        | "model" | "scoped-models" | "login" | "logout" | "export" | "import" | "share"
        | "copy" => self.handle_pending_slash_command(&command),
        _ => self.transcript.push(TranscriptItem::system(format!(
            "unknown command: {} - type /help for available commands",
            command.original
        ))),
    }
}
```

Add:

```rust
fn handle_pending_slash_command(&mut self, command: &ParsedSlashCommand) {
    self.transcript.push(TranscriptItem::system(format!(
        "/{} is recognized but not implemented in the Rust interactive UI yet.",
        command.name
    )));
}
```

Update `help_text()` to generate from `BUILTIN_SLASH_COMMANDS`, with aliases on the first line:

```rust
fn help_text() -> String {
    let mut lines = vec!["commands:".to_string(), "  /help, /h, /? - show this help".to_string()];
    for command in BUILTIN_SLASH_COMMANDS {
        lines.push(format!("  /{:<13} - {}", command.name, command.description));
    }
    lines.push("  /q, /exit      - aliases for /quit".to_string());
    lines.join("\n")
}
```

- [ ] **Step 4: Run command tests and verify they pass**

Run:

```bash
cargo test -p pi-coding-agent help_text_lists_all_builtin_commands handle_slash_command_help_pushes_system_item handle_known_pending_command_reports_not_implemented_without_submit handle_unknown_slash_command_reports_error_without_submit
```

Expected: command tests pass.

## Task 3: Implement Low-Risk Local Commands

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`

- [ ] **Step 1: Write failing tests for local commands**

Add tests:

```rust
#[test]
fn name_command_without_args_shows_current_session_label() {
    let mut root = InteractiveRoot::new(
        PathBuf::from("."),
        "faux-model".to_string(),
        "session-123".to_string(),
    );
    root.handle_slash_command(ParsedSlashCommand {
        name: "name".to_string(),
        args: String::new(),
        original: "/name".to_string(),
    });
    let text = last_system_text(&root);
    assert!(text.contains("session-123"), "{text}");
}

#[test]
fn name_command_with_args_updates_session_label() {
    let mut root = InteractiveRoot::new(
        PathBuf::from("."),
        "faux-model".to_string(),
        "no-session".to_string(),
    );
    root.handle_slash_command(ParsedSlashCommand {
        name: "name".to_string(),
        args: "Project Phoenix".to_string(),
        original: "/name Project Phoenix".to_string(),
    });
    assert_eq!(root.session_label, "Project Phoenix");
    let text = last_system_text(&root);
    assert!(text.contains("Session name set: Project Phoenix"), "{text}");
}

#[test]
fn session_command_reports_current_footer_state() {
    let mut root = InteractiveRoot::new(
        PathBuf::from("/tmp/project"),
        "faux-model".to_string(),
        "Project Phoenix".to_string(),
    );
    root.usage = (1234, 5678);
    root.handle_slash_command(ParsedSlashCommand {
        name: "session".to_string(),
        args: String::new(),
        original: "/session".to_string(),
    });
    let text = last_system_text(&root);
    assert!(text.contains("Session Info"), "{text}");
    assert!(text.contains("Project Phoenix"), "{text}");
    assert!(text.contains("faux-model"), "{text}");
    assert!(text.contains("1k"), "{text}");
    assert!(text.contains("5k"), "{text}");
}

#[test]
fn hotkeys_command_mentions_core_interactive_bindings() {
    let mut root = InteractiveRoot::new(
        PathBuf::from("."),
        "faux-model".to_string(),
        "no-session".to_string(),
    );
    root.handle_slash_command(ParsedSlashCommand {
        name: "hotkeys".to_string(),
        args: String::new(),
        original: "/hotkeys".to_string(),
    });
    let text = last_system_text(&root);
    assert!(text.contains("Navigation"), "{text}");
    assert!(text.contains("Ctrl+C"), "{text}");
    assert!(text.contains("Ctrl+O"), "{text}");
}
```

- [ ] **Step 2: Run local command tests and verify they fail**

Run:

```bash
cargo test -p pi-coding-agent name_command_without_args_shows_current_session_label name_command_with_args_updates_session_label session_command_reports_current_footer_state hotkeys_command_mentions_core_interactive_bindings
```

Expected: fail because handlers do not exist yet.

- [ ] **Step 3: Implement local handlers**

Add methods to `InteractiveRoot`:

```rust
fn handle_name_command(&mut self, args: &str) {
    if args.is_empty() {
        self.transcript
            .push(TranscriptItem::system(format!("Session name: {}", self.session_label)));
        return;
    }
    self.session_label = args.to_string();
    self.transcript
        .push(TranscriptItem::system(format!("Session name set: {}", self.session_label)));
}

fn handle_session_command(&mut self) {
    let cwd = abbreviate_cwd(&self.cwd);
    self.transcript.push(TranscriptItem::system(format!(
        "Session Info\n\nName: {}\nModel: {}\nCwd: {}\nTokens\nInput: {}\nOutput: {}",
        self.session_label,
        self.model_id,
        cwd,
        format_tokens(self.usage.0),
        format_tokens(self.usage.1)
    )));
}

fn handle_hotkeys_command(&mut self) {
    let keybindings = KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default());
    let submit = key_hint(&keybindings, "tui.input.submit", "submit");
    let newline = key_hint(&keybindings, "tui.input.newLine", "newline");
    let interrupt = app_key_hint(&keybindings, "app.interrupt", "interrupt/exit");
    let expand = app_key_hint(&keybindings, "app.tools.expand", "expand tools");
    let page_up = key_hint(&keybindings, "tui.editor.pageUp", "scroll up");
    let page_down = key_hint(&keybindings, "tui.editor.pageDown", "scroll down");
    self.transcript.push(TranscriptItem::system(format!(
        "Hotkeys\n\nNavigation\n- {page_up}\n- {page_down}\n\nEditing\n- {submit}\n- {newline}\n\nApp\n- {interrupt}\n- {expand}"
    )));
}

fn handle_changelog_command(&mut self) {
    self.transcript.push(TranscriptItem::system(
        "Changelog display is not implemented in the Rust interactive UI yet.".to_string(),
    ));
}
```

- [ ] **Step 4: Run local command tests and verify they pass**

Run:

```bash
cargo test -p pi-coding-agent name_command_without_args_shows_current_session_label name_command_with_args_updates_session_label session_command_reports_current_footer_state hotkeys_command_mentions_core_interactive_bindings
```

Expected: tests pass.

## Task 4: Scripted Integration Tests

**Files:**
- Modify: `crates/pi-coding-agent/tests/interactive_mode.rs`

- [ ] **Step 1: Write failing scripted tests**

Add tests:

```rust
#[tokio::test]
async fn scripted_interactive_help_lists_registry_commands() {
    let output = run_scripted_idle_interactive("/help\r/quit\r")
        .await
        .unwrap();
    let frame = output.rendered_lines.join("\n");
    assert!(frame.contains("/model"), "{frame}");
    assert!(frame.contains("/reload"), "{frame}");
    assert_eq!(output.exit_code, 0);
}

#[tokio::test]
async fn scripted_interactive_known_pending_command_is_not_sent_to_provider() {
    let output = run_scripted_idle_interactive("/model gpt-5\r/quit\r")
        .await
        .unwrap();
    let frame = output.rendered_lines.join("\n");
    assert!(frame.contains("/model"), "{frame}");
    assert!(frame.contains("not implemented"), "{frame}");
    assert_eq!(output.exit_code, 0);
}

#[tokio::test]
async fn scripted_interactive_unknown_command_is_not_sent_to_provider() {
    let output = run_scripted_idle_interactive("/definitely-unknown\r/quit\r")
        .await
        .unwrap();
    let frame = output.rendered_lines.join("\n");
    assert!(frame.contains("unknown command: /definitely-unknown"), "{frame}");
    assert_eq!(output.exit_code, 0);
}

#[tokio::test]
async fn scripted_interactive_name_updates_footer_session_label() {
    let output = run_scripted_idle_interactive("/name Project Phoenix\r/quit\r")
        .await
        .unwrap();
    let frame = output.rendered_lines.join("\n");
    assert!(frame.contains("Session name set: Project Phoenix"), "{frame}");
    assert!(frame.contains("session: Project Phoenix"), "{frame}");
    assert_eq!(output.exit_code, 0);
}
```

- [ ] **Step 2: Run scripted tests and verify they fail before integration is complete**

Run:

```bash
cargo test -p pi-coding-agent scripted_interactive_help_lists_registry_commands scripted_interactive_known_pending_command_is_not_sent_to_provider scripted_interactive_unknown_command_is_not_sent_to_provider scripted_interactive_name_updates_footer_session_label
```

Expected: fail until Tasks 2 and 3 are complete.

- [ ] **Step 3: Run scripted tests and verify they pass after Tasks 2 and 3**

Run the same command. Expected: all four tests pass.

## Task 5: Roadmap Update And Verification

**Files:**
- Modify: `docs/roadmap/M11-interactive-ux.md`

- [ ] **Step 1: Update roadmap status**

Under “slash 命令（2 → 21）”, add a progress note:

```markdown
> 进度：Rust 已有内置 slash command registry，覆盖 TS 的 21 个 built-in 命令名；`/help`、`/quit`、`/name`、`/session`、`/hotkeys` 已有本地行为，其余选择器/会话/外设命令先返回显式未实现提示，后续随对应组件和 M13 后端补齐。
```

- [ ] **Step 2: Run focused checks**

Run:

```bash
cargo fmt --check
cargo test -p pi-coding-agent
```

Expected: both pass.

- [ ] **Step 3: Run workspace verification with stable color environment**

Run:

```bash
env -u NO_COLOR TERM=xterm-256color cargo test --workspace
cargo check --workspace
```

Expected: both pass. If `cargo test --workspace` is run without overriding this session’s `NO_COLOR=1` and `TERM=dumb`, known `pi-tui` markdown color-path tests fail because they intentionally assert ANSI output.

- [ ] **Step 4: Commit this M11 slice**

Run:

```bash
git add crates/pi-coding-agent/src/interactive/app.rs crates/pi-coding-agent/tests/interactive_mode.rs docs/roadmap/M11-interactive-ux.md docs/superpowers/plans/2026-06-20-m11-slash-command-registry.md
git commit -m "feat: expand interactive slash command registry"
```

Expected: commit succeeds with only M11 slash-registry files included.

## Self-Review

- Spec coverage: This plan covers M11 item 1’s command framework and full built-in command registry, but not selector UIs, `pi-tui` component additions, fuzzy/autocomplete, theme expansion, advanced wrapping, terminal-image protocols, or TUI-7 smoke scripts.
- Placeholder scan: No `TODO`, `TBD`, or open-ended implementation steps remain.
- Type consistency: `ParsedSlashCommand`, `BuiltinSlashCommand`, `BUILTIN_SLASH_COMMANDS`, and `InteractiveRoot` handler names are consistent across tasks.
