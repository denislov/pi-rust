#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FIXTURE="$ROOT/tools/architecture-prototypes/typescript-component"
OUTPUT="$ROOT/target/architecture-prototypes/typescript-component"

mkdir -p "$OUTPUT"

npm --prefix "$FIXTURE" run typecheck
"$FIXTURE/node_modules/.bin/tsc" \
  --project "$FIXTURE/tsconfig.json" \
  --noEmit false \
  --outDir "$OUTPUT"
"$FIXTURE/node_modules/.bin/jco" componentize \
  "$OUTPUT/extension.js" \
  --wit "$FIXTURE/prototype.wit" \
  --world-name extension \
  -o "$OUTPUT/extension.wasm"
"$FIXTURE/node_modules/.bin/jco" wit "$OUTPUT/extension.wasm" > "$OUTPUT/embedded.wit"

grep -F 'export greet: func(name: string) -> string;' "$OUTPUT/embedded.wit" >/dev/null
test -s "$OUTPUT/extension.wasm"

echo "TypeScript/WIT component prototype passed: $OUTPUT/extension.wasm"
