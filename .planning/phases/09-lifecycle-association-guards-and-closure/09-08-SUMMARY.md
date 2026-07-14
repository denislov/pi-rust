---
phase: 09-lifecycle-association-guards-and-closure
plan: 08
subsystem: runtime-boundary-closure
tags: [rust, source-guards, cargo-json, lifecycle, security, verification]
requires:
  - phase: 09-lifecycle-association-guards-and-closure
    provides: typed lifecycle, exact terminal association, two-phase shutdown, and adapter projection from Plans 09-01 through 09-07
provides:
  - Recursive structural discovery and exact ownership classification for production adapter candidates
  - Cargo JSON diagnostic-bound external compile fixtures for all 12 forbidden-surface cases
  - Final lifecycle, Compact cancellation, event inventory, association, security, crate, and workspace closure evidence
affects: [runtime-boundary, public-api, adapter-ownership, milestone-v1.1]
tech-stack:
  added: []
  patterns:
    - Production adapter candidates are discovered from source structure before exact ownership classification
    - External compile failures are accepted only when the first rustc error matches the declared code and primary source span
    - Lifecycle and cancellation authority is guarded by reachability and signature audits in addition to behavior tests
key-files:
  created:
    - .planning/phases/09-lifecycle-association-guards-and-closure/09-08-SUMMARY.md
  modified:
    - crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs
    - crates/pi-coding-agent/tests/api_boundary_guards.rs
    - .planning/phases/09-lifecycle-association-guards-and-closure/09-VALIDATION.md
key-decisions:
  - "Discover adapter candidates recursively from operation, connection, event, mode, and output structure, then require an exact three-class ownership ledger with rationale."
  - "Treat only the first rustc error as compile-fail evidence and bind it to an exact code, primary main.rs span, forbidden path/symbol, and fragments."
  - "Keep Compact cancellation crate-private and exact-id scoped while the public shutdown handle exposes only idempotent Phase A request authority."
patterns-established:
  - "Discovery before classification: new structural adapter boundaries fail as unclassified instead of silently escaping a fixed root list."
  - "Diagnostic attribution: incidental nonzero Cargo status cannot satisfy a negative public-boundary fixture."
requirements-completed: [COMPAT-03, CLIENT-04, CONTROL-02, GUARD-01, GUARD-02]
coverage:
  - id: D1
    description: "Every production adapter candidate is recursively discovered and classified exactly once as a canonical caller, state/replay/control consumer, or approved non-runtime boundary."
    requirement: GUARD-01
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test product_runtime_boundary_guards adapter --quiet"
        status: pass
    human_judgment: false
  - id: D2
    description: "All 12 external negative fixtures prove the intended forbidden surface through structured Cargo/rustc diagnostics while the adjacent stable facade compiles."
    requirement: GUARD-02
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test api_boundary_guards external --quiet"
        status: pass
    human_judgment: false
  - id: D3
    description: "Lifecycle, operation association, Compact cancellation, compatibility, source/security, crate, and workspace closure gates all pass without widening public authority."
    requirement: COMPAT-03
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test public_api --test operation_association --test api_boundary_guards --test event_boundary_guards --test product_runtime_boundary_guards --test rpc_mode --test protocol_events --test interactive_mode --quiet"
        status: pass
      - kind: unit
        ref: "cargo test -p pi-coding-agent compact_cancellation --lib --quiet"
        status: pass
      - kind: other
        ref: "cargo test --workspace --quiet && cargo check --workspace && cargo fmt --all --check && git diff --check"
        status: pass
    human_judgment: false
duration: 23 min
completed: 2026-07-14
status: complete
---

# Phase 09 Plan 08: Runtime Boundary Closure Summary

**Production adapters now fail closed through recursive ownership discovery, external compile guards prove exact rustc diagnostics, and every v1.1 lifecycle/security/workspace gate is green.**

## Performance

- **Duration:** 23 min
- **Started:** 2026-07-14T09:28:29Z
- **Completed:** 2026-07-14T09:51:02Z
- **Tasks:** 3
- **Files modified:** 3 implementation/validation files plus this summary

## Accomplishments

- Replaced the fixed interactive/protocol/print root assumption with recursive `src/**/*.rs` discovery based on ordinary-operation, client connection, product-event, mode, and output boundaries.
- Added a 15-row exact ownership ledger with non-empty rationale and fixture coverage proving new siblings, stale rows, duplicate rows, comments, strings, `cfg(test)`, and near-misses behave correctly.
- Strengthened all 12 external compile-fail fixtures to parse Cargo compiler-message JSON and require the intended first E0432/E0603 diagnostic, primary `src/main.rs` coordinates, forbidden path/symbol, and diagnostic fragments.
- Added final source guards proving one canonical ordinary dispatcher, Prompt-scoped control, Phase A-only shutdown authority, and crate-private exact-id Compact cancellation reachability.
- Completed every Phase 9 validation row and the full focused, crate, workspace, format, compile, source/security, and diff closure chain.

## Task Commits

1. **Task 1 RED: add failing adapter ownership discovery cases** - `e651484` (test)
2. **Task 1 GREEN: enforce discovered adapter ownership ledger** - `f8b304c` (test)
3. **Task 2 RED: add failing structured diagnostic matcher case** - `886a10c` (test)
4. **Task 2 GREEN: bind compile fixtures to rustc diagnostics** - `f25f305` (test)
5. **Task 3: close lifecycle authority and milestone gates** - `a6c25b1` (test)

## Files Created/Modified

- `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` - Recursive candidate discovery, exact ownership classification, prohibited-call audits, and narrow lifecycle/Compact authority guards.
- `crates/pi-coding-agent/tests/api_boundary_guards.rs` - Structured rustc diagnostic expectations and stable-facade privacy assertions.
- `.planning/phases/09-lifecycle-association-guards-and-closure/09-VALIDATION.md` - All task rows, Wave 0 requirements, and final blocking evidence marked green from actual commands.
- `.planning/phases/09-lifecycle-association-guards-and-closure/09-08-SUMMARY.md` - Canonical plan completion and verification record.

## Decisions Made

- Runtime-owner files that structurally match adapter signals remain explicit `ApprovedNonRuntimeAdapter` rows; this prevents scanner false positives without excluding ownership boundaries from drift detection.
- Negative fixture evidence uses the first compiler error only. A later matching diagnostic cannot conceal an earlier syntax, dependency, or unrelated privacy failure.
- The Compact cancellation guard audits both visibility and reachability: only `operation_control.rs` and the colocated session owner may name or acquire the private authority.

## TDD Gate Compliance

- **Task 1 RED:** `e651484` failed to compile because discovery/classification contracts did not exist.
- **Task 1 GREEN:** `f8b304c` made recursive discovery, exact classification, and sanitized synthetic sibling fixtures pass.
- **Task 2 RED:** `886a10c` failed to compile because structured diagnostic matching did not exist.
- **Task 2 GREEN:** `f25f305` made all 12 negative cases and the positive stable facade pass with exact Cargo JSON attribution.
- **REFACTOR:** No separate refactor commit was needed; the Task 1 GREEN implementation replaced an initially quadratic production-line scan with one linear pass before the focused gate was committed.

## Deviations from Plan

None - plan behavior and authority scope were implemented exactly as specified.

## Issues Encountered

- The first workspace run was intentionally discarded because a PTY changed `default_prompt_routes_to_interactive_mode` into a real interactive session. The unchanged suite passed in the required non-TTY environment.
- The restricted sandbox denied local socket creation in four `pi-ai` transport contract tests. The unchanged workspace command was rerun with approved local-socket permission and passed completely.
- Existing dead-code and test-only unused-import warnings remain non-blocking and pre-existing.

## User Setup Required

None - no external services or configuration required.

## Next Phase Readiness

- Phase 9 and milestone v1.1 have complete executable evidence for all five assigned requirements and all six high-threat mitigations.
- New adapter roots, public authority leaks, incidental compile failures, event inventory drift, association drift, or workspace regressions now block closure.
- `docs/next stage.md` remains untouched, untracked, and unstaged.

## Self-Check: PASSED

- FOUND all three modified implementation/validation files and this summary.
- FOUND Task commits `e651484`, `f8b304c`, `886a10c`, `f25f305`, and `a6c25b1`.
- PASS: Compact cancellation lib family (3 tests) and all focused lifecycle/association/adapter/security/source/compile suites.
- PASS: `cargo test -p pi-coding-agent --quiet` (663 passed, 1 ignored in the main lib suite; all integration suites green).
- PASS: `cargo test --workspace --quiet` in the required non-TTY, local-socket-capable environment.
- PASS: `cargo check --workspace`, `cargo fmt --all --check`, and `git diff --check`.
- PASS: unrelated `docs/next stage.md` remains untouched and untracked.

---
*Phase: 09-lifecycle-association-guards-and-closure*
*Completed: 2026-07-14*
