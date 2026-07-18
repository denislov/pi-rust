# ADR-003: Isolated Wasm Invocation

- Status: Accepted 2026-07-18; implementation remains scheduled for `0.4.2`
- Date: 2026-07-18
- Owner: `0.4.0` `RIF-006`
- Planned implementation: `0.4.2` `EKR-004`
- Evidence: `tools/architecture-prototypes/runtime-contracts.mjs`
- Real-engine fixture: `tools/architecture-prototypes/wasmtime-harness/`
- TypeScript/WIT fixture: `tools/architecture-prototypes/typescript-component/`

## Decision

Cache only immutable compiled Wasm Components by engine/version/target/digest.
Every admitted extension operation creates a distinct Store, Instance, resource
table, mutable guest memory, lease binding, deadline, fuel budget, and output
budget. Cancelled, trapped, OOM, out-of-fuel, invalid, or over-limit instances
are destroyed and never reused.

Invocation uses async Host API bindings, Tokio cancellation/deadlines, Wasmtime
epoch interruption, deterministic fuel, memory/table limits, host-call
allocation limits, and bounded output/progress/logs. Ambient WASI, detached guest
tasks, native loading, and shared mutable instance state are prohibited.

## Acceptance Evidence

The zero-dependency offline prototype compiles one immutable WebAssembly module,
instantiates it twice, mutates the first guest memory, and proves the second
memory remains isolated. This supports the no-shared-mutable-instance direction.

The locked Wasmtime `46.0.1` fixture then demonstrates with the real engine:

- reuse of an immutable compiled module while distinct Stores retain isolated
  guest memory;
- rejection of memory growth beyond a Store limit;
- deterministic fuel exhaustion;
- epoch interruption of non-yielding async guest compute;
- a Tokio deadline cancelling a pending async Host API call;
- disposal of the cancelled Store and successful execution only through a fresh
  replacement Store.

The locked TypeScript/Jco fixture strictly type-checks a TypeScript guest,
componentizes it against a WIT world, and reads the expected typed export back
from the generated Component. Both runners force generated output beneath the
project `target/architecture-prototypes/` directory.

Run the acceptance evidence from the repository root:

```bash
node tools/architecture-prototypes/runtime-contracts.mjs
bash tools/architecture-prototypes/run-wasmtime.sh
bash tools/architecture-prototypes/run-typescript-component.sh
```

`EKR-004` must still turn the accepted policy into production Component bindings
and cover the complete memory/table/output/host-allocation/resource-cleanup
matrix. Passing these disposable fixtures does not authorize production native
loading, ambient WASI, instance pooling, or mutable Store reuse.

## Alternatives Rejected

- one long-lived Store/Instance per extension;
- pooled mutable instances reset between calls;
- process-per-invocation isolation;
- cooperative guest cancellation without epoch/fuel enforcement.

The first two conflict with memory/resource isolation; process isolation carries
higher startup/coordination cost; cooperative-only cancellation cannot bound
malicious or stuck compute. The real-engine evidence confirms fuel and epoch
interruption can bound non-yielding compute without mutable instance pooling.
