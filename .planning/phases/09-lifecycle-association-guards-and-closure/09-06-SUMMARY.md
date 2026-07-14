---
phase: 09-lifecycle-association-guards-and-closure
plan: 06
subsystem: rpc-lifecycle
tags: [rust, rpc, detach, shutdown, lifecycle, compatibility]

requires:
  - phase: 09-lifecycle-association-guards-and-closure
    provides: public idempotent detach and two-phase runtime shutdown from Plans 09-03 and 09-05
provides:
  - Additive typed RPC detach/shutdown request, response, status, and lifecycle-event wire values
  - One public detach cleanup path for explicit commands, EOF, transport errors, loop errors, and session replacement
  - Deferred RPC shutdown response after Phase A request, admitted-work drain, owner restoration, and Phase B finalization
affects: [09-07-interactive-lifecycle, rpc-wire-contract, runtime-ownership]

tech-stack:
  added: []
  patterns:
    - RPC lifecycle messages use dedicated additive payloads instead of changing legacy envelopes
    - Running operations retain only an opaque shutdown handle and pending response correlation
    - Session-owned terminal finalization remains valid while runtime authority is shutting down

key-files:
  created: []
  modified:
    - crates/pi-coding-agent/src/protocol/types.rs
    - crates/pi-coding-agent/src/protocol/rpc.rs
    - crates/pi-coding-agent/src/protocol/rpc/state.rs
    - crates/pi-coding-agent/src/protocol/rpc/commands.rs
    - crates/pi-coding-agent/src/protocol/rpc/prompt.rs
    - crates/pi-coding-agent/src/protocol/rpc/wire.rs
    - crates/pi-coding-agent/src/coding_session/snapshot_coordinator.rs
    - crates/pi-coding-agent/tests/rpc_mode.rs
    - crates/pi-coding-agent/tests/protocol_events.rs

key-decisions:
  - "Keep lifecycle wire values additive and independently typed; do not add lifecycle fields to existing protocol or response envelopes."
  - "Capture the opaque runtime shutdown handle before every asynchronous RPC owner move and retain only pending response correlation in adapter state."
  - "Allow exact submitted terminal finalization during ShuttingDown while continuing to reject ordinary client mutation and all post-ShutDown mutation."

requirements-completed: [CLIENT-04, COMPAT-03]

coverage:
  - id: D1
    description: "Standalone detach/shutdown request, response, event, and status values serialize to exact additive JSON without changing legacy envelopes."
    requirement: COMPAT-03
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test protocol_events lifecycle_wire --quiet"
        status: pass
    human_judgment: false
  - id: D2
    description: "Explicit detach is typed and idempotent, emits a dedicated lifecycle event on transition, and does not cancel already-admitted Prompt work."
    requirement: CLIENT-04
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_lifecycle_detach_during_prompt_is_observable_without_cancelling_work"
        status: pass
    human_judgment: false
  - id: D3
    description: "Shutdown requests Phase A during active work but withholds the response until the unique owner is restored and Phase B completes."
    requirement: CLIENT-04
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_lifecycle_shutdown_waits_for_owner_restoration_and_uses_stable_rejection_code"
        status: pass
    human_judgment: false
  - id: D4
    description: "All prior RPC, protocol-event, overflow, control, PartialCommit, ordering, and omission behavior remains green under full focused suites."
    requirement: COMPAT-03
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test rpc_mode --quiet"
        status: pass
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test protocol_events --quiet"
        status: pass
      - kind: unit
        ref: "cargo test -p pi-coding-agent --lib protocol::rpc --quiet"
        status: pass
    human_judgment: false

duration: 16min
completed: 2026-07-14
status: complete
---

# Phase 09 Plan 06: RPC Lifecycle Projection Summary

**RPC now projects detach and two-phase shutdown through the public lifecycle authorities with exact additive wire messages and unchanged legacy protocol shapes.**

## Performance

- **Duration:** 16 min
- **Started:** 2026-07-14T07:58:36Z
- **Completed:** 2026-07-14T08:14:39Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments

- Added standalone typed detach/shutdown requests, responses, lifecycle events, and stable snake_case statuses with exact JSON snapshots.
- Added exhaustive `detach` and `shutdown` RPC command routing without modifying fields, omission rules, or codes on any existing wire envelope.
- Replaced adapter-local connection dropping with one `detach_client()` path used by explicit detach, EOF, transport/loop return, and `new_session` replacement.
- Captured the opaque shutdown request handle before each of the four asynchronous RPC owner moves; active shutdown requests Phase A immediately and stores only response correlation.
- Restored the unique `CodingAgentSession`, completed Phase B, emitted the lifecycle event, and only then returned the typed shutdown response.
- Preserved exact terminal evidence for admitted work during detach and Phase A shutdown without reconnect, retry, cancellation, or control retargeting.

## Task Commits

1. **Task 1 RED: freeze additive lifecycle wire values** - `4a14825` (test)
2. **Task 1 GREEN: add standalone lifecycle wire values** - `e296332` (feat)
3. **Task 2 RED: add failing RPC lifecycle routing cases** - `91b1822` (test)
4. **Task 2 GREEN: route RPC lifecycle through public ownership** - `54b105d` (feat)

## Files Created/Modified

- `protocol/types.rs` - Standalone lifecycle payloads/statuses plus additive command variants.
- `protocol/rpc.rs`, `rpc/state.rs`, `rpc/commands.rs`, and `rpc/wire.rs` - Converged cleanup, exhaustive routing, stable errors, and lifecycle emission.
- `protocol/rpc/prompt.rs` - Pre-move shutdown-handle capture and owner-restored Phase B response completion.
- `coding_session/snapshot_coordinator.rs` - Session-owned terminal/final association remains legal during drain, but not after shutdown completes.
- `tests/rpc_mode.rs` and `tests/protocol_events.rs` - Deterministic lifecycle timing, non-cancellation, rejection-code, and exact serialization coverage.

## Decisions Made

- Lifecycle requests, statuses, responses, and events remain dedicated wire values; existing `ProtocolEvent` and `RpcResponse` layouts are unchanged.
- RPC holds no lifecycle truth: a running task carries only the public opaque shutdown handle and an optional response ID correlation.
- Explicit detach emits an additive event only when the public connection actually transitions; idempotent repeats return typed status without fabricating another transition.

## TDD Gate Compliance

- **Task 1 RED:** `4a14825` failed to compile on the eight missing standalone lifecycle wire types.
- **Task 1 GREEN:** `e296332` made the exact lifecycle JSON snapshots pass without extending exhaustive command routing.
- **Task 2 RED:** `91b1822` failed on unsupported lifecycle commands and the premature shutdown response.
- **Task 2 GREEN:** `54b105d` made explicit/active detach, deferred shutdown, stable rejection, and all full RPC/protocol suites pass.
- **REFACTOR:** No separate refactor commit; cleanup convergence and the directly required terminal-drain correction shipped atomically with Task 2 GREEN.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical Functionality] Captured shutdown authority at all RPC owner moves**

- **Found during:** Task 2
- **Issue:** The plan action requires Phase A while the unique owner is moved, but `protocol/rpc/prompt.rs` was omitted from `files_modified` even though it owns all four async move sites.
- **Fix:** Captured `CodingAgentRuntimeShutdownHandle` immediately before each owner move and stored it on the active task envelope.
- **Files modified:** `crates/pi-coding-agent/src/protocol/rpc/prompt.rs`
- **Verification:** Active shutdown timing test and complete RPC lib suite pass.
- **Committed in:** `54b105d`

**2. [Rule 1 - Bug] Allowed admitted terminal association during detach and shutdown drain**

- **Found during:** Task 2 GREEN verification
- **Issue:** `mark_terminal` and `finalize_terminal_association` reused the ordinary runtime mutation gate, rejecting terminal completion after Phase A despite the admitted-work drain contract.
- **Fix:** Added a terminal-only lifecycle gate that permits `Running` and `ShuttingDown` but rejects completed `ShutDown`.
- **Files modified:** `crates/pi-coding-agent/src/coding_session/snapshot_coordinator.rs`
- **Verification:** Deterministic active detach and shutdown tests finish with `agent_end`/terminal evidence and all focused suites pass.
- **Committed in:** `54b105d`

---

**Total deviations:** 2 auto-fixed (1 missing critical integration point, 1 lifecycle correctness bug).
**Impact on plan:** Both changes are required to realize the planned public-lifecycle behavior; no public authority, legacy wire field, or unrelated feature was added.

## Issues Encountered

- Existing dead-code and disabled-test import warnings remain non-fatal and unchanged in character.
- Stub scan found only the pre-existing manual-compaction compatibility message; no new placeholder or unwired lifecycle value was introduced.

## User Setup Required

None - no external services or configuration required.

## Next Phase Readiness

- Plan 09-07 can project top-level and interactive lifecycle behavior over the same public detach/shutdown authorities.
- No lifecycle adapter flags, recovery retries, or alternate dispatchers were introduced.
- `docs/next stage.md` remains untouched and untracked.

## Self-Check: PASSED

- FOUND all nine modified implementation/test files.
- FOUND Task commits `4a14825`, `e296332`, `91b1822`, and `54b105d`.
- PASS: `cargo fmt --all --check`.
- PASS: full `rpc_mode` (43 tests), `protocol_events` (13 tests), and RPC lib (11 focused tests) suites.
- PASS: `git diff --check` and no tracked-file deletion in Task 2.
- PASS: unrelated `docs/next stage.md` remains untouched and untracked.

---
*Phase: 09-lifecycle-association-guards-and-closure*
*Completed: 2026-07-14*
