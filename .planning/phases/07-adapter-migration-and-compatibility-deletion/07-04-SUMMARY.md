---
phase: 07-adapter-migration-and-compatibility-deletion
plan: 04
subsystem: event-runtime
tags: [rust, typed-events, broadcast, compatibility, public-api]

requires:
  - phase: 07-adapter-migration-and-compatibility-deletion
    provides: Typed machine and interactive consumers from Plans 07-02 and 07-03
provides:
  - Typed receiver coverage for session, EventService, and stable public facade behavior
  - One sequenced ProductEvent broadcast with retained replay and lag mapping
  - Deleted CodingAgentEventReceiver and legacy session/EventService subscription paths
  - Fail-closed source guard for legacy receiver and duplicate broadcast reintroduction
affects: [07-05-compatibility-storage-deletion, client-lifecycle]

tech-stack:
  added: []
  patterns:
    - "Receiver tests assert CodingAgentProductEventKind payload families while preserving exact event order and identifiers."
    - "EventService retains once and broadcasts once through ProductEventReceiver."

key-files:
  created:
    - .planning/phases/07-adapter-migration-and-compatibility-deletion/07-04-SUMMARY.md
  modified:
    - crates/pi-coding-agent/src/coding_session/mod.rs
    - crates/pi-coding-agent/src/coding_session/event_service.rs
    - crates/pi-coding-agent/tests/public_api.rs
    - crates/pi-coding-agent/tests/event_boundary_guards.rs

key-decisions:
  - "Keep startup recovery emission at the typed subscription/query boundaries while deleting only the legacy subscription path."
  - "Retain ProductEvent compatibility_event raw storage through Plan 07-04; Plan 07-05 owns its compiler-guided deletion."
  - "Guard legacy receiver, subscription, duplicate sender/send, and scoped suppressions without broadening this plan into raw storage deletion."

patterns-established:
  - "Typed receiver authority: first-party session tests consume ProductEvent and external tests consume CodingAgentProductEventReceiver."
  - "Single publication path: assign sequence, retain ProductEvent, then send exactly once on the typed broadcast."

requirements-completed: [COMPAT-01, COMPAT-02]

coverage:
  - id: D1
    description: "Session, EventService, and public facade receiver tests consume typed product events while preserving recovery, durability, navigation, delegation, PartialCommit, ordering, and payload assertions."
    requirement: COMPAT-01
    verification:
      - kind: unit
        ref: "cargo test -p pi-coding-agent --lib coding_session::tests --quiet"
        status: pass
      - kind: unit
        ref: "cargo test -p pi-coding-agent --lib coding_session::event_service::tests --quiet"
        status: pass
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test public_api --quiet"
        status: pass
    human_judgment: false
  - id: D2
    description: "Legacy receiver/session subscription and duplicate CodingAgentEvent broadcast are deleted while typed replay, lag, sequence, and public contracts remain green."
    requirement: COMPAT-02
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test event_boundary_guards --test public_api --test product_event_contract --quiet"
        status: pass
      - kind: integration
        ref: "cargo test -p pi-coding-agent --quiet"
        status: pass
    human_judgment: false

duration: 9min
completed: 2026-07-13
status: complete
---

# Phase 07 Plan 04: Typed Receiver Migration and Legacy Broadcast Deletion Summary

**All first-party live receiver tests now consume typed product events, and EventService publishes one sequenced retained ProductEvent stream with the legacy receiver and duplicate broadcast removed.**

## Performance

- **Duration:** 9 min
- **Started:** 2026-07-13T09:47:34Z
- **Completed:** 2026-07-13T09:56:49Z
- **Tasks:** 2
- **Files modified:** 4 production/test files plus this summary

## Accomplishments

- Migrated every remaining receiver-dependent `coding_session`, `event_service`, and `public_api` test to `ProductEventReceiver` or `CodingAgentProductEventReceiver`, retaining exact typed payload, operation ID, recovery marker, durability, delegation, navigation, control, and `PartialCommit` evidence.
- Deleted `CodingAgentEventReceiver`, `CodingAgentSession::subscribe`, `EventService::subscribe`, the duplicate `broadcast::Sender<CodingAgentEvent>`, its send path, and obsolete path-scoped deprecation suppressions.
- Preserved sequence assignment, retention-before-broadcast ordering, bounded channel capacity, replay/cursor behavior, startup recovery emission, and Closed/Lagged error mapping on the single typed transport.
- Added a scoped fail-closed guard for the deleted receiver/subscription/sender surface while intentionally retaining `ProductEvent.compatibility_event` storage for Plan 07-05.

## Task Commits

Each task was committed atomically:

1. **Task 1: Migrate receiver-based session and facade tests** - `04945ea` (refactor)
2. **Task 2: Delete the legacy receiver and duplicate broadcast** - `c14691a` (refactor)

## Files Created/Modified

- `crates/pi-coding-agent/src/coding_session/mod.rs` - Typed co-located receiver assertions and removal of the legacy public session subscription/export.
- `crates/pi-coding-agent/src/coding_session/event_service.rs` - Typed receiver tests and the single ProductEvent sender/receiver transport.
- `crates/pi-coding-agent/tests/public_api.rs` - Stable facade subscription and typed workflow payload assertions.
- `crates/pi-coding-agent/tests/event_boundary_guards.rs` - Scoped absence checks for the legacy receiver, subscription, duplicate sender/send, and obsolete suppressions.
- `.planning/phases/07-adapter-migration-and-compatibility-deletion/07-04-SUMMARY.md` - Execution evidence and verification record.

## Decisions Made

- Startup recovery markers remain emitted through `subscribe_product_events`, `subscribe_product_events_public`, replay, and snapshot/query boundaries; deleting the legacy method does not move or duplicate recovery emission.
- The raw `ProductEvent.compatibility_event` field/accessor and its remaining white-box assertions stay intact in this plan because 07-05 owns that final storage deletion.
- EventService keeps the existing publication lock and retains the ProductEvent before broadcasting it, so sequence and replay authority do not change with the duplicate sender removal.

## Deviations from Plan

None - plan executed exactly as written.

## TDD Gate Compliance

Both tasks used compiler-guided RED/GREEN loops: changing receiver types and removing compatibility APIs first exposed the expected type/source failures, followed by typed assertion and guard updates until all gates passed. The plan requested one atomic commit per task, so the transient RED states were not committed separately.

## Issues Encountered

The test suite reports pre-existing dead-code/deprecation warnings, including the intentionally retained `compatibility_event` accessor that Plan 07-05 will delete. No test or verification failure remains.

The shared checkout also contains an unrelated modified `.planning/STATE.md` and untracked `docs/next stage.md`; both were left untouched and excluded from every 07-04 commit.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 07-05 can now delete raw `ProductEvent` compatibility storage and its remaining accessor/assertions against a single typed receiver transport. No receiver-side compatibility API remains to restore.

## Verification

- `cargo test -p pi-coding-agent --lib coding_session::tests --quiet` - pass (56 tests)
- `cargo test -p pi-coding-agent --lib coding_session::event_service::tests --quiet` - pass (25 tests)
- `cargo test -p pi-coding-agent --test public_api --quiet` - pass (23 tests)
- `cargo test -p pi-coding-agent --test event_boundary_guards --test public_api --test product_event_contract --quiet` - pass (19 + 23 + 1 tests)
- `cargo test -p pi-coding-agent --quiet` - pass (full crate suite; 656 library tests passed, 1 ignored, plus all integration targets)
- `cargo fmt --check` - pass
- `git diff --check` - pass
- Source audit confirms legacy receiver/session/EventService subscription and `Sender<CodingAgentEvent>` occur only as guard literals; `compatibility_event()` remains only in the explicitly deferred 07-05 storage/test locations.

## Known Stubs

None.

## Self-Check: PASSED

- [x] Summary exists at the required 07-04 path.
- [x] Task commits `04945ea` and `c14691a` exist in Git history.
- [x] All task acceptance criteria and plan-level focused gates pass.
- [x] `.planning/STATE.md` and `docs/next stage.md` remain untouched by 07-04 commits.
- [x] `ProductEvent.compatibility_event` storage remains present for Plan 07-05.

---
*Phase: 07-adapter-migration-and-compatibility-deletion*
*Plan: 04*
*Completed: 2026-07-13*
