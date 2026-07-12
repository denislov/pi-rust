---
phase: 05-boundary-enforcement-and-stage-9-closure
plan: 02
subsystem: testing
tags: [rust, cargo, public-api, compile-fail, boundary-guards]
requires:
  - phase: 02-canonical-facade-correctness
    provides: stable operation facade and independent positive signature inventory
  - phase: 04-test-convergence-and-compatibility-deletion
    provides: closed session-owner and internal-contract boundary
provides:
  - deterministic offline external-consumer compile harness
  - explicit positive facade fixture covering all 15 canonical operations and outcome families
  - categorized negative access matrix for internal runtime contracts
affects: [05-03-stage-9-closure, stable-api, boundary-enforcement]
tech-stack:
  added: []
  patterns: [locked external Cargo consumer, compiler-first privacy matrix]
key-files:
  created:
    - crates/pi-coding-agent/tests/fixtures/api_boundary/pass/stable_facade.rs
    - crates/pi-coding-agent/tests/fixtures/api_boundary/fail/*.rs
  modified:
    - crates/pi-coding-agent/tests/api_boundary_guards.rs
key-decisions:
  - "Use a copied workspace Cargo.lock with --offline and an isolated target directory so nested consumer builds are deterministic and cannot access the network."
  - "Classify negative fixtures by rustc error code category (E0432 or E0603) without matching full diagnostics."
patterns-established:
  - "External API boundary: compile a dependent temporary crate instead of inferring visibility from production source exports."
  - "Negative matrix: independently enumerate four contract categories across api, root, and doc-hidden paths."
requirements-completed: [GUARD-03, GUARD-04]
coverage:
  - id: D1
    description: "External consumers compile against the explicit 15-operation stable facade and its outcome/support families."
    requirement: GUARD-03
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/api_boundary_guards.rs#external_consumer_fixtures_enforce_the_stable_facade_boundary"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/public_api.rs#stable_api_signature_closure_is_importable"
        status: pass
    human_judgment: false
  - id: D2
    description: "Internal operation/dispatch, service, plugin registry/options, and Flow contracts fail through api, root, and doc-hidden paths."
    requirement: GUARD-04
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test api_boundary_guards --test public_api -- --nocapture"
        status: pass
    human_judgment: false
duration: 4min
completed: 2026-07-12
status: complete
---

# Phase 5 Plan 2: External Facade Boundary Summary

**A locked offline Cargo consumer now proves both the complete canonical facade and the compiler-enforced privacy of every internal runtime category.**

## Performance

- **Duration:** 4 min
- **Started:** 2026-07-12T19:47:26Z
- **Completed:** 2026-07-12T19:50:10Z
- **Tasks:** 1 TDD task
- **Files modified:** 14

## Accomplishments

- Added an independent positive external consumer that explicitly constructs all 15 `CodingAgentOperation` variants and references every required outcome/support family.
- Added 12 negative fixtures covering operation/dispatch, services, plugin options/registries, and Flow contracts through `api`, crate-root, and doc-hidden paths.
- Added an offline nested-Cargo harness with a copied workspace lockfile, isolated target directory, and stable rustc error-category assertions.

## Task Commits

1. **Task 1 RED: Add external facade fixtures** - `6f94c2b` (test)
2. **Task 1 GREEN: Enforce external API privacy matrix** - `3e81df2` (feat)

## Files Created/Modified

- `crates/pi-coding-agent/tests/api_boundary_guards.rs` - Runs positive and negative fixtures as external dependent crates.
- `crates/pi-coding-agent/tests/fixtures/api_boundary/pass/stable_facade.rs` - Independent canonical facade inventory.
- `crates/pi-coding-agent/tests/fixtures/api_boundary/fail/*.rs` - Twelve categorized forbidden-access attempts.

## Decisions Made

- Reused the workspace lockfile in the temporary crate so `--offline` resolves the exact already-vendored dependency graph rather than selecting newer uncached transitive versions.
- Asserted only `E0432` (unresolved import) or `E0603` (private item/module), preserving failure meaning without coupling to complete compiler wording.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Nested offline Cargo initially selected an uncached transitive version**
- **Found during:** Task 1 GREEN verification
- **Issue:** A lockfile-free temporary crate selected `time-macros 0.2.29`, which was unavailable offline.
- **Fix:** Copy the repository `Cargo.lock` into the temporary consumer before compilation.
- **Files modified:** `crates/pi-coding-agent/tests/api_boundary_guards.rs`
- **Verification:** The full external fixture matrix and both focused integration-test targets pass offline.
- **Committed in:** `3e81df2`

---

**Total deviations:** 1 auto-fixed (1 Rule 3)
**Impact on plan:** The fix is required for the plan's deterministic offline contract and adds no production behavior.

## Issues Encountered

None unresolved.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for 05-03 final-tree verification and Stage 9 closure reporting. No blockers.

## Self-Check: PASSED

- `cargo test -p pi-coding-agent --test api_boundary_guards --test public_api -- --nocapture`: 29 passed, 0 failed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed.

---
*Phase: 05-boundary-enforcement-and-stage-9-closure*
*Completed: 2026-07-12*
