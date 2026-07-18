#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

OUT_DIR="${PI_RUST_BASELINE_DIR:-$ROOT/target/perf-baseline}"
mkdir -p "$OUT_DIR"
OUT="$OUT_DIR/latest.tsv"
printf 'case\telapsed_seconds\tmax_seconds\n' >"$OUT"

run_case() {
  local name="$1"
  local max_seconds="$2"
  shift 2
  local timing
  echo "+ $name: $*"
  timing=$( { /usr/bin/time -f '%e' "$@" >/dev/null; } 2>&1 )
  local elapsed="${timing##*$'\n'}"
  printf '%s\t%s\t%s\n' "$name" "$elapsed" "$max_seconds" >>"$OUT"
  awk -v elapsed="$elapsed" -v limit="$max_seconds" -v name="$name" \
    'BEGIN { if (elapsed > limit) { printf "%s exceeded baseline: %ss > %ss\n", name, elapsed, limit > "/dev/stderr"; exit 1 } }'
}

# Thresholds are intentionally provisional and measure correctness harnesses,
# not optimized release binaries. Keep the cases deterministic and offline.
run_case admission 5 cargo test -p pi-coding-agent --lib runtime::scheduler::tests --quiet
run_case writer_pressure 5 cargo test -p pi-coding-agent --lib session::transaction::tests::bounded_writer_rejects_when_queue_is_saturated --quiet
run_case session_commit_outbox 5 cargo test -p pi-coding-agent --lib session::service::tests::terminal_session_writes_persist_outbox_records --quiet
run_case snapshot_reconnect 5 cargo test -p pi-coding-agent --lib services::event::tests::context_snapshot_survives_eviction_from_the_replay_window --quiet
run_case recovery_scan 5 cargo test -p pi-coding-agent --test recovery --quiet

echo "baseline written to $OUT"
