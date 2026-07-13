---
phase: 07-adapter-migration-and-compatibility-deletion
plan: 02
subsystem: protocol
tags: [rust, typed-events, json, rpc, compatibility]

requires:
  - phase: 07-adapter-migration-and-compatibility-deletion
    provides: Typed ProductEvent payload hierarchy and ownership contract from Plan 07-01
provides:
  - Typed protocol projection for JSON and RPC machine-facing adapters
  - Regression coverage for ordered protocol payloads and bounded RPC queue recovery
affects: [phase-07, protocol-adapters, rpc, json-output]

tech-stack:
  added: []
  patterns:
    - Stateful protocol projection matches CodingAgentProductEventKind payload families directly
    - RPC forwarding remains a thin wrapper over the shared typed adapter

key-files:
  created: []
  modified:
    - crates/pi-coding-agent/src/protocol/events.rs
    - crates/pi-coding-agent/src/protocol/rpc/events.rs
    - crates/pi-coding-agent/tests/protocol_events.rs

key-decisions:
  - Preserve the existing wire event ordering, payload formatting, and null fallback for invalid tool arguments while changing only the source event hierarchy.
  - Keep bounded RPC queue overflow and fresh_snapshot recovery semantics unchanged.

patterns-established:
  - "Typed adapter boundary: production protocol projection consumes ProductEvent::event() and matches typed family payloads directly."
  - "Compatibility fixtures: tests may construct legacy events to create ProductEvent values, but production adapters do not recover or inspect raw compatibility events."

requirements-completed: [COMPAT-01]

coverage:
  - id: D1
    description: "JSON/RPC protocol adapters project typed product-event payloads with existing ordered machine-visible output."
    requirement: COMPAT-01
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test protocol_events --quiet"
        status: pass
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test json_mode --quiet"
        status: pass
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test rpc_mode --quiet"
        status: pass
    human_judgment: false
  - id: D2
    description: "RPC bounded event queue retains explicit overflow reporting and fresh-snapshot recovery behavior."
    verification:
      - kind: unit
        ref: "cargo test -p pi-coding-agent --lib protocol::rpc::event_queue --quiet"
        status: pass
    human_judgment: false

duration: 0min
completed: 2026-07-13
status: complete
---

# Phase 07 Plan 02: Typed Machine Protocol Projection Summary

**JSON and RPC adapters now consume the owned typed ProductEvent hierarchy while preserving protocol payloads, ordering, delegation projections, failure formatting, and queue overflow recovery.**

## Performance

- **Duration:** continuation verification and close-out
- **Started:** 2026-07-13
- **Completed:** 2026-07-13
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Reworked the stateful protocol matcher around `CodingAgentProductEventKind` and its typed family payloads, including assistant text/thinking, tools, compaction, self-healing, delegation, capability, provider/model, failures, and terminal events.
- Kept RPC forwarding as a thin typed adapter and retained JSON/RPC wire assertions, including ordered `TurnEnd`/`AgentEnd` output and JSON argument null fallback.
- Preserved bounded RPC queue ordering and explicit `event_stream_lag` / `fresh_snapshot` recovery behavior.

## Task Commits

1. **Task 1: Rewrite the stateful protocol matcher for typed payloads** - `6f8cb35` (test), `bb73fdf` (refactor)
2. **Task 2: Preserve RPC forwarding and machine-output regressions** - `74d691a` (test), `fd5c96d` (test)

## Files Created/Modified

- `crates/pi-coding-agent/src/protocol/events.rs` - Typed stateful protocol event projection.
- `crates/pi-coding-agent/src/protocol/rpc/events.rs` - RPC adapter forwarding and typed fixtures.
- `crates/pi-coding-agent/tests/protocol_events.rs` - Source guards and machine-output regression coverage.

## Decisions Made

- Preserved all established wire semantics and event ordering while replacing production raw-event matching with typed payload matching.
- Left the bounded queue implementation and overflow recovery lifecycle untouched.

## Deviations from Plan

### Handoff Recovery

**1. Prior executor rate-limit handoff (429)**
- **Found during:** Plan close-out continuation
- **Issue:** The previous typed `gsd-executor` completed all four production/test commits but hit a 429 before returning and creating the required summary.
- **Fix:** Verified the existing commits and all plan acceptance/verification commands, then created this summary without redoing or reverting completed work.
- **Files modified:** `.planning/phases/07-adapter-migration-and-compatibility-deletion/07-02-SUMMARY.md`
- **Verification:** Focused protocol, JSON, RPC, queue, format, and diff checks all passed.
- **Commit:** This docs commit.

**Total deviations:** 0 implementation deviations; 1 operational handoff documented.
**Impact on plan:** No behavioral or scope impact.

## Issues Encountered

The focused test runs emitted pre-existing dead-code warnings for `load_plugins` and `ensure_idle`; no failures or new defects were found.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Machine-facing protocol consumers are migrated to typed product events and regression-covered. The next Phase 07 plan may proceed with remaining adapter migration and compatibility deletion work.

## Self-Check: PASSED

- Summary file exists at the required path.
- Commits `6f8cb35`, `bb73fdf`, `74d691a`, and `fd5c96d` are present in Git history.
- `cargo test -p pi-coding-agent --test protocol_events --quiet` passed (12 tests).
- `cargo test -p pi-coding-agent --test json_mode --quiet` passed (4 tests).
- `cargo test -p pi-coding-agent --test rpc_mode --quiet` passed (40 tests).
- `cargo test -p pi-coding-agent --lib protocol::rpc::event_queue --quiet` passed (2 tests).
- `cargo fmt --check` and `git diff --check` passed.

---
*Phase: 07-adapter-migration-and-compatibility-deletion*
*Plan: 02*
*Completed: 2026-07-13*
