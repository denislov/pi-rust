---
phase: 09-lifecycle-association-guards-and-closure
plan: 05
subsystem: runtime-lifecycle
tags: [rust, shutdown, product-events, lifecycle, drain]

requires:
  - phase: 09-lifecycle-association-guards-and-closure
    provides: exact admitted operation identity and terminal association from Plan 09-04
provides:
  - Additive typed Runtime.ShutDown product event with a closed 46-row inventory
  - Opaque Phase A runtime shutdown request authority
  - Idempotent owner Phase B drain, final publication, and receiver closure
affects: [09-06-rpc-lifecycle, 09-07-interactive-lifecycle, runtime-ownership]

tech-stack:
  added: []
  patterns:
    - Coordinator lifecycle watch separates immediate authority revocation from owner finalization
    - Product receivers drain the final lifecycle event before reporting closure

key-files:
  created: []
  modified:
    - crates/pi-coding-agent/src/coding_session/mod.rs
    - crates/pi-coding-agent/src/coding_session/snapshot_coordinator.rs
    - crates/pi-coding-agent/src/coding_session/event_service.rs
    - crates/pi-coding-agent/src/coding_session/public_event.rs
    - crates/pi-coding-agent/src/coding_session/public_projection.rs
    - crates/pi-coding-agent/src/interactive/event_bridge.rs
    - crates/pi-coding-agent/src/protocol/events.rs
    - crates/pi-coding-agent/tests/public_api.rs
    - crates/pi-coding-agent/tests/event_boundary_guards.rs
    - docs/product-event-contract.md

key-decisions:
  - "Keep shutdown as two explicit phases: a cloneable coordinator-only request handle closes authority, while the restored unique session owner drains and finalizes."
  - "Treat Runtime.ShutDown as a typed, operationless, live-only lifecycle event that produces no legacy protocol or interactive transcript projection."
  - "Close receivers through coordinator lifecycle state after final-event publication rather than relying on broadcast sender ownership."

requirements-completed: [CLIENT-04, COMPAT-03]

coverage:
  - id: D1
    description: "The product-event contract contains exactly 46 variants and Runtime.ShutDown preserves every prior serialized field and representation."
    requirement: COMPAT-03
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test event_boundary_guards --quiet"
        status: pass
      - kind: unit
        ref: "cargo test -p pi-coding-agent event_contract --quiet"
        status: pass
    human_judgment: false
  - id: D2
    description: "Protocol and interactive consumers exhaustively accept shutdown without synthesizing Prompt, compaction, transcript, or legacy protocol content."
    requirement: COMPAT-03
    verification:
      - kind: unit
        ref: "cargo test -p pi-coding-agent --lib interactive::event_bridge --quiet"
        status: pass
    human_judgment: false
  - id: D3
    description: "Phase A rejects new admission, connection mutation, and Prompt control without cancelling admitted work."
    requirement: CLIENT-04
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/public_api.rs#shutdown_drains_admitted_work_before_lifecycle_event_and_receiver_close"
        status: pass
    human_judgment: false
  - id: D4
    description: "Phase B publishes admitted terminal evidence before one shutdown lifecycle event and closes receivers last; repeated shutdown is typed and idempotent."
    requirement: CLIENT-04
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test public_api shutdown --quiet"
        status: pass
    human_judgment: false

duration: 29min
completed: 2026-07-14
status: complete
---

# Phase 09 Plan 05: Two-Phase Runtime Shutdown Summary

**Runtime shutdown now revokes admission and control immediately, drains already-admitted work to its exact terminal evidence, publishes one operationless lifecycle event, and closes receivers last.**

## Performance

- **Duration:** 29 min
- **Started:** 2026-07-14T07:23:06Z
- **Completed:** 2026-07-14T07:51:37Z
- **Tasks:** 2
- **Files modified:** 12

## Accomplishments

- Expanded the closed product-event inventory from 45 to 46 with `Runtime.ShutDown`, stable snake_case Serde identity, no operation ID, no terminal association, and unchanged prior rows.
- Updated exhaustive typed consumers so shutdown emits neither legacy `ProtocolEvent` values nor interactive transcript, Prompt, compaction, or system-notice projections.
- Added `CodingAgentRuntimeShutdownHandle` as opaque coordinator-only Phase A authority that rejects new runtime/client/control mutations without waiting, publishing, or aborting active work.
- Added idempotent `CodingAgentSession::shutdown()` Phase B behavior that waits outside standard mutexes, publishes the lifecycle event after admitted terminal evidence, marks the runtime shut down, and wakes/closes receivers.
- Proved active drain ordering with deterministic oneshot gates and preserved the closed owner API ledger.

## Task Commits

1. **Task 1 RED: require the 46th lifecycle event** - `ebff811` (test)
2. **Task 1 GREEN: add typed shutdown event and empty adapter projections** - `4245927` (feat)
3. **Task 2 RED: add deterministic active-drain shutdown contract** - `bee4f7e` (test)
4. **Task 2 GREEN: implement Phase A/Phase B runtime shutdown** - `e0c8d46` (feat)
5. **Verification fix: register lifecycle owner methods in the closed ledger** - `23e33aa` (fix)

## Files Created/Modified

- `coding_session/event.rs` and `public_event.rs` - Internal/public shutdown event, exact naming, mapping, fixture, and inventory semantics.
- `coding_session/snapshot_coordinator.rs` - Runtime lifecycle transition, client authority revocation, drain notification, and final closure state.
- `coding_session/event_service.rs` - Lifecycle-aware product receivers that drain the final event before closure.
- `coding_session/mod.rs` and `public_projection.rs` - Opaque request handle and unique-owner shutdown API.
- `interactive/event_bridge.rs` and `protocol/events.rs` - Explicit empty compatibility projections.
- `tests/public_api.rs` and `tests/event_boundary_guards.rs` - Deterministic shutdown ordering and closed 46-row contract coverage.
- `docs/product-event-contract.md` - Authoritative additive lifecycle event inventory and semantics.

## Decisions Made

- Phase A and Phase B remain separate authorities: cloned handles can only request shutdown; only the unique restored owner can wait, publish, and finalize.
- Lifecycle closure is coordinator-state-driven so cloned adapter/event-service handles cannot keep public receivers artificially open.
- Reconnect receivers may observe the final shutdown event while still rejecting detach/stale generations; they close only after the coordinator reaches `ShutDown`.

## TDD Gate Compliance

- **Task 1 RED:** `ebff811` raised authoritative counts/documentation to 46 while source remained 45; the boundary test failed at the exact inventory assertion.
- **Task 1 GREEN:** `4245927` added the variant, exhaustive mapping, fixtures, serialization names, and empty adapter projections; all Task 1 focused tests passed.
- **Task 2 RED:** `bee4f7e` failed to compile because the opaque handle and owner shutdown methods did not exist.
- **Task 2 GREEN:** `e0c8d46` made the gated admitted operation, lifecycle rejection, terminal ordering, closure, and repeat outcome assertions pass.
- **REFACTOR:** No separate refactor commit; the focused boundary-ledger synchronization was committed as `23e33aa`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Synchronized the closed owner method ledger**

- **Found during:** Full workspace verification
- **Issue:** The structural `CodingAgentSession` ledger rejected the two planned lifecycle methods because the exact allowlist still described the pre-shutdown surface.
- **Fix:** Classified `runtime_shutdown_handle` and `shutdown` as retained public lifecycle helpers without widening any dispatcher or internal authority export.
- **Files modified:** `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs`
- **Verification:** The targeted final receiver-aware ledger guard passes.
- **Committed in:** `23e33aa`

---

**Total deviations:** 1 auto-fixed blocking guard synchronization.
**Impact on plan:** Required structural enforcement was updated to recognize only the two planned lifecycle methods; scope and runtime behavior are unchanged.

## Issues Encountered

- Existing dead-code, unused-import, and deprecated compatibility warnings remain non-fatal and unchanged in character.
- The final all-workspace rerun reached the long-running external boundary-guard suite under the command harness limit; the previously failing exact ledger case was rerun directly after its fix, while all plan-focused suites, the full `pi-coding-agent` suite before the ledger-only adjustment, workspace check, formatting, and diff audit passed.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plan 09-06 can add RPC lifecycle wire projection against the stable typed shutdown event and outcome.
- Plan 09-07 can assign top-level process ownership and interactive detach-versus-shutdown behavior without changing the runtime boundary.
- No functional blockers; `docs/next stage.md` remains untouched and untracked.

## Self-Check: PASSED

- FOUND all implementation, contract, adapter, and deterministic public test changes.
- FOUND Task commits `ebff811`, `4245927`, `bee4f7e`, `e0c8d46`, and verification fix `23e33aa`.
- PASS: all five plan-level focused verification commands.
- PASS: full `cargo test -p pi-coding-agent --quiet` before the ledger-only test adjustment.
- PASS: targeted receiver-aware owner ledger guard after synchronization.
- PASS: `cargo fmt --all --check`, `cargo check --workspace --quiet`, and `git diff --check`.
- PASS: unrelated `docs/next stage.md` remains untouched and untracked.

---
*Phase: 09-lifecycle-association-guards-and-closure*
*Completed: 2026-07-14*
