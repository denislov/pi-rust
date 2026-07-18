# Coding-Agent Runtime Contract

## Status

Normative target for `0.4.0`. Items not yet present are listed as gaps in
`current-state.md` and must not be described as current behavior.

## Owner Graph

```text
Adapter
  -> OperationSupervisor
  -> Typed Workflow
  -> SessionCoordinator
  -> committed outbox
  -> OutboxPublisher
  -> EventHub
  -> ClientProjectionCoordinator
  -> TUI / RPC / JSON / Embedding
```

- `RuntimeHost` constructs these owners, coordinates lifetime/shutdown, and
  exposes supported facades. It is not an operation-time service locator.
- `OperationSupervisor` owns admission, immutable execution identity, lineage,
  permits, cancellation, terminal authority, `RecoveryPending`, and recovery
  ownership.
- `SessionCoordinator` owns one bounded logical writer per persistent session,
  transaction execution, the committed read model, durable ProductEvent outbox,
  and snapshot cursor.
- `OutboxPublisher` delivers committed obligations and records delivery progress;
  it cannot decide outcomes or mutate session truth.
- `EventHub` sequences/fans out live and committed events under bounded pressure;
  it cannot invent durable facts or terminals.
- `ClientProjectionCoordinator` creates snapshots, applies committed events and
  transient overlays, and detects gaps; it cannot repair session storage.

Prohibited edges are enforced by boundary tests: adapters do not call workflows
or repositories, EventHub does not call SessionCoordinator mutation, projections
do not append facts, and SessionCoordinator does not call providers, tools,
extensions, publishers, or clients inside the writer.

## Operation Execution

Admission creates one immutable `OperationExecution`:

```text
operation_id
root_operation_id
parent_operation_id
operation kind and descriptor revision
initiator/origin
capability generation
admitted runtime/session identity
admitted_at
```

Root IDs share one uniqueness domain. Child admission requires a registered live
parent, preserves root lineage, and cannot silently outlive it. Workflows,
services, adapters, transactions, plugins, and recovery cannot generate or
replace an admitted root ID.

One authoritative descriptor table declares, per public operation:

- operation kind and public outcome family;
- terminal policy (`ProductEvent` or exact outcome acknowledgement);
- session access (`None`, `Read`, `Write`);
- admission class and dispatch owner;
- priority and capacity claims;
- durability/finalization requirements;
- cancellation and child policy;
- required capabilities and scheduling inputs.

Legacy class/mode projections may be derived from this table during migration,
but no second hand-maintained registry may survive release.

## Admission And Concurrency

| Class | Rule |
| --- | --- |
| Query | no transaction; admitted unless shutdown/policy rejects |
| ReadOnly | committed state only; may coexist with a writer |
| SessionWriteRoot | at most one active writer per session |
| NonSessionRoot | bounded runtime concurrency, no session mutation |
| RuntimeWrite | generation-safe or exclusive runtime mutation |
| Child | requires a live parent and bounded structured lifetime |
| Control | priority signal; never commits session facts |

Controls cannot starve behind ordinary work. Closing admission during shutdown or
extension update rejects new work deterministically while existing owned work
drains, cancels, completes, or enters recovery.

## Lifecycle And Finalization

```text
Admitted -> Running -> Completed | Failed | Aborted
                    -> RecoveryPending -> Recovering
                                       -> Completed | Failed | Aborted
```

`RecoveryPending` and `Recovering` are durable non-terminal states. A caller that
cannot remain attached receives a typed receipt containing the original
operation ID and recovery ID; it does not receive a contradictory success/failure
terminal.

The only durable finalization path is:

```text
OperationSupervisor freezes FinalizationDecision / CommitIntent
  -> SessionCoordinator validates admitted identity and stages
     SessionEvents + terminal/outcome ProductEvent outbox
  -> one session consistency point returns
     Committed | DefinitelyFailed | InDoubt(recovery_id)
  -> OperationSupervisor publishes/acknowledges terminal,
     or retains RecoveryPending ownership
```

The decision contains the intended terminal class, semantic event identity,
durability requirement, operation/root lineage, capability generation, and safe
error/abort payload. SessionCoordinator may reject an invalid decision but cannot
reinterpret it. Supervisor cannot advertise durable success before `Committed`.

- validation or definite pre-commit failure resolves `Failed`;
- user/host cancellation resolves `Aborted` after already committed effects are
  represented truthfully;
- uncertain append/manifest/outbox state resolves `InDoubt` and emits no
  speculative terminal;
- terminal and outbox identities are idempotent across retry/redelivery;
- outcome acknowledgement cannot synthesize a ProductEvent terminal.

## Session Consistency And Outbox

Each persistent session uses one bounded Tokio command actor or an ADR-approved
equivalent with the same ownership properties. Different sessions stay
concurrent. The writer command stages or validates pure data; provider/tool/
extension work completes outside it.

```text
stage SessionEvents + committed ProductEvent obligations
  -> append both or persist in-doubt evidence
  -> advance committed read model and snapshot cursor
  -> return commit result
  -> publish committed outbox asynchronously
  -> recover unpublished obligations after restart
```

SessionEvent remains authoritative. Outbox records are durable delivery
obligations derived from committed facts/decisions, not a second business-history
store. Snapshot state and cursor are captured at one consistency point:

```text
UiState(N) = Snapshot(N) + matching ProductEvents after N in order
```

Slow subscribers never block commit. Retention exhaustion or sequence gaps cause
an explicit fresh-snapshot/resync requirement. Live optimistic output remains a
transient overlay until committed facts reconcile it.

## Recovery

Startup scans durable non-terminal operations, incomplete message/tool families,
commit/outbox uncertainty, cursor mismatch, and staged artifacts. Recovery uses
bounded retries/backoff and idempotent semantic identities. It preserves the
original operation/root/capability associations and resolves exactly one
terminal.

Authenticated operator controls may inspect, retry, or choose an evidence-backed
resolution. Normal completion requires durable proof. Every action is audited and
redacted. Per-session subsequent writes are blocked or allowed according to the
accepted recovery policy; no adapter makes this decision locally.

## Product Events, Snapshots, And Adapters

ProductEvents have a common typed envelope with stream sequence, operation/root/
parent/session association, capability generation, durability, semantic family,
and optional authoritative terminal association. Adapters consume only
ProductEvents and snapshots for runtime projection.

Print/JSON/RPC/TUI/embedding adapters are peers. They parse syntax, construct
typed intents, display admission results, apply events, request snapshots, and
maintain client-local state. Machine-readable stdout stays protocol-clean.

Each connection has a bounded subscription and cursor. Detach does not imply
abort unless explicit policy says so. Abort is a shared runtime control visible
to authorized clients. Stale shared mutations use expected cursor/generation and
fail closed.

## Shutdown

Shutdown closes admission, rejects new children, requests policy-defined
cancellation, drains owner trees and session writer commands, persists recovery
evidence for in-doubt work, flushes committed outbox progress within a bounded
deadline, disconnects clients, and terminates deterministically. Dropping an
adapter, workflow future, or operation context cannot discard a durable
obligation or release recovery ownership.
