# 0.5.5 Operation-Tree Failure Model

Status: frozen baseline for `RPF-001` on 2026-07-22. This document describes
the implementation owners and legal convergence states used by the 0.5.5 fault
tests. The normative runtime invariants remain in `runtime.md` and
`principles.md`.

## Terminal Classes

Every admitted operation tree must finish in exactly one class:

1. `Committed`: one authoritative durable terminal, at most one parent-visible
   delegation result, and no running or recovery-pending residue.
2. `DefiniteFailure`: cancellation or typed failure before a durable obligation,
   with no false completion and no orphan resource.
3. `RecoveryPending`: a durable boundary was crossed but completion cannot be
   proven. The recovery record retains the operation, root, and capability
   generation and no contradictory terminal is published.

`RecoveryPending` is not a fourth live execution state. It transfers ownership
from the live operation tree to durable recovery.

## Phase And Owner Inventory

| Phase | Sole owner | Cancellation/waiter/task lifetime | Terminal, persistence, and projection |
| --- | --- | --- | --- |
| root admission | `OperationScheduler` + `OperationControl` | root `OperationGuard` and root cancellation token | descriptor/finalization pipeline; no adapter authority |
| delegation authorization wait | `AuthorizationService` | one generation-scoped oneshot entry | authorization SessionEvents, EventService publication, snapshot projection |
| child admission/run | `OperationScheduler::admit_child` + `OperationControl` | `ChildOperationGuard`, child cancellation token, structured joined task | child runner returns to delegation executor; no detached terminal owner |
| child tool authorization wait | `AuthorizationService` | child operation-scoped oneshot entry | child authorization facts/events remain child-scoped |
| child finalization | child typed runner + scheduler permit | guard stays owned until runner/task exit | one typed child outcome; ProductEvent lineage comes from the admitted context |
| parent-result delivery | delegation tool executor | the awaited tool future owns delivery | one `DelegationToolResult` resumes the parent model loop |
| folded-state persistence | session transaction/writer | bounded writer command and reply | SessionEvent fact is authoritative; outbox carries publication obligation |
| committed terminal | operation finalization owner | cancellation is closed after the decision freezes | terminal fact/outbox first, EventService second, adapters only project |
| definite failure | phase owner before durable obligation | all owned waiters/guards/tasks are resolved or dropped | typed error/cancel; no durable success |
| recovery pending | repository/outbox recovery owner | live resources are released | durable recovery ID keeps operation/root/generation association |

## Required Fault Matrix

| Injection or race | Legal result | Forbidden residue or observation |
| --- | --- | --- |
| delegation wait abort/deny/approve/drop | one reject, cancel, or continuation | waiter, child admission after losing decision, duplicate tool result |
| child admission abort/shutdown/revoke | definite cancel/failure | child guard/task or actionable authorization |
| provider pending before first event | cancellation/timeout wakes once | detached stream/task or running root |
| partial provider output then error/late event | one failure/cancel with bounded child-only partial output | root transcript/model mutation or post-terminal delta |
| child tool authorization approve/deny/abort/reconnect | one child-scoped resolution | parent grant, orphan waiter, unrelated wakeup |
| child completion versus parent abort | exactly one winning terminal | committed completion rewritten to cancel or late completion accepted after cancel |
| parent result receiver drop/shutdown | definite failure or recovery pending at a durable boundary | detached execution or duplicate delivery |
| folded write definite failure | typed failure | false folded completion or terminal ProductEvent |
| manifest/outbox uncertainty | recovery pending | definite-success/definite-failure claim or lost identity |
| subscriber lag/retention gap | commit progresses and client performs explicit resync | blocked provider/commit or silent event loss |
| snapshot/live reconnect interleave | ordered deduplicated projection | duplicate child items or live-before-replay inversion |
| child retention overflow/active eviction | deterministic bounded eviction/fallback | stale page identity or root-state mutation |
| TUI I/O/panic/resize/image failure | balanced cleanup and valid main/child page | leaked terminal modes, cursor, focus, or composer input |
| shutdown in any active phase | closed admission and drained/cancelled owned tree | waiter, child, guard, task, running root, or lost durable obligation |

## Test-Only Resource Evidence

Production mutation authority is unchanged. `AuthorizationService::resource_snapshot`
is compiled only for unit tests and reports waiter IDs, operation-grant count,
and registry revision. `OperationControl::resource_snapshot` is compiled only
for unit tests and reports root IDs, child IDs, cancelled IDs, and released
owner IDs. These immutable observations let every fault case assert its
pre-case and post-case resource baseline without exposing a stable facade.

Future fault queues must follow the same rule: fault commands and queue
snapshots are directly `cfg(test)` and cannot add a production fallback.

## Frozen Capacity Baseline

| Resource | 0.5.4/0.5.5 baseline |
| --- | ---: |
| concurrent non-session roots | 4 |
| SessionTransactionWriter commands | 32 |
| ProductEvent broadcast channel | 128 |
| retained ProductEvents | 128 |
| client projection operations | 32 |
| client projection delegations | 32 |
| child conversations | 32 |
| queued UI events per child | 2,048 |
| rendered transcript items per child | 1,024 |
| interactive input chunks | 32 |
| prompt-task controls | 32 |

Runtime timings and repeated-schedule results are performance/release evidence,
not architectural constants. They are recorded under
`target/perf-baseline/0.5.5-operation-tree/` by `RPF-008`.

## Boundary Rules

- `runtime/scheduler.rs` is the only operation scheduler implementation.
- `AuthorizationService` and its single `AuthorizationState.pending` map are the
  only authorization waiter registry.
- operation finalization remains the only terminal decision authority;
  adapters never synthesize durable terminals.
- production source cannot contain fault queues, fault modes, or fault fallback
  branches. Injection is scoped to existing test-only provider, repository,
  writer, event, and virtual-terminal boundaries.
