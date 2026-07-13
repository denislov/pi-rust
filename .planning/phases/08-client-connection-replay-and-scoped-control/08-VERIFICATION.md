---
phase: 08-client-connection-replay-and-scoped-control
verified: 2026-07-14T00:00:00+08:00
status: passed
score: 4/4 roadmap must-haves verified
behavior_unverified: 0
behavior_unverified_items: []
---

# Phase 8: Client Connection, Replay, and Scoped Control — Verification Report

**Phase Goal:** Promote snapshot, retained replay, cursor recovery, submitted-operation, draft, and prompt-control foundations into a public reconnectable client contract.

**Verified:** 2026-07-14 (Asia/Shanghai)

## Goal Achievement

### Roadmap Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | A client can connect, receive a snapshot cursor, resume retained events, and handle stale cursors with a typed recovery result. | **VERIFIED** | `CodingAgentClientConnection::reconnect` now calls the generation-validated `EventService::recovery_boundary_after_for_client`, and `Replayed` carries converted replay events, the captured cursor, and the receiver established in the same critical section. The former coordinator-only `retained_events_after` path was deleted and a source guard prevents its return. |
| 2 | Reconnect semantics distinguish replayable history from fresh-snapshot-required recovery. | **VERIFIED** | Retained gaps remain `FreshSnapshotRequired(RetainedHistoryGap)` with exact requested/oldest metadata. `CodingAgentReconnectReceiver::{recv,try_recv}` converts broadcast lag into `FreshSnapshotRequired(LiveReceiverLag)` with an authoritative fresh client snapshot. The deterministic capacity-one public test exercises this branch. |
| 3 | Submitted operation and client-local draft state are queryable/mutable through stable APIs without exposing internals. | **VERIFIED** | `CodingAgentSnapshot` exposes complete typed drafts/submitted operation (`public_projection.rs:262-271`); connection methods delegate to the shared coordinator (`:289-410`); takeover/draft and submission lease tests pass. `CodingAgentSession::run` remains the ordinary dispatcher (`mod.rs:343-357`); no connection `run`/`submit` method is exported. |
| 4 | Abort, steer, and follow-up remain scoped control signals outside the ordinary operation queue. | **VERIFIED** | Public immutable `CodingAgentPromptControl` remains outside `run`. `public_scoped_control_receipts_are_idempotent_fifo_and_acceptance_clears_drafts` proves accepted Abort/Steer/FollowUp transport, same-id retry deduplication, payload conflict, distinct-id FIFO order, and acceptance-only draft removal. |

**Roadmap score:** 4/4 truths verified; Phase 08 completion criteria are satisfied.

### Plan Must-Haves and Artifacts

All 23 declared plan artifacts exist and are substantive according to `gsd-tools query verify.artifacts` (08-01: 3/3, 08-02: 4/4, 08-03: 1/1, 08-04: 3/3, 08-05: 4/4, 08-06: 4/4, 08-07: 4/4). Manual source inspection confirms the curated API re-exports, single `SnapshotCoordinator` registry, coordinator-backed client facade, submission lease, scoped control, and RPC migration are wired.

Plan link checker reported false negatives for links authored as symbols instead of relative paths (08-02, 08-03, 08-05, 08-06, 08-07). Those links were manually traced where possible. The 08-03 atomic EventService boundary is real and tested, but its absence from the public connection is the blocker above.

## Key Link Verification

| From | To | Status | Evidence |
|---|---|---|---|
| `CodingAgentSession::connect` | shared `SnapshotCoordinator` / `ClientService` | **WIRED** | `mod.rs:569-597`; connection stores the coordinator Arc and generation-scoped handle. |
| `CodingAgentClientConnection` | client state/drafts/submission | **WIRED** | `public_projection.rs:289-410`; all state mutations call coordinator methods and map typed errors. |
| `EventService::emit` | coordinator publication state | **WIRED** | `event_service.rs` publication tests and `public_api::snapshot_writers_5_event_commit_releases_coordinator_before_broadcast`. |
| `EventService::recovery_boundary_after` | public reconnect live receiver | **WIRED** | The client-scoped wrapper validates generation under the coordinator publication lock and returns replay/cursor/live receiver together; the public receiver projects lag as typed fresh-snapshot recovery. |
| public Prompt control | `PromptControlHandle` | **WIRED** | `snapshot_coordinator.rs:260-325`; owner/target checks precede sender enqueue and receipts are inserted only after send succeeds. |
| RPC state/prompt | public connection + canonical run | **WIRED** | `rpc/state.rs:153-201`, `rpc/prompt.rs:185-201, 401, 621, 799, 919-932, 995-1006, 1154-1193`; recursive runtime guards pass. |

## Requirements Coverage

| Requirement | Status | Evidence |
|---|---|---|
| CLIENT-01 | **SATISFIED** | Public connect/reconnect now returns the no-gap replay/live boundary, with explicit acknowledgement unchanged. |
| CLIENT-02 | **SATISFIED** | Both retained gap and public live receiver lag return typed fresh-snapshot metadata. |
| CLIENT-03 | **SATISFIED** | Typed drafts, submitted state, takeover, lease/drop/commit behavior, terminal retention, and exact terminal acknowledgement clearing are covered. |
| CONTROL-01 | **SATISFIED** | Abort/steer/follow-up remain outside ordinary `run`; accepted/retry/conflict/FIFO/draft-clear and rejection behavior are covered. |

## Automated Verification

- `cargo fmt --check` — passed.
- `cargo test -p pi-coding-agent` — passed (654 unit tests passed, 1 ignored, plus all integration/doc tests).
- Focused named tests for takeover, retained replay, submission lease, scoped rejection, atomic recovery boundary, receiver lag, and Prompt control transport — all passed.
- `cargo test -p pi-coding-agent --test api_boundary_guards --test product_runtime_boundary_guards --test rpc_mode --test protocol_events --quiet` — all passed.
- `cargo check --workspace` — passed.
- `git diff --check` — passed.
- Decision coverage: 21/21 context decisions honored (`check.decision-coverage-verify`).

Existing warnings include dead-code methods (`ProductEventReplayHandle`, coordinator transition helpers) and unused imports in RPC tests; they do not fail the build but are consistent with the missing public wiring noted above.

## Gap Closure

1. **Public replay/live handoff closed.** Public reconnect now consumes the atomic EventService boundary and returns the already-established typed receiver. RPC continues projecting the same replay events/cursor and ignores the receiver, so JSONL wire behavior is unchanged.
2. **Public behavior coverage closed.** Deterministic tests cover live lag recovery, accepted control receipt idempotency/FIFO/conflict/draft clearing, and terminal acknowledgement removal.

## Verification Metadata

**Verification approach:** Goal-backward with inversion/disconfirmation pass.
**Must-haves source:** ROADMAP success criteria plus all seven PLAN frontmatters.
**Automated checks:** artifact checks 23/23; focused and workspace checks passed; public replay/live wiring failed manual audit.
**Human checks required:** None; both previously unverified behaviors now have deterministic automated coverage.
**Workspace note:** pre-existing untracked `docs/next stage.md` was preserved.

---
*Verified by independent Phase 08 verifier on 2026-07-14.*
