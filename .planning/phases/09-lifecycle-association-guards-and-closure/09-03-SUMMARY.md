---
phase: 09-lifecycle-association-guards-and-closure
plan: 03
subsystem: client-lifecycle
tags: [rust, detach, generation, replay, prompt-control, lifecycle-notification]

requires:
  - phase: 09-lifecycle-association-guards-and-closure
    provides: compile-ready lifecycle values and closed operation association descriptor from Plans 09-01 and 09-02
provides:
  - Generation-scoped idempotent detach under the sole SnapshotCoordinator authority
  - Typed fail-closed lifecycle validation for state, acknowledgement, drafts, submission, replay, and Prompt control
  - Detach-aware reconnect receiver wake-up with pre-delivery validation
  - Same-id reconnect preservation and active Prompt control rebinding
affects: [09-04-operation-association, 09-05-shutdown, 09-06-rpc-lifecycle, interactive-lifecycle]

tech-stack:
  added: []
  patterns:
    - Coordinator-owned runtime and connection lifecycle state with a monotonic notification epoch
    - Lock-transition-release-notify ordering for lifecycle invalidation
    - Receiver select plus coordinator validation as the event/detach linearization boundary

key-files:
  created: []
  modified:
    - crates/pi-coding-agent/src/coding_session/snapshot_coordinator.rs
    - crates/pi-coding-agent/src/coding_session/client_service.rs
    - crates/pi-coding-agent/src/coding_session/public_projection.rs
    - crates/pi-coding-agent/src/coding_session/event_service.rs
    - crates/pi-coding-agent/tests/public_api.rs
    - crates/pi-coding-agent/tests/api_boundary_guards.rs

key-decisions:
  - "Keep reconnectable client contents in place and model detach as connection validity, not record deletion or operation cancellation."
  - "Use a coordinator-owned lifecycle epoch with watch notification so blocked receivers wake without moving transport authority into ClientService."
  - "Allow session-owned terminal finalization after detach while rejecting every connection-owned mutation through the shared lifecycle gate."

patterns-established:
  - "Lifecycle gate: runtime state, generation, and attached state are checked centrally before connection-owned reads or mutations."
  - "Delivery gate: reconnect receivers observe lifecycle epochs and validate under coordinator authority immediately before returning a business event."

requirements-completed: [CLIENT-04, CONTROL-02]

coverage:
  - id: D1
    description: "Detach is idempotent and generation-scoped while acknowledgement, drafts, submitted state, receipts, and active work remain reconnectable."
    requirement: CLIENT-04
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent detach --quiet"
        status: pass
      - kind: unit
        ref: "crates/pi-coding-agent/src/coding_session/snapshot_coordinator.rs#detach_is_idempotent_generation_scoped_and_preserves_reconnectable_facts"
        status: pass
      - kind: unit
        ref: "crates/pi-coding-agent/src/coding_session/snapshot_coordinator.rs#detach_keeps_prompt_running_and_reconnect_rebinds_control"
        status: pass
    human_judgment: false
  - id: D2
    description: "Detached and stale handles fail closed across connection-owned state, acknowledgement, outcome acknowledgement, draft, submission, replay, and control paths."
    requirement: CONTROL-02
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent lifecycle_rejection --quiet"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/public_api.rs#detach_outcomes_and_lifecycle_rejection_paths_are_typed_and_preserve_state"
        status: pass
    human_judgment: false
  - id: D3
    description: "A reconnect receiver blocked in recv wakes on detach and cannot return a business event after lifecycle invalidation wins."
    requirement: CLIENT-04
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/public_api.rs#detach_wakes_a_blocked_reconnect_receiver_without_leaking_an_event"
        status: pass
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test public_api detach --quiet"
        status: pass
    human_judgment: false
  - id: D4
    description: "The public detach method derives private generation authority only from its connection and exposes no arbitrary client or generation selector."
    requirement: CONTROL-02
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test api_boundary_guards public_lifecycle --quiet"
        status: pass
    human_judgment: false

duration: 17min
completed: 2026-07-14
status: complete
---

# Phase 09 Plan 03: Generation-Scoped Detach and Receiver Closure Summary

**Recoverable generation-scoped detach now invalidates connection authority and wakes replay receivers without cancelling session-owned Prompt work or discarding reconnectable client facts.**

## Performance

- **Duration:** 17 min
- **Started:** 2026-07-14T05:18:10Z
- **Completed:** 2026-07-14T05:35:41Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments

- Added the sole coordinator lifecycle state machine with exact `Detached`, `AlreadyDetached`, and `StaleGeneration` outcomes and shared typed rejection for detached, stale-generation, and runtime-closed authority.
- Preserved acknowledgement, drafts, submitted operation state, accepted control receipts, and active operation/control binding across detach and same-id reconnect.
- Added lifecycle epoch notification to the atomic replay/live boundary so blocked receivers wake promptly and validate coordinator authority before event delivery.
- Published connection-derived `detach` and `acknowledge_outcome` methods and strengthened facade guards against arbitrary lifecycle selectors.

## Task Commits

The two TDD tasks were committed atomically by gate:

1. **Task 1 RED: failing coordinator detach lifecycle contract** - `4fe9515` (test)
2. **Task 1 GREEN: coordinator lifecycle authority and preservation** - `add8483` (feat)
3. **Task 2 RED: failing public detach and receiver contract** - `4f39119` (test)
4. **Task 2 GREEN: public detach-aware receiver and Prompt rebind** - `ed320b1` (feat)

## Files Created/Modified

- `crates/pi-coding-agent/src/coding_session/snapshot_coordinator.rs` - Owns runtime/connection lifecycle, shared validation, lifecycle epoch notification, detach transition, preserved finalization, and Prompt control rebind.
- `crates/pi-coding-agent/src/coding_session/client_service.rs` - Remains a zero-authority forwarding facade over coordinator detach.
- `crates/pi-coding-agent/src/coding_session/public_projection.rs` - Publishes connection-derived detach/outcome acknowledgement and lifecycle-aware reconnect delivery.
- `crates/pi-coding-agent/src/coding_session/event_service.rs` - Captures lifecycle receiver and epoch in the atomic replay/live boundary.
- `crates/pi-coding-agent/tests/public_api.rs` - Proves exact outcomes, typed lifecycle rejection families, state preservation, and blocked receiver wake-up.
- `crates/pi-coding-agent/tests/api_boundary_guards.rs` - Locks the exact public detach signature and rejects arbitrary client/generation lifecycle selectors.

## Decisions Made

- Detach mutates only connection validity; it does not remove `ClientRecord`, increment generation, clear Prompt control, or cancel canonical work.
- Same-id reconnect increments generation, marks the record attached, preserves contents, and rebinds any still-running Prompt control owner to the new generation.
- Internal terminal finalization is session-owned and may complete after detach; connection-owned state/mutation/replay/control paths remain fail-closed.
- Lifecycle notification uses a monotonic epoch plus Tokio `watch`; state changes occur under the coordinator mutex, then notification occurs after releasing the mutex.

## TDD Gate Compliance

- **Task 1 RED:** `4fe9515` failed because detach outcomes, lifecycle state, and the shared lifecycle error did not exist.
- **Task 1 GREEN:** `add8483` implemented the coordinator state machine and made both detach and lifecycle rejection fixtures pass.
- **Task 2 RED:** `4f39119` failed because the public detach/outcome-acknowledgement methods and detach-aware receiver behavior did not exist.
- **Task 2 GREEN:** `ed320b1` implemented the stable facade, receiver notification/validation, and active Prompt rebind; all focused and full `public_api` tests pass.
- **REFACTOR:** No separate refactor commit was necessary after formatting and verification.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Preserved session-owned terminal finalization after connection detach**

- **Found during:** Task 2 active Prompt lifecycle verification
- **Issue:** Applying the connection lifecycle gate directly to `mark_terminal` would leave a detached-but-running Prompt permanently in `Running`, violating D-01/D-02 preservation.
- **Fix:** Kept connection-owned paths on the shared gate while allowing terminal finalization to locate the admitted submitted operation by client and exact operation identity, including after detach or generation rebind.
- **Files modified:** `crates/pi-coding-agent/src/coding_session/snapshot_coordinator.rs`
- **Verification:** `detach_keeps_prompt_running_and_reconnect_rebinds_control` and all plan-level focused gates pass.
- **Committed in:** `ed320b1`

---

**Total deviations:** 1 auto-fixed (1 missing critical functionality).
**Impact on plan:** The fix is required for the stated detach semantics and adds no new workflow or public authority.

## Issues Encountered

- Existing unrelated dead-code and deprecated-field compiler warnings remain unchanged; they do not fail the planned gates.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plan 09-04 can attach generalized terminal anchors and outcome acknowledgement semantics to the lifecycle-safe submitted-operation record.
- Plan 09-05 can drive the already exhaustive runtime lifecycle states through two-phase shutdown and reuse the same notification/validation boundary.
- No blockers; `docs/next stage.md` remains untouched and untracked.

## Self-Check: PASSED

- FOUND all six modified implementation/test files.
- FOUND task commits `4fe9515`, `add8483`, `4f39119`, and `ed320b1`.
- PASS: `cargo test -p pi-coding-agent detach --quiet`.
- PASS: `cargo test -p pi-coding-agent lifecycle_rejection --quiet`.
- PASS: `cargo test -p pi-coding-agent --test api_boundary_guards public_lifecycle --quiet`.
- PASS: full `cargo test -p pi-coding-agent --test public_api --quiet`.
- PASS: `cargo fmt --all --check` and `git diff --check`.

---
*Phase: 09-lifecycle-association-guards-and-closure*
*Completed: 2026-07-14*
