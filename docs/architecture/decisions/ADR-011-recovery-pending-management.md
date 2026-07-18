# ADR-011: RecoveryPending Management

- Status: Accepted
- Date: 2026-07-18
- Owner: `0.4.0` `RIF-006`
- Implementation: `RIF-002`, `RIF-009`

## Context

After an append, manifest update, process interruption, or outbox transition, the
runtime may be unable to prove whether the intended durable finalization
committed. Returning success risks exposing uncommitted history; returning normal
failure risks retrying already committed effects. Caller exit or restart cannot
erase this uncertainty, and adapters cannot choose a resolution independently.

## Decision

`RecoveryPending` and `Recovering` are durable non-terminal operation states owned
by OperationSupervisor:

```text
Running -> RecoveryPending -> Recovering
                           -> RecoveryPending (bounded retry failed)
                           -> Completed | Failed | Aborted
```

### Evidence

An `InDoubt` finalization persists or references a durable recovery record with:

- recovery ID and original operation/root/parent IDs;
- operation kind and descriptor revision;
- session/runtime/capability generation associations;
- intended finalization semantic identity and durability requirement;
- transaction/session ranges or staged artifact references where known;
- last confirmed phase and uncertainty source;
- retry count, next-attempt time, safe diagnostic category, and audit history.

No secret, raw credential, unrestricted path, or provider payload is stored in
operator/client-visible evidence.

### Ownership Across Exit And Restart

OperationSupervisor retains the operation after a caller detaches or receives a
non-terminal receipt. Startup reconstructs pending ownership before accepting
conflicting work. Recovery retries use the original semantic identity and are
idempotent. The original operation eventually receives exactly one terminal; a
new replacement root is not created.

### Retry

Automatic retries are bounded, cancellable where safe, and use deterministic
backoff with recorded attempts. Recovery first inspects durable facts/outbox/
manifest state, then completes missing derived work or records evidence-backed
failure/abort. It never reruns an external side effect merely because its result
is absent from a projection.

### Operator Controls

Authenticated, authorized controls expose inspect, retry-now, and resolve. Every
control requires the recovery ID plus expected generation/version and emits a
redacted audit record. Operator resolution may select failure or abort with a
reason. Completion is permitted only when durable evidence proves the intended
commit. Deleting or ignoring a recovery record is not a resolution.

### Subsequent Work

The default policy blocks new SessionWriteRoot work for the affected session while
an unresolved record could conflict with its history or active leaf. Query,
committed ReadOnly, recovery controls, and unrelated sessions remain available.
Non-conflicting non-session work may continue. A descriptor may declare a
narrower block only with proof and tests; adapters cannot override it.

### Client Mapping

Rust API, ProductEvent, Snapshot, RPC, JSON, and TUI distinguish:

- accepted/running operation;
- non-terminal `RecoveryPending` receipt/status;
- recovery progress/control results;
- final Completed/Failed/Aborted terminal.

`RecoveryRequired` may be retained only as a compatibility spelling for a
non-terminal recovery receipt during an explicitly documented protocol window;
it cannot also mean terminal failure. Machine-readable modes remain protocol
clean, reconnect reconstructs pending recovery from a fresh snapshot, and an
outcome acknowledgement cannot synthesize a terminal event.

## Alternatives Rejected

1. **Treat uncertainty as failure.** Could duplicate already committed effects.
2. **Treat uncertainty as success.** Could expose history that never committed.
3. **Leave recovery to startup session replay only.** Loses runtime ownership,
   operator control, client visibility, and non-session associations.
4. **Let operators force success without evidence.** Corrupts durable truth.
5. **Block the whole runtime.** Unnecessarily prevents unrelated sessions and
   safe reads/controls.

## Failure And Security Consequences

Recovery storage failure before evidence is durable cannot be described as owned
recovery; admission/finalization must fail closed and preserve all locally known
diagnostics without publishing a terminal. The implementation must define a
durable recovery journal/record location and crash points before `RIF-002` closes.

Recovery controls are privileged and audited. Rate limits, bounded record size,
retention, redaction, generation checks, and denial behavior are mandatory.
Repeated automatic failure leaves the operation pending and visible; it does not
spin or silently discard the obligation.

## Compatibility And Migration

Existing incomplete-operation/startup-recovery facts are migrated or projected
into the new recovery inventory without fabricating normal completion. Supported
SessionEvent versions remain readable. Public protocol changes and any temporary
`RecoveryRequired` spelling receive explicit version negotiation and migration
notes.

## Verification

- uncertainty at every transaction/outbox/manifest crash point;
- caller detach/drop and process restart;
- bounded retry, repeated failure, retry-now, stale operator generation;
- evidence-backed failure/abort/completion rules;
- exactly one eventual terminal with original identity;
- affected-session write blocking with unrelated-session concurrency;
- snapshot/reconnect/RPC/JSON/TUI pending and resolution behavior;
- audit, authorization, redaction, pressure, retention, and shutdown matrices.
