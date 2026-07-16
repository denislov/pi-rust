#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="${PI_RUST_RELEASE_VERSION:-}"
if [[ -z "$VERSION" ]]; then
  VERSION="$(
    cargo metadata --format-version 1 --no-deps \
      | jq -er '
          [.packages[] | select(.source == null) | .version] | unique |
          if length == 1 then .[0] else error("workspace package versions differ") end
        '
  )"
fi
OUT_DIR="${PI_RUST_RELEASE_ARTIFACT_DIR:-$ROOT/target/release-artifacts/$VERSION/public-api}"
BUILD_DIR="${PI_RUST_RELEASE_RUSTDOC_TARGET_DIR:-$ROOT/target/release-rustdoc}"
TOOL_TARGET_DIR="${PI_RUST_RELEASE_TOOL_TARGET_DIR:-$ROOT/target/release-tools}"
TOOL_MANIFEST="$ROOT/tools/public-api-snapshot/Cargo.toml"
BASELINE_DIR="${PI_RUST_API_BASELINE_DIR:-}"
BASELINE_MANIFEST="${PI_RUST_API_BASELINE_MANIFEST:-}"
PACKAGES=(pi-ai pi-agent-core pi-tui pi-coding-agent)

mkdir -p "$OUT_DIR"
rm -f \
  "$OUT_DIR"/*.rustdoc.json \
  "$OUT_DIR"/*.public-api.json \
  "$OUT_DIR/SHA256SUMS" \
  "$OUT_DIR/toolchain.txt" \
  "$OUT_DIR/workspace-metadata.json"

# Rustdoc JSON is still unstable. The release environment selects and validates
# the compiler, and enables only this rustdoc output capability.
for package in "${PACKAGES[@]}"; do
  crate_name="${package//-/_}"
  echo "+ rustdoc JSON: $package"
  RUSTC_BOOTSTRAP=1 cargo rustdoc \
    -p "$package" \
    --lib \
    --all-features \
    --target-dir "$BUILD_DIR" \
    -- \
    -Z unstable-options \
    --output-format json
  cp "$BUILD_DIR/doc/$crate_name.json" "$OUT_DIR/$package.rustdoc.json"
  echo "+ stable public API surface: $package"
  cargo run \
    --quiet \
    --locked \
    --manifest-path "$TOOL_MANIFEST" \
    --target-dir "$TOOL_TARGET_DIR" \
    -- \
    "$package" \
    "$OUT_DIR/$package.rustdoc.json" \
    "$OUT_DIR/$package.public-api.json"
done

cargo metadata --format-version 1 --no-deps > "$OUT_DIR/workspace-metadata.json"
{
  rustc -Vv
  cargo -V
} > "$OUT_DIR/toolchain.txt"
(
  cd "$OUT_DIR"
  sha256sum \
    ./*.public-api.json \
    ./*.rustdoc.json \
    ./toolchain.txt \
    ./workspace-metadata.json \
    | sort > SHA256SUMS
)

if [[ -n "$BASELINE_DIR" ]]; then
  for package in "${PACKAGES[@]}"; do
    baseline="$BASELINE_DIR/$package.public-api.json"
    if [[ ! -f "$baseline" ]]; then
      echo "missing public API baseline: $baseline" >&2
      exit 1
    fi
    echo "+ compare public API baseline: $package"
    diff -u "$baseline" "$OUT_DIR/$package.public-api.json"
  done
fi

if [[ -n "$BASELINE_MANIFEST" ]]; then
  if [[ ! -f "$BASELINE_MANIFEST" ]]; then
    echo "missing public API baseline manifest: $BASELINE_MANIFEST" >&2
    exit 1
  fi
  baseline_manifest="$(cd "$(dirname "$BASELINE_MANIFEST")" && pwd)/$(basename "$BASELINE_MANIFEST")"
  echo "+ verify public API baseline manifest"
  (
    cd "$OUT_DIR"
    sha256sum -c "$baseline_manifest"
  )
fi

echo "Public API release artifacts: $OUT_DIR"
