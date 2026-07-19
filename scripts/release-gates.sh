#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

EXPECTED_VERSION="${PI_RUST_EXPECTED_VERSION:-0.4.2}"
API_BASELINE_MANIFEST="${PI_RUST_API_BASELINE_MANIFEST:-$ROOT/docs/api-snapshots/0.4.2/SHA256SUMS}"

run() {
  echo "+ $*"
  "$@"
}

actual_versions="$(
  cargo metadata --format-version 1 --no-deps \
    | jq -r '[.packages[] | select(.source == null) | .version] | unique | join(" ")'
)"
if [[ "$actual_versions" != "$EXPECTED_VERSION" ]]; then
  echo "workspace version mismatch: expected $EXPECTED_VERSION, found $actual_versions" >&2
  exit 1
fi

PI_RUST_ARCH_RELEASE=1 \
PI_RUST_ARCH_FULL=1 \
PI_RUST_API_BASELINE_MANIFEST="$API_BASELINE_MANIFEST" \
PI_RUST_RELEASE_VERSION="$EXPECTED_VERSION" \
  run scripts/architecture-gates.sh

version_output="$(cargo run --quiet -p pi-coding-agent -- --version)"
if [[ "$version_output" != "$EXPECTED_VERSION" ]]; then
  echo "binary version mismatch: expected $EXPECTED_VERSION, found $version_output" >&2
  exit 1
fi

run scripts/tui-smoke.sh
run git diff --check

echo "Release gates passed for $EXPECTED_VERSION"
