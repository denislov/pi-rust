# Operation Runtime Stage 6 Snapshot Client Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make RPC, JSON, and interactive adapters thin clients of a session-owned UI snapshot plus product-event projection boundary.

**Architecture:** Add an internal `UiSnapshot`/`ClientConnection` model under `pi-coding-agent::coding_session`, backed by `ProductEventSequence` cursors and retained product events. `CodingAgentSession` remains the runtime owner; adapters receive snapshots and project `ProductEvent`s without mutating session/runtime internals directly.

**Tech Stack:** Rust 2024, `pi-coding-agent`, Tokio broadcast channels, existing `ProductEvent`, `CodingAgentSessionView`, `RpcSessionState`, `CodingEventBridge`, deterministic offline tests.

---

## Current Context

Stage 5 closed operation-local capability snapshots. Stage 6 starts from the existing internal product-event stream:

- `crates/pi-coding-agent/src/coding_session/event.rs` defines `ProductEvent`, `ProductEventKind`, `ProductEventFamily`, `ProductEventSequence`, and durability metadata.
- `crates/pi-coding-agent/src/coding_session/event_service.rs` emits compatibility `CodingAgentEvent`s and internal `ProductEvent`s, and exposes `ProductEventReceiver`.
- `crates/pi-coding-agent/src/coding_session/mod.rs` exposes `subscribe_product_events()` internally and `view()` publicly.
- `crates/pi-coding-agent/src/coding_session/context.rs` defines the current narrow `CodingAgentSessionView`.
- `crates/pi-coding-agent/src/protocol/rpc/state.rs` stores RPC-local running state, messages, steering/follow-up drafts, and active session metadata.
- `crates/pi-coding-agent/src/protocol/rpc/events.rs` already adapts `ProductEvent` to protocol events.
- `crates/pi-coding-agent/src/interactive/prompt_task.rs` already transports `ProductEvent` from prompt tasks to interactive code.

Stage 6 does not remove public compatibility event APIs. It adds the snapshot/client projection contract and migrates adapters behind it while preserving current wire/UI behavior.

## File Structure

- Create: `crates/pi-coding-agent/src/coding_session/client_projection.rs`
  - Internal `UiSnapshot`, cursor, client connection, retained-event reconnect result, and client-local draft types.
- Modify: `crates/pi-coding-agent/src/coding_session/event.rs`
  - Give `ProductEventSequence` comparison and helper methods needed by cursor math.
- Modify: `crates/pi-coding-agent/src/coding_session/event_service.rs`
  - Retain recent `ProductEvent`s, expose current cursor, and serve events after a cursor or require a fresh snapshot on gaps.
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
  - Own client connection creation and expose crate-internal `ui_snapshot()`/`connect_client()` methods.
- Modify: `crates/pi-coding-agent/src/coding_session/context.rs`
  - Keep `CodingAgentSessionView` as the public lightweight view while adding conversion into snapshot session metadata.
- Modify: `crates/pi-coding-agent/src/protocol/types.rs`
  - Add protocol fields for snapshot cursor and client id without removing existing state fields.
- Modify: `crates/pi-coding-agent/src/protocol/rpc/state.rs`
  - Store connected client metadata and client-local drafts separately from runtime-owned submitted operations.
- Modify: `crates/pi-coding-agent/src/protocol/rpc/stats.rs`
  - Build `RpcSessionState` from `UiSnapshot` rather than direct ad hoc session/runtime fields.
- Modify: `crates/pi-coding-agent/src/protocol/rpc/prompt.rs`
  - Drain retained events after a cursor when reconnecting a running prompt.
- Modify: `crates/pi-coding-agent/src/interactive/event_bridge.rs`
  - Add a `UiProjection` that consumes `UiSnapshot` plus `ProductEvent`.
- Modify: `crates/pi-coding-agent/src/interactive/prompt_task.rs`
  - Keep task events as `ProductEvent` and route initial hydration through `UiSnapshot`.
- Modify: `crates/pi-coding-agent/tests/event_boundary_guards.rs`
  - Extend source guards so adapters consume snapshot/product-event facades instead of session/runtime internals.
- Modify: `docs/TODO.md`
  - Track Stage 6 start and each slice.

## Task 1: Product Event Cursor Primitives

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/event.rs`

- [ ] **Step 1: Write failing cursor tests**

Add these tests to the existing `#[cfg(test)] mod tests` in `event.rs`:

```rust
#[test]
fn product_event_sequence_exposes_stable_cursor_math() {
    let first = ProductEventSequence::new(1);
    let second = first.next();

    assert_eq!(first.get(), 1);
    assert_eq!(second.get(), 2);
    assert!(second > first);
    assert_eq!(ProductEventSequence::default(), ProductEventSequence::new(0));
}

#[test]
fn product_event_keeps_sequence_accessor_for_projection() {
    let event = ProductEvent::from_compat_event(
        ProductEventSequence::new(42),
        CodingAgentEvent::SessionOpened {
            session_id: "sess_cursor".into(),
        },
    );

    assert_eq!(event.sequence(), ProductEventSequence::new(42));
}
```

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent product_event_sequence_exposes_stable_cursor_math --lib
cargo test -p pi-coding-agent product_event_keeps_sequence_accessor_for_projection --lib
```

Expected: fail because `ProductEventSequence::new()`, `get()`, `next()`, `Default`, and ordering are not all available.

- [ ] **Step 3: Add cursor helpers**

Change `ProductEventSequence` in `crates/pi-coding-agent/src/coding_session/event.rs` to:

```rust
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ProductEventSequence(pub(crate) u64);

impl ProductEventSequence {
    pub(crate) fn new(value: u64) -> Self {
        Self(value)
    }

    pub(crate) fn get(self) -> u64 {
        self.0
    }

    pub(crate) fn next(self) -> Self {
        Self(self.0 + 1)
    }
}
```

Keep the existing `ProductEvent::sequence()` accessor and remove its `#[allow(dead_code)]` once Stage 6 callers use it.

- [ ] **Step 4: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent product_event_sequence_exposes_stable_cursor_math --lib
cargo test -p pi-coding-agent product_event_keeps_sequence_accessor_for_projection --lib
```

Expected: both tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/event.rs
git commit -m "feat: add product event cursor primitives"
```

## Task 2: Internal UI Snapshot And Client Model

**Files:**
- Create: `crates/pi-coding-agent/src/coding_session/client_projection.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`

- [ ] **Step 1: Write failing snapshot model tests**

Create `crates/pi-coding-agent/src/coding_session/client_projection.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::capability_snapshot::CapabilityGeneration;
    use crate::coding_session::context::CodingAgentSessionView;
    use crate::coding_session::event::ProductEventSequence;
    use crate::coding_session::operation_control::OperationKind;
    use crate::coding_session::profiles::ProfileId;
    use crate::coding_session::{CapabilityStatus, CodingAgentCapabilities};

    fn capabilities() -> CodingAgentCapabilities {
        CodingAgentCapabilities {
            prompt: CapabilityStatus::Available,
            abort: CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            },
            steer: CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            },
            follow_up: CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            },
            compact: CapabilityStatus::Available,
            fork: CapabilityStatus::Available,
            clone_session: CapabilityStatus::Available,
            branch_summary: CapabilityStatus::Available,
            switch_session: CapabilityStatus::Unsupported {
                reason: "session switching is not exposed on CodingAgentSession yet".into(),
            },
            export: CapabilityStatus::Available,
            plugin_reload: CapabilityStatus::Available,
            agent_profiles: CapabilityStatus::Available,
            team_profiles: CapabilityStatus::Available,
            delegation: CapabilityStatus::Available,
            self_healing_edit: CapabilityStatus::Available,
            tools: CapabilityStatus::Available,
            shell: CapabilityStatus::Available,
            plugins: CapabilityStatus::Available,
        }
    }

    #[test]
    fn ui_snapshot_carries_cursor_session_and_runtime_state() {
        let snapshot = UiSnapshot::new(
            UiSnapshotCursor {
                last_event_sequence: ProductEventSequence::new(7),
                capability_generation: CapabilityGeneration::new(3),
            },
            CodingAgentSessionView {
                session_id: "sess_ui".into(),
                default_agent_profile_id: ProfileId::from("reviewer"),
            },
            capabilities(),
            Some(OperationKind::Prompt),
            Vec::new(),
        );

        assert_eq!(snapshot.cursor.last_event_sequence.get(), 7);
        assert_eq!(snapshot.cursor.capability_generation.get(), 3);
        assert_eq!(snapshot.session.session_id, "sess_ui");
        assert_eq!(snapshot.active_operation, Some(OperationKind::Prompt));
        assert!(snapshot.client_drafts.is_empty());
    }

    #[test]
    fn client_connection_starts_from_snapshot_cursor() {
        let snapshot = UiSnapshot::new(
            UiSnapshotCursor {
                last_event_sequence: ProductEventSequence::new(11),
                capability_generation: CapabilityGeneration::new(2),
            },
            CodingAgentSessionView {
                session_id: "sess_client".into(),
                default_agent_profile_id: ProfileId::from("default"),
            },
            capabilities(),
            None,
            vec![ClientDraft::new(ClientDraftKind::Prompt, "draft text")],
        );

        let connection = ClientConnection::new(ClientConnectionId::new("rpc-1"), snapshot.clone());

        assert_eq!(connection.id.as_str(), "rpc-1");
        assert_eq!(connection.cursor, snapshot.cursor);
        assert_eq!(connection.client_drafts.len(), 1);
        assert!(connection.submitted_operation.is_none());
    }
}
```

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent ui_snapshot_carries_cursor_session_and_runtime_state --lib
cargo test -p pi-coding-agent client_connection_starts_from_snapshot_cursor --lib
```

Expected: fail because `client_projection` and its types are not declared.

- [ ] **Step 3: Add the client projection model**

Replace the file body before the test module with:

```rust
use super::capability_snapshot::CapabilityGeneration;
use super::context::CodingAgentSessionView;
use super::event::ProductEventSequence;
use super::operation_control::OperationKind;
use super::CodingAgentCapabilities;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UiSnapshotCursor {
    pub(crate) last_event_sequence: ProductEventSequence,
    pub(crate) capability_generation: CapabilityGeneration,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UiSnapshot {
    pub(crate) cursor: UiSnapshotCursor,
    pub(crate) session: CodingAgentSessionView,
    pub(crate) capabilities: CodingAgentCapabilities,
    pub(crate) active_operation: Option<OperationKind>,
    pub(crate) client_drafts: Vec<ClientDraft>,
}

impl UiSnapshot {
    pub(crate) fn new(
        cursor: UiSnapshotCursor,
        session: CodingAgentSessionView,
        capabilities: CodingAgentCapabilities,
        active_operation: Option<OperationKind>,
        client_drafts: Vec<ClientDraft>,
    ) -> Self {
        Self {
            cursor,
            session,
            capabilities,
            active_operation,
            client_drafts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClientConnectionId(String);

impl ClientConnectionId {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClientDraftKind {
    Prompt,
    Steer,
    FollowUp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClientDraft {
    pub(crate) kind: ClientDraftKind,
    pub(crate) text: String,
}

impl ClientDraft {
    pub(crate) fn new(kind: ClientDraftKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SubmittedOperation {
    pub(crate) operation_id: String,
    pub(crate) kind: OperationKind,
}

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

Declare the module and re-export crate-internal types in `crates/pi-coding-agent/src/coding_session/mod.rs`:

```rust
mod client_projection;

pub(crate) use client_projection::{
    ClientConnection, ClientConnectionId, ClientDraft, ClientDraftKind, SubmittedOperation,
    UiSnapshot, UiSnapshotCursor,
};
```

- [ ] **Step 4: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent ui_snapshot_carries_cursor_session_and_runtime_state --lib
cargo test -p pi-coding-agent client_connection_starts_from_snapshot_cursor --lib
cargo check -p pi-coding-agent
```

Expected: selected tests and crate check pass.

- [ ] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/client_projection.rs crates/pi-coding-agent/src/coding_session/mod.rs
git commit -m "feat: add UI snapshot client model"
```

## Task 3: Retained Product Events And Gap Detection

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/event_service.rs`

- [ ] **Step 1: Write failing retention tests**

Add tests to `event_service.rs`:

```rust
#[test]
fn retained_product_events_can_resume_after_cursor() {
    let service = EventService::new();
    service.emit(CodingAgentEvent::SessionOpened {
        session_id: "sess_retained".into(),
    });
    service.emit(CodingAgentEvent::Diagnostic {
        operation_id: None,
        message: "ready".into(),
    });

    let retained = service
        .product_events_after(ProductEventSequence::new(1))
        .unwrap();

    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].sequence(), ProductEventSequence::new(2));
}

#[test]
fn retained_product_events_report_gap_before_oldest_sequence() {
    let service = EventService::with_event_capacity_for_tests(2);
    for index in 0..4 {
        service.emit(CodingAgentEvent::Diagnostic {
            operation_id: None,
            message: format!("event {index}"),
        });
    }

    let error = service
        .product_events_after(ProductEventSequence::new(1))
        .unwrap_err();

    assert_eq!(error.code(), "event_stream_gap");
    assert!(error.to_string().contains("fresh UI snapshot"));
}
```

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent retained_product_events_can_resume_after_cursor --lib
cargo test -p pi-coding-agent retained_product_events_report_gap_before_oldest_sequence --lib
```

Expected: fail because retained product events and `event_stream_gap` do not exist.

- [ ] **Step 3: Add retained product event storage**

Change `EventService` to carry retained events:

```rust
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub(crate) struct EventService {
    sender: broadcast::Sender<CodingAgentEvent>,
    product_sender: broadcast::Sender<ProductEvent>,
    next_sequence: Arc<AtomicU64>,
    retained_product_events: Arc<Mutex<VecDeque<ProductEvent>>>,
    retained_capacity: usize,
}
```

Add constructors and helpers:

```rust
impl EventService {
    pub(crate) fn new() -> Self {
        Self::with_event_capacity(EVENT_CHANNEL_CAPACITY)
    }

    fn with_event_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        let (product_sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            product_sender,
            next_sequence: Arc::new(AtomicU64::new(1)),
            retained_product_events: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            retained_capacity: capacity,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_event_capacity_for_tests(capacity: usize) -> Self {
        Self::with_event_capacity(capacity)
    }

    pub(crate) fn current_product_sequence(&self) -> ProductEventSequence {
        ProductEventSequence::new(self.next_sequence.load(Ordering::Relaxed).saturating_sub(1))
    }

    pub(crate) fn product_events_after(
        &self,
        cursor: ProductEventSequence,
    ) -> Result<Vec<ProductEvent>, CodingSessionError> {
        let retained = self.retained_product_events.lock().unwrap();
        let Some(oldest) = retained.front().map(ProductEvent::sequence) else {
            return Ok(Vec::new());
        };
        if cursor < oldest && cursor != ProductEventSequence::default() {
            return Err(CodingSessionError::EventStreamGap {
                requested_after: cursor.get(),
                oldest_available: oldest.get(),
            });
        }
        Ok(retained
            .iter()
            .filter(|event| event.sequence() > cursor)
            .cloned()
            .collect())
    }

    fn retain_product_event(&self, event: ProductEvent) {
        let mut retained = self.retained_product_events.lock().unwrap();
        if retained.len() == self.retained_capacity {
            retained.pop_front();
        }
        retained.push_back(event);
    }
}
```

Update `emit()` so it retains before broadcasting:

```rust
pub(crate) fn emit(&self, event: CodingAgentEvent) -> ProductEvent {
    let sequence = ProductEventSequence::new(self.next_sequence.fetch_add(1, Ordering::Relaxed));
    let product_event = ProductEvent::from_compat_event(sequence, event);
    self.retain_product_event(product_event.clone());
    let _ = self.product_sender.send(product_event.clone());
    let _ = self
        .sender
        .send(product_event.compatibility_event().clone());
    product_event
}
```

Add this error variant to `CodingSessionError` in its existing definition:

```rust
EventStreamGap {
    requested_after: u64,
    oldest_available: u64,
},
```

Its `code()` arm must return `"event_stream_gap"` and its display text must contain:

```rust
"event stream gap after sequence {requested_after}; oldest available product event is {oldest_available}; client must request a fresh UI snapshot"
```

- [ ] **Step 4: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent retained_product_events_can_resume_after_cursor --lib
cargo test -p pi-coding-agent retained_product_events_report_gap_before_oldest_sequence --lib
cargo test -p pi-coding-agent event_service --lib
cargo check -p pi-coding-agent
```

Expected: event-service tests and crate check pass.

- [ ] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/event_service.rs crates/pi-coding-agent/src/coding_session/error.rs
git commit -m "feat: retain product events for client reconnect"
```

## Task 4: Session-Owned UI Snapshot And Client Connection API

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/client_projection.rs`

- [ ] **Step 1: Write failing session snapshot tests**

Add tests to `coding_session/mod.rs`:

```rust
#[test]
fn ui_snapshot_uses_session_view_capabilities_and_event_cursor() {
    let session = CodingAgentSession::new(CodingAgentSessionOptions::default()).unwrap();
    let snapshot = session.ui_snapshot(Vec::new());

    assert_eq!(snapshot.session.session_id, session.view().session_id);
    assert_eq!(
        snapshot.cursor.last_event_sequence,
        session.event_service.current_product_sequence()
    );
    assert_eq!(
        snapshot.cursor.capability_generation,
        session.current_capability_generation_for_tests()
    );
    assert_eq!(snapshot.active_operation, None);
}

#[test]
fn connect_client_returns_connection_and_initial_snapshot() {
    let session = CodingAgentSession::new(CodingAgentSessionOptions::default()).unwrap();

    let (connection, snapshot) = session.connect_client(
        ClientConnectionId::new("rpc-primary"),
        vec![ClientDraft::new(ClientDraftKind::Prompt, "hello")],
    );

    assert_eq!(connection.id.as_str(), "rpc-primary");
    assert_eq!(connection.cursor, snapshot.cursor);
    assert_eq!(connection.client_drafts.len(), 1);
}
```

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent ui_snapshot_uses_session_view_capabilities_and_event_cursor --lib
cargo test -p pi-coding-agent connect_client_returns_connection_and_initial_snapshot --lib
```

Expected: fail because `ui_snapshot()` and `connect_client()` are missing.

- [ ] **Step 3: Add session-owned snapshot API**

Add methods to `impl CodingAgentSession`:

```rust
pub(crate) fn ui_snapshot(&self, client_drafts: Vec<ClientDraft>) -> UiSnapshot {
    IntentRouter::admit_query(&self.operation_control, QueryIntent::SessionView);
    UiSnapshot::new(
        UiSnapshotCursor {
            last_event_sequence: self.event_service.current_product_sequence(),
            capability_generation: self.capability_snapshots.current_generation(),
        },
        self.view(),
        self.capabilities(),
        self.operation_control.active(),
        client_drafts,
    )
}

pub(crate) fn connect_client(
    &self,
    id: ClientConnectionId,
    client_drafts: Vec<ClientDraft>,
) -> (ClientConnection, UiSnapshot) {
    let snapshot = self.ui_snapshot(client_drafts);
    let connection = ClientConnection::new(id, snapshot.clone());
    (connection, snapshot)
}

pub(crate) fn product_events_after(
    &self,
    cursor: ProductEventSequence,
) -> Result<Vec<ProductEvent>, CodingSessionError> {
    self.event_service.product_events_after(cursor)
}
```

Use existing crate-internal imports from `client_projection` and `event`.

- [ ] **Step 4: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent ui_snapshot_uses_session_view_capabilities_and_event_cursor --lib
cargo test -p pi-coding-agent connect_client_returns_connection_and_initial_snapshot --lib
cargo check -p pi-coding-agent
```

Expected: selected tests and crate check pass.

- [ ] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/mod.rs crates/pi-coding-agent/src/coding_session/client_projection.rs
git commit -m "feat: expose session UI snapshots"
```

## Task 5: RPC State Consumes UI Snapshot

**Files:**
- Modify: `crates/pi-coding-agent/src/protocol/types.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/state.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/stats.rs`

- [ ] **Step 1: Write failing RPC snapshot tests**

Add tests to `protocol/rpc/stats.rs`:

```rust
#[test]
fn rpc_state_includes_snapshot_cursor_and_client_id() {
    let state = rpc_state_with_coding_session();
    let response = get_state_response(&state).unwrap();

    assert_eq!(response.session.client_id.as_deref(), Some("rpc-primary"));
    assert!(response.session.snapshot_sequence >= 0);
    assert!(response.session.capability_generation >= 1);
}

#[test]
fn rpc_pending_message_count_comes_from_client_drafts() {
    let mut state = rpc_state_with_coding_session();
    state.client_drafts = vec![
        ClientDraft::new(ClientDraftKind::Steer, "steer"),
        ClientDraft::new(ClientDraftKind::FollowUp, "follow"),
    ];

    let response = get_state_response(&state).unwrap();

    assert_eq!(response.session.pending_message_count, 2);
}
```

Add these local helpers in the same `#[cfg(test)]` module:

```rust
fn rpc_state_with_coding_session() -> RpcState {
    let mut state = RpcState::new(CliRunOptions::default()).unwrap();
    state.coding_session =
        Some(CodingAgentSession::new(CodingAgentSessionOptions::default()).unwrap());
    state.client_id = Some(ClientConnectionId::new("rpc-primary"));
    state
}

fn get_state_response(state: &RpcState) -> Result<RpcStateSnapshotForTests, serde_json::Error> {
    let value = serde_json::to_value(state.session_state()).unwrap();
    serde_json::from_value(value)
}

#[derive(Debug, serde::Deserialize)]
struct RpcStateSnapshotForTests {
    session: RpcSessionStateForTests,
}

#[derive(Debug, serde::Deserialize)]
struct RpcSessionStateForTests {
    #[serde(rename = "clientId")]
    client_id: Option<String>,
    #[serde(rename = "snapshotSequence")]
    snapshot_sequence: u64,
    #[serde(rename = "capabilityGeneration")]
    capability_generation: u64,
    #[serde(rename = "pendingMessageCount")]
    pending_message_count: usize,
}
```

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent rpc_state_includes_snapshot_cursor_and_client_id --lib
cargo test -p pi-coding-agent rpc_pending_message_count_comes_from_client_drafts --lib
```

Expected: fail because RPC state does not store client id/drafts and `RpcSessionState` has no snapshot cursor fields.

- [ ] **Step 3: Extend RPC protocol state fields**

Add fields to `RpcSessionState` in `protocol/types.rs`:

```rust
#[serde(rename = "clientId", skip_serializing_if = "Option::is_none")]
pub client_id: Option<String>,
#[serde(rename = "snapshotSequence")]
pub snapshot_sequence: u64,
#[serde(rename = "capabilityGeneration")]
pub capability_generation: u64,
```

Add fields to `RpcState` in `protocol/rpc/state.rs`:

```rust
pub(super) client_id: Option<ClientConnectionId>,
pub(super) client_drafts: Vec<ClientDraft>,
```

Initialize them where `RpcState` is constructed:

```rust
client_id: Some(ClientConnectionId::new("rpc-primary")),
client_drafts: Vec::new(),
```

- [ ] **Step 4: Build RPC state from `UiSnapshot`**

In `protocol/rpc/stats.rs`, replace direct session/capability reads for coding sessions with:

```rust
let client_drafts = state.client_drafts.clone();
let projection = RpcStateProjection::from_state(state, client_drafts);
```

Map `RpcSessionState` fields from the snapshot:

```rust
RpcSessionState {
    model: Some(state.model.clone()),
    thinking_level: state.thinking_level,
    is_streaming: state.running.is_some(),
    is_compacting: state.is_compacting,
    steering_mode: state.steering_mode,
    follow_up_mode: state.follow_up_mode,
    session_file: state.active_session_path.as_ref().map(|path| path.display().to_string()),
    session_id: projection.session_id,
    session_name: state.session_name.clone(),
    auto_compaction_enabled: state.auto_compaction_enabled,
    message_count: state.messages.len(),
    pending_message_count: projection.pending_message_count,
    capabilities: RpcCapabilities::from(projection.capabilities),
    client_id: state.client_id.as_ref().map(|id| id.as_str().to_owned()),
    snapshot_sequence: projection.snapshot_sequence,
    capability_generation: projection.capability_generation,
}
```

Add this helper in `protocol/rpc/stats.rs`:

```rust
struct RpcStateProjection {
    session_id: String,
    pending_message_count: usize,
    capabilities: CodingAgentCapabilities,
    snapshot_sequence: u64,
    capability_generation: u64,
}

impl RpcStateProjection {
    fn from_state(state: &RpcState, client_drafts: Vec<ClientDraft>) -> Self {
        match state.coding_session.as_ref() {
            Some(session) => {
                let snapshot = session.ui_snapshot(client_drafts);
                Self {
                    session_id: snapshot.session.session_id,
                    pending_message_count: snapshot.client_drafts.len(),
                    capabilities: snapshot.capabilities,
                    snapshot_sequence: snapshot.cursor.last_event_sequence.get(),
                    capability_generation: snapshot.cursor.capability_generation.get(),
                }
            }
            None => Self {
                session_id: state
                    .active_leaf_id
                    .clone()
                    .or_else(|| {
                        state
                            .active_session_path
                            .as_ref()
                            .and_then(|path| path.file_stem())
                            .and_then(|stem| stem.to_str())
                            .map(ToString::to_string)
                    })
                    .unwrap_or_else(|| "in-memory".into()),
                pending_message_count: state.steering.len() + state.follow_up.len(),
                capabilities: state.capabilities(),
                snapshot_sequence: 0,
                capability_generation: 1,
            },
        }
    }
}
```

- [ ] **Step 5: Run GREEN and RPC regression tests**

Run:

```bash
cargo test -p pi-coding-agent rpc_state_includes_snapshot_cursor_and_client_id --lib
cargo test -p pi-coding-agent rpc_pending_message_count_comes_from_client_drafts --lib
cargo test -p pi-coding-agent --test rpc_mode get_state
cargo check -p pi-coding-agent
```

Expected: selected RPC tests and crate check pass.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/protocol/types.rs crates/pi-coding-agent/src/protocol/rpc/state.rs crates/pi-coding-agent/src/protocol/rpc/stats.rs
git commit -m "feat: build RPC state from UI snapshots"
```

## Task 6: RPC Reconnect Uses Snapshot Cursor

**Files:**
- Modify: `crates/pi-coding-agent/src/protocol/types.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/prompt.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/commands.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/state.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`

- [x] **Step 1: Write failing reconnect tests**

Add tests to the RPC prompt module or `rpc_mode` test suite:

```rust
#[tokio::test]
async fn rpc_reconnect_replays_retained_product_events_after_snapshot_cursor() {
    let mut state = rpc_state_with_running_coding_prompt().await;
    let cursor = state
        .coding_session
        .as_ref()
        .unwrap()
        .ui_snapshot(Vec::new())
        .cursor
        .last_event_sequence;
    state
        .coding_session
        .as_ref()
        .unwrap()
        .event_service_for_tests()
        .emit(CodingAgentEvent::Diagnostic {
            operation_id: None,
            message: "after snapshot".into(),
        });

    let events = reconnect_running_prompt_after(&mut state, cursor).await.unwrap();

    assert!(
        events
            .iter()
            .any(|event| format!("{event:?}").contains("after snapshot"))
    );
}

#[tokio::test]
async fn rpc_reconnect_gap_returns_fresh_snapshot_required_error() {
    let mut state = rpc_state_with_running_coding_prompt_and_small_event_buffer().await;
    for index in 0..4 {
        state
            .coding_session
            .as_ref()
            .unwrap()
            .event_service_for_tests()
            .emit(CodingAgentEvent::Diagnostic {
                operation_id: None,
                message: format!("event {index}"),
            });
    }

    let error = reconnect_running_prompt_after(&mut state, ProductEventSequence::new(1))
        .await
        .unwrap_err();

    assert_eq!(error.code(), "event_stream_gap");
}
```

Actual adaptation: current RPC moves `CodingAgentSession` into the background task while a prompt is running, so reconnect cannot rely on `state.coding_session`. The implementation exposes a narrow crate-internal `ProductEventReplayHandle` from `CodingAgentSession`, stores it on `CodingRunningPrompt`, and uses cfg(test) session helpers only to construct a small retained-event buffer and emit retained product events. The focused replay test emits an assistant delta marker because plain diagnostic product events are intentionally RPC-silent in the existing adapter.

- [x] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent rpc_reconnect_replays_retained_product_events_after_snapshot_cursor --lib
cargo test -p pi-coding-agent rpc_reconnect_gap_returns_fresh_snapshot_required_error --lib
```

Expected: fail because reconnect-by-cursor does not exist.

- [x] **Step 3: Add reconnect request fields**

Add to the relevant RPC command/request type in `protocol/types.rs`:

```rust
#[serde(rename = "afterSnapshotSequence", skip_serializing_if = "Option::is_none")]
pub after_snapshot_sequence: Option<u64>,
```

Convert it at command handling time:

```rust
let cursor = request
    .after_snapshot_sequence
    .map(ProductEventSequence::new)
    .unwrap_or_default();
```

- [x] **Step 4: Replay retained events before live receiver drain**

Add a helper in `protocol/rpc/prompt.rs`:

```rust
pub(super) async fn reconnect_running_prompt_after(
    state: &mut RpcState,
    cursor: ProductEventSequence,
) -> Result<Vec<ProtocolEvent>, CodingSessionError> {
    let Some(session) = state.coding_session.as_ref() else {
        return Ok(Vec::new());
    };
    let retained = session.product_events_after(cursor)?;
    let Some(RunningPrompt::Coding(running)) = state.running.as_mut() else {
        return Ok(Vec::new());
    };
    let mut protocol_events = Vec::new();
    for event in retained {
        running.adapter.push_product_event(&event).into_iter().for_each(|event| {
            protocol_events.push(event);
        });
        running.events_closed = false;
    }
    Ok(protocol_events)
}
```

Call this helper when a reconnect or resume command supplies `afterSnapshotSequence`.

- [x] **Step 5: Run GREEN and reconnect regressions**

Run:

```bash
cargo test -p pi-coding-agent rpc_reconnect_replays_retained_product_events_after_snapshot_cursor --lib
cargo test -p pi-coding-agent rpc_reconnect_gap_returns_fresh_snapshot_required_error --lib
cargo test -p pi-coding-agent --test rpc_mode reconnect
cargo check -p pi-coding-agent
```

Expected: reconnect tests and crate check pass. If the `rpc_mode reconnect` filter has no tests, it should run zero tests and exit successfully; keep the focused module tests as the binding verification.

- [x] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/protocol/types.rs crates/pi-coding-agent/src/protocol/rpc/prompt.rs crates/pi-coding-agent/src/protocol/rpc/commands.rs crates/pi-coding-agent/src/coding_session/mod.rs
git commit -m "feat: replay retained product events on RPC reconnect"
```

## Task 7: Interactive Projection Consumes Snapshot Plus Product Events

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/event_bridge.rs`
- Modify: `crates/pi-coding-agent/src/interactive/prompt_task.rs`
- Modify: `crates/pi-coding-agent/src/interactive/loop.rs`

- [ ] **Step 1: Write failing interactive projection tests**

Add tests to `interactive/event_bridge.rs`:

```rust
#[test]
fn ui_projection_hydrates_from_snapshot() {
    let snapshot = UiSnapshot::new(
        UiSnapshotCursor {
            last_event_sequence: ProductEventSequence::new(3),
            capability_generation: CapabilityGeneration::new(1),
        },
        CodingAgentSessionView {
            session_id: "sess_interactive".into(),
            default_agent_profile_id: ProfileId::from("default"),
        },
        capabilities(),
        None,
        Vec::new(),
    );

    let mut projection = UiProjection::from_snapshot(snapshot);

    assert_eq!(projection.last_sequence(), ProductEventSequence::new(3));
    assert!(
        projection
            .drain()
            .iter()
            .any(|event| matches!(event, UiEvent::SystemNotice { text } if text.contains("sess_interactive")))
    );
}

#[test]
fn ui_projection_applies_product_events_in_sequence_order() {
    let snapshot = UiSnapshot::new(
        UiSnapshotCursor {
            last_event_sequence: ProductEventSequence::new(0),
            capability_generation: CapabilityGeneration::new(1),
        },
        CodingAgentSessionView {
            session_id: "sess_projection".into(),
            default_agent_profile_id: ProfileId::from("default"),
        },
        capabilities(),
        Some(OperationKind::Prompt),
        Vec::new(),
    );
    let mut projection = UiProjection::from_snapshot(snapshot);
    let event = ProductEvent::from_compat_event(
        ProductEventSequence::new(1),
        CodingAgentEvent::AssistantMessageDelta {
            operation_id: "op_projection".into(),
            turn_id: "turn_projection".into(),
            message_id: Some("msg_projection".into()),
            text: "hello".into(),
        },
    );

    projection.apply_product_event(&event);

    assert_eq!(projection.last_sequence(), ProductEventSequence::new(1));
    assert!(
        projection
            .drain()
            .iter()
            .any(|event| matches!(event, UiEvent::AssistantDelta { text } if text == "hello"))
    );
}
```

Add this local helper in the `interactive/event_bridge.rs` test module and use it instead of a global test constructor:

```rust
fn capabilities() -> CodingAgentCapabilities {
    CodingAgentCapabilities {
        prompt: CapabilityStatus::Available,
        abort: CapabilityStatus::Disabled {
            reason: "no prompt is running".into(),
        },
        steer: CapabilityStatus::Disabled {
            reason: "no prompt is running".into(),
        },
        follow_up: CapabilityStatus::Disabled {
            reason: "no prompt is running".into(),
        },
        compact: CapabilityStatus::Available,
        fork: CapabilityStatus::Available,
        clone_session: CapabilityStatus::Available,
        branch_summary: CapabilityStatus::Available,
        switch_session: CapabilityStatus::Unsupported {
            reason: "session switching is not exposed on CodingAgentSession yet".into(),
        },
        export: CapabilityStatus::Available,
        plugin_reload: CapabilityStatus::Available,
        self_healing_edit: CapabilityStatus::Available,
        agent_profiles: CapabilityStatus::Available,
        team_profiles: CapabilityStatus::Available,
        delegation: CapabilityStatus::Available,
        tools: CapabilityStatus::Available,
        shell: CapabilityStatus::Available,
        plugins: CapabilityStatus::Available,
    }
}
```

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent ui_projection_hydrates_from_snapshot --lib
cargo test -p pi-coding-agent ui_projection_applies_product_events_in_sequence_order --lib
```

Expected: fail because `UiProjection` does not exist.

- [ ] **Step 3: Add `UiProjection`**

In `interactive/event_bridge.rs`, keep `CodingEventBridge` for compatibility and add:

```rust
pub(crate) struct UiProjection {
    bridge: CodingEventBridge,
    last_sequence: ProductEventSequence,
    pending: Vec<UiEvent>,
}

impl UiProjection {
    pub(crate) fn from_snapshot(snapshot: UiSnapshot) -> Self {
        let mut pending = Vec::new();
        pending.push(UiEvent::SystemNotice {
            text: format!("Session {}", snapshot.session.session_id),
        });
        if let Some(kind) = snapshot.active_operation {
            pending.push(UiEvent::SystemNotice {
                text: format!("Active operation: {kind:?}"),
            });
        }
        Self {
            bridge: CodingEventBridge::new(),
            last_sequence: snapshot.cursor.last_event_sequence,
            pending,
        }
    }

    pub(crate) fn last_sequence(&self) -> ProductEventSequence {
        self.last_sequence
    }

    pub(crate) fn apply_product_event(&mut self, event: &ProductEvent) {
        self.last_sequence = event.sequence();
        self.pending
            .extend(self.bridge.push_product_event(event));
    }

    pub(crate) fn drain(&mut self) -> Vec<UiEvent> {
        std::mem::take(&mut self.pending)
    }
}
```

Add `CodingEventBridge::push_product_event()` as the only adapter-facing method used by `UiProjection`:

```rust
pub(crate) fn push_product_event(&mut self, event: &ProductEvent) -> Vec<UiEvent> {
    self.push(event.compatibility_event().clone())
}
```

- [ ] **Step 4: Route interactive prompt hydration through snapshot**

When `PromptTask` starts a coding prompt, build:

```rust
let snapshot = session.ui_snapshot(Vec::new());
let projection = UiProjection::from_snapshot(snapshot);
```

Store `projection.last_sequence()` in the interactive loop state and apply incoming `PromptTaskEvent::Coding(product_event)` through `projection.apply_product_event(&product_event)` before rendering UI events.

Do not change visible transcript rendering in this task. The output event stream must remain the same after snapshot hydration notices are filtered or rendered as existing system notices.

- [ ] **Step 5: Run GREEN and interactive regressions**

Run:

```bash
cargo test -p pi-coding-agent ui_projection_hydrates_from_snapshot --lib
cargo test -p pi-coding-agent ui_projection_applies_product_events_in_sequence_order --lib
cargo test -p pi-coding-agent interactive --lib
cargo check -p pi-coding-agent
```

Expected: projection tests, interactive unit tests, and crate check pass.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/interactive/event_bridge.rs crates/pi-coding-agent/src/interactive/prompt_task.rs crates/pi-coding-agent/src/interactive/loop.rs
git commit -m "feat: project interactive UI from snapshots"
```

## Task 8: Separate Client Drafts From Submitted Operations

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/client_projection.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/state.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/commands.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/prompt.rs`

- [ ] **Step 1: Write failing draft separation tests**

Add tests to `client_projection.rs`:

```rust
#[test]
fn submitted_operation_clears_matching_prompt_draft() {
    let snapshot = UiSnapshot::new(
        UiSnapshotCursor {
            last_event_sequence: ProductEventSequence::new(0),
            capability_generation: CapabilityGeneration::new(1),
        },
        CodingAgentSessionView {
            session_id: "sess_drafts".into(),
            default_agent_profile_id: ProfileId::from("default"),
        },
        capabilities(),
        None,
        vec![ClientDraft::new(ClientDraftKind::Prompt, "build this")],
    );
    let mut connection = ClientConnection::new(ClientConnectionId::new("rpc-1"), snapshot);

    connection.mark_submitted(SubmittedOperation {
        operation_id: "op_submitted".into(),
        kind: OperationKind::Prompt,
    });

    assert!(connection.client_drafts.is_empty());
    assert_eq!(
        connection.submitted_operation.as_ref().unwrap().operation_id,
        "op_submitted"
    );
}

#[test]
fn steer_and_follow_up_drafts_remain_client_local_until_submitted() {
    let snapshot = UiSnapshot::new(
        UiSnapshotCursor {
            last_event_sequence: ProductEventSequence::new(0),
            capability_generation: CapabilityGeneration::new(1),
        },
        CodingAgentSessionView {
            session_id: "sess_local".into(),
            default_agent_profile_id: ProfileId::from("default"),
        },
        capabilities(),
        Some(OperationKind::Prompt),
        vec![
            ClientDraft::new(ClientDraftKind::Steer, "steer"),
            ClientDraft::new(ClientDraftKind::FollowUp, "follow"),
        ],
    );
    let mut connection = ClientConnection::new(ClientConnectionId::new("rpc-1"), snapshot);

    connection.mark_submitted(SubmittedOperation {
        operation_id: "op_prompt".into(),
        kind: OperationKind::Prompt,
    });

    assert_eq!(connection.client_drafts.len(), 2);
}
```

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent submitted_operation_clears_matching_prompt_draft --lib
cargo test -p pi-coding-agent steer_and_follow_up_drafts_remain_client_local_until_submitted --lib
```

Expected: fail because `mark_submitted()` does not exist.

- [ ] **Step 3: Add draft/submission semantics**

Add to `ClientConnection`:

```rust
impl ClientConnection {
    pub(crate) fn mark_submitted(&mut self, submitted: SubmittedOperation) {
        if submitted.kind == OperationKind::Prompt {
            self.client_drafts
                .retain(|draft| draft.kind != ClientDraftKind::Prompt);
        }
        self.submitted_operation = Some(submitted);
    }

    pub(crate) fn clear_submitted_operation(&mut self, operation_id: &str) {
        if self
            .submitted_operation
            .as_ref()
            .is_some_and(|submitted| submitted.operation_id == operation_id)
        {
            self.submitted_operation = None;
        }
    }
}
```

- [ ] **Step 4: Route RPC draft state through `ClientConnection`**

When RPC queues `steering` or `follow_up`, mirror those strings into `state.client_drafts`:

```rust
state.client_drafts = state
    .steering
    .iter()
    .cloned()
    .map(|text| ClientDraft::new(ClientDraftKind::Steer, text))
    .chain(
        state
            .follow_up
            .iter()
            .cloned()
            .map(|text| ClientDraft::new(ClientDraftKind::FollowUp, text)),
    )
    .collect();
```

When a prompt is submitted and an operation id is known, call:

```rust
connection.mark_submitted(SubmittedOperation {
    operation_id: operation_id.clone(),
    kind: OperationKind::Prompt,
});
```

When a terminal product event for that operation is observed, call `clear_submitted_operation()`.

- [ ] **Step 5: Run GREEN and RPC queue tests**

Run:

```bash
cargo test -p pi-coding-agent submitted_operation_clears_matching_prompt_draft --lib
cargo test -p pi-coding-agent steer_and_follow_up_drafts_remain_client_local_until_submitted --lib
cargo test -p pi-coding-agent --test rpc_mode steer
cargo test -p pi-coding-agent --test rpc_mode follow
cargo check -p pi-coding-agent
```

Expected: selected tests and crate check pass.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/client_projection.rs crates/pi-coding-agent/src/protocol/rpc/state.rs crates/pi-coding-agent/src/protocol/rpc/commands.rs crates/pi-coding-agent/src/protocol/rpc/prompt.rs
git commit -m "feat: separate client drafts from submitted operations"
```

## Task 9: Boundary Guards For Snapshot/Product-Event Adapters

**Files:**
- Modify: `crates/pi-coding-agent/tests/event_boundary_guards.rs`
- Modify: `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs`
- Modify: `docs/TODO.md`

- [x] **Step 1: Write source guards**

Add these guards to `event_boundary_guards.rs` (the current repository's event/adapter boundary guard target):

```rust
#[test]
fn rpc_state_consumes_ui_snapshot_boundary() {
    let source = include_str!("../src/protocol/rpc/stats.rs");

    assert!(
        source.contains(".ui_snapshot("),
        "RPC get_state must consume CodingAgentSession::ui_snapshot()"
    );
    assert!(
        !source.contains(".persistent_session_service("),
        "RPC get_state must not reach into session persistence internals"
    );
}

#[test]
fn interactive_projection_consumes_product_events() {
    let source = include_str!("../src/interactive/event_bridge.rs");

    assert!(
        source.contains("UiProjection"),
        "interactive UI must route through UiProjection"
    );
    assert!(
        source.contains("push_product_event"),
        "interactive projection must consume ProductEvent instead of direct Flow/runtime state"
    );
}
```

Add this guard to `product_runtime_boundary_guards.rs`:

```rust
#[test]
fn adapters_do_not_access_event_service_directly_for_projection() {
    for path in [
        "src/protocol/rpc/stats.rs",
        "src/protocol/rpc/prompt.rs",
        "src/interactive/loop.rs",
    ] {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path),
        )
        .unwrap();
        assert!(
            !source.contains(".event_service."),
            "{path} must use session snapshot/product-event facade instead of raw EventService"
        );
    }
}
```

- [x] **Step 2: Run guard checks**

Run:

```bash
cargo test -p pi-coding-agent rpc_state_consumes_ui_snapshot_boundary --test event_boundary_guards
cargo test -p pi-coding-agent interactive_projection_consumes_product_events --test event_boundary_guards
cargo test -p pi-coding-agent adapters_do_not_access_event_service_directly_for_projection --test product_runtime_boundary_guards
```

Result: all three guards passed immediately because the previous tasks had already routed adapters through snapshot/product-event facades. The plan's original placeholder guard target was adapted to the existing `event_boundary_guards` target in this repository.

- [x] **Step 3: Keep only facade calls in adapters**

If a guard fails, replace direct service access with these facade methods:

```rust
session.ui_snapshot(client_drafts)
session.product_events_after(cursor)
session.subscribe_product_events()
```

Do not expose `EventService`, `SessionService`, `RuntimeService`, `FlowService`, or `OperationControl` to protocol or interactive adapters.

- [x] **Step 4: Update TODO progress**

Add a progress log entry to `docs/TODO.md`:

```markdown
- 2026-07-09: Stage 6 snapshot/client boundary started. The implementation plan now defines `UiSnapshot`, `ClientConnection`, retained `ProductEvent` cursor replay, RPC state projection from snapshots, interactive projection from `UiSnapshot + ProductEvent`, client-local draft separation, and adapter source guards.
```

Update the active architecture item so the final sentence becomes:

```markdown
Stage 6 now has an implementation plan for snapshot/reconnect/multi-client projection; Stages 7-8 (backpressure/versioning/recovery hardening and public facade narrowing) remain future work under the same reference architecture.
```

- [x] **Step 5: Run GREEN guards**

Run:

```bash
cargo test -p pi-coding-agent rpc_state_consumes_ui_snapshot_boundary --test event_boundary_guards
cargo test -p pi-coding-agent interactive_projection_consumes_product_events --test event_boundary_guards
cargo test -p pi-coding-agent adapters_do_not_access_event_service_directly_for_projection --test product_runtime_boundary_guards
```

Expected: all three guards pass.

- [x] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/tests/event_boundary_guards.rs crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs docs/TODO.md
git commit -m "test: guard snapshot client adapter boundaries"
```

## Task 10: Stage 6 Verification And Closure

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-09-operation-runtime-stage-6-snapshot-client-boundary-plan.md`

- [x] **Step 1: Run full Stage 6 verification**

Run:

```bash
cargo fmt --check
cargo test -p pi-coding-agent product_event_sequence --lib
cargo test -p pi-coding-agent client_connection --lib
cargo test -p pi-coding-agent retained_product_events --lib
cargo test -p pi-coding-agent ui_snapshot --lib
cargo test -p pi-coding-agent rpc_state --lib
cargo test -p pi-coding-agent rpc_reconnect --lib
cargo test -p pi-coding-agent ui_projection --lib
cargo test -p pi-coding-agent --test rpc_mode get_state
cargo test -p pi-coding-agent interactive --lib
cargo test -p pi-coding-agent --test event_boundary_guards
cargo test -p pi-coding-agent --test product_runtime_boundary_guards
cargo check -p pi-coding-agent
git diff --check
```

Expected: every command exits with code 0.

- [x] **Step 2: Update this plan's verification checklist**

After the commands pass, mark these checkboxes:

```markdown
- [x] `cargo fmt --check`
- [x] `cargo test -p pi-coding-agent product_event_sequence --lib`
- [x] `cargo test -p pi-coding-agent client_connection --lib`
- [x] `cargo test -p pi-coding-agent retained_product_events --lib`
- [x] `cargo test -p pi-coding-agent ui_snapshot --lib`
- [x] `cargo test -p pi-coding-agent rpc_state --lib`
- [x] `cargo test -p pi-coding-agent rpc_reconnect --lib`
- [x] `cargo test -p pi-coding-agent ui_projection --lib`
- [x] `cargo test -p pi-coding-agent --test rpc_mode get_state`
- [x] `cargo test -p pi-coding-agent interactive --lib`
- [x] `cargo test -p pi-coding-agent --test event_boundary_guards`
- [x] `cargo test -p pi-coding-agent --test product_runtime_boundary_guards`
- [x] `cargo check -p pi-coding-agent`
- [x] `git diff --check`
```

- [x] **Step 3: Update `docs/TODO.md` top-level architecture status**

Replace the Stage 6 portion of the active architecture item with:

```markdown
Stage 6 snapshot/reconnect/multi-client projection is complete: `CodingAgentSession` exposes a session-owned `UiSnapshot` cursor, clients connect through `ClientConnection`, RPC state is projected from snapshots, retained `ProductEvent`s replay after snapshot cursors or require fresh snapshots on gaps, interactive UI projection consumes `UiSnapshot + ProductEvent`, and client-local drafts are separate from submitted operations.
```

Add a progress log entry:

```markdown
- 2026-07-09: Stage 6 snapshot/client boundary completed. `UiSnapshot` and `ClientConnection` now define adapter cursor semantics, RPC state and reconnect consume session snapshots plus retained product events, interactive projection consumes `UiSnapshot + ProductEvent`, client-local drafts no longer live in runtime-owned submitted operation state, and adapter guards prevent projection paths from bypassing the snapshot/product-event facade.
```

- [x] **Step 4: Commit closure documentation**

```bash
git add docs/TODO.md docs/superpowers/plans/2026-07-09-operation-runtime-stage-6-snapshot-client-boundary-plan.md
git commit -m "docs: close snapshot client boundary stage"
```

## Verification Checklist

- [x] `cargo fmt --check`
- [x] `cargo test -p pi-coding-agent product_event_sequence --lib`
- [x] `cargo test -p pi-coding-agent client_connection --lib`
- [x] `cargo test -p pi-coding-agent retained_product_events --lib`
- [x] `cargo test -p pi-coding-agent ui_snapshot --lib`
- [x] `cargo test -p pi-coding-agent rpc_state --lib`
- [x] `cargo test -p pi-coding-agent rpc_reconnect --lib`
- [x] `cargo test -p pi-coding-agent ui_projection --lib`
- [x] `cargo test -p pi-coding-agent --test rpc_mode get_state`
- [x] `cargo test -p pi-coding-agent interactive --lib`
- [x] `cargo test -p pi-coding-agent --test event_boundary_guards`
- [x] `cargo test -p pi-coding-agent --test product_runtime_boundary_guards`
- [x] `cargo check -p pi-coding-agent`
- [x] `git diff --check`

## Spec Coverage

- Consistent `UiSnapshot` cursor semantics: Tasks 1, 2, 4.
- TUI/interactive projection consumes snapshot plus product events: Task 7.
- RPC/GUI clients modeled as `ClientConnection`: Tasks 2, 4, 5.
- Client-local drafts separate from runtime-owned submitted operations: Task 8.
- Gap handling and fresh-snapshot recovery: Tasks 3, 6.
- Adapter boundary guards: Task 9.

## Execution Notes

- Keep compatibility event APIs until Stage 8 public facade narrowing. Stage 6 migrates internal adapter implementations without removing external callers.
- Keep retained event buffering intentionally simple. Stage 7 owns bounded queue policy, overflow strategy, protocol negotiation, and restart recovery hardening.
- Do not expose raw `EventService`, `SessionService`, `RuntimeService`, `FlowService`, `OperationControl`, provider state, or plugin internals to adapters while implementing this plan.
