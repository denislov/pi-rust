---
phase: 09-lifecycle-association-guards-and-closure
plan: 02
subsystem: operation-runtime
tags: [rust, operation-association, terminal-evidence, tdd, fail-closed]

requires:
  - phase: 06-product-event-inventory-and-typed-contract
    provides: typed root-terminal operation kinds and event inventory
  - phase: 09-lifecycle-association-guards-and-closure
    provides: submitted terminal anchor vocabulary from Plan 09-01
provides:
  - exhaustive descriptor for all 15 public operations
  - exact permitted root-terminal evidence for five terminal-associated operations
  - closed-set unit fixture enforcing five TerminalAssociated, ten OutcomeOnly, and zero NotApplicable rows
affects: [09-03, 09-04, operation-finalization, submission-provenance]

tech-stack:
  added: []
  patterns: [exhaustive Rust match as association ledger, branch-specific terminal evidence]

key-files:
  created: []
  modified:
    - crates/pi-coding-agent/src/coding_session/public_operation.rs

key-decisions:
  - "Represent Compact failure with a dedicated CompactPromptFailed evidence variant so ordinary PromptCompleted remains excluded from Compact root association."
  - "Derive TerminalAssociated versus OutcomeOnly from the descriptor's closed permitted-evidence set while retaining NotApplicable as a zero-row guard variant."

patterns-established:
  - "Every CodingAgentOperation variant must return submitted kind, outcome family, association class, and exact permitted root evidence from one exhaustive match."
  - "Compatibility events may satisfy an admitted root only through an explicit branch-specific evidence variant, never generic terminal status."

requirements-completed: [CONTROL-02]

coverage:
  - id: D1
    description: "All 15 public operations are classified exactly once into the closed association ledger."
    requirement: CONTROL-02
    verification:
      - kind: unit
        ref: "crates/pi-coding-agent/src/coding_session/public_operation.rs#association_matrix_classifies_all_public_operations_exactly_once"
        status: pass
      - kind: other
        ref: "cargo test -p pi-coding-agent association_matrix --quiet"
        status: pass
    human_judgment: false
  - id: D2
    description: "Compact root evidence excludes PromptCompleted and admits only CompactionCompleted or the exact failed Compact branch."
    requirement: CONTROL-02
    verification:
      - kind: unit
        ref: "crates/pi-coding-agent/src/coding_session/public_operation.rs#association_matrix_classifies_all_public_operations_exactly_once"
        status: pass
    human_judgment: false

duration: 10min
completed: 2026-07-14
status: complete
---

# Phase 09 Plan 02: Closed Operation Association Descriptor Summary

**A compile-closed 15-operation ledger now binds each public operation to its submitted kind, outcome family, association class, and exact permitted root-terminal evidence.**

## Performance

- **Duration:** 10 min
- **Started:** 2026-07-14T05:02:36Z
- **Completed:** 2026-07-14T05:12:30Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Added one exhaustive descriptor match covering all 15 `CodingAgentOperation` variants without a wildcard or dependence on optional internal `static_kind`.
- Classified exactly five operations as `TerminalAssociated`, ten as `OutcomeOnly`, and retained `NotApplicable` with zero current rows.
- Encoded branch-specific root evidence so Compact success accepts only `CompactionCompleted`, while Compact failure uses a dedicated failed compatibility evidence variant and never admits `PromptCompleted`.
- Extended the existing 15-row contract fixture with exact submitted-kind, outcome-family, association, uniqueness, and cardinality assertions.

## Task Commits

The TDD task was committed atomically by gate:

1. **RED: failing association matrix contract** - `5bd9552` (test)
2. **GREEN: closed operation association descriptor** - `9d90e4a` (feat)

## Files Created/Modified

- `crates/pi-coding-agent/src/coding_session/public_operation.rs` - Adds the executable association descriptor, exact root-evidence vocabulary, and closed 15-row tests.

## Decisions Made

- Used `OperationKind` as the stable admitted submitted kind, with outcome family distinguishing public variants that intentionally share an internal kind, such as delegation decisions and export forms.
- Kept Compact failure normalization explicit as `CompactPromptFailed`; the later runtime validator must additionally require matching operation id, admitted Compact descriptor, and failed Compact outcome branch.
- Kept Abort/Steer/FollowUp outside the operation ledger and introduced no synthetic control rows or wire events.

## TDD Gate Compliance

- **RED:** `5bd9552` added the complete matrix expectations; `cargo test -p pi-coding-agent association_matrix --quiet` failed because the descriptor types and method did not exist.
- **GREEN:** `9d90e4a` implemented the minimal exhaustive descriptor; the focused association test and all colocated public-operation tests pass.
- **REFACTOR:** No separate refactor was necessary after formatting and verification.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Git metadata writes required the workspace's approved elevated Git path; normal commit hooks ran, and no verification was bypassed.
- Existing unrelated compiler warnings remained unchanged and do not affect the focused contract verification.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plans 09-03 and 09-04 can consume `CodingAgentOperation::descriptor()` as the single association authority for generalized submission provenance and exact finalization.
- Runtime cardinality and Compact cancellation fixtures remain intentionally owned by Plan 09-04.
- No blockers; `docs/next stage.md` remains untouched and untracked.

## Self-Check: PASSED

- FOUND: `crates/pi-coding-agent/src/coding_session/public_operation.rs`
- FOUND: `.planning/phases/09-lifecycle-association-guards-and-closure/09-02-SUMMARY.md`
- FOUND commit: `5bd9552`
- FOUND commit: `9d90e4a`
- PASS: `cargo test -p pi-coding-agent association_matrix --quiet`
- PASS: `cargo test -p pi-coding-agent public_operation::tests --lib --quiet`
- PASS: `cargo fmt --check`
- PASS: `git diff --check`

---
*Phase: 09-lifecycle-association-guards-and-closure*
*Completed: 2026-07-14*
