#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPONENT="$ROOT/target/extension-sdk/build/component.wasm"

bash "$ROOT/scripts/extension-sdk.sh" run-conformance

PI_RUST_EXTENSION_COMPONENT_FIXTURE="$COMPONENT" \
  cargo test --manifest-path "$ROOT/Cargo.toml" \
    -p pi-coding-agent --all-features \
    extensions::runtime::tests::typescript_component_invokes_through_lease_backed_host \
    -- --exact

echo "Extension runtime vertical slice passed: $COMPONENT"
