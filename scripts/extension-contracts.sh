#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONTRACT_DIR="$ROOT/contracts/extensions/0.1.0"

cd "$CONTRACT_DIR"
sha256sum -c SHA256SUMS
jq -e . manifest-v2.schema.json >/dev/null
jq -e . contributions-v1.schema.json >/dev/null
jq -e . lock-v1.schema.json >/dev/null
jq -e . grant-v1.schema.json >/dev/null
jq -e . activation-v1.schema.json >/dev/null

for forbidden in runtime language source trust grant lease; do
  if jq -e --arg field "$forbidden" '.properties[$field] != null' manifest-v2.schema.json >/dev/null; then
    echo "forbidden manifest field is declared: $forbidden" >&2
    exit 1
  fi
done

cd "$ROOT"
cargo test -p pi-coding-agent --lib extensions:: --quiet

echo "Extension contract candidate checks passed"
