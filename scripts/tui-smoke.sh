#!/usr/bin/env bash
set -euo pipefail

if ! command -v tmux >/dev/null 2>&1; then
  echo "tmux is required for the TUI smoke suite" >&2
  exit 127
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SESSION="pi-rust-tui-smoke-$(date +%Y%m%d%H%M%S)"
OUT_DIR="${OUT_DIR:-$ROOT/target/tui-smoke/$SESSION}"
BIN="$ROOT/target/debug/pi-coding-agent"

mkdir -p "$OUT_DIR"

cd "$ROOT"
cargo build -p pi-coding-agent >/dev/null

capture() {
  local name="$1"
  tmux capture-pane -t "$SESSION" -peJS -32768 > "$OUT_DIR/$name.txt"
}

tmux new-session -d -s "$SESSION" -x 100 -y 30
tmux send-keys -t "$SESSION" "printf 'scrollback sentinel before pi-rust TUI\\n'" Enter
tmux send-keys -t "$SESSION" "$BIN --no-session --no-tools" Enter
sleep 0.5
capture "01-start"

tmux send-keys -t "$SESSION" "a"
sleep 0.2
capture "02-first-char"
tmux send-keys -t "$SESSION" C-c
sleep 0.2
capture "03-clear-text"

tmux send-keys -t "$SESSION" "好"
sleep 0.2
capture "04-wide-unicode"
tmux send-keys -t "$SESSION" C-c
sleep 0.2

tmux resize-window -t "$SESSION" -x 60 -y 18
sleep 0.3
capture "05-resize-narrow"
tmux resize-window -t "$SESSION" -x 120 -y 32
sleep 0.3
capture "06-resize-wide"

tmux send-keys -t "$SESSION" "/help" Enter
sleep 0.3
capture "07-help-command"

if [[ "${PI_RUST_TUI_SMOKE_REAL_PROMPT:-}" != "" ]]; then
  tmux send-keys -t "$SESSION" "$PI_RUST_TUI_SMOKE_REAL_PROMPT" Enter
  sleep "${PI_RUST_TUI_SMOKE_REAL_WAIT:-8}"
  capture "08-real-provider-stream"
fi

tmux send-keys -t "$SESSION" C-c
sleep 0.2
capture "99-after-exit"
tmux kill-session -t "$SESSION" >/dev/null 2>&1 || true

cat > "$OUT_DIR/README.txt" <<EOF
pi-rust TUI smoke capture

Session: $SESSION
Command: $BIN --no-session --no-tools

Review checklist:
- 01-start keeps "scrollback sentinel before pi-rust TUI" above the inline TUI.
- 02-first-char keeps the prompt cursor after the typed character.
- 04-wide-unicode keeps the prompt cursor after the wide character.
- 05-resize-narrow and 06-resize-wide do not clear unrelated scrollback.
- 07-help-command shows slash commands without submitting to a provider.
- 08-real-provider-stream exists only when PI_RUST_TUI_SMOKE_REAL_PROMPT is set.
- 99-after-exit shows terminal cleanup after Ctrl+C exit.
EOF

echo "TUI smoke captures written to $OUT_DIR"
