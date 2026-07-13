---
phase: 08-client-connection-replay-and-scoped-control
plan: 07
subsystem: rpc
tags: [rust, rpc, client-connection, replay, scoped-control]
requires:
  - phase: 08-06
    provides: generation-scoped prompt control and stable receipts
provides:
  - RPC projection over CodingAgentClientConnection state, replay, acknowledgement, drafts, submission, and control
  - fail-closed RPC authority mirror guards and complete Phase 8 validation evidence
affects: [phase-09-client-lifecycle]
tech-stack:
  added: []
  patterns: [public connection authority with adapter-local rendering dedup]
key-files:
  created: [.planning/phases/08-client-connection-replay-and-scoped-control/08-07-SUMMARY.md]
  modified: [crates/pi-coding-agent/src/protocol/rpc/state.rs, crates/pi-coding-agent/src/protocol/rpc/prompt.rs, crates/pi-coding-agent/src/protocol/rpc/commands.rs, crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs]
key-decisions:
  - "RPC retains only adapter rendering and ordinary-command idempotency state; connection owns client truth."
  - "Legacy busy and in-memory wire projections remain adapter-level compatibility values."
patterns-established:
  - "Reconnect projects typed public recovery and acknowledges the replay cursor after adapter application."
requirements-completed: [CLIENT-01, CLIENT-02, CLIENT-03, CONTROL-01]
coverage:
  - id: D1
    description: RPC uses the public client connection without duplicate draft, submitted, replay, or control authority.
    requirement: CLIENT-01
    verification:
      - kind: integration
        ref: cargo test -p pi-coding-agent --test rpc_mode --test protocol_events --quiet
        status: pass
    human_judgment: false
  - id: D2
    description: Workspace behavior and source boundaries remain compatible and fail closed.
    requirement: CONTROL-01
    verification:
      - kind: integration
        ref: cargo test --workspace --quiet && cargo check --workspace
        status: pass
    human_judgment: false
duration: 24min
completed: 2026-07-14
status: complete
---

# Phase 08 Plan 07 Summary

**RPC now projects the public connection contract while ordinary operations continue exclusively through `CodingAgentSession::run`.**

## Performance

- **Duration:** 24 min
- **Completed:** 2026-07-14
- **Tasks:** 3
- **Files modified:** 8

## Accomplishments

- Replaced RPC-local draft, submitted-operation, replay, and raw-control mirrors with `CodingAgentClientConnection` state and scoped control.
- Preserved JSON/RPC event ordering, overflow and fresh-snapshot responses, queue behavior, canonical dispatch, and disabled-session wire identity.
- Added recursive source guards and recorded passing evidence for the exact 18-row Phase 8 validation map.

## Task Commits

1. **Task 1: Freeze RPC wire parity** - covered by the 08-06 supplemental contract tests and retained RPC/protocol suites.
2. **Task 2: Centralize RPC client authority** - `a622ee4`
3. **Task 3: Close guards and validation** - `26ef65a`

## Decisions Made

- Kept `adapter_applied_sequence` solely for at-least-once rendering dedup; replay truth and acknowledgement live on the connection.
- Derived scoped controls from the connection's submitted operation, preserving stable RPC IDs as control receipt IDs.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Wire compatibility] Preserve immediate busy projection**
- **Found during:** Task 3 full crate gate
- **Issue:** Public snapshot admission can lag the RPC task-start response by one scheduling turn.
- **Fix:** Use existing `running` state only for the legacy RPC capability rendering window.
- **Verification:** `rpc_state_reports_prompt_busy_while_running`
- **Committed in:** `26ef65a`

**2. [Rule 1 - Wire compatibility] Preserve disabled-session display identity**
- **Found during:** Task 3 workspace gate
- **Issue:** Public runtime snapshot exposes `runtime_sess_*`, while RPC contract requires `in-memory` when persistence is disabled.
- **Fix:** Retain `in-memory` as an adapter-only display value.
- **Verification:** `rpc_disabled_session_prompt_uses_non_persistent_runtime_without_session_files`
- **Committed in:** `26ef65a`

**Total deviations:** 2 auto-fixed Rule 1 compatibility issues. **Impact:** Wire behavior preserved without restoring duplicate authority.

## Verification

- `cargo fmt --check`
- Focused public API, protocol event, RPC, API-boundary, and product-runtime guard suites
- `cargo test -p pi-coding-agent --quiet`
- `cargo test --workspace --quiet`
- `cargo check --workspace`
- `git diff --check`

All passed. Existing compiler warnings remain non-failing and were not expanded into unrelated cleanup.

## Next Phase Readiness

Phase 8 client connection, replay, submission, and scoped-control convergence is complete. Phase 9 can own lifecycle and exhaustive association work without RPC-local client truth.

---
*Phase: 08-client-connection-replay-and-scoped-control*
*Completed: 2026-07-14*
