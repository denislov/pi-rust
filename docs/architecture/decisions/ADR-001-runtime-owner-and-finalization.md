# ADR-001: Runtime Owners And Typed Finalization

- Status: Accepted
- Date: 2026-07-18
- Owner: `0.4.0` `RIF-006`
- Implementation: `RIF-001`, `RIF-002`, `RIF-007`, `RIF-008`, `RIF-009`

## Context

The `0.3.1` runtime concentrates admission, operations, session persistence,
events, snapshots, clients, capabilities, plugins, and shutdown around
`CodingAgentSession` and collaborating services. Identity and lifecycle behavior
has improved, but mutable authority still overlaps. A durable session terminal
must be decided by operation policy while being committed atomically with session
facts and an outbox by the session writer. Letting either side own both concerns
creates a service locator, duplicate business decision, or pre-commit terminal.

## Decision

Use the following one-way collaborator graph:

```text
Adapter
  -> OperationSupervisor
  -> Typed Workflow
  -> SessionCoordinator
  -> committed outbox
  -> OutboxPublisher
  -> EventHub
  -> ClientProjectionCoordinator
```

`RuntimeHost` is the composition/lifetime root and supported facade owner. It is
not passed to operations as a mutable service container.

### OperationSupervisor

Owns admission, immutable `OperationExecution`, root/parent lineage, descriptor
resolution, permits/capacity, cancellation, terminal authority,
`RecoveryPending`, operator recovery commands, and shutdown ownership.

### SessionCoordinator

Owns one bounded logical writer per persistent session, transaction validation and
commit, committed read model, durable ProductEvent outbox, and snapshot cursor.
It does not run providers/tools/extensions/publishers/clients or decide business
outcomes.

### Publication And Projection

OutboxPublisher delivers committed obligations without deciding outcomes.
EventHub sequences and fans out under bounded pressure without session mutation.
ClientProjectionCoordinator builds client views and overlays without durable
repair.

### Typed Finalization

OperationSupervisor freezes an immutable `FinalizationDecision`/`CommitIntent`
containing admitted identity/lineage, descriptor revision, intended terminal,
durability requirement, semantic outbox identity, capability generation, and
safe payload.

SessionCoordinator validates the command and returns exactly one of:

- `Committed` with committed session/outbox/cursor references;
- `DefinitelyFailed` with a typed safe failure;
- `InDoubt(recovery_id)` with durable uncertainty evidence.

Only OperationSupervisor maps this result to `Completed`, `Failed`, `Aborted`, or
non-terminal `RecoveryPending`. It cannot publish durable success before
`Committed`; SessionCoordinator cannot reinterpret the intended business result.

## Alternatives Rejected

1. **Keep `CodingAgentSession` as the mutable owner of everything.** Rejected
   because authority remains overlapping and operation tasks cannot have clear
   restart/shutdown ownership.
2. **Let SessionCoordinator decide terminal from transaction result.** Rejected
   because persistence would own business policy and outcome-only operations
   would not fit.
3. **Let workflow publish terminal then persist.** Rejected because durable
   success could precede commit and retry could duplicate terminals.
4. **Publish from the writer.** Rejected because slow clients/publication would
   enter the consistency point.
5. **Use a global lock instead of per-session writers.** Rejected because
   independent sessions and non-session roots must remain concurrent.

## Prohibited Edges

- Adapter -> workflow/Flow node/repository/provider.
- Workflow -> raw RuntimeHost/service container/EventHub terminal publication.
- SessionCoordinator writer -> provider/tool/extension/client/outbox delivery.
- EventHub/OutboxPublisher/Projection -> session mutation or outcome decision.
- Any collaborator -> generation of a replacement admitted root ID.
- A second descriptor registry, session writer, terminal authority, or durable
  session source.

## Failure And Security Consequences

The typed handoff makes authority auditable and prevents forged success. Commands
are validated against the live admitted identity, descriptor, capability
generation, session, and writer generation. Duplicate semantic finalizations are
idempotent; conflicting decisions fail closed. Secrets and authorization material
are excluded from public/outbox payloads. Unknown operation IDs cannot create
projected roots.

Writer queue, EventHub, publisher, and client buffers are bounded. Slow delivery
cannot block commit, abort, revocation, or recovery. Dropped callers do not drop
owned operation/recovery state.

## Compatibility And Migration

Internal service and event wiring may break during `0.4.0`. Public Rust APIs,
ProductEvent, Snapshot, and RPC changes require explicit release evidence.
Supported SessionEvent logs retain versioned decoding; the migration cannot make
ProductEvent or snapshots the durable session truth.

## Verification

- one identity across admission, transactions, facts, outbox, outcomes, controls,
  snapshots, and adapters;
- one terminal or one explicit recovery owner per ProductEvent-policy root;
- descriptor exhaustiveness and invalid-combination tests;
- prohibited-edge/boundary tests;
- per-session writer concurrency/pressure tests;
- finalization success/definite-failure/in-doubt crash matrix;
- outbox redelivery and snapshot consistency matrix;
- deterministic shutdown with live children, slow clients, and recovery-pending
  operations.
