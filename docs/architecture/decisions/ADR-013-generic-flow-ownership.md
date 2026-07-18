# ADR-013: Generic Flow Ownership And Migration

## Status

Accepted 2026-07-19 for `0.4.1` `AWC-004`.

## Context

The workspace has a generic `Flow<C>` graph runner. It is useful for the Agent
turn's temporarily dynamic state machine and for deterministic tests, but
several product operations currently use the graph only to sequence fixed
steps. Treating all of these forms as one workflow abstraction obscures
cancellation, structured concurrency, and durable ownership boundaries.

## Decision

`Flow<C>` is an in-memory cooperative graph runner, never a durable workflow
engine. It remains available to the existing test/compatibility surface and as
the temporary migration scaffold for `AgentTurnFlow`. Fixed product flows move
to typed async pipelines; team/member fan-out moves to admitted structured
children; only genuinely branching Agent execution may retain state-machine
semantics, and that state machine must use typed states and transitions.

The migration inventory and ordering are normative in
[`flow-inventory-0.4.1.md`](../flow-inventory-0.4.1.md). `AWC-002` owns the Agent
state-machine replacement. `AWC-003` owns fixed pipelines and structured
concurrency. Neither task may add a new product dependency on generic Flow.

## Contract

- `FlowRunOptions::cancel` produces `FlowError::Cancelled` at graph boundaries.
- `max_steps` is a deterministic guard, not a durability or retry policy.
- Missing transitions are strict errors or explicit lenient completion according
  to the caller; they are never silently treated as durable success.
- Flow callbacks observe execution and cannot write sessions, publish terminal
  ProductEvents, grant capabilities, or resolve recovery.
- Flow contexts remain operation-local. Durable effects stay behind the
  SessionCoordinator and operation services.

## Alternatives Rejected

- **Make Flow the durable workflow engine:** rejected because it would duplicate
  SessionCoordinator/outbox/recovery ownership and make crash recovery implicit.
- **Delete Flow immediately:** rejected because AgentTurn still needs a
  migration scaffold and the generic graph tests are a supported compatibility
  surface during 0.4.1.
- **Keep every product Flow indefinitely:** rejected because fixed pipelines pay
  dynamic graph complexity and hide cancellation/structured child boundaries.

## Consequences

The generic Flow API is intentionally retained during this release, so
`AWC-D002` remains open until fixed workflow migration completes. New product
work must follow the inventory and cannot use Flow as a sequential-call wrapper.
The API/protocol surface changes are internal to 0.4.1 until the release
convergence task freezes the updated snapshot.

## Evidence

- `docs/architecture/flow-inventory-0.4.1.md` classifies every production Flow
  owner and test/compatibility use.
- `crates/pi-agent-core/tests/flow.rs` covers cancellation, max-step, strict and
  lenient missing-transition behavior.
- `crates/pi-coding-agent/tests/events_snapshot/event_boundary_guards.rs`
  prevents adapters from bypassing canonical production operation entry points.
