# ADR-003: Isolated Wasm Invocation

- Status: Proposed; implementation remains blocked
- Date: 2026-07-18
- Owner: `0.4.0` `RIF-006`
- Planned implementation: `0.4.2` `EKR-004`
- Partial prototype: `tools/architecture-prototypes/runtime-contracts.mjs`

## Proposed Decision

Cache only immutable compiled Wasm Components by engine/version/target/digest.
Every admitted extension operation creates a distinct Store, Instance, resource
table, mutable guest memory, lease binding, deadline, fuel budget, and output
budget. Cancelled, trapped, OOM, out-of-fuel, invalid, or over-limit instances
are destroyed and never reused.

Invocation uses async Host API bindings, Tokio cancellation/deadlines, Wasmtime
epoch interruption, deterministic fuel, memory/table limits, host-call
allocation limits, and bounded output/progress/logs. Ambient WASI, detached guest
tasks, native loading, and shared mutable instance state are prohibited.

## Evidence Present

The zero-dependency offline prototype compiles one immutable WebAssembly module,
instantiates it twice, mutates the first guest memory, and proves the second
memory remains isolated. This supports the no-shared-mutable-instance direction.

## Missing Acceptance Evidence

ADR acceptance still requires a real engine prototype demonstrating:

- async Host API calls cancelled by an operation token;
- a Tokio deadline covering guest compute and host awaits;
- epoch interruption of non-yielding guest compute;
- deterministic fuel exhaustion;
- memory/table/output/host-allocation limits;
- destruction and non-reuse after cancel/trap/limit;
- resource-table cleanup and immutable compiled cache reuse.

The local environment currently has no Wasmtime binary/crate cache and no
TypeScript/WIT componentizer. JavaScript simulation would not prove these engine
properties. No production extension runtime may land while this ADR is Proposed.

## Alternatives Pending Final Review

- one long-lived Store/Instance per extension;
- pooled mutable instances reset between calls;
- process-per-invocation isolation;
- cooperative guest cancellation without epoch/fuel enforcement.

The first two conflict with memory/resource isolation; process isolation carries
higher startup/coordination cost; cooperative-only cancellation cannot bound
malicious or stuck compute. The real-engine prototype must quantify these claims
before final acceptance.
