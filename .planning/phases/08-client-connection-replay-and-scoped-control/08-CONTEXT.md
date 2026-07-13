# Phase 8: Client Connection, Replay, and Scoped Control - Context

**Gathered:** 2026-07-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Promote the existing snapshot cursor, retained product-event replay, stale-cursor recovery, submitted-operation projection, client-local drafts, and Prompt control channel into a stable public reconnectable-client contract exported through `pi_coding_agent::api`.

This phase defines connection identity and takeover, atomic snapshot/replay/live handoff, client acknowledgement, submitted-operation and draft state, and Prompt-scoped control targeting and receipts. Ordinary product work must continue through `CodingAgentSession::run(CodingAgentOperation)`, while abort, steer, and follow-up remain control signals outside the ordinary operation queue.

Detach/close, runtime shutdown, complete operation/outcome/terminal-event association, adapter-root discovery guards, and compile-fail diagnostic hardening remain Phase 9 work. A separately named runtime owner, multi-session daemon routing, collaborative cross-client control, and new product workflows remain out of scope.

</domain>

<decisions>
## Implementation Decisions

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

### Agent Discretion

- Choose exact Rust type, enum, method, and module names while keeping all stable contracts under `pi_coding_agent::api` and internal services, queues, operation metadata, and Flow nodes private.
- Choose the internal synchronization and storage strategy for client generations, acknowledgements, receipts, and idempotency records, provided the observable takeover, atomicity, ordering, and recovery decisions above hold.
- Choose retained-event and idempotency-record capacity/configuration details. The existing retained capacity of 128 is an implementation baseline, not a newly locked public constant.
- Choose how accepted/running transitions are derived from existing admission and event boundaries. Phase 9 still owns exhaustive operation/outcome/terminal association closure.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Milestone Scope and Requirements

- `.planning/PROJECT.md` — Defines the v1.1 client-lifecycle goal, architectural constraints, stable-API boundary, canonical dispatcher, and explicit exclusions.
- `.planning/REQUIREMENTS.md` — Defines Phase 8 requirements `CLIENT-01`, `CLIENT-02`, `CLIENT-03`, and `CONTROL-01`, plus the Phase 9 lifecycle and association boundary.
- `.planning/ROADMAP.md` — Defines the Phase 8 goal and success criteria and separates detach/close, shutdown, association closure, and guard work into Phase 9.
- `.planning/phases/06-product-event-inventory-and-typed-contract/06-CONTEXT.md` — Locks the typed product-event identity, durability, terminal, replay-compatibility, and adapter-preservation foundations consumed here.

### Historical Design Evidence

- `docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md` — Historical Stage 9 design input. It explicitly retained snapshot/connect/product-event subscription and private Prompt control while broad workflow methods were deleted; current source and this context remain authoritative where it differs.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `crates/pi-coding-agent/src/coding_session/client_projection.rs`: Existing private `ClientConnection`, `ClientConnectionId`, `UiSnapshot`, `UiSnapshotCursor`, `ClientDraft`, `ClientDraftKind`, and `SubmittedOperation` establish the initial data model. Current tests already cover Prompt-draft clearing and preservation of queued Steer/FollowUp drafts.
- `crates/pi-coding-agent/src/coding_session/public_projection.rs`: Existing public `CodingAgentClientId`, `CodingAgentClientConnection`, `CodingAgentSnapshot`, and `CodingAgentSnapshotCursor` provide the facade to evolve. The current public snapshot exposes only draft count and must be enriched without leaking private services.
- `crates/pi-coding-agent/src/coding_session/event_service.rs`: `EventService` already owns monotonically sequenced publication, broadcast delivery, bounded retained replay, backpressure status, and `product_events_after`. `EventStreamGap` already carries requested and oldest-available sequences.
- `crates/pi-coding-agent/src/coding_session/operation_control.rs`: `PromptControlHandle`, `PromptControlCommand`, and `OperationControl` already keep abort, steer, and follow-up outside the ordinary operation queue.
- `crates/pi-coding-agent/src/coding_session/mod.rs`: `CodingAgentSession::connect`, `snapshot`, private replay handles, and private Prompt control admission are the runtime-owner integration points.
- `crates/pi-coding-agent/src/protocol/rpc/state.rs` and `crates/pi-coding-agent/src/protocol/rpc/prompt.rs`: RPC already mirrors drafts and submitted Prompt state, performs retained replay after a cursor, deduplicates the replay/live boundary with applied sequence markers, and maps gaps to fresh-snapshot recovery. These are behavior evidence to centralize behind the public contract rather than duplicate.

### Established Patterns

- Stable contracts are curated through `pi_coding_agent::api`; root compatibility exports and private service ownership must not be widened.
- Ordinary product work is admitted and dispatched only through `CodingAgentSession::run(CodingAgentOperation)`; query and control intents remain separate metadata classes.
- Product events use stable sequence, typed payload, operation identity where present, terminal classification, and durability metadata. Recovery must preserve that authoritative event model.
- RPC and interactive adapters project shared runtime state rather than owning durable truth. Client-local state may be retained for reconnect, but Rust-native session replay remains the authority for durable session facts.
- Error surfaces use typed `CodingSessionError` variants and stable `code()` values. New public recovery/control outcomes should follow typed-domain conventions rather than string parsing.

### Integration Points

- Extend the curated facade in `crates/pi-coding-agent/src/lib.rs` and lock it through `crates/pi-coding-agent/tests/public_api.rs` and API/boundary guards.
- Centralize connection state and reconnect semantics under `crates/pi-coding-agent/src/coding_session/`; migrate RPC-local mirrors only after public behavior and deterministic tests exist.
- Preserve product-event sequence and receiver behavior used by `crates/pi-coding-agent/src/protocol/rpc/` and `crates/pi-coding-agent/src/interactive/`.
- Extend deterministic retained-capacity, gap, lag, takeover, acknowledgement, draft, submitted-state, control-target, and idempotency tests without weakening existing adapter assertions.

</code_context>

<specifics>
## Specific Ideas

- The names `Replayed`, `FreshSnapshotRequired`, `Accepted`, `Running`, `Terminal`, and `Enqueued` describe the chosen semantics; exact Rust naming remains planner discretion.
- A fresh-snapshot recovery response should be sufficient to rebuild immediately and should explain why incremental recovery failed.
- A control receipt is deliberately an enqueue acknowledgement, not an agent-application acknowledgement.
- Stable draft ids should flow naturally into Steer/FollowUp control ids so response-loss retries do not duplicate user intent.

</specifics>

<deferred>
## Deferred Ideas

- Client detach/close, runtime shutdown, and idempotent lifecycle teardown — Phase 9.
- Exhaustive operation id/submitted state/terminal outcome/terminal event association — Phase 9.
- Adapter-root discovery and external compile-fail diagnostic hardening — Phase 9.
- A separately named `CodingAgentRuntime` owner — future requirement `RUNTIME-01`, only if later evidence requires it.
- Multi-session daemon orchestration and cross-session client routing — future requirement `RUNTIME-02`.
- Collaborative control in which one client can control another client's Prompt — future capability, not part of the current single-owner scoped-control contract.

</deferred>

---

*Phase: 8-Client Connection, Replay, and Scoped Control*
*Context gathered: 2026-07-13*
