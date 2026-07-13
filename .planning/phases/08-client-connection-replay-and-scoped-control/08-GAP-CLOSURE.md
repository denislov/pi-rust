---
phase: 08-client-connection-replay-and-scoped-control
type: gap_closure
status: complete
closed: 2026-07-14
requirements: [CLIENT-01, CLIENT-02, CLIENT-03, CONTROL-01]
---

# Phase 08 Gap Closure

## Root Cause

Plan 08-03 implemented an atomic retained-replay/live-receiver boundary in `EventService`, but Plan 08-05's public connection facade independently read `SnapshotCoordinator::retained_events_after`. That second path returned only replay values and a cursor, so the public API could neither guarantee the replay-to-live handoff nor surface broadcast lag as `LiveReceiverLag`.

The control and submitted-operation implementations were present, but their accepted/idempotent/FIFO/draft-clear and terminal acknowledgement transitions lacked public boundary tests.

## Changes

- `CodingAgentClientConnection::reconnect` now uses a client-generation-validated EventService recovery boundary.
- `CodingAgentReconnect::Replayed` carries replay events, the atomic cursor, and `CodingAgentReconnectReceiver`.
- `CodingAgentReconnectReceiver` projects live events and typed `FreshSnapshotRequired(LiveReceiverLag)` recovery.
- The non-atomic `SnapshotCoordinator::retained_events_after` path was deleted and guarded by source tests.
- RPC replay projection ignores the new receiver and preserves existing protocol events/errors.
- Deterministic tests prove public live lag, control receipt retry/conflict/FIFO/draft clearing, and exact terminal acknowledgement clearing.

## Verification

- Focused public API, EventService, RPC, and boundary suites passed.
- `cargo test -p pi-coding-agent` passed (654 unit tests passed, 1 ignored, plus all integration/doc tests).
- `cargo test --workspace` passed.
- `cargo check --workspace` passed.
- `cargo fmt --all --check` passed.
- `git diff --check` passed.

## Deviations

- The existing API boundary guard's text window ended at the old generic receiver declaration. It was narrowed to end at the new reconnect receiver declaration so it continues to reject raw receiver leakage from `CodingAgentClientConnection` without misclassifying the typed wrapper implementation.

## Self-Check: PASSED
