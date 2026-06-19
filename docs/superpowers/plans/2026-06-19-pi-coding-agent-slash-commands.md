# Slash Commands (/quit + /help) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Do not commit unless the user explicitly requests a commit.

**Goal:** Add `/quit` and `/help` slash commands to the interactive TUI, parsed at submit time, without sending command text to the model.

**Architecture:** Single-layer change in `pi-coding-agent/src/interactive/app.rs`. A `SlashCommand` enum and `parse_slash_command` function intercept submitted text starting with `/`. `handle_slash_command` executes the command: `/quit` reuses `InteractiveAction::Exit`/`AbortRunning` (same as Ctrl+C), `/help` pushes a `TranscriptItem::System` with the command list, unknown commands push an error message. The welcome line gets a `/help commands` hint.

**Tech Stack:** Rust edition 2024; existing `pi-coding-agent` interactive loop (`InteractiveRoot`, `InteractiveAction`, `TranscriptItem::System`), existing scripted test harness.

## Global Constraints

- Slash command text is never sent to the model — `action` is not set to `Submit`, `pending_submit` stays `None`.
- `/quit` reuses existing `InteractiveAction::Exit` (idle) and `InteractiveAction::AbortRunning` (running) — same code paths as Ctrl+C.
- `/help` and unknown commands use `TranscriptItem::System` — same rendering as the welcome line.
- Case-insensitive: `/QUIT`, `/Quit`, `/quit` all match.
- The editor is already cleared by `Editor::submit()` after `on_submit` fires — no extra cleanup needed.
- Tests are deterministic and offline; no real provider key, no network, no real TTY.
- Run checks from `pi-rust/`: `cargo fmt --check`, `cargo test -p pi-coding-agent`, `cargo test --workspace`, `cargo check --workspace`.

## Reference: existing signatures the plan builds on

These already exist; do not re-implement them:

```rust
// crates/pi-coding-agent/src/interactive/app.rs (current)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InteractiveAction {
    None,
    Submit,
    AbortRunning,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InteractiveStatus {
    Idle,
    Running,
}

// InteractiveRoot fields include: status, action, transcript, pending_submit
// TranscriptItem::System { text: String } exists and renders via render_transcript_lines

// The submit path in InteractiveRoot::handle_input (idle branch, ~line 400):
if let Some(prompt) = self.take_submitted() {
    self.pending_submit = Some(prompt);
    self.action = InteractiveAction::Submit;
}

// welcome_line function (~line 1076):
fn welcome_line(keybindings: &KeybindingsManager) -> String {
    let parts = [
        key_hint(keybindings, "tui.input.submit", "submit"),
        key_hint(keybindings, "tui.input.newLine", "newline"),
        app_key_hint(keybindings, "app.interrupt", "interrupt/exit"),
        app_key_hint(keybindings, "app.tools.expand", "expand tools"),
        key_hint(keybindings, "tui.editor.pageUp", "scroll up"),
        key_hint(keybindings, "tui.editor.pageDown", "scroll down"),
    ];
    format!("pi · {}", parts.join(" · "))
}
```

## File Structure

- Modify: `crates/pi-coding-agent/src/interactive/app.rs` — add `SlashCommand` enum, `parse_slash_command` function, `handle_slash_command` method, `help_text` function, submit path interception, welcome line update, in-file tests.
- Modify: `crates/pi-coding-agent/tests/interactive_mode.rs` — add `/quit` and `/help` scripted tests.

No new files, no changes to `pi-tui` or other crates.

---

## Task 1: Slash command parsing, execution, and submit path integration

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`

**Interfaces:**
- Consumes: `InteractiveAction`, `InteractiveStatus`, `TranscriptItem`, `Transcript` (all existing).
- Produces: `SlashCommand` enum, `parse_slash_command` function, `handle_slash_command` method, `help_text` function, updated submit path.

- [ ] **Step 1: Write failing tests**

In `crates/pi-coding-agent/src/interactive/app.rs`, the existing `#[cfg(test)] mod tests` block (near the end of the file) contains the spinner and transcript tests. Append these new tests inside the same `mod tests` block:

```rust
    #[test]
    fn parse_slash_command_recognizes_quit_variants() {
        assert_eq!(parse_slash_command("/quit"), Some(SlashCommand::Quit));
        assert_eq!(parse_slash_command("/QUIT"), Some(SlashCommand::Quit));
        assert_eq!(parse_slash_command("/Quit"), Some(SlashCommand::Quit));
        assert_eq!(parse_slash_command("/q"), Some(SlashCommand::Quit));
        assert_eq!(parse_slash_command("/exit"), Some(SlashCommand::Quit));
    }

    #[test]
    fn parse_slash_command_recognizes_help_variants() {
        assert_eq!(parse_slash_command("/help"), Some(SlashCommand::Help));
        assert_eq!(parse_slash_command("/h"), Some(SlashCommand::Help));
        assert_eq!(parse_slash_command("/?"), Some(SlashCommand::Help));
        assert_eq!(parse_slash_command("/HELP"), Some(SlashCommand::Help));
    }

    #[test]
    fn parse_slash_command_rejects_non_slash() {
        assert_eq!(parse_slash_command("hello"), None);
        assert_eq!(parse_slash_command("  /quit"), None);
    }

    #[test]
    fn parse_slash_command_unknown_command() {
        assert_eq!(
            parse_slash_command("/foo"),
            Some(SlashCommand::Unknown("/foo".to_string()))
        );
    }

    #[test]
    fn handle_slash_command_quit_sets_exit_when_idle() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(SlashCommand::Quit);
        assert_eq!(root.action, InteractiveAction::Exit);
    }

    #[test]
    fn handle_slash_command_quit_sets_abort_when_running() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_status(InteractiveStatus::Running);
        root.handle_slash_command(SlashCommand::Quit);
        assert_eq!(root.action, InteractiveAction::AbortRunning);
    }

    #[test]
    fn handle_slash_command_help_pushs_system_item() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(SlashCommand::Help);
        let items = root.transcript.items();
        let last = items.last().expect("transcript should have an item");
        match last {
            TranscriptItem::System { text } => {
                assert!(text.contains("/quit"), "help text should mention /quit: {text}");
            }
            _ => panic!("expected System item, got {last:?}"),
        }
        assert_ne!(root.action, InteractiveAction::Submit);
        assert!(root.pending_submit.is_none());
    }

    #[test]
    fn handle_slash_command_unknown_pushs_error() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(SlashCommand::Unknown("/foo".to_string()));
        let items = root.transcript.items();
        let last = items.last().expect("transcript should have an item");
        match last {
            TranscriptItem::System { text } => {
                assert!(text.contains("unknown command"), "error should mention 'unknown command': {text}");
            }
            _ => panic!("expected System item, got {last:?}"),
        }
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent --lib parse_slash_command
```

Expected: compile errors — `SlashCommand` enum and `parse_slash_command` function do not exist; `handle_slash_command` method does not exist on `InteractiveRoot`.

- [ ] **Step 3: Add `SlashCommand` enum and `parse_slash_command` function**

In `crates/pi-coding-agent/src/interactive/app.rs`, after the `TranscriptScrollCommand` enum (line ~149), add:

```rust
enum SlashCommand {
    Quit,
    Help,
    Unknown(String),
}

fn parse_slash_command(text: &str) -> Option<SlashCommand> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') {
        return None;
    }
    let command = trimmed[1..].to_lowercase();
    let command_name = command.split_whitespace().next().unwrap_or("");
    Some(match command_name {
        "quit" | "exit" | "q" => SlashCommand::Quit,
        "help" | "h" | "?" => SlashCommand::Help,
        _ => SlashCommand::Unknown(trimmed.to_string()),
    })
}
```

- [ ] **Step 4: Add `help_text` function**

In `app.rs`, near the `welcome_line` function (~line 1076), add:

```rust
fn help_text() -> String {
    "commands:\n  /help, /h, /?  — show this help\n  /quit, /q, /exit — exit interactive mode".to_string()
}
```

- [ ] **Step 5: Add `handle_slash_command` method to `InteractiveRoot`**

In the `impl InteractiveRoot` block, after the `set_status` method (~line 285), add:

```rust
    fn handle_slash_command(&mut self, command: SlashCommand) {
        match command {
            SlashCommand::Quit => {
                match self.status {
                    InteractiveStatus::Idle => self.action = InteractiveAction::Exit,
                    InteractiveStatus::Running => self.action = InteractiveAction::AbortRunning,
                }
            }
            SlashCommand::Help => {
                self.transcript.push(TranscriptItem::system(help_text()));
            }
            SlashCommand::Unknown(cmd) => {
                self.transcript.push(TranscriptItem::system(format!(
                    "unknown command: {cmd} — type /help for available commands"
                )));
            }
        }
    }
```

- [ ] **Step 6: Update the submit path to intercept slash commands**

In `InteractiveRoot::handle_input`, the idle submit path (~line 400) currently is:

```rust
            if let Some(prompt) = self.take_submitted() {
                self.pending_submit = Some(prompt);
                self.action = InteractiveAction::Submit;
            }
```

Replace with:

```rust
            if let Some(text) = self.take_submitted() {
                if let Some(command) = parse_slash_command(&text) {
                    self.handle_slash_command(command);
                } else {
                    self.pending_submit = Some(text);
                    self.action = InteractiveAction::Submit;
                }
            }
```

- [ ] **Step 7: Update the welcome line to mention `/help`**

In the `welcome_line` function (~line 1076), the current `parts` array is:

```rust
    let parts = [
        key_hint(keybindings, "tui.input.submit", "submit"),
        key_hint(keybindings, "tui.input.newLine", "newline"),
        app_key_hint(keybindings, "app.interrupt", "interrupt/exit"),
        app_key_hint(keybindings, "app.tools.expand", "expand tools"),
        key_hint(keybindings, "tui.editor.pageUp", "scroll up"),
        key_hint(keybindings, "tui.editor.pageDown", "scroll down"),
    ];
```

Replace with (inserting `"/help commands".to_string()` before the interrupt line):

```rust
    let parts = [
        key_hint(keybindings, "tui.input.submit", "submit"),
        key_hint(keybindings, "tui.input.newLine", "newline"),
        "/help commands".to_string(),
        app_key_hint(keybindings, "app.interrupt", "interrupt/exit"),
        app_key_hint(keybindings, "app.tools.expand", "expand tools"),
        key_hint(keybindings, "tui.editor.pageUp", "scroll up"),
        key_hint(keybindings, "tui.editor.pageDown", "scroll down"),
    ];
```

- [ ] **Step 8: Run tests to verify they pass**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent --lib parse_slash_command
cargo test -p pi-coding-agent --lib handle_slash_command
```

Expected: PASS (8 tests).

- [ ] **Step 9: Run the full pi-coding-agent suite to check for regressions**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent
```

Expected: PASS (all existing tests green; the welcome line test `scripted_interactive_shows_welcome_line_on_empty_transcript` asserts `frame.contains("submit")` and `frame.contains("pi · ")` — both still hold).

- [ ] **Step 10: Commit**

```bash
cd pi-rust
git add crates/pi-coding-agent/src/interactive/app.rs
git commit -m "feat(interactive): add /quit and /help slash commands"
```

---

## Task 2: Scripted integration tests + final verification

**Files:**
- Modify: `crates/pi-coding-agent/tests/interactive_mode.rs`

- [ ] **Step 1: Write scripted tests for /quit and /help**

Append to `crates/pi-coding-agent/tests/interactive_mode.rs` (after the existing tests):

```rust
#[tokio::test]
async fn scripted_interactive_quit_exits_when_idle() {
    let output = run_scripted_idle_interactive("/quit\r").await.unwrap();
    assert_eq!(output.exit_code, 0, "exit code should be 0 for /quit");
}

#[tokio::test]
async fn scripted_interactive_help_shows_commands() {
    let output = run_scripted_idle_interactive("/help\r\x03").await.unwrap();
    let frame = output.rendered_lines.join("\n");
    assert!(
        frame.contains("/quit"),
        "help output should mention /quit: {frame}"
    );
    assert!(
        frame.contains("/help"),
        "help output should mention /help: {frame}"
    );
}
```

Note: `run_scripted_idle_interactive` is the existing test harness function that runs the interactive loop with no provider (idle mode). It feeds the input string as stdin bytes and returns when the loop exits. `/quit\r` types `/quit` and presses Enter, triggering the Exit action. `/help\r\x03` types `/help`, presses Enter (pushes help to transcript), then Ctrl+C (exits idle with empty editor — but the editor is already empty after `/help` was submitted and cleared).

- [ ] **Step 2: Run the new tests to verify they pass**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent --test interactive_mode scripted_interactive_quit_exits_when_idle scripted_interactive_help_shows_commands
```

Expected: PASS (2 tests).

- [ ] **Step 3: Run the full pi-coding-agent suite**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
cd pi-rust
git add crates/pi-coding-agent/tests/interactive_mode.rs
git commit -m "test(interactive): add /quit and /help scripted integration tests"
```

- [ ] **Step 5: Final verification — formatting, workspace tests, check**

Run from `pi-rust/`:

```bash
cargo fmt --check
cargo test --workspace
cargo check --workspace
```

Expected: all PASS.

- [ ] **Step 6: Inspect git log**

Run from `pi-rust/`:

```bash
git log --oneline -5
```

Expected: the commits sit on top of the spec commit (`50c3fdc docs: add slash commands (/quit + /help) design`):
1. `feat(interactive): add /quit and /help slash commands`
2. `test(interactive): add /quit and /help scripted integration tests`
