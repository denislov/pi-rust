# ADR-002: Extension Grants And Operation Leases

- Status: Accepted
- Date: 2026-07-18
- Owner: `0.4.0` `RIF-006`
- Implementation: `0.4.2` `EKR-003`
- Prototype: `tools/architecture-prototypes/runtime-contracts.mjs`

## Context

Extension activation, registered contributions, long-lived subscriptions, and
individual privileged calls have different lifetimes. A single capability
boolean or mutable global permission object cannot express revocation,
operation identity, workspace/session scope, deadlines, or generation safety.
Passing product services or raw handles would also create ambient authority.

## Decision

Use three explicit levels:

```text
persisted GrantRecord
  -> revocable ExtensionInstanceGrant(generation)
  -> frozen OperationCapabilityLease(operation, generation, scope, deadline)
```

`GrantRecord` stores the reviewed permission decision for an extension identity,
source/trust context, declared scopes, and compatible contract version.

`ExtensionInstanceGrant` is created for one activated extension instance and
generation. It authorizes contribution/subscription definitions but exposes no
raw product service. Activation of a new grant installs a new generation.

Every privileged callback, command, tool, hook, action, data fetch, view refresh,
or job tick is admitted as an operation. Admission intersects the instance grant
with operation descriptor claims and creates an immutable lease bound to:

- extension/instance identity and generation;
- exact operation/root identity;
- permission and resource scope;
- workspace/session/branch/client association where applicable;
- deadline, cancellation, and quota budget.

Every Host API call validates these fields. A lease cannot mint another lease or
transfer a handle. Dependency permissions do not transfer; cross-extension calls
admit the callee under its own grant.

Revocation installs a new generation, blocks stale admission, closes affected
registrations, requests policy-defined cancellation for old operations, and
discards uncommitted late results. It does not pretend arbitrary trusted Rust
host code can be forcibly preempted.

## Alternatives Rejected

- ambient extension service/container access;
- one mutable permission object observed live by active operations;
- authorization only at contribution registration;
- inheritance of dependency grants;
- possession of raw receivers, repositories, providers, or adapter state.

## Security And Failure Consequences

Unknown permissions, scopes, generations, operations, or required features fail
closed. Secrets and grant internals are redacted from diagnostics/events. Lease
validation and revoke races are audited. Handles and buffers are bounded and
closed on cancel, revoke, update, session close, or shutdown.

## Prototype Evidence

The offline prototype proves operation binding, deadline checks,
stale-generation rejection, and absence of implicit dependency authority. It is
decision evidence, not the production Host API implementation.

## Verification

`EKR-003` must add grant/deny/revoke/stale-generation matrices, per-call scope and
deadline validation, late-result rejection, cross-extension denial, redaction,
and boundary tests forbidding raw services or ambient handles.
