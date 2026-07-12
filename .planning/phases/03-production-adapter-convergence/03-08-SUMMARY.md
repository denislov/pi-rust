---
phase: 03-production-adapter-convergence
plan: 08
subsystem: runtime-error-and-test-boundary
tags: [rust, partial-commit, persistence, fault-injection, delegation, boundary-guards, tdd, generic-agent-workaround]

requires:
  - phase: 03-production-adapter-convergence
    plan: 07
    provides: Owner-preserving interactive task completion and restore-before-projection behavior
  - phase: 02-canonical-facade-correctness
    plan: 03
    provides: Durable PartialCommit operation identity and closed CodingAgentSession method ledger
provides:
  - Lossless CodingSessionError::PartialCommit conversion into a structured CliError variant
  - Two specialized cfg(test) owner methods for AppendEvents and UpdateManifest failure injection
  - One specialized cfg(test) owner method that persists a real default-agent pending delegation fixture
  - Source guards proving the fixture bridge remains crate-only, test-only, and outside the stable facade
affects: [03-09, interactive-uat, adapter-errors, session-durability]

tech-stack:
  added: []
  patterns:
    - "Lossless adapter error conversion: durable uncertainty retains classification, operation ID, message, and canonical Display text."
    - "Owner-local test seams: specialized cfg(test) pub(crate) methods delegate to private persistence services without exposing selectors or service types."

key-files:
  created:
    - .planning/phases/03-production-adapter-convergence/03-08-SUMMARY.md
  modified:
    - crates/pi-coding-agent/src/error.rs
    - crates/pi-coding-agent/src/coding_session/error.rs
    - crates/pi-coding-agent/src/coding_session/mod.rs
    - crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs

key-decisions:
  - "Represent PartialCommit as a structured CliError variant instead of embedding operation identity in SessionFailure text."
  - "Expose exactly two action-specific store-failure methods and one pending-delegation fixture method; keep StoreFailurePoint and all persistence/delegation internals private."
  - "Build the pending fixture with a real runtime-backed PromptTurnOptions value and persist it through DelegationConfirmationService with persist=true."

patterns-established:
  - "Adapter-visible durable errors preserve machine-matchable identity as well as unchanged user-visible text."
  - "Cross-module crate tests obtain privileged persistence setup only through specialized owner methods directly gated by cfg(test)."

requirements-completed: [INTER-01, INTER-02, INTER-03, INTER-04]

coverage:
  - id: D1
    description: "PartialCommit conversion retains its exact operation ID, message, classification, and canonical display text while non-partial mappings remain unchanged."
    requirement: INTER-01
    verification:
      - kind: unit
        ref: "crates/pi-coding-agent/src/coding_session/error.rs#partial_commit_conversion_preserves_operation_identity"
        status: pass
      - kind: unit
        ref: "crates/pi-coding-agent/src/coding_session/error.rs#non_partial_conversion_contract_remains_unchanged"
        status: pass
    human_judgment: false
  - id: D2
    description: "Interactive crate tests can arm real AppendEvents and UpdateManifest faults and persist a real pending delegation through CodingAgentSession without importing internal selectors or services."
    requirement: INTER-02
    verification:
      - kind: unit
        ref: "crates/pi-coding-agent/src/coding_session/mod.rs#interactive_store_and_pending_delegation_bridge_arms_real_fixtures"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs#session_store_failure_controls_remain_test_only"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs#canonical_operation_facade_has_no_new_workflow_wrappers"
        status: pass
    human_judgment: false

duration: 17min
completed: 2026-07-12
status: complete
execution_adapter: generic-agent-workaround
---

# Phase 03 Plan 08: Partial Commit and Interactive Test Boundary Summary

**Structured partial-commit identity now survives the CLI adapter boundary, and interactive unit tests have a narrowly guarded owner-local bridge to real persistence faults and pending delegation state.**

## Performance

- **Duration:** 17 min
- **Started:** 2026-07-12T13:40:28Z
- **Completed:** 2026-07-12T13:57:53Z
- **Tasks:** 2
- **Files modified:** 4 production/test files
- **Execution:** Generic-agent workaround for unavailable typed `gsd-executor` dispatch

## Accomplishments

- Added `CliError::PartialCommit { operation_id, message }` and changed only the corresponding `CodingSessionError` conversion branch, preserving the canonical visible text and all non-partial mappings.
- Added two directly `#[cfg(test)] pub(crate)` owner methods that fix the fault selection to `AppendEvents` or `UpdateManifest` while accepting only the successful-call count.
- Added one directly `#[cfg(test)] pub(crate)` owner method that constructs a runtime-backed default-agent pending delegation and persists it through the real confirmation service.
- Extended structural guards so all three methods are required exactly once, directly test-gated, classified in the closed method ledger, and absent from the stable `api` facade.

## Task Commits

TDD tasks produced explicit RED and GREEN commits:

1. **Task 1 RED: partial-commit conversion contract** - `f11d17b` (test)
2. **Task 1 GREEN: structured CliError partial commit** - `bdf8fdc` (fix)
3. **Task 2 RED: interactive owner fixture bridge contract** - `2cba965` (test)
4. **Task 2 GREEN: specialized persistent fixture bridge** - `98af2c2` (feat)

## Files Created/Modified

- `crates/pi-coding-agent/src/error.rs` - Defines the structured adapter-visible `PartialCommit` variant.
- `crates/pi-coding-agent/src/coding_session/error.rs` - Performs lossless conversion and locks partial/non-partial contracts with focused tests.
- `crates/pi-coding-agent/src/coding_session/mod.rs` - Adds the three specialized test-only owner methods and a real persistence/reopen fixture test.
- `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` - Enforces direct test gating, crate visibility, closed-ledger membership, and stable-facade exclusion.

## Decisions Made

- Kept `CliError::SessionFailure` unchanged for every existing non-partial case; durable uncertainty alone receives a structured adapter variant.
- Used action-specific method names instead of a generic fault selector, preventing `StoreFailurePoint` from crossing the `coding_session` ownership boundary.
- Reused the same semantic pending state as existing canonical delegation tests: default agent target, runtime-backed prompt options, and real durable queue persistence.

## Deviations from Plan

None - plan executed within the specified production and test boundary. No new dependency, production fault hook, public API export, Stage 10 event change, compatibility deletion, or unrelated refactor was introduced.

## Issues Encountered

- The first GREEN fixture attempt used default-profile mutation to exercise both failure points. That operation reaches `UpdateManifest` but not `AppendEvents`; the test was corrected to use real pending-delegation rejection, whose persistence path deterministically reaches append followed by manifest update.
- The first pending fixture used a bare `PromptTurnOptions`, which the real confirmation service correctly rejected because it lacked a runtime snapshot. The fixture now builds the same runtime-backed shape used by existing coding-session delegation tests.
- Existing dead-code warnings for compatibility methods and `OperationControl::ensure_idle` remain expected later-phase deletion work; this plan introduced no new warning.

## TDD Gate Compliance

- Task 1 RED failed because `CliError::PartialCommit` did not exist, then passed after `bdf8fdc`.
- Task 2 RED failed because the three owner bridge methods did not exist, then passed after `98af2c2`.
- Git history contains both RED commits before their corresponding GREEN commits.

## Known Stubs

None. No TODO, FIXME, placeholder, mock return, empty production data source, or generic fault-selection hook was added.

## Threat Flags

None. The plan changes no network, authentication, filesystem access policy, external schema, or production privilege surface. T-03-31 is mitigated by structural error preservation; T-03-32 by direct cfg(test) gates and the closed method ledger; T-03-33 by stable-facade exclusion.

## Verification

- `cargo fmt --check` passed.
- `cargo test -p pi-coding-agent --lib coding_session::error::tests -- --nocapture` passed 3 tests.
- `coding_session::tests::interactive_store_and_pending_delegation_bridge_arms_real_fixtures` passed.
- `session_store_failure_controls_remain_test_only` passed.
- `canonical_operation_facade_has_no_new_workflow_wrappers` passed.
- `cargo check -p pi-coding-agent` passed.
- `git diff --check` passed.

## User Setup Required

None - all coverage uses deterministic tempfile persistence and existing offline test infrastructure.

## Next Phase Readiness

- Plan 03-09 can now drive real profile, rejection, prompt, and fork failures through `PromptTask.done` without fabricated completion envelopes.
- Task-level rejection errors can assert structured `CliError::PartialCommit`; prompt failures can retain their existing `PromptTurnOutcome::Failed(CodingSessionError::PartialCommit)` contract.
- No blocker remains for the final Phase 3 UAT gap closure plan.

## Self-Check: PASSED

- All four modified production/test files and this SUMMARY exist.
- Commits `f11d17b`, `bdf8fdc`, `2cba965`, and `98af2c2` exist in repository history in RED/GREEN order.
- No tracked files were deleted.
- The stable facade contains none of the three test bridge methods or internal persistence types.
- All plan-required focused tests and final gates passed.

---
*Phase: 03-production-adapter-convergence*
*Completed: 2026-07-12*
