# ADR-004: Extension State And Facts

- Status: Accepted
- Date: 2026-07-18
- Owner: `0.4.0` `RIF-006`
- Implementation: `0.4.3` `ESS-001`–`ESS-003`, `ESS-006`
- Prototype: `tools/architecture-prototypes/runtime-contracts.mjs`

## Context

Extensions need mutable preferences/workspace data, replayable session/branch
state, immutable historical evidence, and invocation scratch state. Storing all
of these in one plugin database would create a second session truth. Claiming an
atomic transaction across unrelated global/workspace and session stores would be
false and make crash recovery undefined.

## Decision

Use five scopes:

```text
global | workspace | session | branch | ephemeral invocation
```

- global/workspace mutable key-value state uses a namespaced extension state
  store with schema versions, quotas, compare/transaction, export, inspection,
  and deletion;
- session/branch mutations are generic versioned SessionEvents committed through
  SessionCoordinator and replayable without loading extension code;
- branch state inherits ancestor-visible mutations and diverges copy-on-write,
  with tombstones for deletion;
- `ExtensionFact` is an immutable historical envelope, not mutable KV state;
- invocation state lives only in the isolated Wasm instance.

Core SessionEvents, session/branch mutations, ExtensionFacts, committed
ProductEvent outbox obligations, and snapshot cursor may share one session
transaction. Global/workspace state never joins it.

Package/state update is a durable phase state machine. Candidate namespaces and
append-only candidate-generation session events remain inactive until an
activation record selects the generation. Atomicity is per store; cross-store
correctness comes from idempotence, generation fencing, explicit unavailable/
migrating states, and recovery. Committed history is never erased to fake
rollback.

Migrations are bounded pure-data transformations without model, network, shell,
secrets, other extensions, or arbitrary operations.

## Alternatives Rejected

- one extension database for session and global state;
- arbitrary extension-defined SessionEvent decoders required for replay;
- mutable facts or rewritten session history;
- cross-store distributed transaction claims;
- migration code with ambient Host API access.

## Prototype Evidence

The offline prototype separates global state from an atomic SessionEvent/outbox
batch, records an immutable fact, prepares candidate state without changing the
active generation, and crosses an explicit activation barrier.

## Verification

Replay/export without extension code; fork/branch inheritance; quota and schema
failure; missing/disabled extension inspection; per-store crash points;
candidate invisibility; lazy migration unavailability; generation fencing;
forward recovery; and absence of a second session store or fake transaction.
