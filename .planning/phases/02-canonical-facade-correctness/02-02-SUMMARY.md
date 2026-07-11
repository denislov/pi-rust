---
phase: 02-canonical-facade-correctness
plan: 02
subsystem: runtime-contracts
tags: [rust, operation-facade, exhaustive-matches, dispatcher-tests, tdd]

requires:
  - phase: 01-evidence-based-baseline
    provides: Source-backed 15-row operation matrix and identified facade evidence gaps
  - phase: 02-canonical-facade-correctness
    plan: 01
    provides: Stable facade closure and privacy enforcement
provides:
  - Independent test-owned 15-row public-to-private operation and dispatch contract
  - Exhaustive fixture coverage for every private-to-public outcome projection
  - Canonical run behavior proof for Async, SyncReadOnly, and SyncMutable metadata families
affects: [02-03, adapter-migration, test-convergence, compatibility-cleanup]

tech-stack:
  added: []
  patterns: [test-owned contract ledger, exhaustive family classifier, metadata-plus-behavior triangulation]

key-files:
  created: []
  modified:
    - crates/pi-coding-agent/src/coding_session/public_operation.rs
    - crates/pi-coding-agent/src/coding_session/mod.rs
    - crates/pi-coding-agent/src/coding_session/profiles.rs
  deleted:
    - crates/pi-coding-agent/tests/profile_registry.rs

key-decisions:
  - "ExportCurrent and ExportCurrentHtml use distinct test-owned internal expectations, validated against ExportOptions::view and ExportOptions::html, even though both share the private Export variant."
  - "Dispatcher routing evidence combines fixed metadata assertions with public run outcomes instead of adding production counters or hooks."
  - "ProfileRegistry behavior tests belong in the private profiles owner module after Plan 02-01 removed registry ownership types from the stable api facade."

patterns-established:
  - "Operation ledger: every public variant owns fixed expected internal, dispatch, and public outcome families in one explicit 15-row table."
  - "Projection ledger: construct each private OperationOutcome directly and classify the public result through an exhaustive test-only match."
  - "Dispatcher proof: pair metadata assertions with observable run behavior for one representative operation from each dispatch family."

requirements-completed: [FACADE-02, FACADE-03]

coverage:
  - id: D1
    description: "All 15 public operations map to independently expected private variants and metadata dispatch families."
    requirement: FACADE-02
    verification:
      - kind: unit
        ref: "crates/pi-coding-agent/src/coding_session/public_operation.rs#operation_contract_covers_all_public_variants"
        status: pass
      - kind: unit
        ref: "crates/pi-coding-agent/src/coding_session/mod.rs#canonical_run_uses_each_metadata_dispatch_family"
        status: pass
    human_judgment: false
  - id: D2
    description: "Every private operation outcome projects through the exhaustive public mapping, including both Export branches."
    requirement: FACADE-03
    verification:
      - kind: unit
        ref: "crates/pi-coding-agent/src/coding_session/public_operation.rs#operation_outcome_projection_covers_all_families"
        status: pass
      - kind: integration
        ref: "cargo test -p pi-coding-agent"
        status: pass
    human_judgment: false

duration: 25min
completed: 2026-07-11
status: complete
---

# Phase 02 Plan 02: Canonical Facade Correctness Summary

**Independent 15-row operation ownership, exhaustive outcome projection fixtures, and behavior-backed proof of all three metadata-selected dispatch families.**

## Performance

- **Duration:** 25 min
- **Started:** 2026-07-11T06:02:51Z
- **Completed:** 2026-07-11T06:27:58Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Added one explicit test-owned matrix covering all 15 `CodingAgentOperation` variants with fixed private variant, dispatch mode, and public outcome expectations.
- Kept approval/rejection delegation asymmetry visible and treated `ExportCurrent` and `ExportCurrentHtml` as separate rows and separate export option expectations.
- Added direct fixtures for all 15 public outcome families, including both branches of private `OperationOutcome::Export`, and cross-checked them against the operation ledger.
- Added one canonical `CodingAgentSession::run` behavior test that proves Async prompt execution, SyncReadOnly export projection, and SyncMutable profile mutation.
- Restored the Wave 1 crate test gate without exposing private profile registries by moving their behavior tests to the owner module.

## Task Commits

Each TDD task was committed with explicit RED and GREEN gates:

1. **Task 1 RED: independent operation ledger contract** - `a6f5969` (test)
2. **Task 1 GREEN: complete 15-row operation contract** - `793e331` (test)
3. **Task 2 RED: projection and dispatcher proof contracts** - `5f4658d` (test)
4. **Task 2 GREEN: exhaustive projection and dispatcher behavior** - `c2cf275` (test)
5. **Rule 3 blocking fix: owner-scope profile registry tests** - `47f45ad` (fix)

## Files Created/Modified

- `crates/pi-coding-agent/src/coding_session/public_operation.rs` - Adds the independent operation and outcome ledgers plus exhaustive test-only classifiers.
- `crates/pi-coding-agent/src/coding_session/mod.rs` - Adds representative canonical `run()` behavior for all metadata dispatch families.
- `crates/pi-coding-agent/src/coding_session/profiles.rs` - Owns registry behavior tests that require private implementation access.
- `crates/pi-coding-agent/tests/profile_registry.rs` - Removed after its implementation-private imports became intentionally unavailable to downstream callers.

## Decisions Made

- Used function-pointer case builders so each contract row constructs its public operation independently while retaining one fixed expected ledger.
- Classified actual internal and public families with exhaustive test-only matches; production `into_internal`, `metadata`, and `from_internal` matches remain unchanged and wildcard-free.
- Used real public outcomes and state effects for dispatcher evidence rather than testing a production-only instrumentation hook.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Moved profile registry integration tests to the private owner module**
- **Found during:** Task 1 verification and Wave 1 gate
- **Issue:** Plan 02-01 removed `ProfileRegistry` and `ProfileRegistryOptions` from `pi_coding_agent::api`, but the existing downstream integration test still imported them, preventing every non-`--lib` Cargo test command from compiling.
- **Fix:** Preserved all five registry behavior tests under `coding_session::profiles::tests` and removed the invalid downstream test file.
- **Files modified:** `crates/pi-coding-agent/src/coding_session/profiles.rs`, `crates/pi-coding-agent/tests/profile_registry.rs`
- **Verification:** Five owner tests pass and the complete `cargo test -p pi-coding-agent` gate passes.
- **Commit:** `47f45ad`

**Total deviations:** 1 auto-fixed blocking issue.
**Impact:** Restored verification while reinforcing, rather than weakening, the stable API privacy decision from Plan 02-01.

## TDD Gate Compliance

- Task 1 follows `a6f5969` RED -> `793e331` GREEN.
- Task 2 follows `5f4658d` RED -> `c2cf275` GREEN.
- Both RED runs failed before their corresponding fixture/helper implementation was added.

## Issues Encountered

- The first full focused command compiled all integration targets and exposed the stale private-registry imports from Plan 02-01. The owner-only focused test already passed; the blocking integration issue was then fixed and the full gate rerun.

## User Setup Required

None - all tests use deterministic offline fixtures.

## Next Phase Readiness

- FACADE-02 and FACADE-03 now have independent, compiler-sensitive owner evidence and behavior-backed dispatcher proof.
- Plan 02-03 can focus on durable high-risk operation semantics without revisiting facade mapping or dispatch classification.
- No blockers remain.

## Self-Check: PASSED

- All modified owner files exist and the intentionally removed integration test is absent.
- Commits `a6f5969`, `793e331`, `5f4658d`, `c2cf275`, and `47f45ad` exist in repository history.
- All three focused owner tests pass.
- `cargo test -p pi-coding-agent`, `cargo check -p pi-coding-agent`, `cargo fmt --check`, and `git diff --check` pass.
- Source audit found exactly 15 public contract rows and no wildcard arms in the production conversion, metadata, or projection matches.

---
*Phase: 02-canonical-facade-correctness*
*Completed: 2026-07-11*
