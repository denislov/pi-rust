---
phase: 09-lifecycle-association-guards-and-closure
plan: 01
subsystem: public-api
tags: [rust, serde, lifecycle, terminal-association, boundary-guards]

requires:
  - phase: 08-client-connection-replay-and-scoped-control
    provides: generation-scoped public connections, submitted state, replay, acknowledgement, and scoped control
provides:
  - Compile-ready typed detach, shutdown, lifecycle rejection, and terminal-anchor values
  - Stable authority-free Serde shapes and lifecycle rejection codes
  - Fail-closed facade guard for lifecycle authority and opaque acknowledgement construction
affects: [09-03-detach, 09-04-operation-association, 09-05-shutdown, 09-06-rpc-lifecycle]

tech-stack:
  added: []
  patterns:
    - Curated public lifecycle values exported only through pi_coding_agent::api
    - Tagged terminal-anchor serialization with separate event, outcome, and uncertainty evidence

key-files:
  created: []
  modified:
    - crates/pi-coding-agent/src/coding_session/public_projection.rs
    - crates/pi-coding-agent/src/coding_session/error.rs
    - crates/pi-coding-agent/src/coding_session/mod.rs
    - crates/pi-coding-agent/src/lib.rs
    - crates/pi-coding-agent/src/tools/edit.rs
    - crates/pi-coding-agent/tests/public_api.rs
    - crates/pi-coding-agent/tests/api_boundary_guards.rs

key-decisions:
  - "Represent submitted terminal evidence as an exhaustive tagged anchor: ProductEvent, OutcomeOnly, or TerminalUncertain."
  - "Expose outcome acknowledgement identity as an opaque string-backed value with no public constructor or embedded generation/signature authority."
  - "Use a curated Durable/Uncertain projection instead of leaking session ids or pending-write implementation details."

patterns-established:
  - "Lifecycle value contract: explicit snake_case Serde values and explicit stable rejection code mappings."
  - "Authority boundary guard: require positive adjacent facade exports while rejecting coordinator, transport, selector, queue, and dispatcher symbols."

requirements-completed: [CLIENT-04, CONTROL-02]

coverage:
  - id: D1
    description: "Compile-ready lifecycle outcomes, rejection categories, and submitted terminal anchors are externally importable and exhaustive."
    requirement: CLIENT-04
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test public_api lifecycle_values --quiet"
        status: pass
    human_judgment: false
  - id: D2
    description: "Lifecycle serialization and the stable facade omit private coordinator, generation, receipt, transport, and dispatch authority."
    requirement: CONTROL-02
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test public_api lifecycle_serialization --quiet"
        status: pass
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test api_boundary_guards public_lifecycle_values --quiet"
        status: pass
    human_judgment: false

duration: 58 min
completed: 2026-07-14
status: complete
---

# Phase 09 Plan 01: Compile-Ready Lifecycle and Terminal Value Contracts Summary

**Typed detach, shutdown, lifecycle rejection, and event/outcome/uncertain terminal anchors now compile through the stable facade with deterministic authority-free serialization.**

## Performance

- **Duration:** 58 min
- **Started:** 2026-07-14T03:55:18Z
- **Completed:** 2026-07-14T04:54:00Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments

- Published exhaustive detach and shutdown outcomes plus typed post-lifecycle rejection categories with stable codes.
- Added submitted terminal anchors that distinguish exact product-event sequence/durability evidence, opaque OutcomeOnly acknowledgement identity, and TerminalUncertain recovery.
- Added external facade/Serde coverage and a fail-closed source guard that rejects internal authority, arbitrary selectors, transport primitives, Debug-derived codes, and a connection dispatcher.

## Task Commits

Each task was committed atomically:

1. **Task 1: Install compile-ready lifecycle and terminal-anchor values** - `c660f44` (feat)
2. **Task 2: Freeze facade authority and serialization privacy** - `743c20c` (test)

## Files Created/Modified

- `crates/pi-coding-agent/src/coding_session/public_projection.rs` - Defines curated lifecycle outcomes, opaque outcome acknowledgement, durability projection, and terminal anchors.
- `crates/pi-coding-agent/src/coding_session/error.rs` - Defines typed lifecycle rejection values and explicit stable codes.
- `crates/pi-coding-agent/src/coding_session/mod.rs` - Re-exports the curated values from the internal module boundary.
- `crates/pi-coding-agent/src/lib.rs` - Adds only the intended lifecycle contracts to `pi_coding_agent::api`.
- `crates/pi-coding-agent/src/tools/edit.rs` - Keeps the existing exhaustive error-to-tool-message projection compile-complete.
- `crates/pi-coding-agent/tests/public_api.rs` - Verifies external imports, exhaustive values, exact Serde shapes, and omission of private authority.
- `crates/pi-coding-agent/tests/api_boundary_guards.rs` - Enforces the positive lifecycle export ledger and negative authority/privacy ledger.

## Decisions Made

- Used a tagged `CodingAgentSubmittedTerminalAnchor` so event acknowledgement, outcome acknowledgement, and recovery uncertainty cannot be collapsed into one guessed sequence domain.
- Kept `CodingAgentOutcomeAcknowledgementId` directly serializable but non-constructible through a public Rust constructor; later runtime validation remains the authority.
- Projected submitted event durability as `Durable` or `Uncertain`, deliberately excluding session identity and pending-write internals.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Completed the existing exhaustive tool error projection**

- **Found during:** Task 1 verification
- **Issue:** Adding the planned lifecycle error variant made `tools/edit.rs`'s exhaustive `CodingSessionError` match fail compilation.
- **Fix:** Routed the typed lifecycle error through the existing `to_string()` branch without changing tool behavior or leaking additional authority.
- **Files modified:** `crates/pi-coding-agent/src/tools/edit.rs`
- **Verification:** Both focused public API tests and `cargo fmt --all --check` pass.
- **Committed in:** `c660f44`

---

**Total deviations:** 1 auto-fixed (1 blocking issue).
**Impact on plan:** The fix was required for compile completeness and preserved the existing projection semantics; no scope expansion.

## Issues Encountered

- The repository's pre-existing dead-code warnings remain visible in focused test output; they do not fail the planned gates and were not modified.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plans 09-03 through 09-06 can compile against the lifecycle outcomes, rejection reasons, terminal anchors, and opaque acknowledgement identity.
- Runtime detach, operation-association finalization, shutdown, and adapter wiring intentionally remain owned by their later plans.

## Self-Check: PASSED

- Verified both task commits exist: `c660f44`, `743c20c`.
- Verified all seven modified implementation/test files exist.
- Re-ran all three plan-level focused tests, `cargo fmt --all --check`, and `git diff --check` successfully.

---
*Phase: 09-lifecycle-association-guards-and-closure*
*Completed: 2026-07-14*
