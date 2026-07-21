#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

ITERATIONS="${PI_RUST_DELEGATION_SOAK_ITERATIONS:-20}"
case "$ITERATIONS" in
  ''|*[!0-9]*|0)
    echo "PI_RUST_DELEGATION_SOAK_ITERATIONS must be a positive integer" >&2
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

echo "Delegation runtime soak schedule=0.5.4-awaited-child-ui-v1 iterations=$ITERATIONS"
for ((iteration = 1; iteration <= ITERATIONS; iteration++)); do
  echo "+ iteration $iteration/$ITERATIONS"
  run_with_deadline cargo test -q -p pi-coding-agent --test operation --all-features \
    delegation_execution::prompt_executes_approved_agent_delegation_before_parent_continues
  run_with_deadline cargo test -q -p pi-coding-agent --test operation --all-features \
    delegation_execution::recursive_agent_delegation_executes_until_depth_budget_is_exhausted
  run_with_deadline cargo test -q -p pi-coding-agent --test rpc --all-features \
    mode::rpc_lists_and_approves_delegation_confirmation
  run_with_deadline cargo test -q -p pi-coding-agent --test rpc --all-features \
    mode::rpc_child_tool_authorization_is_scoped_to_child_operation
  run_with_deadline cargo test -q -p pi-coding-agent --lib --all-features child_
done

echo "Delegation runtime soak passed schedule=0.5.4-awaited-child-ui-v1 iterations=$ITERATIONS"
