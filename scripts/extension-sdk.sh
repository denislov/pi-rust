#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SDK="$ROOT/sdk/typescript"
WIT="$ROOT/contracts/extensions/0.1.0/extension.wit"
CONTRACTS="$ROOT/contracts/extensions/0.1.0"
FIXTURE="$ROOT/tests/fixtures/extensions/typescript-minimal/src/extension.ts"
OUTPUT="$ROOT/target/extension-sdk"
TOOLCHAIN="$OUTPUT/toolchain"
GENERATED="$OUTPUT/generated"
BUILD="$OUTPUT/build"
BIN="$TOOLCHAIN/node_modules/.bin"

prepare_toolchain() {
  mkdir -p "$TOOLCHAIN"
  cp "$SDK/package.json" "$SDK/package-lock.json" "$TOOLCHAIN/"
  npm ci --offline --ignore-scripts --prefix "$TOOLCHAIN" \
    --cache "${PI_RUST_EXTENSION_NPM_CACHE:-$HOME/.npm}" >/dev/null

  test "$("$BIN/jco" --version)" = "1.25.2"
  test "$("$BIN/tsc" --version)" = "Version 7.0.2"
  test "$("$BIN/esbuild" --version)" = "0.25.12"
  test -x "$BIN/json2ts"
}

build_fixture() {
  prepare_toolchain
  mkdir -p "$GENERATED" "$BUILD"

  "$BIN/jco" guest-types "$WIT" \
    --world-name extension \
    --strict \
    --quiet \
    --out-dir "$GENERATED"

  (
    cd "$CONTRACTS"
    "$BIN/json2ts" --input manifest-v2.schema.json \
      --output "$GENERATED/manifest-v2.d.ts" \
      --bannerComment "/* Generated from contracts/extensions/0.1.0/manifest-v2.schema.json; do not edit. */"
    "$BIN/json2ts" --input contributions-v1.schema.json \
      --output "$GENERATED/contributions-v1.d.ts" \
      --bannerComment "/* Generated from contracts/extensions/0.1.0/contributions-v1.schema.json; do not edit. */"
    "$BIN/json2ts" --input lock-v1.schema.json \
      --output "$GENERATED/lock-v1.d.ts" \
      --bannerComment "/* Generated from contracts/extensions/0.1.0/lock-v1.schema.json; do not edit. */"
    "$BIN/json2ts" --input grant-v1.schema.json \
      --output "$GENERATED/grant-v1.d.ts" \
      --bannerComment "/* Generated from contracts/extensions/0.1.0/grant-v1.schema.json; do not edit. */"
    "$BIN/json2ts" --input activation-v1.schema.json \
      --output "$GENERATED/activation-v1.d.ts" \
      --bannerComment "/* Generated from contracts/extensions/0.1.0/activation-v1.schema.json; do not edit. */"
  )

  mapfile -t generated_types < <(find "$GENERATED" -type f -name '*.d.ts' -print | sort)
  "$BIN/tsc" \
    --noEmit \
    --strict \
    --target ES2022 \
    --module ES2022 \
    --moduleResolution bundler \
    --lib ES2022,DOM \
    "$SDK/src/index.ts" "$FIXTURE" "${generated_types[@]}"

  "$BIN/esbuild" "$FIXTURE" \
    --bundle \
    --format=esm \
    --platform=neutral \
    --target=es2022 \
    --external:pi:extension/* \
    --metafile="$BUILD/esbuild-meta.json" \
    --outfile="$BUILD/extension.js" >/dev/null

  "$BIN/jco" componentize "$BUILD/extension.js" \
    --wit "$WIT" \
    --world-name extension \
    --disable all \
    --out "$BUILD/component.wasm"

  "$BIN/jco" wit "$BUILD/component.wasm" > "$BUILD/embedded.wit"
  sha256sum "$CONTRACTS"/*.json "$WIT" "$SDK/package-lock.json" \
    "$SDK/src/index.ts" "$GENERATED"/*.d.ts "$GENERATED"/interfaces/*.d.ts \
    "$BUILD/extension.js" "$BUILD/component.wasm" > "$BUILD/SHA256SUMS"
}

validate_fixture() {
  test -s "$BUILD/component.wasm"
  test -s "$BUILD/extension.js"
  test -s "$GENERATED/extension.d.ts"
  test -s "$GENERATED/manifest-v2.d.ts"
  test -s "$GENERATED/contributions-v1.d.ts"
  test -s "$GENERATED/lock-v1.d.ts"
  test -s "$GENERATED/grant-v1.d.ts"
  test -s "$GENERATED/activation-v1.d.ts"
  jq -e '[.outputs[].imports[] | select(.external == true) | .path]
    == ["pi:extension/host-ui@0.1.0"]' "$BUILD/esbuild-meta.json" >/dev/null
  jq -e '[.outputs[].imports[] | select(.external != true)] | length == 0' \
    "$BUILD/esbuild-meta.json" >/dev/null
  grep -F 'import pi:extension/host-diagnostics@0.1.0;' "$BUILD/embedded.wit" >/dev/null
  grep -F 'import pi:extension/host-workspace@0.1.0;' "$BUILD/embedded.wit" >/dev/null
  grep -F 'import pi:extension/host-model@0.1.0;' "$BUILD/embedded.wit" >/dev/null
  grep -F 'import pi:extension/host-process@0.1.0;' "$BUILD/embedded.wit" >/dev/null
  grep -F 'import pi:extension/host-ui@0.1.0;' "$BUILD/embedded.wit" >/dev/null
  grep -F 'export pi:extension/guest@0.1.0;' "$BUILD/embedded.wit" >/dev/null
  if grep -F 'wasi:' "$BUILD/embedded.wit" >/dev/null; then
    echo "fixture Component contains forbidden ambient WASI imports" >&2
    exit 1
  fi
  if grep -E '(^|[^[:alnum:]_])(require\s*\(|import\s*\(|eval\s*\(|new[[:space:]]+Function|WebAssembly\.)|\.node([^[:alnum:]_]|$)' \
    "$SDK/src/index.ts" "$FIXTURE" >/dev/null; then
    echo "SDK or fixture uses forbidden dynamic/native loading or runtime code generation" >&2
    exit 1
  fi
  (cd "$ROOT" && sha256sum -c "$BUILD/SHA256SUMS")
}

case "${1:-run-conformance}" in
  build-fixture)
    build_fixture
    ;;
  validate-fixture)
    validate_fixture
    ;;
  run-conformance)
    build_fixture
    validate_fixture
    ;;
  *)
    echo "usage: $0 {build-fixture|validate-fixture|run-conformance}" >&2
    exit 2
    ;;
esac

echo "Extension SDK ${1:-run-conformance} passed: $OUTPUT"
