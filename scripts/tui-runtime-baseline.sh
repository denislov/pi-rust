#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

command -v jq >/dev/null || {
  echo "jq is required to inventory multiline lint attributes" >&2
  exit 1
}

OUT_DIR="${PI_RUST_TUI_BASELINE_DIR:-$ROOT/target/perf-baseline/0.5.3-fullscreen-tui}"
mkdir -p "$OUT_DIR"

rustc --version --verbose >"$OUT_DIR/rustc.txt"
cargo --version >"$OUT_DIR/cargo.txt"
git rev-parse HEAD >"$OUT_DIR/source-commit.txt"
git status --short >"$OUT_DIR/worktree-status.txt"

rg --json -U \
  '#!?\[(?:allow|cfg_attr)\([^]]*dead_code[^]]*\)\]' \
  crates --glob '*.rs' \
  | jq -r \
    'select(.type == "match") | "\(.data.path.text):\(.data.line_number):\(.data.lines.text | gsub("\\n"; " "))"' \
  >"$OUT_DIR/dead-code-allowances.txt" || true

rg --json -U \
  '#!?\[(?:allow|cfg_attr)\([^]]*dead_code[^]]*\)\]' \
  crates --glob '*.rs' \
  | jq -r \
    'select(.type == "match" and (.data.lines.text | contains("reason") | not)) | "\(.data.path.text):\(.data.line_number):\(.data.lines.text | gsub("\\n"; " "))"' \
  >"$OUT_DIR/unreasoned-dead-code-allowances.txt" || true

rg -n \
  'unbounded_channel|UnboundedSender|UnboundedReceiver' \
  crates/pi-coding-agent/src/adapters/interactive \
  crates/pi-coding-agent/src/runtime \
  --glob '*.rs' \
  | rg -v '\.contains\(' >"$OUT_DIR/unbounded-channels.txt" || true

rg -n \
  'stdout\(\)|set_progress|set_title|render_once|SYNC_(START|END)|synchronized' \
  crates/pi-tui/src \
  crates/pi-coding-agent/src/adapters/interactive \
  --glob '*.rs' >"$OUT_DIR/terminal-writers.txt" || true

rg -n \
  'Duration::from_millis|INTERVAL|POLL|KEEPALIVE|sleep\(' \
  crates/pi-tui/src \
  crates/pi-coding-agent/src/adapters/interactive \
  --glob '*.rs' >"$OUT_DIR/timers.txt" || true

{
  printf 'measure\tcount\n'
  printf 'dead_code_allowances\t%s\n' "$(wc -l <"$OUT_DIR/dead-code-allowances.txt" | tr -d ' ')"
  printf 'unreasoned_dead_code_allowances\t%s\n' "$(wc -l <"$OUT_DIR/unreasoned-dead-code-allowances.txt" | tr -d ' ')"
  printf 'unbounded_channel_references\t%s\n' "$(wc -l <"$OUT_DIR/unbounded-channels.txt" | tr -d ' ')"
  printf 'terminal_writer_references\t%s\n' "$(wc -l <"$OUT_DIR/terminal-writers.txt" | tr -d ' ')"
  printf 'timer_references\t%s\n' "$(wc -l <"$OUT_DIR/timers.txt" | tr -d ' ')"
} >"$OUT_DIR/counts.tsv"

if [[ -s "$OUT_DIR/unreasoned-dead-code-allowances.txt" ]]; then
  echo "unreasoned dead_code allowances found:" >&2
  cat "$OUT_DIR/unreasoned-dead-code-allowances.txt" >&2
  exit 1
fi

printf 'case\telapsed_seconds\tmax_rss_kib\n' >"$OUT_DIR/test-cases.tsv"

# Keep compilation and Cargo lock acquisition outside the timed cases.
cargo test -p pi-tui --lib --no-run >"$OUT_DIR/warmup-pi-tui.log" 2>&1
cargo test -p pi-coding-agent --lib --no-run >"$OUT_DIR/warmup-pi-coding-agent.log" 2>&1

run_case() {
  local name="$1"
  shift
  local timing_file="$OUT_DIR/$name.time"
  local output_file="$OUT_DIR/$name.log"

  echo "+ $name: $*"
  /usr/bin/time -f '%e\t%M' -o "$timing_file" "$@" >"$output_file" 2>&1
  printf '%s\t%s\n' "$name" "$(cat "$timing_file")" >>"$OUT_DIR/test-cases.tsv"
}

run_case terminal_lifecycle \
  cargo test -p pi-tui terminal::lifecycle --lib
run_case progress_owner \
  cargo test -p pi-coding-agent --lib \
  adapters::interactive::r#loop::tests::terminal_progress_transitions_through_the_owned_terminal
run_case interactive_render \
  cargo test -p pi-coding-agent --lib adapters::interactive::render::tests
run_case interactive_prompt_task \
  cargo test -p pi-coding-agent --lib adapters::interactive::prompt_task::tests
run_case fullscreen_1k_blocks \
  cargo test -p pi-coding-agent --lib \
  adapters::interactive::app::tests::fullscreen_1k_block_runtime_baseline -- --nocapture
run_case fullscreen_10k_blocks \
  cargo test -p pi-coding-agent --lib \
  adapters::interactive::app::tests::fullscreen_10k_block_runtime_baseline -- --nocapture

echo "TUI runtime baseline written to $OUT_DIR"
