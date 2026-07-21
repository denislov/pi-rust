#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

ITERATIONS="${PI_RUST_OPERATION_TREE_FAULT_SOAK_ITERATIONS:-50}"
CASE_TIMEOUT="${PI_RUST_OPERATION_TREE_FAULT_CASE_TIMEOUT_SECONDS:-180}"
for value in "$ITERATIONS" "$CASE_TIMEOUT"; do
  case "$value" in
    ''|*[!0-9]*|0)
      echo "operation-tree soak iterations and case timeout must be positive integers" >&2
      exit 2
      ;;
  esac
done

run_case() {
  local name="$1"
  local status
  shift
  echo "+ case=$name"
  if command -v timeout >/dev/null 2>&1; then
    if timeout "$CASE_TIMEOUT" "$@"; then
      return 0
    else
      status=$?
      echo "operation-tree fault case failed: case=$name status=$status timeout_seconds=$CASE_TIMEOUT" >&2
      return "$status"
    fi
  elif "$@"; then
    return 0
  else
    status=$?
    echo "operation-tree fault case failed: case=$name status=$status" >&2
    return "$status"
  fi
}

echo "Operation-tree fault soak schedule=0.5.5-operation-tree-v1 iterations=$ITERATIONS case_timeout_seconds=$CASE_TIMEOUT"
for ((iteration = 1; iteration <= ITERATIONS; iteration++)); do
  echo "+ iteration $iteration/$ITERATIONS"
  run_case authorization-drop \
    cargo test -q -p pi-coding-agent --lib --all-features \
    dropped_authorization_future_clears_the_waiter_registry
  run_case delegation-wait-abort \
    cargo test -q -p pi-coding-agent --lib --all-features \
    delegation_wait_abort_resolves_once_without_leaking_the_waiter
  run_case child-authorization-abort \
    cargo test -q -p pi-coding-agent --test rpc --all-features \
    rpc_parent_abort_cancels_pending_child_tool_authorization
  run_case authorization-race \
    cargo test -q -p pi-coding-agent --lib --all-features \
    _wins_before_
  run_case completion-abort-arbitration \
    cargo test -q -p pi-coding-agent --lib --all-features \
    cancellation_gate_arbitrates_commit_against_abort
  run_case shutdown-tree-cancellation \
    cargo test -q -p pi-coding-agent --lib --all-features \
    shutdown_cancellation_reaches_every_open_root_and_child_once
  run_case shutdown-authorization-wait \
    cargo test -q -p pi-coding-agent --lib --all-features \
    runtime_phase_a_shutdown_resolves_pending_authorization
  run_case stalled-child-cancellation \
    cargo test -q -p pi-coding-agent --test operation --all-features \
    parent_abort_drops_stalled_child_stream_and_prevents_late_continuation
  run_case child-partial-provider-failure \
    cargo test -q -p pi-coding-agent --test operation --all-features \
    child_partial_then_error_stays_child_scoped_and_parent_receives_one_failure
  run_case provider-terminal-and-truncation \
    cargo test -q -p pi-agent-core --test agent --all-features \
    loop_runtime::
  run_case delegation-terminal-persistence \
    cargo test -q -p pi-coding-agent --lib --all-features \
    automatic_delegation_terminal_write_distinguishes_definite_failure_and_uncertainty
  run_case retained-gap-reconnect \
    cargo test -q -p pi-coding-agent --lib --all-features \
    retained_gap_handoff_installs_fresh_snapshot_cursor
  run_case child-projection-pressure \
    cargo test -q -p pi-coding-agent --lib --all-features \
    child_event_pressure_evicts_oldest_conversation_and_oldest_events
  run_case active-child-render-pressure \
    cargo test -q -p pi-coding-agent --lib --all-features \
    child_page_render_is_bounded_for_zero_narrow_unicode_and_large_tool_output
done

echo "Operation-tree fault soak passed schedule=0.5.5-operation-tree-v1 iterations=$ITERATIONS"
