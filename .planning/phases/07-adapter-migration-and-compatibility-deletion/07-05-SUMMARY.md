---
phase: 07-adapter-migration-and-compatibility-deletion
plan: 05
subsystem: product-event-runtime
tags: [rust, typed-events, compatibility-deletion, source-guards, serde]

requires:
  - phase: 07-adapter-migration-and-compatibility-deletion
    provides: Typed protocol/interactive consumers and the single ProductEvent receiver/broadcast from Plans 07-01 through 07-04
provides:
  - Compatibility-free ProductEvent storage containing one typed payload plus independent metadata
  - Exactly-once raw-to-typed conversion at the internal EventService emit boundary
  - Final recursive deletion guards for raw storage, accessor, receiver, broadcast, conversion names, and scoped suppressions
  - Post-migration contract documentation retaining transitional serialized family/kind fields and the exhaustive 45-event inventory
affects: [phase-08-client-lifecycle, phase-09-association-closure, product-event-contract]

tech-stack:
  added: []
  patterns:
    - "Typed-only runtime envelope: EventService converts once, then ProductEvent retains and broadcasts only the owned typed payload."
    - "Deterministic raw fixtures use an explicitly named cfg(test) constructor unavailable to production builds."

key-files:
  created:
    - .planning/phases/07-adapter-migration-and-compatibility-deletion/07-05-SUMMARY.md
  modified:
    - crates/pi-coding-agent/src/coding_session/event.rs
    - crates/pi-coding-agent/src/coding_session/event_service.rs
    - crates/pi-coding-agent/src/coding_session/mod.rs
    - crates/pi-coding-agent/src/coding_session/public_event.rs
    - crates/pi-coding-agent/src/interactive/event_bridge.rs
    - crates/pi-coding-agent/src/interactive/loop.rs
    - crates/pi-coding-agent/src/interactive/prompt_task.rs
    - crates/pi-coding-agent/src/protocol/events.rs
    - crates/pi-coding-agent/src/protocol/json_mode.rs
    - crates/pi-coding-agent/src/protocol/rpc/event_queue.rs
    - crates/pi-coding-agent/src/protocol/rpc/events.rs
    - crates/pi-coding-agent/src/protocol/rpc/prompt.rs
    - crates/pi-coding-agent/src/protocol/rpc/state.rs
    - crates/pi-coding-agent/tests/event_boundary_guards.rs
    - crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs
    - docs/product-event-contract.md

key-decisions:
  - "Remove the duplicate private ProductEventKind hierarchy and derive the five root-terminal associations directly from the owned typed payload."
  - "Keep CodingAgentEvent only as the private EventService emit input and in explicitly named cfg(test) fixture construction; never retain or rebroadcast it."
  - "Retain transitional serialized family/kind fields while treating typed enums and snake_case Serde identity as authoritative."

patterns-established:
  - "Final event admission: raw CodingAgentEvent -> one exhaustive typed conversion in EventService::emit -> sequence retention -> one ProductEvent broadcast."
  - "Fail-closed compatibility audits split guard literals from forbidden tokens and exclude the source-guard file from recursive receiver-call scans."

requirements-completed: [COMPAT-01, COMPAT-02]

coverage:
  - id: D1
    description: "ProductEvent stores only the owned typed payload and independent sequence, operation, terminal, and durability metadata."
    requirement: COMPAT-02
    verification:
      - kind: unit
        ref: "cargo test -p pi-coding-agent --lib coding_session::event --quiet"
        status: pass
      - kind: unit
        ref: "cargo test -p pi-coding-agent --lib coding_session::event_service::tests --quiet"
        status: pass
      - kind: unit
        ref: "cargo test -p pi-coding-agent --lib coding_session::tests --quiet"
        status: pass
    human_judgment: false
  - id: D2
    description: "Production roots reject raw compatibility storage, accessors, receivers, broadcasts, obsolete conversion names, and path-scoped suppressions without guard-literal false positives."
    requirement: COMPAT-02
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test event_boundary_guards --test product_runtime_boundary_guards --quiet"
        status: pass
      - kind: other
        ref: "production source audits for compatibility symbols and allow(deprecated)"
        status: pass
    human_judgment: false
  - id: D3
    description: "RPC, interactive, public serialization, replay/overflow, durability, control/navigation, PartialCommit, and exactly five terminal associations remain behavior-compatible."
    requirement: COMPAT-01
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test product_event_contract --test protocol_events --test interactive_event_bridge --test public_api --quiet"
        status: pass
      - kind: integration
        ref: "cargo test -p pi-coding-agent --quiet"
        status: pass
      - kind: other
        ref: "cargo test --workspace --quiet"
        status: pass
      - kind: other
        ref: "cargo check --workspace"
        status: pass
    human_judgment: false
  - id: D4
    description: "The documented 45-event inventory, five root-terminal associations, and transitional serialized family/kind fields remain intact after compatibility deletion."
    requirement: COMPAT-01
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/event_boundary_guards.rs#full_event_inventory_is_source_fixture_and_document_complete"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/product_event_contract.rs#public_receiver_preserves_typed_order_and_metadata"
        status: pass
    human_judgment: false

duration: 18min
completed: 2026-07-13
status: complete
---

# Phase 07 Plan 05: Final Product Event Compatibility Deletion Summary

**The live event runtime now converts raw events once at admission, then stores, replays, and broadcasts only the exhaustive typed product-event payload while preserving all established adapter and wire behavior.**

## Performance

- **Duration:** 18 min
- **Started:** 2026-07-13T09:59:59Z
- **Completed:** 2026-07-13T10:18:08Z
- **Tasks:** 2
- **Files modified:** 16 production/test/documentation files plus this summary

## Accomplishments

- Deleted the final `ProductEvent` raw clone and compatibility accessor, along with the duplicate private kind/family hierarchy and obsolete `from_compat_event` construction path.
- Made `EventService::emit` the only production raw admission boundary: it derives metadata, performs one exhaustive typed conversion, retains the typed envelope, and publishes once on the existing ProductEvent broadcast.
- Migrated remaining event, EventService, session, protocol, RPC, and interactive white-box fixtures to typed payload assertions or the explicitly named `cfg(test)` raw constructor.
- Added recursive fail-closed guards for raw compatibility storage/transport and migration-path deprecation suppressions while preserving the 45-row inventory, transitional wire identity, and exactly five root-terminal associations.
- Verified the complete crate and workspace with deterministic offline tests, formatting, compilation, source audits, and diff checks.

## Task Commits

1. **Task 1 RED: Require typed-only ProductEvent storage** - `c857b99` (test)
2. **Task 1 GREEN/REFACTOR: Remove raw ProductEvent storage and migrate fixtures** - `845be5f` (refactor)
3. **Task 2: Close deletion guards and product-event contract** - `68ecfeb` (test/docs)

## Files Created/Modified

- `crates/pi-coding-agent/src/coding_session/event.rs` - Typed-only envelope, typed root-terminal matching, and cfg(test)-only raw fixture constructor.
- `crates/pi-coding-agent/src/coding_session/event_service.rs` - Exactly-once raw admission conversion with unchanged sequence/retention/broadcast order.
- `crates/pi-coding-agent/src/coding_session/mod.rs` - Typed recovery, navigation, profile, and delegation white-box assertions.
- `crates/pi-coding-agent/src/coding_session/public_event.rs` - Test fixture construction updated without changing transitional serialization.
- `crates/pi-coding-agent/src/interactive/` and `src/protocol/` fixture files - Deterministic raw fixture calls explicitly test-gated by the constructor contract.
- `crates/pi-coding-agent/tests/event_boundary_guards.rs` - Final recursive compatibility deletion and suppression guards.
- `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` - Deleted `subscribe` ledger entry and source-guard false-positive exclusion.
- `docs/product-event-contract.md` - Typed-only internal projection, retained wire identity, and raw-boundary documentation.

## Decisions Made

- Removed rather than renamed the duplicate private `ProductEventKind` taxonomy because the owned public typed payload is already exhaustive and directly supports root-terminal classification.
- Kept raw input at `EventService::emit` because first-party emitters still construct the private `CodingAgentEvent`; the raw value is converted once and dropped before retention/broadcast.
- Kept `family` and `kind` serialized compatibility fields because Phase 7 explicitly excludes wire removal; their values continue to be generated from the typed payload.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Corrected stale receiver expectations in the Stage 9 owner-method ledger**

- **Found during:** Task 2 workspace verification
- **Issue:** `product_runtime_boundary_guards` still required the `CodingAgentSession::subscribe` method deleted by Plan 07-04, so the full workspace gate failed despite the compatibility receiver being correctly absent.
- **Fix:** Moved `subscribe` from the retained-method ledger to the explicit absent ledger and excluded `event_boundary_guards.rs` from recursive call/suppression scans so guard literals are not reported as production consumers.
- **Files modified:** `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs`
- **Verification:** The focused owner ledger test and subsequent `cargo test --workspace --quiet` both passed.
- **Committed in:** `68ecfeb`

---

**Total deviations:** 1 auto-fixed bug.
**Impact on plan:** The correction closes a directly related stale guard and strengthens fail-closed compatibility deletion without changing runtime behavior or scope.

## TDD Gate Compliance

- RED commit `c857b99` added a source assertion that failed on the stored raw field/accessor.
- GREEN/REFACTOR commit `845be5f` removed the raw path and made the focused event/session suites pass.
- Task 2 then added the final cross-tree guards and documentation in `68ecfeb`.

## Issues Encountered

- Workspace verification exposed the stale Stage 9 ledger described above; it was fixed and the full gate was rerun successfully.
- Existing warnings for owner-private `load_plugins`, `ensure_idle`, and transitional `family`/`kind` assertions in `agent_profile_session` remain unchanged and non-blocking.

## Verification

- `cargo fmt --check` - pass
- `cargo test -p pi-coding-agent --lib coding_session::event --quiet` - pass (37 tests)
- `cargo test -p pi-coding-agent --lib coding_session::event_service::tests --quiet` - pass (25 tests)
- `cargo test -p pi-coding-agent --lib coding_session::tests --quiet` - pass (56 tests)
- `cargo test -p pi-coding-agent --test event_boundary_guards --test product_event_contract --test protocol_events --test interactive_event_bridge --test public_api --quiet` - pass
- `cargo test -p pi-coding-agent --quiet` - pass (657 passed, 1 ignored in the library target plus all integration targets)
- `cargo test --workspace --quiet` - pass
- `cargo check --workspace` - pass with existing warnings
- Production compatibility symbol/suppression audits - pass with zero matches
- Documented authoritative inventory row audit - pass (45 rows)
- `git diff --check` - pass

## Known Stubs

None introduced by this plan. The pre-existing interactive extension placeholder and unrelated availability message were not modified.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 7 is complete. Phase 8 can build the reconnectable client lifecycle contract on a single typed ProductEvent transport with no production raw compatibility storage, receiver, or broadcast path.

## Self-Check: PASSED

- [x] Task commits `c857b99`, `845be5f`, and `68ecfeb` exist in Git history.
- [x] All runtime, guard, contract, crate, and workspace gates passed after the final fix.
- [x] The authoritative inventory remains exactly 45 events and root-terminal associations remain exactly five.
- [x] `.planning/STATE.md` and `docs/next stage.md` were left untouched and excluded from all 07-05 commits.

---
*Phase: 07-adapter-migration-and-compatibility-deletion*
*Plan: 05*
*Completed: 2026-07-13*
