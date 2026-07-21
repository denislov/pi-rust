#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

ITERATIONS="${PI_RUST_TUI_SOAK_ITERATIONS:-20}"
case "$ITERATIONS" in
  ''|*[!0-9]*|0)
    echo "PI_RUST_TUI_SOAK_ITERATIONS must be a positive integer" >&2
    exit 2
    ;;
esac

run_with_deadline() {
  if command -v timeout >/dev/null 2>&1; then
    timeout 180 "$@"
  else
    "$@"
  fi
}

echo "TUI runtime soak schedule=0.5.3-mixed-v1 iterations=$ITERATIONS"
for ((iteration = 1; iteration <= ITERATIONS; iteration++)); do
  echo "+ iteration $iteration/$ITERATIONS"
  run_with_deadline cargo test -q -p pi-coding-agent --lib --all-features \
    adapters::interactive::input::tests
  run_with_deadline cargo test -q -p pi-coding-agent --lib --all-features \
    runtime::client::projection::product_event_projection_tests
  run_with_deadline cargo test -q -p pi-coding-agent --lib --all-features \
    adapters::interactive::r#loop::tests
  run_with_deadline cargo test -q -p pi-coding-agent --lib --all-features \
    adapters::interactive::app::tests::fullscreen_10k_block_runtime_baseline
  run_with_deadline cargo test -q -p pi-tui --test render --all-features tui::
done

echo "TUI runtime soak passed schedule=0.5.3-mixed-v1 iterations=$ITERATIONS"
