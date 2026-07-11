---
phase: 01-evidence-based-baseline
plan: 02
subsystem: testing
tags: [audit, evidence, operation-matrix, caller-inventory, compatibility, authority-reconciliation, codegraph, cargo-tests]

# Dependency graph
requires:
  - phase: 01-01
    provides: "Machine-scannable 01-AUDIT.md draft scaffold with 15-row Operation Matrix and validate-audit.sh three-mode validator"
provides:
  - "Fully evidenced 15-row Operation Matrix with live-source mappings, metadata, outcomes, callers, implementation/verification status, and evidence references"
  - "46 registered evidence IDs (SRC, TEST, GUARD, GIT, SCAN) with 11 command ledger entries including 3 focused Cargo commands"
  - "26-row Production Caller Inventory and 32-row Test Caller Inventory distinguishing canonical run() from compatibility paths"
  - "16-row Compatibility Inventory with corrected deprecation status and caller evidence"
  - "4 Authority Reconciliation entries and 8 Findings covering active gaps, obsolete claims, and Stage 10 deferrals"
  - "Fixed validator SIGPIPE and taxonomy validation bugs"
affects: [01-03, phase-2, phase-3, phase-4, phase-5]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Evidence ID namespaces (SRC/TEST/GUARD/GIT/SCAN) linking matrix rows to stable source/test/guard/history references"
    - "Canonical-call-path vs compatibility-path separation: passing dispatcher tests do not imply adapter convergence"
    - "Live-source authority: current tree overrides scaffold assumptions and historical plan narrative"

key-files:
  created:
    - ".planning/phases/01-evidence-based-baseline/01-02-SUMMARY.md"
  modified:
    - ".planning/phases/01-evidence-based-baseline/01-AUDIT.md"
    - ".planning/phases/01-evidence-based-baseline/validate-audit.sh"

key-decisions:
  - "Marked ApproveDelegation as dynamic-kind per operation.rs static_kind=None and intent_router dynamic resolution"
  - "Corrected compatibility inventory: set_default_agent_profile_id, approve_delegation_confirmation, reject_delegation_confirmation are NOT deprecated (scaffold incorrectly marked them as #[deprecated])"
  - "Rated ExportCurrentHtml as verify=not_run with medium confidence due to absence of any focused behavior test"
  - "Fixed validator SIGPIPE bug (set -euo pipefail -> set -uo pipefail) and taxonomy validation logic (grep alternation pattern)"
  - "Routed missing Stage 9 adapter-call/deprecation guards to Phase 5 hardening rather than claiming existing tests cover them"

patterns-established:
  - "Pattern: Evidence ID namespaces (SRC-OP-*, TEST-API-*, TEST-OWNER-*, TEST-INT-*, GUARD-*, GIT-STAGE9-*, SCAN-*) for stable cross-referencing"
  - "Pattern: Separate canonical-call-path evidence from underlying workflow behavior evidence per D-06/D-10"
  - "Pattern: Authority conflict recording (CONFLICT-NN) with explicit resolution per D-18"

requirements-completed: []

# Coverage metadata
coverage:
  - id: D1
    description: "15-row Operation Matrix populated with live-source mappings, metadata, outcomes, callers, implementation/verification status, disposition, confidence, evidence IDs, gaps, and blockers for all 15 public CodingAgentOperation variants"
    requirement: "AUDIT-02"
    verification:
      - kind: other
        ref: "bash .planning/phases/01-evidence-based-baseline/validate-audit.sh --evidence-only"
        status: pass
      - kind: unit
        ref: "cargo test -p pi-coding-agent --test public_api canonical_operation_runtime_variants_are_public -- --nocapture"
        status: pass
      - kind: unit
        ref: "cargo test -p pi-coding-agent --test api_boundary_guards coding_session_run_is_the_canonical_operation_dispatcher -- --nocapture"
        status: pass
    human_judgment: false
  - id: D2
    description: "Production and test caller inventories distinguishing canonical run() from compatibility paths across all JSON, print, RPC, and interactive adapter roots"
    requirement: "AUDIT-02"
    verification:
      - kind: other
        ref: "bash .planning/phases/01-evidence-based-baseline/validate-audit.sh --evidence-only"
        status: pass
    human_judgment: false
  - id: D3
    description: "Compatibility inventory with 16 methods, corrected deprecation status, caller evidence, and excluded non-replacement helpers under DELETE-04"
    requirement: "AUDIT-02"
    verification:
      - kind: other
        ref: "bash .planning/phases/01-evidence-based-baseline/validate-audit.sh --evidence-only"
        status: pass
    human_judgment: false
  - id: D4
    description: "Authority Reconciliation with 4 conflict entries and 8 Findings distinguishing active gaps, obsolete historical claims, and Stage 10 deferrals"
    requirement: "AUDIT-03"
    verification:
      - kind: other
        ref: "bash .planning/phases/01-evidence-based-baseline/validate-audit.sh --evidence-only"
        status: pass
      - kind: other
        ref: "git diff --check && test -z \"$(git diff --name-only -- docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md docs/TODO.md)\""
        status: pass
    human_judgment: true
    rationale: "Semantic quality of authority conflict findings cannot be fully inferred from Markdown structure; verifier must confirm each conflict resolution follows D-17 through D-20"
  - id: D5
    description: "Command Ledger with 3 focused Cargo commands recording positive executed-test counts (1, 1, 7) and 8 source-scan/git commands with dates and exit statuses"
    requirement: "AUDIT-01"
    verification:
      - kind: other
        ref: "bash .planning/phases/01-evidence-based-baseline/validate-audit.sh --evidence-only"
        status: pass
      - kind: unit
        ref: "cargo test -p pi-coding-agent --test product_runtime_boundary_guards -- --nocapture"
        status: pass
    human_judgment: false

# Metrics
duration: 38min
completed: 2026-07-11
status: complete
---

# Phase 1 Plan 2: Evidence Collection Summary

**Populated 15-row Operation Matrix from live source plus 46 evidence IDs, 26 production callers, 32 test callers, 16 compatibility methods, 4 authority conflicts, and 8 findings with validator bug fixes**

## Performance

- **Duration:** 38 min
- **Started:** 2026-07-10T19:05:57Z
- **Completed:** 2026-07-11T01:26:29Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments
- Populated all 15 Operation Matrix rows with live-source mappings, metadata, dispatch modes, outcomes, callers, implementation/verification status, disposition, confidence, evidence IDs, gaps, and blockers from CodeGraph exploration and targeted source inspection
- Registered 46 evidence IDs across 5 namespaces (SRC, TEST, GUARD, GIT, SCAN) and recorded 11 command ledger entries including 3 focused Cargo commands with positive test counts (1, 1, 7)
- Inventoried 26 production caller roots and 32 test caller roots distinguishing canonical run() from compatibility method paths, including #[allow(deprecated)] suppression status
- Corrected compatibility inventory: 3 methods (set_default_agent_profile_id, approve_delegation_confirmation, reject_delegation_confirmation) are NOT deprecated despite scaffold claims; added self_healing_edit as 16th entry
- Populated 4 Authority Reconciliation entries and 8 Findings covering adapter convergence gap, test convergence gap, compatibility deletion, missing deprecation attributes, missing Stage 9 guards, ExportCurrentHtml evidence gap, obsolete historical claims, and Stage 10 event subscription deferral

## Task Commits

Each task was committed atomically:

1. **Task 1: Populate operation matrix and focused evidence ledger** - `72cfcf6` (feat)
2. **Task 2: Inventory production callers, test callers, and compatibility methods** - `15e8191` (feat)
3. **Task 3: Reconcile Stage 9 history only where it changes interpretation** - `6522783` (feat)

## Files Created/Modified
- `.planning/phases/01-evidence-based-baseline/01-AUDIT.md` - Populated all 12 sections: Evidence Index (46 IDs), Command Ledger (11 entries), Operation Matrix (15 rows), Production Caller Inventory (26 rows), Test Caller Inventory (32 rows), Compatibility Inventory (16 rows), Authority Reconciliation (4 entries), Findings (8 entries)
- `.planning/phases/01-evidence-based-baseline/validate-audit.sh` - Fixed SIGPIPE bug (set -euo pipefail -> set -uo pipefail) and taxonomy validation logic (grep pattern was checking value against entire pipe-delimited string instead of as alternation)

## Decisions Made
- Marked ApproveDelegation as dynamic-kind per operation.rs static_kind=None and intent_router dynamic resolution from pending team target (TEST-OWNER-05, TEST-OWNER-06)
- Corrected compatibility inventory deprecation status: set_default_agent_profile_id, approve_delegation_confirmation, reject_delegation_confirmation are pub with NO #[deprecated] attribute (CONFLICT-01)
- Rated ExportCurrentHtml as verify=not_run, conf=medium due to absence of any focused behavior test through any path
- Cited fork/switch persistence, event continuity, and partial-commit evidence (TEST-OWNER-02, TEST-OWNER-03, TEST-OWNER-04) rather than enum presence alone
- Routed missing Stage 9 adapter-call/deprecation guards to Phase 5 hardening (F-GUARD-01) rather than claiming existing product_runtime_boundary_guards tests cover them

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed validator SIGPIPE abort**
- **Found during:** Task 1 (running --evidence-only validator)
- **Issue:** `set -euo pipefail` combined with `grep | head -1` pipelines caused SIGPIPE (exit 141) when grep output exceeded one line, aborting the validator before it could report errors
- **Fix:** Changed `set -euo pipefail` to `set -uo pipefail` (removed `-e`); error tracking via ERRORS array and explicit exit code 1 remains unchanged
- **Files modified:** `.planning/phases/01-evidence-based-baseline/validate-audit.sh`
- **Verification:** Validator now runs to completion and reports errors correctly
- **Committed in:** `72cfcf6` (Task 1 commit)

**2. [Rule 1 - Bug] Fixed validator taxonomy validation logic**
- **Found during:** Task 1 (validator reported all taxonomy values as invalid)
- **Issue:** `echo "$ALLOWED" | grep -qE "^${value}$"` checked if the value matched the entire pipe-delimited string (e.g., `^complete$` against `complete|partial|missing|not_applicable`), which always fails; the bug was hidden in schema mode because scaffold cells were empty
- **Fix:** Changed to `echo "$value" | grep -qE "^($ALLOWED)$"` which checks if the value matches any alternative in the pipe-delimited set; fixed 12 occurrences across schema and evidence validation
- **Files modified:** `.planning/phases/01-evidence-based-baseline/validate-audit.sh`
- **Verification:** All taxonomy values (complete, passed, active, high, not_run, medium, etc.) now validate correctly
- **Committed in:** `72cfcf6` (Task 1 commit)

**3. [Rule 3 - Blocking] Added initial finding F-ADAPT-01 in Task 1**
- **Found during:** Task 1 (evidence-only validator rejected placeholder Findings row)
- **Issue:** The Findings table had a placeholder row `_(populated by Plan 01-02)_` that the evidence-only validator rejects; Findings are assigned to Task 3 but the Task 1 verify requires --evidence-only to pass
- **Fix:** Added F-ADAPT-01 (production adapter convergence gap) as a real finding row in Task 1; expanded to 8 findings in Task 3
- **Files modified:** `.planning/phases/01-evidence-based-baseline/01-AUDIT.md`
- **Verification:** Evidence-only validator passes after adding the finding
- **Committed in:** `72cfcf6` (Task 1 commit)

---

**Total deviations:** 3 auto-fixed (2 bugs, 1 blocking)
**Impact on plan:** All auto-fixes necessary for validator functionality and task completion. No scope creep. No production source, historical plan, or TODO content modified.

## Issues Encountered
None beyond the validator bugs documented above.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Audit evidence collection is complete; ready for Plan 01-03 final closure
- Evidence-only validator passes with all 15 operation rows, 46 evidence IDs, 11 command ledger entries, 8 findings, and 4 authority reconciliation entries
- All 3 focused Cargo commands pass with positive test counts (1, 1, 7)
- No production source, historical plan, TODO, ROADMAP, or STATE file was modified in task commits
- Validator bug fixes (SIGPIPE + taxonomy logic) are the only non-audit-file changes; both are pre-existing bugs in the Plan 01-01 frozen validator that were hidden by empty scaffold cells

---
*Phase: 01-evidence-based-baseline*
*Completed: 2026-07-11*

## Self-Check: PASSED

- FOUND: .planning/phases/01-evidence-based-baseline/01-02-SUMMARY.md
- FOUND: .planning/phases/01-evidence-based-baseline/01-AUDIT.md
- FOUND: .planning/phases/01-evidence-based-baseline/validate-audit.sh
- FOUND: 72cfcf6 (Task 1 commit)
- FOUND: 15e8191 (Task 2 commit)
- FOUND: 6522783 (Task 3 commit)
- FOUND: a0af238 (SUMMARY commit)
- Evidence-only validator: PASS
- No restricted files (production Rust, TODO, historical plan) changed in task commits: PASS
