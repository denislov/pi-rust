# Design: pi-coding-agent interactive slash commands (/quit + /help)

- Date: 2026-06-19
- Status: Draft (pending review)
- Scope: TUI-8 follow-up slice — add slash command parsing to the interactive mode, starting with `/quit` and `/help`.
- Depends on: `pi-coding-agent` interactive loop (`InteractiveRoot`, `InteractiveAction`, `TranscriptItem::System`), TUI-8 first slice (footer coloring, welcome line).

## 1. Context

The interactive TUI has no slash command system. When the user types text and presses Enter, the text is always sent to the model as a prompt. There is no way to exit via a typed command (only Ctrl+C), and no in-app help for available commands/shortcuts.

This slice adds minimal slash command support: `/quit` (exit interactive mode, equivalent to Ctrl+C) and `/help` (show available commands as a transcript system message). Commands are parsed at submit time — no overlay menu, no live autocomplete. This is the foundation for future command expansion.

Behavioral reference (TS): `pi/packages/coding-agent/src/` has a full slash command system with 21+ commands. This Rust slice ports only the two most essential commands, using a simple submit-time interception pattern rather than the TS command registry.

## 2. Goals and success criteria

Add `/quit` and `/help` slash commands to the interactive TUI, parsed at submit time.

Done when:

1. `cargo fmt --check`, `cargo test -p pi-coding-agent`, `cargo test --workspace`, and `cargo check --workspace` pass from `pi-rust/`.
2. Typing `/quit` (or `/q`, `/exit`, case-insensitive) and pressing Enter exits interactive mode — same behavior as Ctrl+C in idle state (exit) or running state (abort then exit).
3. Typing `/help` (or `/h`, `/?`, case-insensitive) and pressing Enter pushes a `TranscriptItem::System` message listing available commands into the transcript. The prompt is NOT sent to the model.
4. Typing any unknown `/foo` command pushes a `TranscriptItem::System` error message ("unknown command: /foo — type /help for available commands"). The prompt is NOT sent to the model.
5. Non-slash text (not starting with `/`) is sent to the model as before — no behavior change.
6. The welcome line mentions `/help commands`.
7. Slash command text is never sent to the model (`action != Submit`, `pending_submit == None`).
8. The offline test suite passes with no network access and no credentials.

## 3. Non-goals (this increment)

- SelectList overlay menu for command autocomplete (typing `/` to popup).
- Additional slash commands beyond `/quit` and `/help` (e.g. `/clear`, `/model`, `/session`).
- Command argument parsing beyond the command name (e.g. `/model sonnet-4` — only the first word is matched).
- Slash command registry or plugin system.
- Changes to `pi-tui` core.
- Changes to `pi-agent-core` or `pi-ai`.

## 4. Design

### 4.1 Architecture and data flow

Single-layer change, entirely in `crates/pi-coding-agent/src/interactive/app.rs`:

    User presses Enter (submit)
      |
      +- editor.on_submit -> submitted: Arc<Mutex<Option<String>>>
      |
      +- InteractiveRoot::handle_input (idle submit path)
          +- take_submitted() -> Option<String>
          +- if text starts with "/" -> parse_slash_command(text)
          |   +- /quit  -> action = Exit (idle) or AbortRunning (running)
          |   +- /help  -> transcript.push(System: help_text())
          |   +- /unknown -> transcript.push(System: "unknown command: ...")
          |   +- action stays None (NOT Submit) -> not sent to model
          +- else -> pending_submit = text; action = Submit (normal path)

**Key invariants:**

- Slash command text is never sent to the model — `action` is not set to `Submit`, `pending_submit` stays `None`.
- `/quit` reuses existing `InteractiveAction::Exit` (idle) and `InteractiveAction::AbortRunning` (running) — same code paths as Ctrl+C.
- `/help` and unknown commands use `TranscriptItem::System` — same rendering as the welcome line.
- Case-insensitive: `/QUIT`, `/Quit`, `/quit` all match.
- The editor is already cleared by `Editor::submit()` after `on_submit` fires — no extra cleanup needed.

### 4.2 SlashCommand enum and parser

New enum and function in `app.rs` (near `InteractiveAction`):

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

- `/quit`, `/exit`, `/q` all trigger Quit (`/q` as a short alias).
- `/help`, `/h`, `/?` all trigger Help.
- `/quit extra args` still matches (only the first word is checked).
- Case-insensitive via `to_lowercase()`.
- Text not starting with `/` returns `None` (normal prompt path).

### 4.3 Command execution

New method on `InteractiveRoot`:

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

`help_text()` function:

```rust
fn help_text() -> String {
    "commands:\n  /help, /h, /?  — show this help\n  /quit, /q, /exit — exit interactive mode".to_string()
}
```

`TranscriptItem::System` text can contain `\n`; `render_transcript_lines` splits on `\n` for multi-line rendering.

### 4.4 Submit path integration

In `InteractiveRoot::handle_input`, the idle submit path currently does:

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

### 4.5 Welcome line update

The current `welcome_line` function produces:

    pi · Enter submit · Shift+Enter newline · Ctrl+C interrupt/exit · Ctrl+O expand tools · PageUp scroll up · PageDown scroll down

Add a `/help commands` hint. In the `parts` array of `welcome_line`, insert `"/help commands".to_string()` before the `app_key_hint(keybindings, "app.interrupt", "interrupt/exit")` entry. The welcome line becomes:

    pi · Enter submit · Shift+Enter newline · /help commands · Ctrl+C interrupt/exit · Ctrl+O expand tools · PageUp scroll up · PageDown scroll down

### 4.6 Error handling

No new error types. `parse_slash_command` returns `Option<SlashCommand>` — `None` for non-slash text (normal path), `Some(Unknown(...))` for unrecognized commands. `handle_slash_command` has no fallible operations. All command actions reuse existing `InteractiveAction` variants and `TranscriptItem::System`.

### 4.7 Testing strategy

All tests are offline and deterministic.

**In-file unit tests** (`app.rs` `#[cfg(test)] mod tests`):

- `parse_slash_command_recognizes_quit_variants`: `/quit`, `/QUIT`, `/Quit`, `/q`, `/exit` → `SlashCommand::Quit`.
- `parse_slash_command_recognizes_help_variants`: `/help`, `/h`, `/?`, `/HELP` → `SlashCommand::Help`.
- `parse_slash_command_rejects_non_slash`: `hello`, `  / ` (leading whitespace then slash) — `hello` → `None`.
- `parse_slash_command_unknown_command`: `/foo` → `Unknown("/foo")`.
- `handle_slash_command_quit_sets_exit_when_idle`: construct root (idle), `handle_slash_command(Quit)`, assert `action == Exit`.
- `handle_slash_command_quit_sets_abort_when_running`: `set_status(Running)`, `handle_slash_command(Quit)`, assert `action == AbortRunning`.
- `handle_slash_command_help_pushes_system_item`: `handle_slash_command(Help)`, assert last transcript item is `System` and contains `/quit`.
- `handle_slash_command_unknown_pushes_error`: `handle_slash_command(Unknown("/foo".into()))`, assert last transcript item is `System` and contains `unknown command`.
- `slash_command_does_not_set_submit_action`: verify that after `handle_slash_command`, `action != Submit` and `pending_submit == None`.

**Scripted tests** (`interactive_mode.rs`):

- `scripted_interactive_quit_exits_when_idle`: input `/quit\r`, assert `exit_code == 0`.
- `scripted_interactive_help_shows_commands`: input `/help\r\x03` (help then Ctrl+C to exit), assert frame contains `/quit`.
- Existing tests unaffected (they don't use `/`-prefixed prompts).

### 4.8 File structure

| File | Operation |
|---|---|
| `pi-coding-agent/src/interactive/app.rs` | edit: add `SlashCommand` enum, `parse_slash_command` function, `handle_slash_command` method, `help_text` function, submit path interception, welcome line update, in-file tests |
| `pi-coding-agent/tests/interactive_mode.rs` | edit: add `/quit` and `/help` scripted tests |

No new files, no changes to `pi-tui` or other crates.

### 4.9 Verification

Run from `pi-rust/`:

```bash
cargo fmt --check
cargo test -p pi-coding-agent
cargo test --workspace
cargo check --workspace
```

All must pass.

## 5. Key decisions and constraints

- **Submit-time interception** — commands parsed when user presses Enter, not when typing `/`. No overlay menu, no live autocomplete. Minimal intrusion, reuses existing submit flow.
- **Case-insensitive** — `/QUIT` and `/quit` both work. User-friendly.
- **Aliases** — `/q` for quit, `/h` and `/?` for help. Matches common CLI conventions.
- **Reuse existing actions** — `/quit` uses `InteractiveAction::Exit`/`AbortRunning`, same as Ctrl+C. No new action variant needed.
- **Transcript system messages** — `/help` and unknown commands push `TranscriptItem::System`, same rendering as welcome line. No overlay needed.
- **Never sent to model** — slash command text never reaches `pending_submit` or `action = Submit`.
- **No `pi-tui` changes** — entirely in `pi-coding-agent`, preserving the app-neutral boundary.
- **Welcome line mentions `/help`** — discoverability without adding a dedicated help keybinding.
