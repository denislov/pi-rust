# Phase 8: Client Connection, Replay, and Scoped Control - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-13
**Phase:** 8-Client Connection, Replay, and Scoped Control
**Areas discussed:** Connection public usage model, reconnect and stale-cursor recovery, submitted operation and client draft semantics, scoped control targeting and feedback

---

## Connection Public Usage Model

### Public connection shape

| Option | Description | Selected |
|--------|-------------|----------|
| Stateful connection handle | Client-scoped handle owns snapshot/reconnect, drafts, submitted state, and control behavior while ordinary operations remain canonical session operations. | ✓ |
| Stateless connection token | Return identity/cursor data and pass the token back to session methods for every client-scoped action. | |
| Hybrid model | Put snapshot/replay on a handle while keeping draft mutation and control on session methods that take client id. | |

**User's choice:** Stateful connection handle.
**Notes:** The handle organizes client-lifecycle behavior but must not replace `CodingAgentSession::run` as the ordinary-operation dispatcher.

### Same-id reconnect

| Option | Description | Selected |
|--------|-------------|----------|
| Restore the same client state | Treat client id as stable within a session and restore drafts, submitted operation, and cursor/recovery state. | ✓ |
| Fresh connection every time | Treat client id as a label and discard old client-local state on every connect. | |
| Explicit new versus resume | Require the caller to choose whether to create or restore client state. | |

**User's choice:** Restore the same client state.
**Notes:** Reconnect must preserve client-local continuity rather than silently create an empty projection.

### Duplicate connection ownership

| Option | Description | Selected |
|--------|-------------|----------|
| New connection takes over | Later connection becomes current; old handle receives a typed stale/disconnected result. | ✓ |
| Reject later connection | Permit only one active handle and fail reconnect until the old one is closed. | |
| Shared concurrent handles | Let every same-id handle mutate the same client state concurrently. | |

**User's choice:** New connection takes over.
**Notes:** Takeover avoids ghost connections blocking recovery and prevents concurrent handles from controlling the same client state.

### State read model

| Option | Description | Selected |
|--------|-------------|----------|
| Unified atomic client snapshot | Return session view, capabilities, active operation, cursor, complete drafts, and submitted operation at one consistent boundary. | ✓ |
| Separate session and client queries | Query session snapshot independently from connection-local state. | |
| Initial snapshot only | Depend on events after connect and reconnect again whenever a new snapshot is required. | |

**User's choice:** Unified atomic client snapshot.
**Notes:** Callers must not assemble a nominal snapshot from independently timed reads.

---

## Reconnect and Stale-Cursor Recovery

### Stale-cursor result

| Option | Description | Selected |
|--------|-------------|----------|
| Typed recovery result with fresh snapshot | Distinguish replay from `FreshSnapshotRequired` and include the authoritative recovery snapshot directly. | ✓ |
| Typed gap error | Return `EventStreamGap` and require a second snapshot call. | |
| Silent fallback | Hide the gap and return whichever recovery representation is available. | |

**User's choice:** Typed recovery result with fresh snapshot.
**Notes:** Callers need to know when they must rebuild their projection, but recovery should not require a race-prone second request.

### Replay-to-live handoff

| Option | Description | Selected |
|--------|-------------|----------|
| Atomic handoff | Establish replay and live subscription at one boundary so concurrent events fall into exactly one side. | ✓ |
| Separate recovery and subscription | Recover first, then subscribe in a second call. | |
| Continuation token | Return a token from recovery and exchange it for live subscription. | |

**User's choice:** Atomic handoff.
**Notes:** The contract must prevent both event gaps and handoff-created duplicates.

### Cursor advancement

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit applied-sequence acknowledgement | Advance only after the client confirms it applied the event; replay is at-least-once. | ✓ |
| Advance on delivery | Update cursor as soon as the receiver yields an event. | |
| Caller-owned cursor only | Keep no acknowledged cursor in connection state and rely entirely on reconnect input. | |

**User's choice:** Explicit applied-sequence acknowledgement.
**Notes:** Stable product-event sequence is the deduplication key. Delivery before a crash must not cause silent loss after reconnect.

### Fresh-snapshot recovery metadata

| Option | Description | Selected |
|--------|-------------|----------|
| Structured gap metadata | Include requested sequence, oldest available sequence, fresh cursor, and typed reason. | ✓ |
| Snapshot only | Return the replacement state without gap diagnostics. | |
| Generic message | Carry only a human-readable error string. | |

**User's choice:** Structured gap metadata.
**Notes:** Retention gaps and live receiver lag both require snapshot recovery but remain distinguishable typed reasons.

---

## Submitted Operation and Client Draft Semantics

### Submitted-operation coverage

| Option | Description | Selected |
|--------|-------------|----------|
| Any current canonical operation | Track any operation submitted by this client; only Prompt exposes scoped control. | ✓ |
| Prompt only | Preserve the current RPC-specific interpretation. | |
| Recent operation history | Retain multiple past submissions and their states. | |

**User's choice:** Any current canonical operation.
**Notes:** Submitted state is a general client-lifecycle projection, not a Prompt alias. Historical operation retention remains out of scope.

### Submitted lifecycle retention

| Option | Description | Selected |
|--------|-------------|----------|
| Accepted to Running to Terminal, then acknowledge | Keep terminal state until the client acknowledges the associated terminal event/outcome. | ✓ |
| Active only | Clear submitted state immediately when execution finishes. | |
| Retain until next submission | Treat the field as both current and most recent operation. | |

**User's choice:** Retain one complete lifecycle until terminal acknowledgement.
**Notes:** A reconnect between terminal publication and client application must not observe the operation as having vanished.

### Draft cardinality and order

| Option | Description | Selected |
|--------|-------------|----------|
| Single Prompt plus ordered control queues | One Prompt editor draft; ordered Steer and FollowUp queues with stable entry ids. | ✓ |
| One draft per kind | New content overwrites the prior Prompt, Steer, or FollowUp draft. | |
| One unified ordered list | Permit every draft kind to repeat in one global queue. | |

**User's choice:** Single Prompt plus ordered control queues.
**Notes:** Prompt editing and queued control have intentionally different cardinality. Full typed entries must be visible in the atomic snapshot.

### Automatic draft clearing

| Option | Description | Selected |
|--------|-------------|----------|
| Clear after runtime acceptance | Remove only after canonical admission or target control-channel acceptance; preserve on rejection. | ✓ |
| Clear on attempt | Remove before knowing whether the target accepted the action. | |
| Never clear automatically | Require explicit deletion even after success. | |

**User's choice:** Clear after runtime acceptance.
**Notes:** Busy, stale handle, target mismatch, target finished, validation failure, and similar rejection paths retain user input for retry.

---

## Scoped Control Targeting and Feedback

### Target binding

| Option | Description | Selected |
|--------|-------------|----------|
| Immutable operation-scoped handle | Bind client id, connection generation, and Prompt operation id; never retarget. | ✓ |
| Current-Prompt lookup | Resolve the connection's current Prompt at every control call. | |
| Operation id per call | Put abort/steer/follow-up methods on the connection and pass the target id every time. | |

**User's choice:** Immutable operation-scoped handle.
**Notes:** Old handles cannot accidentally control a later Prompt or survive a connection takeover.

### Control authority

| Option | Description | Selected |
|--------|-------------|----------|
| Submitting client only, recoverable after reconnect | Ownership follows submission; same-id reconnect can restore a new-generation handle for an active Prompt. | ✓ |
| Any connected client | Allow cross-client control within the session. | |
| Original handle only | Permanently lose control authority after a disconnect. | |

**User's choice:** Submitting client only, recoverable after reconnect.
**Notes:** Collaborative cross-client control would be a separate future capability.

### Control acknowledgement

| Option | Description | Selected |
|--------|-------------|----------|
| Typed enqueue receipt or typed rejection | Identify control and target; clearly distinguish enqueue from agent application; expose stable rejection reasons. | ✓ |
| `Result<(), CodingSessionError>` | Preserve the current private handle's minimal response. | |
| Wait for application | Block until the agent consumes the control or Prompt reaches a terminal state. | |

**User's choice:** Typed enqueue receipt or typed rejection.
**Notes:** Stable rejection reasons include stale connection, not owner, target mismatch, target not running, closed control channel, and invalid input.

### Retry, deduplication, and ordering

| Option | Description | Selected |
|--------|-------------|----------|
| Stable control id with idempotent retry | Scope deduplication by client, target operation, and control id; return the original receipt on retry and preserve distinct-control order. | ✓ |
| Duplicate every retry | Treat every call as a new queued control. | |
| No idempotency | Require the caller to infer uncertain enqueue state before retrying. | |

**User's choice:** Stable control id with idempotent retry.
**Notes:** Steer/FollowUp reuse draft ids as control ids. Identical text with different ids remains distinct. Abort follows the same idempotency rule.

---

## Agent Discretion

- Exact public Rust names and module layout.
- Internal synchronization, storage, and bounded-retention details.
- Exact API shape for acknowledging an applied event sequence.
- Derivation details for accepted/running transitions, subject to Phase 9 association closure.

## Deferred Ideas

- Detach/close, runtime shutdown, and teardown idempotency — Phase 9.
- Exhaustive operation/outcome/terminal-event association — Phase 9.
- Adapter-root and compile-fail guard hardening — Phase 9.
- Separately named runtime owner and multi-session daemon routing — future milestones.
- Collaborative cross-client Prompt control — future capability.
