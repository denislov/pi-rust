---
phase: 02-canonical-facade-correctness
plan: 01
subsystem: api
tags: [rust, public-api, facade, boundary-tests]

requires:
  - phase: 01-evidence-based-baseline
    provides: Source-backed operation inventory and facade gap evidence
provides:
  - Facade-only signature closure covering all 15 public operations and live-session contracts
  - Exact stable-facade privacy guard for internal runtime contracts
  - Removal of profile registry implementation types from the stable facade
affects: [02-02, 02-03, adapter-migration, compatibility-cleanup]

tech-stack:
  added: []
  patterns: [facade-only downstream compilation, exact-identifier source guards]

key-files:
  created: []
  modified:
    - crates/pi-coding-agent/src/lib.rs
    - crates/pi-coding-agent/tests/public_api.rs
    - crates/pi-coding-agent/tests/api_boundary_guards.rs

key-decisions:
  - "The existing stable facade already closed the positive caller-facing signature graph, so Task 1 added evidence without widening production exports."
  - "ProfileRegistry and ProfileRegistryOptions are implementation-owned registries and were removed from pi_coding_agent::api."

patterns-established:
  - "Stable facade closure: name operation, outcome, lifecycle, observation, subscription, and control contracts from one explicit api import block."
  - "Negative facade guard: isolate the balanced api module body and compare exact identifiers to avoid Operation/CodingAgentOperation substring false positives."

requirements-completed: [FACADE-01, FACADE-04]

coverage:
  - id: D1
    description: "Downstream callers can name all 15 operation variants and the complete live-session signature closure through pi_coding_agent::api alone."
    requirement: FACADE-01
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/public_api.rs#stable_api_signature_closure_is_importable"
        status: pass
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test public_api -- --nocapture"
        status: pass
    human_judgment: false
  - id: D2
    description: "Internal operation metadata, plugin options, services, registries, and Flow contracts are excluded from the stable facade while compatibility exports remain intact."
    requirement: FACADE-04
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/api_boundary_guards.rs#stable_api_excludes_internal_runtime_contracts"
        status: pass
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test api_boundary_guards -- --nocapture"
        status: pass
    human_judgment: false

duration: 8min
completed: 2026-07-11
status: complete
---

# Phase 02 Plan 01: Canonical Facade Correctness Summary

**Complete facade-only signature evidence plus exact privacy enforcement that removes registry implementation types without disturbing crate-root compatibility.**

## Performance

- **Duration:** 8 min
- **Started:** 2026-07-11T05:47:18Z
- **Completed:** 2026-07-11T05:55:26Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Added a downstream signature-closure ledger that constructs all 15 `CodingAgentOperation` variants and names every operation outcome, lifecycle, observation, subscription, profile, and delegation contract from `pi_coding_agent::api`.
- Added a balanced-module, exact-identifier privacy guard covering internal operations, metadata, dispatch modes, raw plugin options, services, registries, and Flow contracts.
- Removed `ProfileRegistry` and `ProfileRegistryOptions` from the stable facade while preserving the existing crate-root compatibility surface and migration annotations.

## Task Commits

Each task was committed atomically:

1. **Task 1: Prove and close the stable facade signature graph** - `86e12ea` (test)
2. **Task 2: Enforce stable facade privacy without widening production visibility** - `d98bcb1` (fix)

## Files Created/Modified

- `crates/pi-coding-agent/src/lib.rs` - Keeps implementation-owned profile registries outside the stable facade.
- `crates/pi-coding-agent/tests/public_api.rs` - Provides the facade-only signature closure and 15-variant ledger.
- `crates/pi-coding-agent/tests/api_boundary_guards.rs` - Enforces exact internal-contract exclusions within the isolated `api` module body.

## Decisions Made

- Kept Task 1 test-only because the new closure ledger compiled against the existing facade without any missing caller-facing exports.
- Treated profile registries as private runtime ownership rather than caller-facing query contracts; callers use `agent_profiles`, `team_profiles`, and `profile_diagnostics` projections instead.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The Task 1 RED test passed immediately, confirming the positive facade closure was already complete. Execution continued with the planned evidence-only result and no unnecessary production expansion.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- The public/private facade boundary is compiler-visible and source-guarded for Plan 02-02 contract-matrix work.
- No blockers remain for the remaining Phase 2 plans.

## Self-Check: PASSED

- All three modified files exist.
- Task commits `86e12ea` and `d98bcb1` exist in repository history.
- `public_api` passed 23/23 tests; `api_boundary_guards` passed 5/5 tests.
- `cargo fmt --check` and `git diff --check` passed.

---
*Phase: 02-canonical-facade-correctness*
*Completed: 2026-07-11*
