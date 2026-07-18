# 0.4.0 Disposable Architecture Prototypes

These disposable, offline-capable prototypes validate decision semantics without
adding an extension runtime or production dependency in `0.4.0`. The JavaScript
contract fixture has no dependencies; the real toolchain fixtures use committed
lockfiles and remain outside the product workspace.

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

The JavaScript fixture alone does **not** close `ADR-003`; its engine simulation
is intentionally limited to Store/Instance memory isolation.

The separate real-engine harness is excluded from the product workspace and pins
Wasmtime `46.0.1`. Its runner forces all Cargo output under the project root:

```bash
bash tools/architecture-prototypes/run-wasmtime.sh
```

It proves per-Store memory isolation, a memory growth limit, deterministic fuel
exhaustion, epoch interruption of non-yielding async guest compute, host deadline
cancellation of a pending async Host API, explicit Store disposal, and successful
execution only in a fresh replacement Store.

The TypeScript fixture pins Jco, componentize-js, and TypeScript with an npm
lockfile. Its runner type-checks the guest, emits JavaScript, creates a Wasm
Component from the WIT world, and verifies the WIT embedded in the result. All
generated output is forced under the project root:

```bash
bash tools/architecture-prototypes/run-typescript-component.sh
```

Together these fixtures close the decision uncertainty needed for `ADR-003`.
Production Host API generation, budget enforcement, and final toolchain support
remain `0.4.2` `EKR-004` implementation concerns.
