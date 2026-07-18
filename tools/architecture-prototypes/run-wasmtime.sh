#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
export CARGO_TARGET_DIR="$ROOT/target/architecture-prototypes/wasmtime"

cargo test --manifest-path \
  "$ROOT/tools/architecture-prototypes/wasmtime-harness/Cargo.toml"
