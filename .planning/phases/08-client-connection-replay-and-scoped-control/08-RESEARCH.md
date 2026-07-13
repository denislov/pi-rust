# Phase 8: Client Connection, Replay, and Scoped Control - Research

**Researched:** 2026-07-13  
**Domain:** Rust/Tokio session-owned reconnectable client contracts  
**Confidence:** HIGH for repository architecture and existing behavior; MEDIUM for Tokio API semantics (Context7 was unavailable in this runtime)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

### Stateful Client Connection

- **D-01:** `connect` returns a stateful, client-scoped connection handle. Snapshot, reconnect/replay, draft state, submitted-operation state, and Prompt control are organized around that handle; it must not become a second ordinary-operation dispatcher alongside `CodingAgentSession::run`.
- **D-02:** `CodingAgentClientId` is a stable identity within one session. Reconnecting with the same id restores that client's drafts, submitted-operation state, and cursor/recovery state rather than creating an empty client projection.
- **D-03:** A newer connection for the same client id takes over the identity. The prior connection generation becomes stale, and later state mutations or control attempts through an old handle return a typed stale/disconnected result.
- **D-04:** The connection exposes one atomic client snapshot containing the session view, capabilities, active operation, cursor, full typed drafts, and submitted operation. Do not require callers to combine independently timed session and client-state reads.

### Replay and Recovery

- **D-05:** Reconnect returns a typed recovery result. A replayable cursor yields a replay branch; a stale cursor or unrecoverable live-stream lag yields `FreshSnapshotRequired` with the authoritative fresh snapshot. Snapshot recovery is neither an opaque ordinary error nor a silent fallback.
- **D-06:** Reconnect establishes the replay-to-live boundary atomically. Every event published during recovery belongs to exactly one side of the handoff—replay batch or live stream—with no omission and no duplicate caused by the handoff itself.
- **D-07:** The connection cursor advances only when the client explicitly acknowledges that it successfully applied a product-event sequence. Delivery alone must not advance the recovery cursor.
- **D-08:** Replay is at-least-once. Stable product-event sequence is the deduplication authority; the contract must not claim exactly-once delivery at the cost of losing an event delivered immediately before a disconnect.
- **D-09:** `FreshSnapshotRequired` exposes structured recovery metadata: requested sequence, oldest available sequence, fresh snapshot cursor, and a typed reason that distinguishes retained-history gaps from live receiver lag.

### Submitted Operation and Drafts

- **D-10:** Submitted-operation state represents the current canonical operation submitted by this client, regardless of operation kind. Prompt alone receives abort/steer/follow-up control; other operation kinds expose observable submitted state only.
- **D-11:** Submitted-operation status progresses monotonically through `Accepted`, `Running`, and `Terminal { status }`. Terminal state remains visible until the client acknowledges the associated terminal event/outcome; it must not disappear immediately when execution ends.
- **D-12:** Each client has at most one Prompt draft. Updating the Prompt draft replaces its text.
- **D-13:** Steer and FollowUp drafts are separate ordered queues. Every queued entry has a stable draft id so callers can update, delete, submit, retry, and deduplicate it precisely across reconnects.
- **D-14:** The atomic client snapshot exposes complete typed draft entries rather than only `client_draft_count`.
- **D-15:** A draft clears automatically only after its target action is accepted by the runtime: a Prompt after canonical-operation acceptance and a Steer/FollowUp entry after the target Prompt control channel accepts it. Busy, stale-connection, target mismatch, target-finished, validation, or other rejection paths preserve the draft for correction, retry, or explicit deletion.

### Prompt-Scoped Control

- **D-16:** Public Prompt control uses an immutable operation-scoped handle bound to client id, connection generation, and Prompt operation id. It never automatically retargets when a later Prompt becomes current.
- **D-17:** Only the client that submitted a Prompt owns its control authority. A takeover reconnect for the same client id may restore a new-generation handle while that Prompt remains active; every old-generation handle is invalidated. Other client ids cannot control the Prompt merely by knowing its operation id.
- **D-18:** A successful control call returns a typed enqueue receipt containing stable control identity, target operation identity, and control kind. `Enqueued` means the target Prompt's control channel accepted the command; it does not claim that the agent has already applied it.
- **D-19:** A rejected control returns a stable typed reason rather than requiring error-string parsing. The public rejection vocabulary must cover at least stale connection, not owner, target mismatch, target not running, control channel closed, and invalid input.
- **D-20:** Every control has a stable control id. A Steer or FollowUp submitted from a draft reuses that draft id. Idempotency is scoped by client id, target operation id, and control id: retry returns the original enqueue receipt without a duplicate enqueue, while identical text under different ids remains distinct.
- **D-21:** Distinct controls accepted from one connection preserve enqueue order. Abort follows the same stable-id and idempotent-retry contract.

### the agent's Discretion

- Choose exact Rust type, enum, method, and module names while keeping all stable contracts under `pi_coding_agent::api` and internal services, queues, operation metadata, and Flow nodes private.
- Choose the internal synchronization and storage strategy for client generations, acknowledgements, receipts, and idempotency records, provided the observable takeover, atomicity, ordering, and recovery decisions above hold.
- Choose retained-event and idempotency-record capacity/configuration details. The existing retained capacity of 128 is an implementation baseline, not a newly locked public constant.
- Choose how accepted/running transitions are derived from existing admission and event boundaries. Phase 9 still owns exhaustive operation/outcome/terminal association closure.

### Deferred Ideas (OUT OF SCOPE)

- Client detach/close, runtime shutdown, and idempotent lifecycle teardown — Phase 9.
- Exhaustive operation id/submitted state/terminal outcome/terminal event association — Phase 9.
- Adapter-root discovery and external compile-fail diagnostic hardening — Phase 9.
- A separately named `CodingAgentRuntime` owner — future requirement `RUNTIME-01`, only if later evidence requires it.
- Multi-session daemon orchestration and cross-session client routing — future requirement `RUNTIME-02`.
- Collaborative control in which one client can control another client's Prompt — future capability, not part of the current single-owner scoped-control contract.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLIENT-01 | Public client connection obtains a snapshot cursor and resumes retained product events | Existing `EventService::current_product_sequence`, retained deque, `product_events_after`, and broadcast receiver provide the implementation seam; public connection must own the replay/live handoff. |
| CLIENT-02 | Stale cursor yields typed fresh-snapshot/event-gap recovery and reconnect distinguishes resumable history | Existing `EventStreamGap` and `EventStreamLag` carry the two failure classes but need a public structured recovery result with fresh snapshot metadata. |
| CLIENT-03 | Public client state exposes submitted-operation identity/status and client-local draft semantics without internals | `ClientConnection`, `ClientDraft`, and `SubmittedOperation` are private foundations; `CodingAgentSnapshot` currently leaks only count and must project full typed state. |
| CONTROL-01 | Scoped public abort/steer/follow-up contract remains outside ordinary operation queue | `PromptControlHandle` and `PromptControlCommand` already use a separate Tokio channel; Phase 8 must hard-bound admission/transport and add owner/generation/operation binding, typed receipts/rejections, and idempotency. |
</phase_requirements>

## Summary

The repository already has the right ownership seams but not the Phase 8 contract. `CodingAgentSession` owns `EventService`, `OperationControl`, capability snapshots, and the private client projection; `connect` currently computes one snapshot and returns a value-only `CodingAgentClientConnection`, so reconnect state is discarded. `[VERIFIED: codegraph/source]` `client_projection.rs` defines `UiSnapshotCursor`, `ClientDraft`, `SubmittedOperation`, and `ClientConnection`, while `[VERIFIED: codegraph/source]` `public_projection.rs` exports only `CodingAgentClientId`, a cursor, snapshot metadata, and draft count.

The implementation should therefore extend the session-owned client projection rather than introduce a second dispatcher or move state into adapters. `[VERIFIED: codegraph/source]` `EventService` serializes product sequence assignment under one `Mutex`, retains a bounded deque (baseline 128), broadcasts through Tokio `broadcast`, and reports `EventStreamGap` for retained-history misses. `[VERIFIED: codegraph/source]` RPC state already demonstrates replay-after-snapshot, applied-sequence deduplication, draft mirroring, and bounded idempotency records; those behaviors are evidence to centralize, not a second source of truth.

**Primary recommendation:** Put the session-owned client registry keyed by stable client id and generation inside the sole SnapshotState; make the public connection a stateful capability handle whose atomic snapshot/recovery methods delegate to its coordinator, while control commands continue through the existing Prompt-only channel and are admitted outside `run(CodingAgentOperation)`.

**Authoritative topology refinement:** `SnapshotCoordinator` contains the single `SnapshotState`. `SnapshotState` owns the sole client registry/state together with session-view projection, capability source/projection, active operation, event cursor/publication projection, and recovery metadata. `ClientService` is only `Arc<SnapshotCoordinator>` plus concrete delegating methods and owns no independent map or state. `CodingAgentClientConnection` likewise holds the shared Arc plus id/generation; snapshot/draft/ack/reconnect never borrow the session or use callbacks. The explicit preparation exception accepts `&mut CodingAgentSession` only to install the session-wide lease.

Snapshot-visible writers use six fixed two-phase routes: startup drains marker ownership before coordinator projection/emission; OperationGuard drop records a pending clear before a coordinator-safe synchronous hook; capability installation computes an immutable generation before transactional source/projection commit and post-release emit; navigation performs IO/await before immutable view swap and post-release publish; EventService builds outside locks, assigns cursor/updates recovery and publication projection transactionally, then broadcasts after release; client mutation validates generation and mutates the sole registry transactionally, returning owned results after release. Each route requires a named test plus bounded deadlock timeout and mixed-revision assertions.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Client identity, generation takeover, draft/submitted state | Product runtime (`pi-coding-agent::coding_session`) | Public API projection | Session owns lifecycle and must invalidate stale generations atomically; adapters must not own durable/live truth. |
| Snapshot cursor and recovery metadata | Product runtime / event service | Public API | Product sequence and retained-history bounds are generated by `EventService`; API converts them into typed recovery outcomes. |
| Replay-to-live handoff and receiver lag | Product runtime / event service | RPC/interactive adapters | Only the publisher can define a no-gap boundary; adapters consume replay/live streams and acknowledge applied sequences. |
| Prompt abort/steer/follow-up | Product runtime / operation control | Public scoped handle | Existing channel is intentionally separate from ordinary operation admission; public layer adds authorization and receipts. |
| JSON/RPC/TUI rendering | Adapter layer | Public API | Preserve existing wire/UI behavior; adapters should stop mirroring connection truth locally as migration permits. |
| Durable session facts | Session log / `SessionService` | Client projection | Client drafts and acknowledgements are client-local; session replay remains authority for durable product facts. |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|-------------|
| Rust standard library `Arc`, `Mutex`, `HashMap`, `VecDeque` | Rust 1.96 toolchain (edition 2024) | Session-owned generation registry, state snapshots, retained/idempotency indexes | Already used by `CodingAgentSession`, `EventService`, and projections; no new dependency is necessary. `[VERIFIED: cargo/rustc and source]` |
| Tokio `broadcast` | 1.52.3 workspace dependency | Live product-event fan-out and lag detection | Existing `EventService` uses it and maps `Lagged` to typed session errors. `[VERIFIED: AGENTS.md and source]` |
| Tokio `mpsc` bounded channel | 1.52.3 workspace dependency | Prompt-scoped abort/steer/follow-up control channel with hard backpressure | Replace the existing unbounded Prompt transport with a bounded private channel while retaining separation from the ordinary operation queue. `[VERIFIED: AGENTS.md and source]` |
| Serde 1.0.228 | Workspace dependency | Stable public snapshot, recovery, draft, submitted-state, and receipt serialization where adapters require it | Existing public event/protocol types use Serde; preserve snake_case wire conventions. `[VERIFIED: AGENTS.md and source]` |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `thiserror` | 2.0.18 | Exceptional connection/preparation failures | `error.rs` owns `StaleClientConnection`, `SubmissionPreparationBusy`, and `ClientCapacityExceeded` plus stable `code()` arms. Expected mutation/control rejection uses public outcome enums, never a duplicate error variant. `[VERIFIED: AGENTS.md/source]` |
| `uuid` | 1.23.2 | Existing operation/session ids | Reuse existing ID generation conventions; do not add a control-id crate. `[VERIFIED: AGENTS.md]` |

**Installation:** None. Phase 8 is a code/config migration inside existing workspace dependencies; no external package addition is presumed or required.

## Architecture Patterns

### System Architecture Diagram

```text
public connect/reconnect(client_id, cursor)
        |
        v
CodingAgentSession client registry (id -> generation + drafts + submitted + ack cursor)
        | atomically capture snapshot + subscribe boundary
        +--> EventService retained replay(cursor)
        |       |-- replayable --> Replay batch + live receiver boundary
        |       `-- gap/lag -----> FreshSnapshotRequired{reason, bounds, snapshot}
        |
        +--> public CodingAgentClientConnection handle
                |-- acknowledge(sequence) updates cursor only after client apply
                |-- draft mutations / submit route through generation validation
                `-- PromptControlHandle(client,generation,operation,control_id)
                          |
                          `--> OperationControl Prompt mpsc channel (outside run queue)
```

### Pattern 1: Generation-checked stateful handle

**What:** Store a monotonically increasing generation with each stable client id. Every public mutation/control call carries the captured generation; the registry rejects old generations with a typed stale/disconnected result. A takeover returns a new handle while preserving the client record.

**When to use:** All state mutation, cursor acknowledgement, draft operations, and Prompt control calls. Read-only snapshot/recovery calls still need a consistent generation to avoid mixing old and new state.

**Code anchor:** `[VERIFIED: source]` `CodingAgentSession::connect` currently converts a public id to private `ClientConnectionId` and calls `connect_client`; `ClientConnection::new` currently has no generation or shared registry. Add the registry at this owner boundary.

### Pattern 2: Atomic replay/live handoff with sequence authority

**What:** Under the same publication/connection coordination lock, capture the current snapshot sequence, establish the live receiver boundary, and derive retained replay after the requested cursor. On reconnect, classify history gap versus receiver lag and return a typed fresh snapshot rather than silently falling back. The sequence is the only deduplication authority; ack is explicit.

**Code anchor:** `[VERIFIED: source]` `EventService::emit` increments `next_sequence`, retains, then broadcasts while holding `publication_state`; `product_events_after` reads the retained deque and rejects a nonzero cursor older than its front. `[VERIFIED: source]` `ProductEventReceiver` maps Tokio `Lagged` to `EventStreamLag`.

### Pattern 3: Client-local drafts with acceptance-driven clearing

**What:** Keep one replaceable Prompt draft and ordered, stable-id Steer/FollowUp queues in the session-owned client record. Submission/control acceptance clears only the accepted item; validation, busy, stale-generation, target mismatch, and closed-channel rejection preserve it. Submitted operation status is monotonic and terminal state persists until explicit terminal acknowledgement (full association closure remains Phase 9).

**Code anchor:** `[VERIFIED: source]` current `ClientConnection::mark_submitted` removes all Prompt drafts immediately and `clear_submitted_operation` removes status by operation id; this must become acceptance/status-aware and preserve draft ids.

### Pattern 4: Immutable operation-scoped control capability

**What:** Public control handle captures client id, generation, Prompt operation id, and stable control id. Admission validates syntax plus immutable identity, looks up the exact scoped key, compares the stored normalized signature, returns the original receipt for an identical retry or typed conflict for mismatch, and only on a miss checks new-key capacity before target-running/channel and enqueue/store. A successful result is an enqueue receipt, not application completion.

**Code anchor:** `[VERIFIED: source]` `PromptControlHandle` currently wraps only an `UnboundedSender<PromptControlCommand>` and returns `CodingSessionError::Session` on closed receiver. Wrap this private sender behind a session-owned admission method; do not expose raw sender or operation control internals.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Live fan-out and lag detection | A custom async broadcast queue | Existing Tokio `broadcast` through `EventService` | Existing receiver/error behavior and adapter tests depend on it. |
| Prompt command transport | A new ordinary `CodingAgentOperation` variant or unbounded bespoke queue | Bounded `PromptControlHandle` + `PromptControlCommand` channel | Locked CONTROL-01 requires controls outside the ordinary operation queue and the high-severity queue threat requires hard backpressure. |
| Durable session recovery | Persist client drafts/acks into session log as session facts | Session-owned client projection plus existing `SessionService` replay for durable facts | Client-local state is not durable session truth; avoid changing log schema in this phase. |
| Event deduplication | Timestamp/hash or exactly-once claim | Stable `ProductEventSequence` + explicit client acknowledgement | Sequence is already assigned by the publisher and supports at-least-once recovery. |
| Public contract exposure | Re-export private `Operation`, service, queue, or Flow types | Curated `pi_coding_agent::api` projection types | Existing API boundary guards require internals to remain private. |

## Runtime State Inventory

This is a migration/promotion phase; runtime state was checked explicitly.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | No external database or persisted client registry found. Rust-native session log stores durable session facts, not current client drafts/acks. `[VERIFIED: source/AGENTS]` | Code edit only; do not add a session-log migration unless implementation discovers a durable requirement. |
| Live service config | None. Product runs as a local native CLI/TUI/RPC process; no external service registry is present. `[VERIFIED: AGENTS.md]` | None. |
| OS-registered state | None found for client ids, cursors, or control ids. | None. |
| Secrets/env vars | No client lifecycle or control-id env keys found; existing auth/env handling is unrelated. | None; preserve auth behavior. |
| Build artifacts / installed packages | No generated artifact embeds these internal type names; `target/` is build output and should not be hand-edited. | Rebuild/test only; no package/data migration. |

## Common Pitfalls

### Pitfall 1: Snapshot and live receiver are read independently

**What goes wrong:** An event emitted between snapshot creation and receiver setup is omitted or delivered twice.  
**Why it happens:** Current `connect_client` creates a snapshot and value-only connection, while `product_events_after` and subscription are separate methods. `[VERIFIED: source]`  
**How to avoid:** Define one session-owned recovery operation that captures the sequence and receiver boundary atomically, then partition events by sequence.  
**Warning signs:** Tests intermittently miss an event around reconnect or require adapter-side sequence guesses.

### Pitfall 2: Delivery advances cursor

**What goes wrong:** A disconnect after delivery but before application loses an event on recovery.  
**How to avoid:** Keep delivered sequence separate from acknowledged sequence; ack only after consumer application. Test response-loss/retry and at-least-once duplicate delivery.

### Pitfall 3: Old connection remains authorized after takeover

**What goes wrong:** A stale UI can mutate drafts or control a newer Prompt for the same id.  
**How to avoid:** Generation-check every mutation/control call and invalidate old immutable handles, including duplicate-id retries.

### Pitfall 4: Re-targeting controls by “current Prompt”

**What goes wrong:** A delayed steer/abort applies to a later Prompt.  
**How to avoid:** Bind control to `(client_id, generation, operation_id)` and reject target mismatch/not-running with typed reasons.

### Pitfall 5: Clearing rejected drafts or terminal state too early

**What goes wrong:** User intent disappears on busy/stale/closed-channel failure, or terminal status cannot be acknowledged after reconnect.  
**How to avoid:** Clear only after acceptance; retain terminal submitted state until explicit terminal-event/outcome acknowledgement. Phase 9 owns exhaustive association closure, so avoid pretending it is solved here.

### Pitfall 6: Leaking internal types through the public API

**What goes wrong:** Public callers depend on `ProductEvent`, `PromptControlHandle`, `OperationKind`, or service/queue types that should remain private.  
**How to avoid:** Add only stable wrapper/enums to `coding_session::public_projection` and re-export via `pi_coding_agent::api`; extend public API and boundary tests.

## Code Examples

### Existing retained replay and typed gap

```rust
// Verified in crates/pi-coding-agent/src/coding_session/event_service.rs
pub(crate) fn product_events_after(
    &self,
    cursor: ProductEventSequence,
) -> Result<Vec<ProductEvent>, CodingSessionError> {
    let state = self.publication_state.lock().unwrap();
    let Some(oldest) = state.retained_product_events.front().map(ProductEvent::sequence)
    else { return Ok(Vec::new()); };
    if cursor < oldest && cursor != ProductEventSequence::default() {
        return Err(CodingSessionError::EventStreamGap {
            requested_after: cursor.get(),
            oldest_available: oldest.get(),
        });
    }
    Ok(state.retained_product_events.iter()
        .filter(|event| event.sequence() > cursor).cloned().collect())
}
```

### Existing control separation

```rust
// Verified in crates/pi-coding-agent/src/coding_session/operation_control.rs
pub(crate) enum PromptControlCommand {
    Abort { reason: String },
    Steer { text: String },
    FollowUp { text: String },
}
// PromptControlHandle sends to an mpsc channel; run(CodingAgentOperation) is unchanged.
```

### Existing RPC behavior to centralize

`protocol/rpc/state.rs` maintains `client_drafts`, `submitted_operation`, `adapter_applied_sequence`, and bounded idempotency records; `protocol/rpc/prompt.rs` replays after a cursor and then streams live events. `[VERIFIED: source]` Preserve these externally visible behaviors while moving authority into the connection contract. The new public API should let RPC project the shared snapshot/recovery result instead of maintaining a second mirror.

## Security Domain

Security enforcement is enabled at ASVS level 1 with high-severity blocking. This phase is primarily authorization and state-integrity work.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | Limited/no | Local in-process client ids are not authentication credentials; do not imply that `CodingAgentClientId` authenticates a user. |
| V3 Session Management | Yes | Treat connection generation as a session capability; invalidate old generations on takeover and make stale handles fail closed. |
| V4 Access Control | Yes | Require owner client id + generation + target Prompt operation id for control; reject cross-client and mismatched-target attempts with typed reasons. |
| V5 Input Validation | Yes | Validate non-empty control text, bounded ids, draft ids, cursor ranges, and control-id format using existing typed input patterns. |
| V6 Cryptography | No | No cryptography is needed; never invent hashes/tokens as a substitute for stable sequence/id scope. |

### Known Threat Patterns

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Stale handle mutates current client state | Elevation of privilege / Tampering | Generation compare-and-reject on every mutation and control call. |
| Client A controls Client B's Prompt by operation id | Spoofing / Elevation | Owner client id is part of immutable handle and idempotency scope; never authorize by operation id alone. |
| Replay boundary omission or duplicate | Tampering / Repudiation | Serialize snapshot/receiver boundary with publisher sequence; at-least-once delivery and sequence deduplication, explicit ack. |
| Unbounded draft/control/idempotency growth | Denial of service | Bound retained events and idempotency records; preserve locked capacity discretion and document chosen limits. |
| Error-string parsing for recovery/control | Repudiation | Stable typed recovery/rejection enums and `code()` values; avoid matching `to_string()`. |

## Validation Architecture

Nyquist validation is enabled (`workflow.nyquist_validation: true`). Existing tests are Rust built-in plus Tokio async tests; no new framework/config is required. `[VERIFIED: AGENTS.md/source]`

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Cargo test, Rust built-in harness, Tokio `#[tokio::test]` |
| Config file | None; crate manifests and inline/integration tests |
| Quick run command | `cargo test -p pi-coding-agent --lib client_projection` |
| Focused contract command | `cargo test -p pi-coding-agent --test public_api --test protocol_events` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CLIENT-01 | Connect returns full snapshot cursor; replay resumes events after acknowledged cursor | async integration | `cargo test -p pi-coding-agent --test public_api client_connection` | Wave 0: add/extend test |
| CLIENT-02 | Retained gap and receiver lag produce typed `FreshSnapshotRequired` metadata | unit + async integration | `cargo test -p pi-coding-agent --lib event_service` | Existing gap/lag tests; public recovery tests needed |
| CLIENT-03 | Same-id reconnect restores drafts/submitted state; takeover stales old handle; status/drafts are typed and acceptance-driven | async integration | `cargo test -p pi-coding-agent --test public_api client_state` | Wave 0: add test module |
| CONTROL-01 | Owner-only immutable Prompt controls enqueue abort/steer/follow-up, preserve order, reject stale/target mismatch, and deduplicate stable control ids | async integration | `cargo test -p pi-coding-agent --test public_api scoped_control` | Wave 0: add test module |

### Required Scenario Matrix

- Connect at sequence 0, emit during recovery, assert each sequence appears once across replay/live boundary.
- Acknowledge only after consumption; disconnect before ack and assert at-least-once replay.
- Retained-history gap returns requested/oldest/fresh cursor and `retained_history_gap` reason.
- Receiver lag returns fresh snapshot with `live_receiver_lag` reason.
- Same client id takeover invalidates old draft mutation, ack, and control handles; new handle sees old state.
- Different client id cannot control Prompt even with operation id.
- Control retry with same `(client, operation, control_id)` returns original receipt without duplicate; same text with a new id enqueues distinctly and preserves order.
- Rejected draft submissions retain drafts; accepted Prompt/Steer/FollowUp clear only their accepted entries.

### Wave 0 Gaps

- [ ] Extend `crates/pi-coding-agent/tests/public_api.rs` or add a focused `client_connection.rs` integration suite for public snapshot/recovery/state/control types.
- [ ] Add deterministic test helpers for controlled event publication, small retained capacity, takeover generations, and a blocked Prompt control receiver.
- [ ] Add source/boundary assertions that no public API exposes private `ProductEvent`, raw `PromptControlHandle`, `OperationControl`, service, queue, or Flow types.
- [ ] Preserve existing RPC and interactive assertions while migrating their state reads to the public connection contract.

### Verification Commands

```bash
cargo fmt --check
cargo test -p pi-coding-agent --lib client_projection
cargo test -p pi-coding-agent --lib event_service
cargo test -p pi-coding-agent --test public_api --test protocol_events
cargo test -p pi-coding-agent
cargo test --workspace
cargo check --workspace
git diff --check
```

## Environment Availability

This phase has no external service dependency and adds no package. The local toolchain is available: `rustc 1.96.0`, `cargo 1.96.0`, Node `v24.16.0`; `codegraph` is installed. `ctx7`/Context7 MCP was unavailable, so Tokio semantic claims are grounded in existing source and should be checked against current Tokio docs during planning if API details change.

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust/Cargo | Build and tests | Yes | 1.96.0 | — |
| Tokio | Existing runtime/channels | Yes (locked) | 1.52.3 | Existing std primitives only for registry if needed |
| CodeGraph CLI | Repository symbol/call discovery | Yes | installed | `rg`/direct reads after CodeGraph if unavailable |
| Context7/ctx7 | Up-to-date Tokio docs | No | — | Code-grounded evidence; planner can consult official Tokio docs |

## Sources

### Primary (HIGH confidence)

- `.planning/phases/08-client-connection-replay-and-scoped-control/08-CONTEXT.md` - locked decisions D-01 through D-21 and deferred Phase 9 scope.
- `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `.planning/PROJECT.md` - CLIENT-01/02/03 and CONTROL-01 boundaries and success criteria.
- `crates/pi-coding-agent/src/coding_session/client_projection.rs` - private snapshot, client drafts, submitted operation, and connection behavior.
- `crates/pi-coding-agent/src/coding_session/public_projection.rs` - current public facade and draft-count limitation.
- `crates/pi-coding-agent/src/coding_session/event_service.rs`, `event.rs`, `error.rs` - sequence assignment, retained replay, broadcast lag, and typed gap/lag errors.
- `crates/pi-coding-agent/src/coding_session/operation_control.rs`, `intent_router.rs` - Prompt-only control channel and admission boundary.
- `crates/pi-coding-agent/src/coding_session/mod.rs` - session owner integration points (`run`, `connect`, snapshot, replay, control).
- `crates/pi-coding-agent/src/protocol/rpc/state.rs`, `protocol/rpc/prompt.rs` - existing adapter behavior evidence for drafts, submitted state, replay deduplication, and idempotency.
- `crates/pi-coding-agent/tests/public_api.rs` and existing event/control tests - public facade and deterministic test conventions.
- CodeGraph exploration of the symbols above, followed by on-disk source reads. `[VERIFIED: codegraph/source]`

### Secondary (MEDIUM confidence)

- `AGENTS.md` / generated stack and conventions - workspace versions, Tokio usage, API boundary, and verification commands. `[VERIFIED: repository instructions]`
- GSD research-plan Context7 fetch plan for Tokio and Rust synchronization. Fetch was unavailable in this runtime; no external claim relies solely on it.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Tokio 1.52.3 `broadcast`/`mpsc` semantics remain compatible with current usage shown in source. `[ASSUMED]` | Standard Stack / Architecture | A changed API or semantic detail could require a small adapter change; verify against official docs before implementation. |
| A2 | Client-local registry need not be persisted in Rust-native session log. `[ASSUMED]` | Runtime State Inventory | Persisting it would expand schema/durability scope; current locked decisions describe reconnect within a live session, not process restart. |

## Open Questions (RESOLVED)

1. **What exact public method shape represents replay plus live receiver?**
   - What we know: Existing `connect` returns a value-only connection and `subscribe_product_events_public` returns an independent receiver.
   - What's unclear: Whether the planner chooses `reconnect(cursor)`, a connection method returning a recovery enum, or a connection-scoped stream wrapper.
   - Resolution: `CodingAgentClientConnection::reconnect(cursor) -> Result<CodingAgentReconnect, CodingAgentConnectionError>` is the sole public recovery entry point. `CodingAgentReconnect::Replayed` carries converted `CodingAgentProductEvent` values, `CodingAgentProductEventReceiver`, and `through_cursor`; `FreshSnapshotRequired` carries the authoritative `CodingAgentSnapshot` and `CodingAgentRecoveryMetadata`. No independent public subscribe method is used for reconnect handoff.

2. **Where should accepted/running transitions be observed?**
   - What we know: `run` admission and event publication are separate boundaries; Phase 9 owns exhaustive terminal association.
   - What's unclear: Exact event/admission hook for `Accepted` and `Running` without changing operation semantics.
   - Resolution: public non-Clone `CodingAgentSubmissionLease` is acquired only by `prepare_submission(&self, session: &mut CodingAgentSession, draft_id, operation)` and owns the generation-scoped, session-wide exclusive precommit guard. Preparation is not dispatch; the caller next invokes unchanged `session.run(operation)`. Drop before run or precommit future cancellation clears Prepared/Consuming and preserves the draft. Canonical run consumes it into internal `SubmissionCommitGuard`; immediately after `IntentRouter::admit_operation` yields permit plus operation id, `commit()` binds the id, sets Accepted, and clears the matching Prompt draft once. Postcommit Drop/future cancellation synchronously records Terminal Cancelled; returned failures record Terminal Failed; neither restores a draft. No-lease run is untracked, takeover invalidates old-generation leases, and wrapper Drop after consumption is a no-op. Terminal observation stores the exact product-event sequence plus operation id as `TerminalAcknowledgementAnchor`; Phase 9 still owns exhaustive terminal-family association.

3. **What capacities should be public/configurable?**
   - What we know: Event retention baseline is 128 and RPC idempotency records are capped at 64.
   - What's unclear: Public exposure and per-client idempotency/draft limits.
   - Resolution: Keep capacities private and deterministic: maximum live client records 64 per session, each Prompt/Steer/FollowUp queue 64 entries, and each client receipt set 64 accepted records. `SnapshotCoordinator::connect_or_takeover` returns exceptional `CodingSessionError::ClientCapacityExceeded` for a new identity at capacity, while same-id takeover remains allowed. Expected queue saturation returns public typed `QueueCapacityExceeded` without mutating drafts. Receipt order is syntax/immutable identity, scoped-key lookup, signature comparison, then on miss new-key capacity before target/channel and enqueue/store. Accepted records never release in Phase 8; Phase 9 owns release. Test constructors may lower limits, but the public API does not expose them.

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| RPC-owned client drafts/submitted state and replay markers | Session-owned typed connection projection | One source of truth across RPC, interactive, and embedders; adapters become projections. |
| Value-only `connect` snapshot | Stateful generation-bound connection handle | Reconnect/takeover and stale-handle rejection become enforceable. |
| Generic `EventStreamGap`/`EventStreamLag` errors | Typed `FreshSnapshotRequired` recovery branch with metadata | Clients can recover deterministically without parsing error strings. |
| Raw Prompt sender handle | Immutable owner/generation/operation-scoped control capability | Prevents cross-client or retargeted controls while preserving separate control channel. |

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH for existing workspace dependencies; MEDIUM for external Tokio documentation because Context7 was unavailable.
- Architecture: HIGH; all recommendations are anchored to current source and locked CONTEXT decisions.
- Pitfalls: HIGH for replay/generation/draft/control risks; exact synchronization implementation remains planner discretion.

**Research date:** 2026-07-13  
**Valid until:** 2026-08-12 for repository architecture; verify dependency docs sooner if Tokio is upgraded.

**generic-agent workaround:** typed `gsd-phase-researcher` dispatch was unavailable; this artifact was produced by the generic agent following the complete phase-researcher instructions.
