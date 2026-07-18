# Architecture Testing And Evidence

## Principles

Tests prove owned contracts at the narrowest authoritative boundary. They do not
freeze private layout, duplicate the same behavior across targets, or replace a
missing owner with fixtures. Ordinary and smoke tests are deterministic, offline,
credential-free, and isolated from developer configuration.

## Ownership

| Crate | Owns test evidence for |
| --- | --- |
| `pi-ai` | provider request/stream conversion, auth/redaction, transport retry/error, catalog and registry invariants |
| `pi-agent-core` | agent transitions, tool ordering/concurrency/cancellation, hooks, Flow semantics, compaction, generic resources |
| `pi-tui` | terminal lifecycle, input, Unicode width, rendering/layout, resize, scrollback, component behavior |
| `pi-coding-agent` | admission/outcomes, identity/lineage, capabilities, session transactions/replay/recovery, ProductEvents/snapshots, extensions, adapters |

Provider wire matrices do not belong in product tests; product session matrices do
not belong in core; product UI semantics do not belong in `pi-tui`.

## Required Runtime Matrices

Every public operation is covered for applicable success, validation failure,
active cancellation before/during/finalize, provider/tool/Flow failure, definite
persistence failure, commit uncertainty, recovery, dropped caller/owner,
persistent/transient session, submission/direct API, and adapter paths.

The lifecycle matrix asserts:

```text
one admitted operation_id across facts/events/outcomes
resolved ProductEvent policy => exactly one authoritative terminal
RecoveryPending => zero terminal + durable supervisor ownership
OutcomeAcknowledgement => zero synthetic ProductEvent terminal
unknown operation_id => no projected root
completed case => no running or recovery-pending root
one authoritative descriptor row per operation
```

Owner-edge tests reject service locators, writer re-entry, provider/tool/extension
work inside the writer, direct adapter publication, projection repair, and a
second descriptor or session writer.

## State, Event, And Client Evidence

- transaction append success, definite failure, partial uncertainty, restart,
  idempotent recovery, and operator resolution;
- outbox crash points, unpublished replay, duplicate delivery, slow publisher,
  and semantic idempotence;
- snapshot consistency under concurrent commit, lag, reconnect, retention gap,
  restart, and live overlay reconciliation;
- bounded EventHub/client queues with explicit pressure/resync behavior;
- durable decoder and protocol negotiation across supported versions;
- machine-readable stdout cleanliness and redacted diagnostics.

## Extension Evidence

From `0.4.2`, offline fixtures cover TypeScript-to-Wasm build, package quarantine,
grant/lease/revoke, active cancellation, deadline, fuel, epoch interruption,
memory/output quotas, traps, cache isolation, forbidden imports, contribution
parity, state/facts, background work, update crash phases, and Workbench protocol.

Security tests include traversal, symlink, decompression, digest/lock, capability
escape, stale generation, revoke races, cancellation races, secret leakage,
unbounded buffers, and alternate-runtime absence.

## Test Value And Layout

Prefer table/property tests for closed contracts, focused unit tests for pure
logic, contract tests for public boundaries, scenario tests for cross-owner
behavior, and smoke tests only for terminal/process lifecycle. Do not create one
integration binary per provider/operation/field or assert incidental allocation,
private module layout, exact timing on uncontrolled hosts, or live provider
availability.

Tests that mutate process environment use the repository environment guards and
temporary `PI_RUST_DIR`. Test-only facades remain behind non-default
`test-support`.

## Release Validation

Every `0.4.x` release runs at least:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
scripts/tui-smoke.sh
```

Each release also runs its focused lifecycle, protocol, conformance, security,
generated-artifact, and provisional performance gates. Timing measurements are
separate from deterministic correctness assertions. A release records commands,
toolchain, relevant environment, results, versions, changelogs, task/debt closure,
and API/protocol evidence.
