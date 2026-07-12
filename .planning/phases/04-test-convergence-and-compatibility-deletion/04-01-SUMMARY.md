---
phase: 04-test-convergence-and-compatibility-deletion
plan: 01
subsystem: testing
tags: [rust, canonical-operations, compatibility-deletion, boundary-guards]
requires:
  - phase: 03-production-adapter-convergence
    provides: Production adapters routed through CodingAgentSession::run
provides:
  - G1 agent, team, and export integration tests routed through typed public operations
  - Receiver-aware absence guard for deleted G1 session methods
  - Removal of invoke_agent, invoke_team, export_current, and export_current_html
affects: [04-02, 04-03, 04-04, phase-05-hardening]
tech-stack:
  added: []
  patterns:
    - Explicit CodingAgentOperation construction with exhaustive typed outcome extraction
    - Sanitized source scan for receiver-aware compatibility absence
key-files:
  created:
    - .planning/phases/04-test-convergence-and-compatibility-deletion/04-01-SUMMARY.md
  modified:
    - crates/pi-coding-agent/src/coding_session/mod.rs
    - crates/pi-coding-agent/tests/agent_invocation.rs
    - crates/pi-coding-agent/tests/agent_team_flow.rs
    - crates/pi-coding-agent/tests/delegation_execution.rs
    - crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs
    - .planning/phases/04-test-convergence-and-compatibility-deletion/04-VALIDATION.md
key-decisions:
  - "G1 integration tests call CodingAgentSession::run with visible typed operations; helpers only extract outcomes."
  - "The boundary guard separates retained method presence from receiver-aware absence of deleted G1 methods."
requirements-completed: [TEST-01, TEST-02, DELETE-01, DELETE-02, DELETE-03, DELETE-04]
coverage:
  - id: D1
    description: "Agent, team, delegation, and export behavior tests use the canonical typed operation dispatcher."
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test agent_invocation --test agent_team_flow --test delegation_execution -- --nocapture"
        status: pass
    human_judgment: false
  - id: D2
    description: "G1 compatibility definitions and receiver calls are absent while retained session contracts remain guarded."
    verification:
      - kind: other
        ref: "cargo test -p pi-coding-agent --test product_runtime_boundary_guards --test api_boundary_guards -- --nocapture"
        status: pass
      - kind: other
        ref: "cargo check -p pi-coding-agent"
        status: pass
    human_judgment: false
metrics:
  duration: 0 min
  completed: 2026-07-13
status: complete
---

# Phase 04 Plan 01: G1 Test Convergence and Compatibility Deletion Summary

**Agent, team, and export behavior now enters the canonical typed dispatcher, with receiver-aware proof and deletion of four obsolete live-session wrappers.**

## Performance

- **Duration:** approximately 10 min
- **Started:** 2026-07-13T00:00:00Z (executor start timestamp unavailable)
- **Completed:** 2026-07-13
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments

- Migrated agent invocation, team invocation, and export assertions to `CodingAgentSession::run(CodingAgentOperation)` with exact `CodingAgentOperationOutcome` extraction.
- Preserved profile validation, member ordering, output, product events, replay/export, and parent-transcript durability assertions.
- Added receiver-aware absence checks and deleted `invoke_agent`, `invoke_team`, `export_current`, and `export_current_html` without adding renamed wrappers.

## Task Commits

1. **Task 1: Migrate G1 behavior tests** - `d945dcb` (test)
2. **Task 2: Prove zero callers and delete G1 methods** - `0187793` (feat)

## Files Created/Modified

- `crates/pi-coding-agent/tests/agent_invocation.rs` - Canonical agent operation calls and typed extraction.
- `crates/pi-coding-agent/tests/agent_team_flow.rs` - Canonical team/export operation calls and typed extraction.
- `crates/pi-coding-agent/tests/delegation_execution.rs` - Canonical export operation call while retaining folded delegation assertions.
- `crates/pi-coding-agent/src/coding_session/mod.rs` - Owner HTML export tests migrated and four compatibility definitions removed.
- `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` - Retained ledger plus G1 definition/call absence checks.
- `.planning/phases/04-test-convergence-and-compatibility-deletion/04-VALIDATION.md` - Marked 04-01 verification rows green.

## Decisions Made

- G1 tests keep operations visible at each call site; extractors accept only an already-produced typed outcome and do not run sessions.
- The guard scans sanitized Rust source for exact receiver-call patterns, avoiding false positives from distinct `Agent`, service, static, or string-literal names.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Completed missing team outcome extraction**
- **Found during:** Task 1
- **Issue:** One migrated team test still accessed fields on `CodingAgentOperationOutcome` instead of the typed `AgentTeamOutcome`.
- **Fix:** Added the exact `AgentTeam` outcome extractor at that call site.
- **Files modified:** `crates/pi-coding-agent/tests/agent_team_flow.rs`
- **Verification:** All 5 agent-team tests passed.
- **Committed in:** `d945dcb`

**2. [Rule 3 - Blocking] Corrected guard initialization order**
- **Found during:** Task 2
- **Issue:** The new absence checks referenced `violations` before its declaration.
- **Fix:** Moved the absence checks after the guard's violations vector initialization.
- **Files modified:** `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs`
- **Verification:** All 13 product runtime boundary guard tests passed.
- **Committed in:** `0187793`

**Total deviations:** 2 auto-fixed (Rule 3: 2). **Impact:** Both fixes were local compile blockers directly caused by the migration/guard changes; no scope expansion.

## Issues Encountered

- Existing dead-code warnings remain for compatibility methods assigned to later Phase 4 plans; they do not affect this plan's focused tests or crate check.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- G1 is complete and the retained ledger remains active.
- Ready for Plan 04-02, which handles G2 prompt/profile/self-healing/plugin compatibility migration and deletion.

## Self-Check: PASSED

- Summary file exists on disk.
- Task commits `d945dcb` and `0187793` exist in Git history.
- Focused behavior tests, both boundary suites, crate check, format check, and `git diff --check` passed.

---
*Phase: 04-test-convergence-and-compatibility-deletion*
*Completed: 2026-07-13*
