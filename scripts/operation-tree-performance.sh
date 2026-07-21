#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

command -v /usr/bin/time >/dev/null || {
  echo "/usr/bin/time is required for the operation-tree baseline" >&2
  exit 1
}

OUT_DIR="${PI_RUST_OPERATION_TREE_BASELINE_DIR:-$ROOT/target/perf-baseline/0.5.5-operation-tree}"
MAX_CASE_SECONDS="${PI_RUST_OPERATION_TREE_MAX_CASE_SECONDS:-10}"
MAX_CASE_RSS_KIB="${PI_RUST_OPERATION_TREE_MAX_CASE_RSS_KIB:-1572864}"
mkdir -p "$OUT_DIR"

rustc --version --verbose >"$OUT_DIR/rustc.txt"
cargo --version >"$OUT_DIR/cargo.txt"
git rev-parse HEAD >"$OUT_DIR/source-commit.txt"
git status --short >"$OUT_DIR/worktree-status.txt"
printf 'case\telapsed_seconds\tmax_rss_kib\tmax_seconds\tmax_rss_kib_budget\n' \
  >"$OUT_DIR/operation-tree-cases.tsv"

# Keep compilation and Cargo lock acquisition outside the timed cases.
cargo test -p pi-coding-agent --lib --all-features --no-run \
  >"$OUT_DIR/warmup-lib.log" 2>&1
cargo test -p pi-coding-agent --test operation --all-features --no-run \
  >"$OUT_DIR/warmup-operation.log" 2>&1

run_case() {
  local name="$1"
  shift
  local timing_file="$OUT_DIR/$name.time"
  local output_file="$OUT_DIR/$name.log"
  local elapsed
  local rss

  echo "+ performance-case=$name"
  /usr/bin/time -f '%e\t%M' -o "$timing_file" "$@" >"$output_file" 2>&1
  read -r elapsed rss <"$timing_file"
  printf '%s\t%s\t%s\t%s\t%s\n' \
    "$name" "$elapsed" "$rss" "$MAX_CASE_SECONDS" "$MAX_CASE_RSS_KIB" \
    >>"$OUT_DIR/operation-tree-cases.tsv"
  awk -v value="$elapsed" -v budget="$MAX_CASE_SECONDS" \
    'BEGIN { exit !(value <= budget) }' || {
      echo "operation-tree performance time budget exceeded: case=$name elapsed=$elapsed budget=$MAX_CASE_SECONDS" >&2
      exit 1
    }
  if ((rss > MAX_CASE_RSS_KIB)); then
    echo "operation-tree performance RSS budget exceeded: case=$name rss_kib=$rss budget_kib=$MAX_CASE_RSS_KIB" >&2
    exit 1
  fi
}

run_case child_event_100k \
  cargo test -q -p pi-coding-agent --lib --all-features \
  child_event_100k_runtime_baseline -- --nocapture
run_case child_transcript_10k \
  cargo test -q -p pi-coding-agent --lib --all-features \
  child_transcript_10k_runtime_baseline -- --nocapture
run_case reconnect_128_event_rebuild \
  cargo test -q -p pi-coding-agent --lib --all-features \
  reconnect_128_event_rebuild_runtime_baseline -- --nocapture
run_case stalled_child_cancellation \
  cargo test -q -p pi-coding-agent --test operation --all-features \
  parent_abort_drops_stalled_child_stream_and_prevents_late_continuation -- --nocapture
run_case recovery_1k_operation_scan \
  cargo test -q -p pi-coding-agent --lib --all-features \
  recovery_1k_operation_scan_runtime_baseline -- --nocapture

echo "Operation-tree performance baseline passed output=$OUT_DIR"
