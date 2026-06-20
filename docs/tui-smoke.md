# TUI-7 Cross-Terminal Smoke Suite

This manual suite catches terminal behavior that `VirtualTerminal` tests cannot model. It runs the Rust interactive UI inside a disposable tmux session and captures pane output before, during, and after key interactions.

## Run

From the `pi-rust` workspace:

```bash
scripts/tui-smoke.sh
```

The script writes captures under `target/tui-smoke/<session>/` and prints the exact directory. It builds `pi-coding-agent`, starts a tmux session, launches interactive mode with `--no-session --no-tools`, and captures:

- existing scrollback before the TUI starts;
- first typed character;
- clearing text with `Ctrl+C`;
- wide Unicode input;
- narrow and wide resize;
- `/help`;
- idle `Ctrl+C` exit and terminal cleanup.

## Optional Real Provider Stream

Set `PI_RUST_TUI_SMOKE_REAL_PROMPT` to send one real prompt through the configured provider. This is intentionally opt-in because it can use network and credentials.

```bash
PI_RUST_TUI_SMOKE_REAL_PROMPT="Reply with one short sentence." scripts/tui-smoke.sh
```

If the global `~/.pi-rust/settings.toml` and `~/.pi-rust/auth.toml` are configured, the default model and provider key are loaded by the normal M7 config path.

## Review Checklist

For each target terminal, inspect the generated capture files and record the result:

| Terminal | Date | Images | Truecolor | Scrollback Preserved | Cursor Stable | Resize Scoped | Ctrl+C Cleanup | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| wezterm |  |  |  |  |  |  |  |  |
| kitty |  |  |  |  |  |  |  |  |
| iTerm2 |  |  |  |  |  |  |  |  |
| Terminal.app |  |  |  |  |  |  |  |  |
| GNOME Terminal |  |  |  |  |  |  |  |  |
| tmux |  |  |  |  |  |  |  |  |
| SSH/tmux |  |  |  |  |  |  |  |  |

Acceptance:

- `01-start.txt` still contains `scrollback sentinel before pi-rust TUI` above the interactive UI.
- `02-first-char.txt` and `04-wide-unicode.txt` show the typed character in the prompt without cursor drift.
- `05-resize-narrow.txt` and `06-resize-wide.txt` do not show a full-screen clear or lost scrollback.
- `07-help-command.txt` shows slash commands and does not submit text to a provider.
- `99-after-exit.txt` shows the shell restored after idle `Ctrl+C`.
