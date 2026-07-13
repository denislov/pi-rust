# Phase 08: Client Connection, Replay, and Scoped Control - Pattern Map

**Mapped:** 2026-07-13
**Mode:** generic-agent workaround for unavailable typed `gsd-pattern-mapper`
**Files classified:** 13 implicated files/file groups
**Strong analogs:** 5

This phase is a promotion and convergence task, not a greenfield subsystem. The current code has strong local analogs for public projections, session-owned services, retained replay, operation control, bounded idempotency, RPC replay, and boundary tests. It does **not** yet have one analog that provides generation takeover, an atomic snapshot/replay/live handoff, explicit acknowledgement, and owner-scoped control together. The planner should compose those properties inside the existing `CodingAgentSession` ownership boundary rather than copy the RPC mirror wholesale.

## File Classification

| New/Modified File | Rust Role | Data Flow | Closest Current Analog | Match |
|---|---|---|---|---|
| `crates/pi-coding-agent/src/coding_session/public_projection.rs` | public model/projection | transform, request-response, streaming handle | same file's `CodingAgentSnapshot`, `CodingAgentClientConnection`, receiver wrapper | exact extension |
| `crates/pi-coding-agent/src/coding_session/client_projection.rs` | private model/store | event-driven state, CRUD | same file's `ClientConnection`, drafts, submitted operation | exact extension |
| `crates/pi-coding-agent/src/coding_session/mod.rs` | runtime owner/controller | request-response, event-driven | existing service-container fields plus `connect`, `ui_snapshot`, `run` | exact owner pattern |
| `crates/pi-coding-agent/src/coding_session/client_service.rs` | session-owned service/store | CRUD, event-driven, receipt accounting | `EventService` ownership and `CapabilitySnapshotService` generation ownership | selected final owner |
| `crates/pi-coding-agent/src/coding_session/snapshot_coordinator.rs` | shared projection coordinator | short synchronous transactions, monotonic revision | `EventService.publication_state` plus session service container ownership | new composition; no exact analog |
| `crates/pi-coding-agent/src/coding_session/public_operation.rs` | canonical dispatcher/provenance owner | preparation lease -> admission -> commit | existing `CodingAgentSession::run` admission path | exact dispatcher seam |
| `crates/pi-coding-agent/src/coding_session/capability_snapshot.rs` and `session_service.rs` | snapshot-relevant writers | generation/session projection mutation | existing private state owners | writer integration |
| `crates/pi-coding-agent/src/coding_session/event_service.rs` | service/provider | pub-sub, retained streaming | `publication_state`, `product_events_after`, `ProductEventReceiver` | exact extension |
| `crates/pi-coding-agent/src/coding_session/operation_control.rs` | control service/guard | event-driven channel | `PromptControlHandle`, `OperationState`, receiver lifecycle | exact transport; authorization missing |
| `crates/pi-coding-agent/src/coding_session/intent_router.rs` | admission middleware | request-response | `admit_operation`, `prompt_control_handle`, query classification | role match |
| `crates/pi-coding-agent/src/coding_session/error.rs` | public error model | transform | `CodingSessionError` typed variants and stable `code()` | exact convention |
| `crates/pi-coding-agent/src/lib.rs` | stable facade/configured barrel | transform | curated `api` re-export list | exact |
| `crates/pi-coding-agent/src/protocol/rpc/state.rs` | adapter-local store to migrate | CRUD, bounded cache | draft/submitted mirrors and historical FIFO behavior (not receipt eviction) | behavioral evidence, not final owner |
| `crates/pi-coding-agent/src/protocol/rpc/prompt.rs` | adapter/controller to migrate | streaming, replay, request-response | replay/live sequence dedup and gap/lag conversion | behavioral evidence, partial match |
| `crates/pi-coding-agent/tests/public_api.rs` | integration/contract test | request-response, async streaming | facade importability and session snapshot/connect tests | exact test style |
| `crates/pi-coding-agent/tests/api_boundary_guards.rs` and `product_runtime_boundary_guards.rs` | compile/source boundary tests | batch/source audit | external fixture matrix and private-type/adaptor guards | exact test style |

The adjusted approach selects `client_service.rs` and `snapshot_coordinator.rs` as private final owners. Both remain behind `CodingAgentSession`; neither is exposed through `api`.

## Strongest Analogs

### 1. Stable Public Projection Types

**Source:** `crates/pi-coding-agent/src/coding_session/public_projection.rs:1-41,80-111`

Use this file for all stable client-facing IDs, snapshots, recovery results, typed drafts, submitted status, control IDs/receipts/rejections, and connection handles. Preserve the existing split: private types are imported from sibling modules, while public types contain only stable projections.

```rust
use super::client_projection::{ClientConnection, ClientConnectionId, UiSnapshot};
use super::context::{CodingAgentCapabilities, CodingAgentSessionView};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CodingAgentClientId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentSnapshot {
    pub cursor: CodingAgentSnapshotCursor,
    pub version: ProtocolFamilyVersion,
    pub session: CodingAgentSessionView,
    pub capabilities: CodingAgentCapabilities,
    pub active_operation: Option<String>,
    pub client_draft_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentClientConnection {
    pub client_id: CodingAgentClientId,
    pub snapshot: CodingAgentSnapshot,
}
```

The conversion pattern is a single explicit field-by-field boundary, including conversion of private typed IDs/generations to stable public scalars:

```rust
impl From<UiSnapshot> for CodingAgentSnapshot {
    fn from(snapshot: UiSnapshot) -> Self {
        Self {
            cursor: CodingAgentSnapshotCursor {
                last_event_sequence: snapshot.cursor.last_event_sequence.get(),
                capability_generation: snapshot.cursor.capability_generation.get(),
            },
            version: snapshot.version,
            session: snapshot.session,
            capabilities: snapshot.capabilities,
            active_operation: snapshot
                .active_operation
                .map(|kind| kind.as_str().to_owned()),
            client_draft_count: snapshot.client_drafts.len(),
        }
    }
}
```

**Planner assignment:** Replace `client_draft_count` with complete public typed draft entries and add submitted operation to the same atomic snapshot. Make the public connection stateful without embedding private `ClientConnection`, `ProductEvent`, raw Tokio receiver/sender, `OperationKind`, or services in public signatures. Recovery gap/lag should be a public enum/result branch; ordinary operational failures continue to use the established error boundary.

### 2. Session-Owned Client State and Service Container

**Sources:**

- `crates/pi-coding-agent/src/coding_session/client_projection.rs:10-126`
- `crates/pi-coding-agent/src/coding_session/mod.rs:147-166,420-455`

The current private projection already keeps cursor, drafts, and submitted operation together:

```rust
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClientConnection {
    pub(crate) id: ClientConnectionId,
    pub(crate) cursor: UiSnapshotCursor,
    pub(crate) client_drafts: Vec<ClientDraft>,
    pub(crate) submitted_operation: Option<SubmittedOperation>,
}

impl ClientConnection {
    pub(crate) fn new(id: ClientConnectionId, snapshot: UiSnapshot) -> Self {
        Self {
            id,
            cursor: snapshot.cursor,
            client_drafts: snapshot.client_drafts,
            submitted_operation: None,
        }
    }
}
```

`CodingAgentSession` is already the explicit owner of all product services and mutable coordination state:

```rust
pub struct CodingAgentSession {
    persistence: SessionPersistence,
    runtime_service: RuntimeService,
    flow_service: FlowService,
    event_service: EventService,
    capability_service: CapabilityService,
    plugin_service: PluginService,
    operation_control: OperationControl,
    // ...other product-owned services...
    capability_snapshots: CapabilitySnapshotService,
    startup_recovery_markers: Mutex<Vec<StartupRecoveryMarker>>,
}
```

Current `connect` is value-only and exposes the exact replacement point:

```rust
pub fn connect(&self, id: CodingAgentClientId) -> CodingAgentClientConnection {
    let internal_id = public_projection::internal_client_id(&id);
    let (connection, snapshot) = self.connect_client(internal_id, Vec::new());
    public_projection::public_client_connection(id, connection, snapshot)
}
```

**Planner assignment:** Add a session-owned registry keyed by `ClientConnectionId` with monotonically increasing generation, acknowledged sequence, one Prompt draft, ordered Steer/FollowUp entries, submitted state, and bounded receipt/idempotency records. A takeover must preserve the record but increment generation. Every mutation checks `(client id, generation)` while holding the registry lock. `connect` remains a query/control entry point; ordinary product submission still calls `CodingAgentSession::run(CodingAgentOperation)` and must not be duplicated on the connection handle.

The existing `mark_submitted` at `client_projection.rs:109-124` clears Prompt drafts and terminal state too eagerly for Phase 08. Copy its matching-ID discipline, but move clearing to acceptance and retain terminal submitted state until acknowledged.

### 3. Retained Replay, Broadcast, and Deterministic Capacity

**Source:** `crates/pi-coding-agent/src/coding_session/event_service.rs:83-160,718-722,985-1012,1210-1263,2418-2437`

`EventService` already puts sequence allocation and retained history behind one shared publication mutex and provides capacity injection for deterministic tests:

```rust
fn with_event_capacities(channel_capacity: usize, retained_capacity: usize) -> Self {
    let channel_capacity = channel_capacity.max(1);
    let (product_sender, _) = broadcast::channel(channel_capacity);
    Self {
        product_sender,
        publication_state: Arc::new(Mutex::new(EventPublicationState {
            next_sequence: 1,
            retained_product_events: VecDeque::with_capacity(retained_capacity),
            dropped_before: None,
        })),
        channel_capacity,
        retained_capacity,
    }
}

#[cfg(test)]
pub(crate) fn with_event_capacity_for_tests(capacity: usize) -> Self {
    Self::with_event_capacities(capacity, capacity)
}
```

Retained replay uses sequence as authority and returns a typed gap with bounds:

```rust
pub(crate) fn product_events_after(
    &self,
    cursor: ProductEventSequence,
) -> Result<Vec<ProductEvent>, CodingSessionError> {
    let state = self.publication_state.lock().unwrap();
    let Some(oldest) = state.retained_product_events.front().map(ProductEvent::sequence)
    else {
        return Ok(Vec::new());
    };
    if cursor < oldest && cursor != ProductEventSequence::default() {
        return Err(CodingSessionError::EventStreamGap {
            requested_after: cursor.get(),
            oldest_available: oldest.get(),
        });
    }
    Ok(state.retained_product_events.iter()
        .filter(|event| event.sequence() > cursor)
        .cloned().collect())
}
```

Lag remains distinct from a retained-history gap:

```rust
Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
    Err(CodingSessionError::EventStreamLag { skipped })
}
```

**Planner assignment:** Put the subscribe boundary and replay partition inside `EventService` (or a method that coordinates on its publication state), not in an adapter. The method must atomically establish the receiver/current sequence boundary and return either replay plus live receiver or structured fresh snapshot recovery. Do not infer acknowledgement from delivery. Keep sequence comparison as the deduplication authority and translate internal `ProductEvent` to public events before it crosses `api`.

**Tests to copy:**

- `retained_product_events_can_resume_after_cursor` at lines 1210-1226.
- `retained_product_events_report_gap_before_oldest_sequence` at lines 1229-1244.
- `zero_retained_capacity_keeps_replay_window_empty` at lines 1247-1263.
- `product_event_receiver_lag_reports_snapshot_recovery` at lines 2418-2437.

Use `with_event_capacity_for_tests(1|2)` rather than publishing 129 events. Add a synchronization hook/barrier fixture for “emit during recovery” so the replay/live boundary test is deterministic rather than timing-based.

### 4. Operation-Scoped Control and Admission

**Sources:**

- `crates/pi-coding-agent/src/coding_session/operation_control.rs:45-85,88-124,128-174`
- `crates/pi-coding-agent/src/coding_session/intent_router.rs:166-210`

The existing private channel correctly keeps control outside the ordinary operation queue:

```rust
pub(crate) enum PromptControlCommand {
    Abort { reason: String },
    Steer { text: String },
    FollowUp { text: String },
}

pub(crate) type PromptControlReceiver = mpsc::UnboundedReceiver<PromptControlCommand>;

#[derive(Debug, Clone)]
pub(crate) struct PromptControlHandle {
    sender: mpsc::UnboundedSender<PromptControlCommand>,
}

pub(crate) fn prompt_control_channel() -> (PromptControlHandle, PromptControlReceiver) {
    let (sender, receiver) = mpsc::unbounded_channel();
    (PromptControlHandle { sender }, receiver)
}
```

Admission distinguishes ordinary work from control/query:

```rust
pub(crate) fn admit_operation(
    control: &OperationControl,
    admission: &OperationAdmission,
    expected: OperationDispatchMode,
) -> Result<OperationPermit, CodingSessionError> {
    Self::validate_dispatch_mode(admission, expected)?;
    if admission.metadata.class == OperationClass::ReadOnly {
        return Ok(OperationPermit::unguarded(/* ... */));
    }
    control.begin(admission.kind).map(|guard| OperationPermit::guarded(/* ... */))
}

pub(crate) fn prompt_control_handle(
    control: &mut OperationControl,
    intent: ControlIntent,
) -> Result<PromptControlHandle, CodingSessionError> {
    let metadata = intent.metadata();
    debug_assert_eq!(metadata.class, OperationClass::Control);
    match metadata.operation_kind {
        OperationKind::Prompt => control.prompt_control_handle(),
        _ => unreachable!("unsupported control intent target"),
    }
}
```

**Planner assignment:** Keep `PromptControlCommand` and raw sender private. Add a public immutable handle containing client ID, connection generation, and Prompt operation ID, whose methods call back into session-owned authorization/idempotency logic. Validate in stable order: input, live generation, owner, target identity, target running/channel open, duplicate key, then enqueue. On success cache and return a typed receipt. On rejection preserve drafts and return a stable typed reason. Never resolve the target as “current Prompt” after the handle was created.

### 5. RPC Migration, Bounded Idempotency, and Replay Dedup

**Sources:**

- `crates/pi-coding-agent/src/protocol/rpc/state.rs:31-55,210-277`
- `crates/pi-coding-agent/src/protocol/rpc/prompt.rs:1143-1201,1408-1503,1649-1690`

RPC currently mirrors the state Phase 08 moves under the session owner:

```rust
pub(super) struct RpcState {
    // ...
    pub(super) coding_session: Option<CodingAgentSession>,
    pub(super) client_id: Option<ClientConnectionId>,
    pub(super) client_drafts: Vec<ClientDraft>,
    pub(super) submitted_operation: Option<SubmittedOperation>,
    pub(super) running: Option<RunningPrompt>,
    pub(super) idempotency_records: HashMap<OperationIdempotencyKey, RpcIdempotencyRecord>,
    pub(super) idempotency_order: VecDeque<OperationIdempotencyKey>,
}
```

The bounded cache pattern is directly reusable internally:

```rust
if !self.idempotency_records.contains_key(&key) {
    self.idempotency_order.push_back(key.clone());
}
self.idempotency_records.insert(key, RpcIdempotencyRecord { /* ... */ });
while self.idempotency_order.len() > RPC_IDEMPOTENCY_RECORD_LIMIT {
    if let Some(expired) = self.idempotency_order.pop_front() {
        self.idempotency_records.remove(&expired);
    }
}
```

RPC's replay/live dedup shows the behavioral invariant to preserve:

```rust
for event in retained_events {
    if event.sequence() <= running.adapter_applied_sequence {
        continue;
    }
    let sequence = event.sequence();
    protocol_events.extend(running.adapter.push_product_event(&event));
    running.adapter_applied_sequence = running.adapter_applied_sequence.max(sequence);
    running.replayed_through_sequence = running.replayed_through_sequence.max(sequence);
}

if sequence <= running.replayed_through_sequence
    || sequence <= running.adapter_applied_sequence
{
    return LiveProductEventPush { accepted: false, protocol_events: Vec::new() };
}
```

**Planner assignment:** Migrate authority in stages. First build and test the public connection contract. Then have RPC consume its atomic snapshot/recovery/control methods. Only after behavior parity should `RpcState.client_drafts`, `submitted_operation`, replay markers, and control-specific idempotency mirrors be removed. Keep wire output and adapter assertions unchanged. The public contract is at-least-once with explicit ack; RPC's current `adapter_applied_sequence` is evidence for adapter dedup, not permission to advance the shared cursor on delivery.

## Shared Patterns

### Stable Errors and Recovery Outcomes

**Source:** `crates/pi-coding-agent/src/coding_session/error.rs:6-84`

`CodingSessionError` uses derived equality/error display and an explicit stable code mapping:

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CodingSessionError {
    #[error("event stream gap after sequence {requested_after}; oldest available product event is {oldest_available}; client must request a fresh UI snapshot")]
    EventStreamGap { requested_after: u64, oldest_available: u64 },
    #[error("event stream lagged by {skipped} events; client must request a fresh UI snapshot")]
    EventStreamLag { skipped: u64 },
}

impl CodingSessionError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::EventStreamGap { .. } => "event_stream_gap",
            Self::EventStreamLag { .. } => "event_stream_lag",
            // ...
        }
    }
}
```

Use public result enums for expected recovery/control branches (`Replayed`, `FreshSnapshotRequired`, `Enqueued`, typed rejection), because D-05 explicitly says fresh snapshot recovery is not an opaque error. If any new true error variant is needed, add a stable `code()` arm in the same change and test the code rather than parsing display text.

### Stable Facade and Boundary Tests

**Sources:**

- `crates/pi-coding-agent/src/lib.rs:60-90`
- `crates/pi-coding-agent/tests/public_api.rs:100-149,382-425`
- `crates/pi-coding-agent/tests/api_boundary_guards.rs:6-154`
- `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs:657-710,1390-1421`

The stable facade is a curated re-export list:

```rust
pub mod api {
    pub use crate::coding_session::{
        CodingAgentClientConnection, CodingAgentClientId,
        CodingAgentProductEvent, CodingAgentProductEventReceiver,
        CodingAgentSession, CodingAgentSnapshot, CodingAgentSnapshotCursor,
        CodingSessionError,
        // ...
    };
}
```

Public API tests combine type importability, method item references, and behavior:

```rust
let _run = CodingAgentSession::run;
let _snapshot = CodingAgentSession::snapshot;
let _connect = CodingAgentSession::connect;

let client_id = CodingAgentClientId::new("public-client");
let connected: CodingAgentClientConnection = session.connect(client_id.clone());
assert_eq!(connected.client_id, client_id);
assert_eq!(connected.snapshot.session.session_id, session_id);
```

Extend this style for every new stable type and method. Extend the external offline fixture matrix so positive consumers can use the public connection contract and negative fixtures prove raw `ProductEvent`, `PromptControlHandle`, `OperationControl`, services, queues, and Flow nodes remain inaccessible. Source guards should additionally prevent RPC/interactive production code from importing new private client services or implementing a second ordinary operation dispatcher.

### Locking and Atomicity

The established state pattern is `Arc<Mutex<...>>` for short synchronous ownership checks (`OperationState`, `EventService.publication_state`). Do not hold a standard mutex across `.await`. Prepare replay, receiver boundary, snapshot metadata, generation checks, and cache updates under the lock; release it before adapter serialization or awaiting channel consumption. If an enqueue must be part of the same logical admission, the existing unbounded Prompt channel permits synchronous `send` while the lock is held briefly.

### IDs, Ordering, and Bounded Growth

- Use typed newtypes with `new`/`as_str`, following `CodingAgentClientId` and `ClientConnectionId`.
- Scope control dedup by `(client_id, target_operation_id, control_id)`, not control ID alone.
- Reuse a Steer/FollowUp draft ID as its control ID; identical text with a different ID remains distinct.
- Preserve per-connection enqueue order through the existing single Prompt mpsc sender.
- Use `HashMap` for lookup plus `VecDeque` only for insertion-order bookkeeping/capacity accounting. Accepted Phase 8 receipts are non-evicting for the live session; Phase 9 detach/close/shutdown owns release.
- Keep capacities private/test-injectable unless a later stable requirement demands configuration.

## Actionable Planner Slices

### 6. Canonical dispatcher provenance analog

`crates/pi-coding-agent/src/coding_session/public_operation.rs` is the provenance analog for this phase: `CodingOperation::into_internal` and the admission path in `CodingAgentSession::run` are the only ordinary-dispatch boundary. A private non-Clone `ClientSubmissionLease` is generation-scoped, session-wide exclusive, RAII-cleared, and consumed by that path before `Accepted` and Prompt-draft clearing. It must never become a second `run`/submit entry point or a public operation type. Plan 08-04 owns its exact commit point; Plan 08-05 references it only for control-vs-operation separation.

For control idempotency, the ordering is deliberately key-first: validate syntax and immutable identity, look up the scoped receipt and compare its stored normalized request signature, then consult volatile running/channel/capacity state only on a cache miss. This preserves response-loss retries after target completion while returning a typed payload conflict for the same key with different kind, text, reason, or draft identity. The prior volatile-check-before-receipt wording is obsolete.

1. **Public contract and private state model:** extend `public_projection.rs`, `client_projection.rs`, `lib.rs`, and `public_api.rs` with typed drafts, submitted status, generation-bound connection, recovery result/metadata/reason, acknowledgement, control IDs/receipt/rejection, and operation-scoped control handle.
2. **Session-owned registry and state transitions:** add the client registry/service to `CodingAgentSession`; implement takeover, stale-generation checks, complete atomic snapshot, draft CRUD, accepted/running/terminal monotonic transitions, and terminal acknowledgement. Keep `run` as the only ordinary dispatcher.
3. **Atomic replay/live recovery:** extend `EventService` with a coordinated recovery boundary; add deterministic small-capacity and emit-during-handoff tests; distinguish retained gap from receiver lag in public metadata.
4. **Scoped control and bounded idempotency:** wrap the private Prompt channel with owner/generation/operation validation, ordered enqueue, receipts, and non-evicting live-session dedup. Test stale handle, wrong owner, mismatch, finished/closed target, invalid input, retry, distinct IDs, capacity rejection, and draft preservation.
5. **Adapter migration and guards:** convert RPC state/prompt paths to the public connection contract, preserve JSON/events, then remove the mirrored authority. Extend public API, compile-fixture, source-boundary, and deterministic protocol tests.

## Gaps / No Exact Analog

| Required Behavior | Why No Exact Current Analog | Planner Direction |
|---|---|---|
| Same-client generation takeover | current `ClientConnection` is a detached value with no registry/generation | session-owned keyed registry; stale every old generation |
| Atomic snapshot/replay/live handoff | current snapshot, replay, and subscribe are separate calls | coordinate inside `EventService`/session under publication boundary |
| Explicit acknowledgement | RPC advances applied markers while projecting delivery | add a separate acknowledged cursor changed only by client call |
| Terminal submitted state retained until ack | current state clears on terminal observation | monotonic status plus associated terminal sequence/outcome reference |
| Owner-scoped immutable Prompt control | raw handle contains only sender | public wrapper captures client/generation/operation and revalidates each call |
| Scoped receipt idempotency | RPC keys are adapter operation keys, not `(client,target,control)` | bounded session-owned receipt cache with original receipt return |

## Anti-Patterns to Guard

- Do not add `submit`/`run` for ordinary operations to `CodingAgentClientConnection`.
- Do not expose raw `ProductEvent`, `PromptControlHandle`, `OperationControl`, Tokio channel types, services, or Flow nodes through `pi_coding_agent::api`.
- Do not independently call `snapshot()`, `product_events_after()`, and `subscribe_product_events_public()` to simulate an atomic handoff.
- Do not advance the recovery cursor when an event is merely delivered.
- Do not silently turn a gap/lag into an empty replay or fresh snapshot.
- Do not authorize control by operation ID alone or retarget to the current Prompt.
- Do not clear a draft before canonical operation/control acceptance.
- Do not persist client-local drafts/acks as durable session-log facts in this phase.
- Do not remove RPC mirrors until public-contract tests exist and protocol behavior is preserved.
- Do not include Phase 9 detach/close, shutdown, exhaustive association closure, or diagnostic hardening.

## Metadata

**Analog search scope:** `crates/pi-coding-agent/src/coding_session`, `crates/pi-coding-agent/src/protocol/rpc`, `crates/pi-coding-agent/tests`, and stable facade in `src/lib.rs`.

**CodeGraph-first discovery:** Used `.codegraph/` via `codegraph explore` for symbol definitions, call paths, callers, and blast radius before targeted source reads.

**Five analog groups:** public projection; session-owned client state; retained replay/broadcast; operation control/admission; RPC migration/idempotency plus contract guards.

**Preserved user file:** untracked `docs/next stage.md` was not read, modified, or staged.
