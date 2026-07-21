#!/usr/bin/env bash
set -euo pipefail

if ! command -v tmux >/dev/null 2>&1; then
  echo "tmux is required for the TUI smoke suite" >&2
  exit 127
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SMOKE_THEME="${PI_RUST_TUI_SMOKE_THEME:-dark}"
SMOKE_COLOR_MODE="${PI_RUST_TUI_SMOKE_COLOR_MODE:-terminal}"
SESSION="pi-rust-tui-smoke-${SMOKE_THEME}-${SMOKE_COLOR_MODE}-$(date +%Y%m%d%H%M%S)"
OUT_DIR="${OUT_DIR:-$ROOT/target/tui-smoke/$SESSION}"
BIN="$ROOT/target/debug/pi-coding-agent"
CONFIG_DIR="$OUT_DIR/config"

mkdir -p "$OUT_DIR" "$CONFIG_DIR"
cat > "$CONFIG_DIR/settings.toml" <<EOF
theme = "$SMOKE_THEME"
quiet_startup = true
EOF
export PI_RUST_DIR="$CONFIG_DIR"
case "$SMOKE_COLOR_MODE" in
  no-color)
    export NO_COLOR=1
    ;;
  basic)
    unset NO_COLOR
    export TERM=xterm
    unset COLORTERM
    ;;
  terminal)
    unset NO_COLOR
    ;;
  *)
    echo "unknown PI_RUST_TUI_SMOKE_COLOR_MODE: $SMOKE_COLOR_MODE" >&2
    exit 2
    ;;
esac

cd "$ROOT"
cargo build -p pi-coding-agent >/dev/null
cargo test -q -p pi-coding-agent --lib \
  fullscreen_authorization_overlay_preserves_queue_focus_until_last_resolution
cargo test -q -p pi-coding-agent --lib \
  fullscreen_delegation_overlay_dispatches_approval_and_restores_focus

capture() {
  local name="$1"
  tmux capture-pane -t "$SESSION" -pJS -32768 > "$OUT_DIR/$name.txt"
  tmux capture-pane -t "$SESSION" -peJS -32768 > "$OUT_DIR/$name.ansi.txt"
}

capture_viewport() {
  local name="$1"
  tmux capture-pane -t "$SESSION" -p > "$OUT_DIR/$name.txt"
  tmux capture-pane -t "$SESSION" -pe > "$OUT_DIR/$name.ansi.txt"
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
capture "08-inline-after-exit"

tmux send-keys -t "$SESSION" "$BIN --no-session --no-tools --tui-mode fullscreen" Enter
sleep 0.5
capture_viewport "10-fullscreen-start"

tmux resize-window -t "$SESSION" -x 160 -y 40
sleep 0.3
capture_viewport "10-fullscreen-resize-160"
tmux resize-window -t "$SESSION" -x 120 -y 32
sleep 0.3

tmux send-keys -t "$SESSION" "/"
sleep 0.2
capture_viewport "10-fullscreen-slash-wide"
tmux resize-window -t "$SESSION" -x 60 -y 18
sleep 0.3
capture_viewport "10-fullscreen-slash-resize-narrow"
tmux send-keys -t "$SESSION" Escape BSpace
sleep 0.2
tmux resize-window -t "$SESSION" -x 120 -y 32
sleep 0.3

tmux resize-window -t "$SESSION" -x 80 -y 24
sleep 0.3
tmux send-keys -t "$SESSION" C-g
sleep 0.2
capture_viewport "10-context-drawer-medium"
tmux send-keys -t "$SESSION" Escape
sleep 0.2
tmux resize-window -t "$SESSION" -x 120 -y 32
sleep 0.3

tmux send-keys -t "$SESSION" "好"
sleep 0.2
capture_viewport "11-fullscreen-wide-unicode"
tmux send-keys -t "$SESSION" C-c
sleep 0.2

tmux resize-window -t "$SESSION" -x 60 -y 18
sleep 0.3
capture_viewport "12-fullscreen-resize-narrow"
tmux send-keys -t "$SESSION" C-g
sleep 0.2
capture_viewport "12-context-modal-narrow"
tmux send-keys -t "$SESSION" Escape
sleep 0.2
tmux resize-window -t "$SESSION" -x 120 -y 32
sleep 0.3
capture_viewport "13-fullscreen-resize-wide"
tmux send-keys -t "$SESSION" Tab Tab Right
sleep 0.2
capture_viewport "13-context-focus-changes"
tmux send-keys -t "$SESSION" Tab
sleep 0.2
tmux send-keys -t "$SESSION" "/settings" Enter
sleep 0.2
capture_viewport "13-settings-overlay-wide"
tmux resize-window -t "$SESSION" -x 60 -y 18
sleep 0.3
capture_viewport "13-settings-overlay-narrow"
tmux send-keys -t "$SESSION" Escape
sleep 0.2
tmux resize-window -t "$SESSION" -x 120 -y 32
sleep 0.3

tmux send-keys -t "$SESSION" "/help" Enter
sleep 0.3
for _ in {1..12}; do
  tmux send-keys -t "$SESSION" -l $'\033[<64;2;2M'
done
sleep 0.2
capture_viewport "14-fullscreen-help-mouse-wheel"
tmux send-keys -t "$SESSION" PageUp
sleep 0.2
capture_viewport "14-fullscreen-help-page-up"

tmux send-keys -t "$SESSION" C-c
sleep 0.2
capture "99-after-fullscreen-exit"
tmux kill-session -t "$SESSION" >/dev/null 2>&1 || true

grep -Fq "scrollback sentinel before pi-rust TUI" "$OUT_DIR/01-start.txt"
grep -Fq "scrollback sentinel before pi-rust TUI" "$OUT_DIR/08-inline-after-exit.txt"
if grep -Fq "scrollback sentinel before pi-rust TUI" "$OUT_DIR/10-fullscreen-start.txt"; then
  echo "fullscreen mode leaked the primary-screen scrollback sentinel" >&2
  exit 1
fi
grep -Fq "pi-rust" "$OUT_DIR/10-fullscreen-start.txt"
grep -Fq "Conversation" "$OUT_DIR/10-fullscreen-start.txt"
grep -Fq "Context [ops]" "$OUT_DIR/10-fullscreen-start.txt"
grep -Fq "Tips" "$OUT_DIR/10-fullscreen-start.txt"
grep -Fq "├" "$OUT_DIR/10-fullscreen-start.txt"
grep -Fq "Context [ops]" "$OUT_DIR/10-fullscreen-resize-160.txt"
grep -Fq "Tips" "$OUT_DIR/10-fullscreen-resize-160.txt"
grep -Fq "/help" "$OUT_DIR/10-fullscreen-slash-wide.txt"
grep -Fq "/help" "$OUT_DIR/10-fullscreen-slash-resize-narrow.txt"
grep -Fq "Context [ops]" "$OUT_DIR/10-context-drawer-medium.txt"
grep -Fq "│" "$OUT_DIR/10-context-drawer-medium.txt"
grep -Fq "idle" "$OUT_DIR/12-fullscreen-resize-narrow.txt"
grep -Fq "Context [ops]" "$OUT_DIR/12-context-modal-narrow.txt"
if grep -Fq "Conversation" "$OUT_DIR/12-context-modal-narrow.txt"; then
  echo "narrow context modal did not replace the work area" >&2
  exit 1
fi
grep -Fq "idle" "$OUT_DIR/13-fullscreen-resize-wide.txt"
grep -Fq "Context ops [changes]" "$OUT_DIR/13-context-focus-changes.txt"
grep -Fq "Settings" "$OUT_DIR/13-settings-overlay-wide.txt"
grep -Fq "Settings" "$OUT_DIR/13-settings-overlay-narrow.txt"
grep -Fq "idle" "$OUT_DIR/13-settings-overlay-narrow.txt"
grep -Fq "show this help" "$OUT_DIR/14-fullscreen-help-mouse-wheel.txt"
grep -Fq "show this help" "$OUT_DIR/14-fullscreen-help-page-up.txt"
grep -Fq "scrollback sentinel before pi-rust TUI" "$OUT_DIR/99-after-fullscreen-exit.txt"

cat > "$OUT_DIR/README.txt" <<EOF
pi-rust TUI smoke capture

Session: $SESSION
Theme: $SMOKE_THEME
Color mode: $SMOKE_COLOR_MODE
Each plain .txt capture has a matching .ansi.txt capture retaining terminal
style sequences for color-level review.
Inline command: $BIN --no-session --no-tools
Fullscreen command: $BIN --no-session --no-tools --tui-mode fullscreen

Review checklist:
- 01-start keeps "scrollback sentinel before pi-rust TUI" above the inline TUI.
- 02-first-char keeps the prompt cursor after the typed character.
- 04-wide-unicode keeps the prompt cursor after the wide character.
- 05-resize-narrow and 06-resize-wide do not clear unrelated scrollback.
- 07-help-command shows slash commands without submitting to a provider.
- 08-real-provider-stream exists only when PI_RUST_TUI_SMOKE_REAL_PROMPT is set.
- 08-inline-after-exit preserves the original shell scrollback.
- 10-fullscreen-start owns the viewport and hides the primary-screen sentinel.
- 10-fullscreen-resize-160 covers the largest release viewport.
- 10-fullscreen-slash-wide and 10-fullscreen-slash-resize-narrow prove composer
  assistance remains visible and bounded across a live resize.
- 10-context-drawer-medium proves medium Context has an explicit drawer edge
  and Escape restores the conversation/composer shell.
- 11-fullscreen-wide-unicode keeps the prompt cursor after the wide character.
- 12-fullscreen-resize-narrow and 13-fullscreen-resize-wide remain bounded.
- 12-context-modal-narrow replaces the narrow work area and Escape restores it.
- 13-context-focus-changes proves Tab focus and Context tab routing.
- 13-settings-overlay-wide and 13-settings-overlay-narrow prove the shared
  product overlay remains bounded through resize; the script also runs offline
  authorization queue and delegation approval round trips before tmux starts.
- 14-fullscreen-help-page-up shows the top of help through transcript scrolling.
- 14-fullscreen-help-mouse-wheel reaches the same scrollable Conversation
  history through typed SGR mouse input while PageUp remains available.
- 99-after-fullscreen-exit restores the primary screen and its sentinel.
EOF

echo "TUI smoke captures written to $OUT_DIR"
