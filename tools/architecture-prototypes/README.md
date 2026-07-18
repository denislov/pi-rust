# 0.4.0 Disposable Architecture Prototypes

These zero-dependency, offline prototypes validate decision semantics without
adding an extension runtime or production dependency in `0.4.0`.

Run from the project root:

```bash
node tools/architecture-prototypes/runtime-contracts.mjs
```

The prototype covers:

- `ADR-002`: GrantRecord -> ExtensionInstanceGrant -> operation-bound lease,
  stale-generation rejection, deadline/operation checks, and no implicit
  dependency permission transfer;
- part of `ADR-003`: immutable compiled WebAssembly module reuse with distinct
  per-invocation instances and guest memories;
- `ADR-004`: global/workspace state outside the session transaction, atomic
  SessionEvent/outbox staging, immutable facts, and candidate-generation
  activation fencing;
- `ADR-005`: two materially different semantic views, revision-based patches,
  deterministic stale-patch resync, and client-local transient state.

This prototype intentionally does **not** close `ADR-003`. The local environment
does not currently contain Wasmtime or a TypeScript/WIT componentizer. Acceptance
still requires a real engine fixture proving async Host API cancellation, Tokio
deadline integration, epoch interruption, deterministic fuel, memory/output
limits, and destruction of cancelled/trapped instances. Simulating those
properties in JavaScript would not be valid evidence.
