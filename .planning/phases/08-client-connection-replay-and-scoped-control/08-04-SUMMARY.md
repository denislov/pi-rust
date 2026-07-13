---
phase: 08-client-connection-replay-and-scoped-control
plan: 04
status: complete
requirements: [CLIENT-01, CLIENT-02, CLIENT-03]
---

# Phase 08 Plan 04 Summary

**One mutex-owned SnapshotState now atomically coordinates client records, session/capability/operation projections, event cursor, retained replay, and recovery metadata.**

## Performance

- **Duration:** 10 min
- **Started:** 2026-07-13T16:45:03Z
- **Completed:** 2026-07-13T16:55:00Z
- **Tasks:** 2
- **Files modified:** 7

## Delivered

- Expanded `SnapshotState` into the sole authority for client generations and drafts, immutable session/capability/active-operation projection, capability generation source, event sequencing, retained replay, dropped-history metadata, and recovery revision.
- Kept `ClientService` as a zero-authority facade containing only the shared `Arc<SnapshotCoordinator>` and added owned atomic client snapshot delegation.
- Wired persistent and transient sessions, `EventService`, `OperationControl`, and `CapabilitySnapshotService` to the same coordinator instance.
- Removed the independent `EventPublicationState` mutex; event construction commits cursor/replay state under the coordinator and broadcasts only after releasing the lock.
- Added two-phase projection refresh for startup recovery, operation guard begin/drop, capability installation, fork/navigation, default-profile mutation, and plugin capability changes.
- Added topology guards, six one-to-one writer-order tests, a bounded Tokio deadlock test, coherent snapshot assertions, and direct coordinator authority tests.

## Verification

- `cargo fmt --check`
- `cargo test -p pi-coding-agent --lib snapshot_coordinator --quiet` (2 passed)
- `cargo test -p pi-coding-agent --lib client_service --quiet` (2 passed)
- `cargo test -p pi-coding-agent --test public_api snapshot_topology --quiet` (1 passed)
- `cargo test -p pi-coding-agent --test public_api snapshot_writers --quiet` (6 passed)
- `cargo test -p pi-coding-agent --lib coding_session::event_service::tests --quiet` (27 passed)
- `cargo test -p pi-coding-agent --lib coding_session::operation_control::tests --quiet` (7 passed)
- `cargo check -p pi-coding-agent --all-targets`
- `git diff --check`

## Commits

- `32e811e feat(phase-08): unify snapshot writer authority`
- `d61ac2f test(phase-08): lock snapshot writer ordering`
- `55a03e7 test(phase-08): exercise snapshot coordinator authority`

## Decisions

- Capability generation is coordinator-owned source state, not an independent counter in `CapabilitySnapshotService`; source and snapshot projection advance in one transaction.
- Retained product events and cursor allocation live directly in `SnapshotState`; the broadcast channel remains transport-only and sends after transaction release.
- Legacy `connect_client` inputs are imported into the sole client record before returning an owned coordinator snapshot, preserving current adapter behavior without restoring a second map.

## Deviations

- `session_service.rs` required no direct edit: persistence and replay IO already complete before the runtime owner performs navigation projection refresh, so the two-phase writer integration belongs at the existing `CodingAgentSession` orchestration boundary.
- Added direct coordinator tests after the first verification pass because the plan's `snapshot_coordinator` filter initially matched zero tests. This closes a verification gap without changing production behavior.

## Known Warnings

- Existing Phase 08 scaffolding for submitted-operation transitions and receipt capacity remains unused until later connection/control plans wire the public stateful API; `cargo check` reports those pre-existing dead-code warnings but succeeds.

## Self-Check: PASSED

- All three implementation/test commits are present in Git history.
- `08-04-SUMMARY.md` exists and records verification evidence.
- The unrelated untracked `docs/next stage.md` file was left untouched.
