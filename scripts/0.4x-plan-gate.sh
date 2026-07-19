#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

completed_plans=(
  docs/0.4.0-architecture-extension-platform-plan.md
  docs/0.4.1-agent-workflow-convergence-plan.md
  docs/0.4.2-extension-kernel-replacement-plan.md
)
skipped_plans=(
  docs/0.4.3-extension-services-state-plan.md
  docs/0.4.4-workbench-application-platform-plan.md
  docs/0.4.5-extension-dx-hardening-plan.md
)
all_plans=("${completed_plans[@]}" "${skipped_plans[@]}")

for plan in "${completed_plans[@]}"; do
  head -n 12 "$plan" | rg -qi 'Status:.*\*\*(complete|completed)'
done

for plan in "${skipped_plans[@]}"; do
  head -n 12 "$plan" | rg -qi 'Status:.*\*\*Skipped'
done

if rg -n '\| (Planned|In Progress) \|' "${all_plans[@]}"; then
  echo "0.4.x plan ledger still contains active tasks" >&2
  exit 1
fi

for prefix in ESS WAP DXH; do
  case "$prefix" in
    ESS) plan="${skipped_plans[0]}" ;;
    WAP) plan="${skipped_plans[1]}" ;;
    DXH) plan="${skipped_plans[2]}" ;;
  esac
  if rg "^\| \`(${prefix}|REL-)" "$plan" | rg -v '\| Skipped \|$'; then
    echo "$plan contains a non-Skipped task or debt row" >&2
    exit 1
  fi
done

workspace_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
if [[ "$workspace_version" != "0.4.2" ]]; then
  echo "reduced 0.4.x train must end at workspace version 0.4.2" >&2
  exit 1
fi

changelogs=(CHANGELOG.md crates/*/CHANGELOG.md)
if rg -n '^## 0\.4\.[345]( |$)' "${changelogs[@]}"; then
  echo "Skipped 0.4.3-0.4.5 plans must not create release changelog entries" >&2
  exit 1
fi

rg -q 'three published version completion records and three reviewed Skip records' \
  docs/0.4.x-architecture-extension-platform-roadmap.md

echo "0.4.x plan gate passed: 0.4.0-0.4.2 complete; 0.4.3-0.4.5 skipped"
