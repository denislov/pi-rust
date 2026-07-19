#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

run() {
  echo "+ $*"
  "$@"
}

run cargo fmt --all --check
run scripts/0.4x-plan-gate.sh
run cargo check --workspace --all-targets --all-features
run cargo test -p pi-ai --test api_boundary_guards --all-features
run cargo test -p pi-agent-core --test api_contract --all-features api_boundary_guards
run cargo test -p pi-coding-agent --test api_contract --all-features api_boundary_guards
run cargo test -p pi-coding-agent --test events_snapshot --all-features
run cargo test -p pi-coding-agent --test recovery --all-features session_compatibility_baseline
run cargo test -p pi-coding-agent --test rpc --all-features mode::rpc_hello_negotiates_supported_protocol_families
run cargo test -p pi-coding-agent --test rpc --all-features mode::rpc_hello_rejects_unsupported_major_protocol_version

if [[ "${PI_RUST_ARCH_RELEASE:-0}" == "1" ]]; then
  run scripts/release-api-snapshots.sh
fi

if [[ "${PI_RUST_ARCH_FULL:-0}" == "1" ]]; then
  run cargo clippy --workspace --all-targets --all-features -- -D warnings
  run cargo test --workspace --all-features
fi
