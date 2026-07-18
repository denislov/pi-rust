# Current Architecture Evidence

## Evidence Stamp

Baseline version: `0.3.1`, released as annotated tag `v0.3.1`.

Source baseline: commit `870d4bb`; dated release record: `180f219`; post-baseline
`0.4.0` commits are recorded below. Last refreshed: 2026-07-18.

This file records implementation facts, not desired behavior. Cargo manifests,
compiled source, tests, and CodeGraph call paths outrank this summary when they
disagree. Every task that changes a listed fact must refresh the stamp and item.

## Workspace

- Active dependency edges are `pi-agent-core -> pi-ai` and
  `pi-coding-agent -> {pi-agent-core, pi-ai, pi-tui}`.
- `pi-ai` and `pi-tui` have no workspace dependencies.
- `pi-mom`, `pi-pods`, and `pi-web-ui` are placeholder crates.
- All workspace packages inherit version `0.3.1` from the root manifest.
- `pi-rust` is a placeholder binary; `pi-coding-agent` is user-facing.

## Runtime And Operations

- `CodingAgentSession` is the current product runtime facade and still owns a
  broad set of long-lived collaborators that `RIF-008` will decompose.
- `IntentRouter`, `OperationScheduler`, `OperationControl`, typed operation
  metadata, root/child lineage, capability snapshots, and generation-scoped
  cancellation exist.
- SessionWriteRoot, NonSessionRoot, RuntimeWrite, Query, ReadOnly, Child, and
  Control admission classes exist; the scheduler has no general work queue.
- PluginCommand, AgentInvocation, and AgentTeam have runtime-owned submitted task
  paths. Other operations still rely on the session facade as execution owner.
- Commit uncertainty is represented by `PartialCommit`, but supervisor-owned
  durable `RecoveryPending` lifetime across caller exit/restart is not complete.

## Events, Sessions, And Clients

- `SessionEventEnvelope`, transaction, append/replay, operation terminal facts,
  recovery markers, manifests, and snapshots exist.
- `CodingAgentProductEvent` is the typed client event envelope; `EventService`
  sequences and broadcasts it through a bounded retained stream.
- Snapshot/reconnect, stream identity, sequence gaps, capability generation,
  client projection, print/JSON, RPC, and interactive adapters exist.
- A durable ProductEvent outbox sharing the SessionEvent commit point does not yet
  exist; `RIF-009` owns it.
- The current transaction may append facts and then fail a manifest refresh,
  producing partial-commit uncertainty that startup recovery can inspect.

## Agent, Flow, And Extensions

- The production Agent turn still uses the generic string-action Flow engine;
  `0.4.1` owns typed state-machine convergence.
- Product fixed workflows still include Flow-based implementations.
- The current extension implementation includes first-party Rust contribution
  providers and Lua through `mlua`; the TypeScript/Wasm/WIT kernel does not exist.
- PluginLoad now uses the admitted snapshot operation ID and publishes typed
  Completed/Failed/Aborted root terminal evidence, recorded by commit `57e6a17`.
- Workbench semantic views, extension state/facts, package update coordination,
  and background extension services do not exist.

## Evidence Maintenance

The archived detailed `0.3.1` inventory is preserved in
[`migrations/0.3.1-monolithic-architecture.md`](migrations/0.3.1-monolithic-architecture.md).
It remains useful historical evidence but is no longer the normative contract.
