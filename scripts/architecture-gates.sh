#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

run() {
  echo "+ $*"
  "$@"
}

run cargo fmt --all --check
run cargo check --workspace --all-targets
run cargo test -p pi-ai --test api_boundary_guards
run cargo test -p pi-agent-core --test api_boundary_guards
run cargo test -p pi-coding-agent --test api_boundary_guards
run cargo test -p pi-coding-agent --test event_boundary_guards
run cargo test -p pi-coding-agent --test product_event_contract
run cargo test -p pi-coding-agent --test session_compatibility_baseline
run cargo test -p pi-coding-agent --test rpc_mode rpc_hello_negotiates_supported_protocol_families
run cargo test -p pi-coding-agent --test rpc_mode rpc_hello_rejects_unsupported_major_protocol_version

if [[ "${PI_RUST_ARCH_FULL:-0}" == "1" ]]; then
  run cargo clippy --workspace --all-targets --all-features -- -D warnings
  run cargo test --workspace --all-features
fi
