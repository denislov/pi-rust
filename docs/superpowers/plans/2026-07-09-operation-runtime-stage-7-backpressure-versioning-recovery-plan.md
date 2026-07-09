# Operation Runtime Stage 7 Backpressure Versioning Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the operation runtime tolerate slow clients, incompatible protocol clients, process restarts, partial commits, and retried client commands without unbounded memory growth or ambiguous recovery state.

**Architecture:** Keep `CodingAgentSession` as the runtime owner and build Stage 7 on the Stage 6 snapshot/client boundary. `EventService` owns product-event retention and lag semantics; RPC supports an explicit `hello` protocol-family negotiation path while preserving legacy clients that do not send `hello`; `SessionService` owns startup recovery marker writes; adapters recover by requesting a fresh `UiSnapshot` plus retained `ProductEvent`s instead of reaching into runtime services. Negotiated protocol state is recorded per RPC process and projected through `get_state`, but Stage 7 does not make `hello` mandatory because that would break existing RPC clients.

**Tech Stack:** Rust 2024, `pi-coding-agent`, Tokio broadcast and bounded mpsc channels, Rust-native `SessionEvent` log, existing `ProductEvent`, `UiSnapshot`, `ClientConnection`, `RpcSessionState`, `CodingProtocolEventAdapter`, deterministic offline tests.

---

## Current Context

Stage 6 completed the snapshot/reconnect/client boundary:

- `crates/pi-coding-agent/src/coding_session/event.rs` defines `ProductEvent`, `ProductEventSequence`, family classification, terminal status, and durability metadata.
- `crates/pi-coding-agent/src/coding_session/event_service.rs` publishes compatibility `CodingAgentEvent`s and internal `ProductEvent`s, retains a fixed replay window, and maps broadcast lag to a generic resource error.
- `crates/pi-coding-agent/src/coding_session/client_projection.rs` defines `UiSnapshot`, `UiSnapshotCursor`, `ClientConnection`, client drafts, and submitted operations.
- `crates/pi-coding-agent/src/coding_session/mod.rs` exposes crate-internal `ui_snapshot()`, `connect_client()`, `product_event_replay_handle()`, and `product_events_after()` helpers.
- `crates/pi-coding-agent/src/protocol/rpc/state.rs` stores client identity, client drafts, submitted operation state, and running prompt replay watermarks.
- `crates/pi-coding-agent/src/protocol/rpc/prompt.rs` still forwards every running `CodingRunningPrompt` product-event stream through unbounded mpsc channels (`prompt`, `invoke_agent`, `invoke_team`, and delegation approval) and relies on Stage 6 replay logic for reconnect overlap.
- `crates/pi-coding-agent/src/protocol/types.rs` exposes `RpcCommand` and `RpcSessionState` with Stage 6 `clientId`, `snapshotSequence`, and `capabilityGeneration` fields, but no explicit protocol-family negotiation.
- `crates/pi-coding-agent/src/coding_session/session_log/replay.rs` can classify operations as `Committed`, `Failed`, `Aborted`, `Recovered`, or `InDoubt`.
- `crates/pi-coding-agent/src/coding_session/session_service.rs` opens sessions and exposes recovery summaries, but opening a session does not yet write recovery markers for incomplete operations.

Stage 7 must not remove broad public methods or compatibility event APIs. Stage 8 owns public facade narrowing and deletion.

## File Structure

- Create: `crates/pi-coding-agent/src/protocol/version.rs`
  - Protocol-family version structs and constants for RPC, product events, and UI snapshots.
- Create: `crates/pi-coding-agent/src/protocol/rpc/event_queue.rs`
  - Bounded RPC product-event queue item type, queue capacity constant, and overflow tests.
- Modify: `crates/pi-coding-agent/src/coding_session/error.rs`
  - Add stable error codes for product-event lag and unsupported protocol versions.
- Modify: `crates/pi-coding-agent/src/coding_session/event_service.rs`
  - Add explicit backpressure status, lag-to-snapshot recovery errors, and retained-window status tests.
- Modify: `crates/pi-coding-agent/src/coding_session/client_projection.rs`
  - Add the shared `ProtocolFamilyVersion` snapshot version to `UiSnapshot` and cursor rebuild policy tests.
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
  - Populate snapshot version and expose recovery marker application through the session owner where needed.
- Modify: `crates/pi-coding-agent/src/coding_session/session_service.rs`
  - Write startup `OperationRecovered` markers for in-doubt operations and make that recovery idempotent.
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/replay.rs`
  - Add focused replay coverage that startup recovery markers finalize formerly in-doubt operations.
- Modify: `crates/pi-coding-agent/src/coding_session/event.rs`
  - Add recovery product-event classification for adapter/protocol projection.
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
  - Add structured idempotency key validation and request metadata.
- Modify: `crates/pi-coding-agent/src/protocol/types.rs`
  - Add `hello` protocol negotiation command, version payloads, snapshot version fields, and optional RPC idempotency keys.
- Modify: `crates/pi-coding-agent/src/protocol/rpc.rs`
  - Route queued product-event overflow to a clear RPC error response.
- Modify: `crates/pi-coding-agent/src/protocol/rpc/commands.rs`
  - Handle `hello`, unsupported protocol versions, and idempotency-aware root command retries.
- Modify: `crates/pi-coding-agent/src/protocol/rpc/prompt.rs`
  - Replace all running `CodingRunningPrompt` product-event forwarding channels with the bounded queue helper, including prompt, invoke-agent, invoke-team, delegation approval, completion drains, and prompt-module test fixtures.
- Modify: `crates/pi-coding-agent/src/protocol/rpc/state.rs`
  - Store negotiated protocol state, bounded idempotency records, and bounded event queue types.
- Modify: `crates/pi-coding-agent/src/protocol/rpc/stats.rs`
  - Project snapshot version and negotiated protocol state into `RpcSessionState`.
- Modify: `crates/pi-coding-agent/src/protocol/events.rs`
  - Map recovery product events to protocol events without exposing recovery internals as Flow nodes.
- Modify: `crates/pi-coding-agent/src/interactive/event_bridge.rs`
  - Render recovery notices through the existing product-event projection boundary.
- Modify: `crates/pi-coding-agent/tests/rpc_mode.rs`
  - Add RPC negotiation, overflow recovery, and idempotent retry coverage.
- Modify: `crates/pi-coding-agent/tests/event_boundary_guards.rs`
  - Guard adapter recovery/projection paths against raw service access and Flow-node leakage.
- Modify: `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs`
  - Guard RPC running event forwarding against unbounded product-event queues.
- Modify: `docs/TODO.md`
  - Track Stage 7 plan start and closure.

## Task 1: Product Event Backpressure Contract

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/error.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/event_service.rs`

- [ ] **Step 1: Write failing event-service backpressure tests**

Add these tests to the existing `#[cfg(test)] mod tests` in `event_service.rs`:

```rust
#[test]
fn event_service_reports_bounded_product_event_window() {
    let service = EventService::with_event_capacity_for_tests(2);

    for index in 0..4 {
        service.emit(CodingAgentEvent::Diagnostic {
            operation_id: None,
            message: format!("event {index}"),
        });
    }

    let status = service.backpressure_status();

    assert_eq!(status.channel_capacity, 2);
    assert_eq!(status.retained_capacity, 2);
    assert_eq!(status.oldest_retained_sequence, Some(ProductEventSequence::new(3)));
    assert_eq!(status.current_sequence, ProductEventSequence::new(4));
    assert_eq!(status.dropped_before, Some(ProductEventSequence::new(3)));
}

#[tokio::test]
async fn product_event_receiver_lag_reports_snapshot_recovery() {
    let service = EventService::with_event_capacity_for_tests(1);
    let mut receiver = service.subscribe_product_events();

    for index in 0..3 {
        service.emit(CodingAgentEvent::Diagnostic {
            operation_id: None,
            message: format!("event {index}"),
        });
    }

    let error = receiver.recv().await.unwrap_err();

    assert_eq!(error.code(), "event_stream_lag");
    assert!(error.to_string().contains("client must request a fresh UI snapshot"));
}
```

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent event_service_reports_bounded_product_event_window --lib
cargo test -p pi-coding-agent product_event_receiver_lag_reports_snapshot_recovery --lib
```

Expected: both tests fail because `backpressure_status()` and `event_stream_lag` do not exist.

- [ ] **Step 3: Add stable lag and unsupported-version errors**

Add these variants to `CodingSessionError` in `error.rs`:

```rust
#[error(
    "event stream lagged by {skipped} events; client must request a fresh UI snapshot"
)]
EventStreamLag { skipped: u64 },
#[error("unsupported protocol version for {family}: requested {requested}, supported {supported}")]
UnsupportedProtocolVersion {
    family: String,
    requested: String,
    supported: String,
},
```

Add these `code()` arms:

```rust
Self::EventStreamLag { .. } => "event_stream_lag",
Self::UnsupportedProtocolVersion { .. } => "unsupported_protocol_version",
```

Add both variants to `From<CodingSessionError> for CliError` as `CliError::SessionFailure(error.to_string())`.

Extend `coding_session_error_codes_are_stable()` with:

```rust
(
    CodingSessionError::EventStreamLag { skipped: 2 },
    "event_stream_lag",
),
(
    CodingSessionError::UnsupportedProtocolVersion {
        family: "rpc".into(),
        requested: "2.0".into(),
        supported: "1.0".into(),
    },
    "unsupported_protocol_version",
),
```

- [ ] **Step 4: Add explicit backpressure state**

In `event_service.rs`, replace the single capacity constant with:

```rust
const EVENT_CHANNEL_CAPACITY: usize = 128;
const EVENT_RETAINED_CAPACITY: usize = 128;
```

Change `EventService` and `EventPublicationState` to:

```rust
#[derive(Debug, Clone)]
pub(crate) struct EventService {
    sender: broadcast::Sender<CodingAgentEvent>,
    product_sender: broadcast::Sender<ProductEvent>,
    publication_state: Arc<Mutex<EventPublicationState>>,
    channel_capacity: usize,
    retained_capacity: usize,
}

#[derive(Debug)]
struct EventPublicationState {
    next_sequence: u64,
    retained_product_events: VecDeque<ProductEvent>,
    dropped_before: Option<ProductEventSequence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EventBackpressureStatus {
    pub(crate) channel_capacity: usize,
    pub(crate) retained_capacity: usize,
    pub(crate) oldest_retained_sequence: Option<ProductEventSequence>,
    pub(crate) current_sequence: ProductEventSequence,
    pub(crate) dropped_before: Option<ProductEventSequence>,
}
```

Initialize the service with:

```rust
pub(crate) fn new() -> Self {
    Self::with_event_capacities(EVENT_CHANNEL_CAPACITY, EVENT_RETAINED_CAPACITY)
}

fn with_event_capacities(channel_capacity: usize, retained_capacity: usize) -> Self {
    let channel_capacity = channel_capacity.max(1);
    let (sender, _) = broadcast::channel(channel_capacity);
    let (product_sender, _) = broadcast::channel(channel_capacity);
    Self {
        sender,
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

Add the status method:

```rust
pub(crate) fn backpressure_status(&self) -> EventBackpressureStatus {
    let state = self.publication_state.lock().unwrap();
    EventBackpressureStatus {
        channel_capacity: self.channel_capacity,
        retained_capacity: self.retained_capacity,
        oldest_retained_sequence: state
            .retained_product_events
            .front()
            .map(ProductEvent::sequence),
        current_sequence: ProductEventSequence::new(state.next_sequence.saturating_sub(1)),
        dropped_before: state.dropped_before,
    }
}
```

Change `retain_product_event()` so dropping an event records the first retained sequence after the drop:

```rust
fn retain_product_event(&self, state: &mut EventPublicationState, event: ProductEvent) {
    if self.retained_capacity == 0 {
        state.dropped_before = Some(event.sequence().next());
        return;
    }
    let dropped = state.retained_product_events.len() == self.retained_capacity;
    if state.retained_product_events.len() == self.retained_capacity {
        state.retained_product_events.pop_front();
    }
    state.retained_product_events.push_back(event);
    if dropped {
        state.dropped_before = state
            .retained_product_events
            .front()
            .map(ProductEvent::sequence);
    }
}
```

Change both product and compatibility lag mappings to use the stable lag error:

```rust
fn map_recv_error(error: broadcast::error::RecvError) -> CodingSessionError {
    match error {
        broadcast::error::RecvError::Closed => CodingSessionError::Cancelled,
        broadcast::error::RecvError::Lagged(skipped) => CodingSessionError::EventStreamLag {
            skipped,
        },
    }
}
```

In both `try_recv()` methods, map `TryRecvError::Lagged(skipped)` to the same `EventStreamLag` variant.

- [ ] **Step 5: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent event_service_reports_bounded_product_event_window --lib
cargo test -p pi-coding-agent product_event_receiver_lag_reports_snapshot_recovery --lib
cargo test -p pi-coding-agent coding_session_error_codes_are_stable --lib
```

Expected: all three tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/error.rs crates/pi-coding-agent/src/coding_session/event_service.rs
git commit -m "feat: define product event backpressure policy"
```

## Task 2: Bounded RPC Product Event Queue

**Files:**
- Create: `crates/pi-coding-agent/src/protocol/rpc/event_queue.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/prompt.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/state.rs`

- [ ] **Step 1: Write failing bounded queue tests**

Create `crates/pi-coding-agent/src/protocol/rpc/event_queue.rs` with this test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::{CodingAgentEvent, ProductEvent, ProductEventSequence};

    fn event(sequence: u64) -> ProductEvent {
        ProductEvent::from_compat_event(
            ProductEventSequence::new(sequence),
            CodingAgentEvent::Diagnostic {
                operation_id: None,
                message: format!("event {sequence}"),
            },
        )
    }

    #[tokio::test]
    async fn rpc_product_event_queue_is_bounded_and_ordered() {
        let (sender, mut receiver) = RpcProductEventQueue::for_tests(2);

        sender.send_event(event(1)).await.unwrap();
        sender.send_event(event(2)).await.unwrap();

        assert!(matches!(
            receiver.recv().await.unwrap(),
            RpcQueuedProductEvent::Event(product_event) if product_event.sequence() == ProductEventSequence::new(1)
        ));
        assert!(matches!(
            receiver.recv().await.unwrap(),
            RpcQueuedProductEvent::Event(product_event) if product_event.sequence() == ProductEventSequence::new(2)
        ));
    }

    #[tokio::test]
    async fn rpc_product_event_queue_can_report_overflow_recovery() {
        let (sender, mut receiver) = RpcProductEventQueue::for_tests(1);

        sender.send_overflow(3).await.unwrap();

        assert_eq!(
            receiver.recv().await.unwrap(),
            RpcQueuedProductEvent::Overflow { skipped: 3 }
        );
    }
}
```

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent rpc_product_event_queue_is_bounded_and_ordered --lib
cargo test -p pi-coding-agent rpc_product_event_queue_can_report_overflow_recovery --lib
```

Expected: fail because `protocol::rpc::event_queue` does not exist.

- [ ] **Step 3: Add the bounded queue helper**

Put this implementation above the test module in `event_queue.rs`:

```rust
use crate::coding_session::ProductEvent;
use tokio::sync::mpsc;

pub(super) const RPC_PRODUCT_EVENT_QUEUE_CAPACITY: usize = 128;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum RpcQueuedProductEvent {
    Event(ProductEvent),
    Overflow { skipped: u64 },
}

#[derive(Clone)]
pub(super) struct RpcProductEventQueue {
    sender: mpsc::Sender<RpcQueuedProductEvent>,
}

impl RpcProductEventQueue {
    pub(super) fn new() -> (Self, mpsc::Receiver<RpcQueuedProductEvent>) {
        Self::with_capacity(RPC_PRODUCT_EVENT_QUEUE_CAPACITY)
    }

    fn with_capacity(capacity: usize) -> (Self, mpsc::Receiver<RpcQueuedProductEvent>) {
        let (sender, receiver) = mpsc::channel(capacity.max(1));
        (Self { sender }, receiver)
    }

    #[cfg(test)]
    pub(super) fn for_tests(capacity: usize) -> (Self, mpsc::Receiver<RpcQueuedProductEvent>) {
        Self::with_capacity(capacity)
    }

    pub(super) async fn send_event(
        &self,
        event: ProductEvent,
    ) -> Result<(), mpsc::error::SendError<RpcQueuedProductEvent>> {
        self.sender.send(RpcQueuedProductEvent::Event(event)).await
    }

    pub(super) async fn send_overflow(
        &self,
        skipped: u64,
    ) -> Result<(), mpsc::error::SendError<RpcQueuedProductEvent>> {
        self.sender
            .send(RpcQueuedProductEvent::Overflow { skipped })
            .await
    }

    #[cfg(test)]
    pub(super) fn try_send_event(
        &self,
        event: ProductEvent,
    ) -> Result<(), mpsc::error::TrySendError<RpcQueuedProductEvent>> {
        self.sender.try_send(RpcQueuedProductEvent::Event(event))
    }
}
```

Declare the module in `protocol/rpc.rs`:

```rust
mod event_queue;
```

- [ ] **Step 4: Replace every running RPC product-event forwarding channel**

In `state.rs`, import the queue item:

```rust
use crate::protocol::rpc::event_queue::RpcQueuedProductEvent;
```

Change `CodingRunningPrompt.events` to:

```rust
pub(super) events: mpsc::Receiver<RpcQueuedProductEvent>,
```

In `prompt.rs`, import the queue:

```rust
use crate::protocol::rpc::event_queue::{RpcProductEventQueue, RpcQueuedProductEvent};
```

Replace every production `mpsc::unbounded_channel()` that feeds `CodingRunningPrompt.events` with the bounded queue. This includes the event channel setup in:

```text
handle_invoke_agent()
handle_invoke_team()
handle_approve_delegation()
start_coding_session_prompt()
```

For each block, replace:

```rust
let (event_tx, event_rx) = mpsc::unbounded_channel();
```

with:

```rust
let (event_tx, event_rx) = RpcProductEventQueue::new();
```

Before each spawned operation's select loop, add:

```rust
let mut product_event_forwarding_open = true;
```

Replace each live event forwarding branch with:

```rust
event = receiver.recv(), if product_event_forwarding_open => {
    match event {
        Ok(event) => {
            if event_tx.send_event(event).await.is_err() {
                product_event_forwarding_open = false;
            }
        }
        Err(crate::coding_session::CodingSessionError::EventStreamLag { skipped }) => {
            let _ = event_tx.send_overflow(skipped).await;
            product_event_forwarding_open = false;
        }
        Err(_) => {
            product_event_forwarding_open = false;
        }
    }
}
```

Replace each post-operation drain with:

```rust
while let Ok(Some(event)) = receiver.try_recv() {
    if event_tx.send_event(event).await.is_err() {
        break;
    }
}
```

Update the prompt-module test fixtures that seed `CodingRunningPrompt.events` so they use `RpcProductEventQueue::for_tests(...)` and `try_send_event(...)` instead of `mpsc::unbounded_channel()`. For example, replace a seeded fixture with:

```rust
let (event_tx, event_rx) = RpcProductEventQueue::for_tests(4);
event_tx.try_send_event(pending).unwrap();
drop(event_tx);
```

The source guard in Task 8 scans the whole `prompt.rs`, so no `mpsc::unbounded_channel` call should remain in that file after this task.

- [ ] **Step 5: Emit explicit RPC overflow recovery responses**

In `protocol/rpc.rs`, import:

```rust
use event_queue::RpcQueuedProductEvent;
```

In `protocol/rpc.rs`, change the loop event type:

```rust
CodingEvent(Option<RpcQueuedProductEvent>),
```

Add this match arm before the existing product-event arm:

```rust
RpcLoopEvent::CodingEvent(Some(RpcQueuedProductEvent::Overflow { skipped })) => {
    write_rpc_response(
        writer,
        RpcResponse::error_with_data(
            None,
            "event_stream",
            format!(
                "event stream lagged by {skipped} events; client must request a fresh UI snapshot"
            ),
            serde_json::json!({
                "code": "event_stream_lag",
                "skipped": skipped,
                "recovery": "fresh_snapshot"
            }),
        ),
    )
    .await?;
    if let Some(RunningPrompt::Coding(running)) = state.running.as_mut() {
        running.events_closed = true;
    }
}
RpcLoopEvent::CodingEvent(Some(RpcQueuedProductEvent::Event(event))) => {
    state.write_product_event(event, writer).await?;
}
```

Keep the `None` arm that sets `events_closed = true`.

In `finish_coding_running_prompt()` in `protocol/rpc/prompt.rs`, update the completion drain to match the queued item type instead of treating every received item as a `ProductEvent`:

```rust
while let Ok(item) = running.events.try_recv() {
    match item {
        RpcQueuedProductEvent::Event(event) => {
            let pushed = push_live_product_event(&mut running, &event);
            if pushed.accepted {
                self.observe_product_event_submission_for_kind(&event, Some(operation_kind));
            }
            for protocol_event in pushed.protocol_events {
                write_json_line(writer, &protocol_event).await?;
            }
        }
        RpcQueuedProductEvent::Overflow { skipped } => {
            write_rpc_response(
                writer,
                RpcResponse::error_with_data(
                    None,
                    "event_stream",
                    format!(
                        "event stream lagged by {skipped} events; client must request a fresh UI snapshot"
                    ),
                    serde_json::json!({
                        "code": "event_stream_lag",
                        "skipped": skipped,
                        "recovery": "fresh_snapshot"
                    }),
                ),
            )
            .await?;
            running.events_closed = true;
            break;
        }
    }
}
```

- [ ] **Step 6: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent rpc_product_event_queue --lib
cargo test -p pi-coding-agent rpc_reconnect --lib
cargo test -p pi-coding-agent --test rpc_mode prompt
```

Expected: bounded queue unit tests pass, existing reconnect tests pass, and RPC prompt behavior remains compatible.

- [ ] **Step 7: Commit**

```bash
git add crates/pi-coding-agent/src/protocol/rpc.rs crates/pi-coding-agent/src/protocol/rpc/event_queue.rs crates/pi-coding-agent/src/protocol/rpc/prompt.rs crates/pi-coding-agent/src/protocol/rpc/state.rs
git commit -m "feat: bound RPC product event forwarding"
```

## Task 3: RPC Protocol Family Negotiation

**Files:**
- Create: `crates/pi-coding-agent/src/protocol/version.rs`
- Modify: `crates/pi-coding-agent/src/protocol/mod.rs`
- Modify: `crates/pi-coding-agent/src/protocol/types.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/wire.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/commands.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/state.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/stats.rs`
- Modify: `crates/pi-coding-agent/tests/rpc_mode.rs`

- [ ] **Step 1: Write failing RPC negotiation tests**

Add these tests to `tests/rpc_mode.rs`:

```rust
#[tokio::test]
async fn rpc_hello_negotiates_supported_protocol_families() {
    let input = b"{\"id\":\"h1\",\"type\":\"hello\",\"protocol\":{\"family\":\"rpc\",\"major\":1,\"minor\":0}}\n";
    let mut output = Vec::new();

    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model("pi-coding-rpc-hello")),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["command"], "hello");
    assert_eq!(lines[0]["success"], true);
    assert_eq!(lines[0]["data"]["protocol"]["family"], "rpc");
    assert_eq!(lines[0]["data"]["protocol"]["major"], 1);
    assert_eq!(lines[0]["data"]["productEvents"]["family"], "product_event");
    assert_eq!(lines[0]["data"]["uiSnapshot"]["family"], "ui_snapshot");
}

#[tokio::test]
async fn rpc_hello_rejects_unsupported_major_protocol_version() {
    let input = b"{\"id\":\"h1\",\"type\":\"hello\",\"protocol\":{\"family\":\"rpc\",\"major\":99,\"minor\":0}}\n";
    let mut output = Vec::new();

    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model("pi-coding-rpc-hello-reject")),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["command"], "hello");
    assert_eq!(lines[0]["success"], false);
    assert_eq!(lines[0]["data"]["code"], "unsupported_protocol_version");
    assert_eq!(lines[0]["data"]["supported"]["major"], 1);
}

#[tokio::test]
async fn rpc_hello_records_negotiated_protocol_state() {
    let input = b"{\"id\":\"h1\",\"type\":\"hello\",\"protocol\":{\"family\":\"rpc\",\"major\":1,\"minor\":0}}\n{\"id\":\"s1\",\"type\":\"get_state\"}\n";
    let mut output = Vec::new();

    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model("pi-coding-rpc-hello-state")),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["command"], "hello");
    assert_eq!(lines[0]["success"], true);
    assert_eq!(lines[1]["command"], "get_state");
    assert_eq!(lines[1]["data"]["negotiatedProtocol"]["rpc"]["family"], "rpc");
    assert_eq!(lines[1]["data"]["negotiatedProtocol"]["rpc"]["major"], 1);
}
```

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent --test rpc_mode rpc_hello_negotiates_supported_protocol_families
cargo test -p pi-coding-agent --test rpc_mode rpc_hello_rejects_unsupported_major_protocol_version
cargo test -p pi-coding-agent --test rpc_mode rpc_hello_records_negotiated_protocol_state
```

Expected: fail because `hello` is unsupported and `RpcSessionState.negotiatedProtocol` does not exist.

- [ ] **Step 3: Add protocol version types**

Create `protocol/version.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProtocolFamilyVersion {
    pub family: &'static str,
    pub major: u32,
    pub minor: u32,
}

impl ProtocolFamilyVersion {
    pub const fn new(family: &'static str, major: u32, minor: u32) -> Self {
        Self {
            family,
            major,
            minor,
        }
    }

    pub fn is_compatible_with(self, requested: &RequestedProtocolVersion) -> bool {
        self.family == requested.family && self.major == requested.major && requested.minor <= self.minor
    }
}

impl fmt::Display for ProtocolFamilyVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {}.{}", self.family, self.major, self.minor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RequestedProtocolVersion {
    pub family: String,
    pub major: u32,
    pub minor: u32,
}

impl fmt::Display for RequestedProtocolVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {}.{}", self.family, self.major, self.minor)
    }
}

pub const RPC_PROTOCOL_VERSION: ProtocolFamilyVersion =
    ProtocolFamilyVersion::new("rpc", 1, 0);
pub const PRODUCT_EVENT_PROTOCOL_VERSION: ProtocolFamilyVersion =
    ProtocolFamilyVersion::new("product_event", 1, 0);
pub const UI_SNAPSHOT_PROTOCOL_VERSION: ProtocolFamilyVersion =
    ProtocolFamilyVersion::new("ui_snapshot", 1, 0);
```

Declare it in `protocol/mod.rs`:

```rust
pub mod version;
```

- [ ] **Step 4: Add the `hello` command and response payload**

In `protocol/types.rs`, import the version types:

```rust
use crate::protocol::version::{ProtocolFamilyVersion, RequestedProtocolVersion};
```

Add this command variant before `Prompt`:

```rust
#[serde(rename = "hello")]
Hello {
    id: Option<String>,
    protocol: RequestedProtocolVersion,
},
```

Add this response payload:

```rust
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RpcHelloResponse {
    pub protocol: ProtocolFamilyVersion,
    #[serde(rename = "productEvents")]
    pub product_events: ProtocolFamilyVersion,
    #[serde(rename = "uiSnapshot")]
    pub ui_snapshot: ProtocolFamilyVersion,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RpcNegotiatedProtocolState {
    pub rpc: Option<ProtocolFamilyVersion>,
    #[serde(rename = "productEvents")]
    pub product_events: ProtocolFamilyVersion,
    #[serde(rename = "uiSnapshot")]
    pub ui_snapshot: ProtocolFamilyVersion,
}
```

Add `"hello"` to `is_supported_m5_command()` in `protocol/rpc/wire.rs`.

- [ ] **Step 5: Store and project negotiated protocol state**

In `state.rs`, import the version constants and the RPC wire state:

```rust
use crate::protocol::types::RpcNegotiatedProtocolState;
use crate::protocol::version::{PRODUCT_EVENT_PROTOCOL_VERSION, UI_SNAPSHOT_PROTOCOL_VERSION};
```

Add this field to `RpcState`:

```rust
pub(super) negotiated_protocol: RpcNegotiatedProtocolState,
```

Initialize it in `RpcState::new()`:

```rust
negotiated_protocol: RpcNegotiatedProtocolState {
    rpc: None,
    product_events: PRODUCT_EVENT_PROTOCOL_VERSION,
    ui_snapshot: UI_SNAPSHOT_PROTOCOL_VERSION,
},
```

In `types.rs`, add this field to `RpcSessionState` after `capability_generation`:

```rust
#[serde(rename = "negotiatedProtocol")]
pub negotiated_protocol: RpcNegotiatedProtocolState,
```

In `protocol/rpc/stats.rs`, project the field from `RpcState`:

```rust
negotiated_protocol: self.negotiated_protocol.clone(),
```

This state is optional for Stage 7 compatibility. Legacy clients that never send `hello` remain accepted and see `"rpc": null`; clients that do send `hello` get a stable state echo through `get_state`.

- [ ] **Step 6: Handle negotiation in RPC commands**

In `commands.rs`, import:

```rust
use crate::protocol::version::{
    PRODUCT_EVENT_PROTOCOL_VERSION, RPC_PROTOCOL_VERSION, UI_SNAPSHOT_PROTOCOL_VERSION,
};
use crate::protocol::types::RpcHelloResponse;
```

Add this match arm before `RpcCommand::Prompt`:

```rust
RpcCommand::Hello { id, protocol } => {
    if !RPC_PROTOCOL_VERSION.is_compatible_with(&protocol) {
        write_rpc_response(
            writer,
            RpcResponse::error_with_data(
                id,
                "hello",
                format!(
                    "unsupported protocol version for rpc: requested {protocol}, supported {RPC_PROTOCOL_VERSION}"
                ),
                serde_json::json!({
                    "code": "unsupported_protocol_version",
                    "requested": {
                        "family": protocol.family,
                        "major": protocol.major,
                        "minor": protocol.minor
                    },
                    "supported": {
                        "family": RPC_PROTOCOL_VERSION.family,
                        "major": RPC_PROTOCOL_VERSION.major,
                        "minor": RPC_PROTOCOL_VERSION.minor
                    }
                }),
            ),
        )
        .await?;
        return Ok(());
    }
    self.negotiated_protocol.rpc = Some(RPC_PROTOCOL_VERSION);
    write_rpc_response(
        writer,
        RpcResponse::success(
            id,
            "hello",
            Some(
                serde_json::to_value(RpcHelloResponse {
                    protocol: RPC_PROTOCOL_VERSION,
                    product_events: PRODUCT_EVENT_PROTOCOL_VERSION,
                    ui_snapshot: UI_SNAPSHOT_PROTOCOL_VERSION,
                })
                .expect("hello response serializes"),
            ),
        ),
    )
    .await
}
```

- [ ] **Step 7: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent --test rpc_mode rpc_hello_negotiates_supported_protocol_families
cargo test -p pi-coding-agent --test rpc_mode rpc_hello_rejects_unsupported_major_protocol_version
cargo test -p pi-coding-agent --test rpc_mode rpc_hello_records_negotiated_protocol_state
cargo test -p pi-coding-agent --test rpc_mode unsupported
```

Expected: hello negotiation passes, `get_state` reports negotiated state after hello, legacy unsupported-command behavior remains unchanged, and non-hello clients remain compatible.

- [ ] **Step 8: Commit**

```bash
git add crates/pi-coding-agent/src/protocol/mod.rs crates/pi-coding-agent/src/protocol/version.rs crates/pi-coding-agent/src/protocol/types.rs crates/pi-coding-agent/src/protocol/rpc/wire.rs crates/pi-coding-agent/src/protocol/rpc/commands.rs crates/pi-coding-agent/src/protocol/rpc/state.rs crates/pi-coding-agent/src/protocol/rpc/stats.rs crates/pi-coding-agent/tests/rpc_mode.rs
git commit -m "feat: negotiate RPC protocol families"
```

## Task 4: Versioned UI Snapshot Rebuild Policy

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/client_projection.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/src/protocol/types.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/stats.rs`

- [ ] **Step 1: Write failing snapshot version tests**

Add this import to the `#[cfg(test)] mod tests` in `client_projection.rs`:

```rust
use crate::protocol::version::UI_SNAPSHOT_PROTOCOL_VERSION;
```

Add this test to `client_projection.rs`:

```rust
#[test]
fn ui_snapshot_carries_projection_version() {
    let snapshot = UiSnapshot::new(
        UiSnapshotCursor {
            last_event_sequence: ProductEventSequence::new(7),
            capability_generation: CapabilityGeneration::new(3),
        },
        UI_SNAPSHOT_PROTOCOL_VERSION,
        CodingAgentSessionView {
            session_id: "sess_version".into(),
            default_agent_profile_id: ProfileId::from("default"),
        },
        capabilities(),
        None,
        Vec::new(),
    );

    assert_eq!(snapshot.version.family, "ui_snapshot");
    assert_eq!(snapshot.version.major, 1);
    assert_eq!(snapshot.version.minor, 0);
    assert_eq!(snapshot.version, UI_SNAPSHOT_PROTOCOL_VERSION);
}
```

Add this assertion to the existing RPC state snapshot test in `protocol/rpc/stats.rs`:

```rust
assert_eq!(state.snapshot_version.family, "ui_snapshot");
assert_eq!(state.snapshot_version.major, 1);
assert_eq!(state.snapshot_version.minor, 0);
```

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent ui_snapshot_carries_projection_version --lib
cargo test -p pi-coding-agent rpc_state --lib
```

Expected: fail because `UiSnapshot.version`, `UI_SNAPSHOT_PROTOCOL_VERSION` imports in `client_projection.rs`, and `RpcSessionState.snapshotVersion` do not exist yet.

- [ ] **Step 3: Add snapshot version to the projection model**

In `client_projection.rs`, import the shared protocol-version type and constant from Task 3:

```rust
use crate::protocol::version::{ProtocolFamilyVersion, UI_SNAPSHOT_PROTOCOL_VERSION};
```

Add the field to `UiSnapshot`:

```rust
pub(crate) version: ProtocolFamilyVersion,
```

Change `UiSnapshot::new()` to accept `version: ProtocolFamilyVersion` immediately after `cursor` and store it. Update every existing `UiSnapshot::new()` call in tests and production to pass `UI_SNAPSHOT_PROTOCOL_VERSION`.

In `CodingAgentSession::ui_snapshot()`, pass:

```rust
UI_SNAPSHOT_PROTOCOL_VERSION,
```

- [ ] **Step 4: Add snapshot version to RPC state**

Add this field to `RpcSessionState` after `capability_generation`:

```rust
#[serde(rename = "snapshotVersion")]
pub snapshot_version: ProtocolFamilyVersion,
```

In `protocol/rpc/stats.rs`, project it from the session snapshot:

```rust
snapshot_version: snapshot.version,
```

- [ ] **Step 5: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent ui_snapshot_carries_projection_version --lib
cargo test -p pi-coding-agent rpc_state --lib
cargo test -p pi-coding-agent --test rpc_mode get_state
```

Expected: snapshot version and RPC state tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/client_projection.rs crates/pi-coding-agent/src/coding_session/mod.rs crates/pi-coding-agent/src/protocol/types.rs crates/pi-coding-agent/src/protocol/rpc/stats.rs
git commit -m "feat: version UI snapshot projection"
```

## Task 5: Startup Recovery Marker Application

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/session_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/replay.rs`

- [ ] **Step 1: Write failing startup recovery tests**

Add these tests to the `#[cfg(test)] mod tests` in `session_service.rs`:

```rust
#[test]
fn open_marks_in_doubt_operations_recovered() {
    let temp = tempfile::tempdir().unwrap();
    let store = SessionLogStore::new(temp.path());
    let handle = store
        .create_session(CreateSessionOptions::new(
            "sess_recover_open",
            "2026-07-09T00:00:00Z",
        ))
        .unwrap();
    let started = SessionEventEnvelope::new(
        "sess_recover_open",
        "evt_started",
        "2026-07-09T00:00:01Z",
        SessionEventData::OperationStarted {
            operation: crate::coding_session::session_log::event::OperationKind::Prompt,
            runtime_generation: Default::default(),
        },
    )
    .with_operation_id("op_in_doubt");
    store.append_events(&handle, &[started]).unwrap();

    let options = CodingAgentSessionOptions::new()
        .with_session_id("sess_recover_open")
        .with_session_log_root(temp.path());
    let service = SessionService::open(&options).unwrap();

    let replay = service.replay().unwrap();
    assert_eq!(
        replay.operation_status("op_in_doubt"),
        Some(OperationReplayStatus::Recovered)
    );
}

#[test]
fn startup_recovery_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let store = SessionLogStore::new(temp.path());
    let handle = store
        .create_session(CreateSessionOptions::new(
            "sess_recover_once",
            "2026-07-09T00:00:00Z",
        ))
        .unwrap();
    let started = SessionEventEnvelope::new(
        "sess_recover_once",
        "evt_started",
        "2026-07-09T00:00:01Z",
        SessionEventData::OperationStarted {
            operation: crate::coding_session::session_log::event::OperationKind::Prompt,
            runtime_generation: Default::default(),
        },
    )
    .with_operation_id("op_recover_once");
    store.append_events(&handle, &[started]).unwrap();

    let options = CodingAgentSessionOptions::new()
        .with_session_id("sess_recover_once")
        .with_session_log_root(temp.path());
    let _first = SessionService::open(&options).unwrap();
    let _second = SessionService::open(&options).unwrap();

    let reopened = SessionLogStore::new(temp.path())
        .open_session_id("sess_recover_once")
        .unwrap();
    let events = SessionLogStore::new(temp.path())
        .read_events(&reopened)
        .unwrap();
    let recovered_count = events
        .iter()
        .filter(|event| {
            event.operation_id.as_deref() == Some("op_recover_once")
                && matches!(event.data, SessionEventData::OperationRecovered { .. })
        })
        .count();

    assert_eq!(recovered_count, 1);
}
```

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent open_marks_in_doubt_operations_recovered --lib
cargo test -p pi-coding-agent startup_recovery_is_idempotent --lib
```

Expected: first test fails because open only exposes the in-doubt summary; second fails because no recovery marker is written.

- [ ] **Step 3: Add startup recovery marker application**

In `session_service.rs`, add a small record for markers written during this open call:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StartupRecoveryMarker {
    pub(crate) operation_id: String,
    pub(crate) recovery_id: String,
    pub(crate) reason: String,
}
```

Add a field to `SessionService`:

```rust
startup_recovery_markers: Vec<StartupRecoveryMarker>,
```

Add a constructor helper and use it anywhere the file currently builds `Self { store, handle }`:

```rust
fn from_handle(store: SessionLogStore, handle: SessionHandle) -> Self {
    Self {
        store,
        handle,
        startup_recovery_markers: Vec::new(),
    }
}
```

In `session_service.rs`, change `open()` to build a mutable service and apply recovery:

```rust
let mut service = Self::from_handle(store, handle);
service.apply_startup_recovery()?;
Ok(service)
```

Do the same in `open_or_create()` when `try_open_session_id()` returns an existing handle:

```rust
let mut service = Self::from_handle(store, handle);
service.apply_startup_recovery()?;
return Ok(service);
```

Add this method to `impl SessionService`:

```rust
fn apply_startup_recovery(&mut self) -> Result<(), CodingSessionError> {
    let replay = self.replay()?;
    let in_doubt_operations = replay.recovery_summary().in_doubt_operations;
    if in_doubt_operations.is_empty() {
        return Ok(());
    }

    let session_id = self.session_id().to_owned();
    let mut ids = SystemIdGenerator;
    let clock = SystemClock;
    let recovered_at = clock.now_rfc3339();
    let reason = "startup recovery marked incomplete operation in-doubt".to_owned();
    let markers = in_doubt_operations
        .into_iter()
        .map(|operation_id| {
            let recovery_id = ids.next_operation_id();
            StartupRecoveryMarker {
                operation_id,
                recovery_id,
                reason: reason.clone(),
            }
        })
        .collect::<Vec<_>>();
    let events = markers
        .iter()
        .map(|marker| {
            SessionEventEnvelope::new(
                session_id.clone(),
                ids.next_event_id(),
                recovered_at.clone(),
                SessionEventData::OperationRecovered {
                    reason: marker.reason.clone(),
                    recovery_id: marker.recovery_id.clone(),
                },
            )
            .with_operation_id(marker.operation_id.clone())
        })
        .collect::<Vec<_>>();

    self.store.append_events(&self.handle, &events)?;
    self.store
        .update_manifest(&self.handle, ManifestPatch::new().updated_at(recovered_at))?;
    self.handle = self.store.open_session_id(&session_id)?;
    self.startup_recovery_markers.extend(markers);
    Ok(())
}
```

Add this accessor so the session owner can project the markers written by this open call:

```rust
pub(crate) fn take_startup_recovery_markers(&mut self) -> Vec<StartupRecoveryMarker> {
    std::mem::take(&mut self.startup_recovery_markers)
}
```

This is idempotent because after the first append, `replay().recovery_summary().in_doubt_operations` no longer includes recovered operations. This intentionally makes every `SessionService::open()` path self-healing, including `hydrate()`, `tree_view()`, clone/fork, and export. If a future caller needs a strictly read-only open, add a separate `open_read_only()` path instead of bypassing `apply_startup_recovery()` in adapters.

- [ ] **Step 4: Add replay-focused recovery marker coverage**

Add this test to `session_log/replay.rs`:

```rust
#[test]
fn operation_recovered_marker_finalizes_in_doubt_operation() {
    let replay = fold_events(&[
        event(
            "evt_started",
            Some("op_recovered"),
            Some("turn_1"),
            SessionEventData::OperationStarted {
                operation: OperationKind::Prompt,
                runtime_generation: Default::default(),
            },
        ),
        event(
            "evt_recovered",
            Some("op_recovered"),
            Some("turn_1"),
            SessionEventData::OperationRecovered {
                reason: "startup recovery marked incomplete operation in-doubt".into(),
                recovery_id: "op_recovery_1".into(),
            },
        ),
    ]);

    assert_eq!(
        replay.operation_status("op_recovered"),
        Some(OperationReplayStatus::Recovered)
    );
    assert!(replay.recovery_summary().in_doubt_operations.is_empty());
}
```

- [ ] **Step 5: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent open_marks_in_doubt_operations_recovered --lib
cargo test -p pi-coding-agent startup_recovery_is_idempotent --lib
cargo test -p pi-coding-agent operation_recovered_marker_finalizes_in_doubt_operation --lib
```

Expected: all three tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/session_service.rs crates/pi-coding-agent/src/coding_session/session_log/replay.rs
git commit -m "feat: recover in-doubt operations on session open"
```

## Task 6: Recovery Product Events And Adapter Projection

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/event.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/event_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/src/protocol/events.rs`
- Modify: `crates/pi-coding-agent/src/protocol/types.rs`
- Modify: `crates/pi-coding-agent/src/interactive/event_bridge.rs`

- [ ] **Step 1: Write failing recovery projection tests**

Add this test to `event_service.rs`:

```rust
#[test]
fn recovery_markers_publish_terminal_product_events() {
    let service = EventService::new();
    let event = service.emit_operation_recovered(
        "op_recovered",
        "recovery_1",
        "startup recovery marked incomplete operation in-doubt",
    );

    assert_eq!(event.operation_id(), Some("op_recovered"));
    assert_eq!(
        event.terminal_status(),
        Some(ProductEventTerminalStatus::Recovered)
    );
    assert_eq!(event.family(), ProductEventFamily::Workflow);
}
```

Add this test to `protocol/events.rs`:

```rust
#[test]
fn protocol_adapter_maps_operation_recovered_to_recovery_event() {
    let mut adapter = CodingProtocolEventAdapter::new_with_provider(
        "faux".into(),
        "faux-provider".into(),
        "faux-model".into(),
    );
    let product_event = ProductEvent::from_compat_event(
        ProductEventSequence::new(1),
        CodingAgentEvent::OperationRecovered {
            operation_id: "op_recovered".into(),
            recovery_id: "recovery_1".into(),
            reason: "startup recovery marked incomplete operation in-doubt".into(),
        },
    );

    let events = adapter.push_product_event(&product_event);

    assert!(matches!(
        &events[0],
        ProtocolEvent::OperationRecovered {
            operation_id,
            recovery_id,
            reason,
        } if operation_id == "op_recovered"
            && recovery_id == "recovery_1"
            && reason.contains("startup recovery")
    ));
}
```

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent recovery_markers_publish_terminal_product_events --lib
cargo test -p pi-coding-agent protocol_adapter_maps_operation_recovered_to_recovery_event --lib
```

Expected: fail because recovery product events and protocol projection do not exist.

- [ ] **Step 3: Add recovered terminal status and compatibility event**

In `event.rs`, add `Recovered` to `ProductEventTerminalStatus`:

```rust
Recovered,
```

Add this `CodingAgentEvent` variant:

```rust
OperationRecovered {
    operation_id: String,
    recovery_id: String,
    reason: String,
},
```

Update `CodingAgentEvent::operation_id()` so `OperationRecovered` participates in operation correlation:

```rust
Self::OperationRecovered { operation_id, .. } => Some(operation_id.as_str()),
```

Update `CodingAgentEvent::terminal_status()` so recovered operations become terminal product events:

```rust
Self::OperationRecovered { .. } => Some(ProductEventTerminalStatus::Recovered),
```

Update `ProductEventKind::from_compat_event()` to classify it as a workflow event:

```rust
CodingAgentEvent::OperationRecovered { .. } => {
    ProductEventKind::Workflow(WorkflowProductEventKind::OperationRecovered)
}
```

Add `OperationRecovered` to `WorkflowProductEventKind`.

Update `ProductEventDurability::from_compat_event()` so recovery notifications are live-only projection events:

```rust
CodingAgentEvent::OperationRecovered { .. } => Self::LiveOnly,
```

- [ ] **Step 4: Add event service emission helper**

In `event_service.rs`, add:

```rust
pub(crate) fn emit_operation_recovered(
    &self,
    operation_id: impl Into<String>,
    recovery_id: impl Into<String>,
    reason: impl Into<String>,
) -> ProductEvent {
    self.emit(CodingAgentEvent::OperationRecovered {
        operation_id: operation_id.into(),
        recovery_id: recovery_id.into(),
        reason: reason.into(),
    })
}
```

In `CodingAgentSession::from_services()`, take the markers recorded by Task 5 before storing the service in the session:

```rust
let mut session_service = session_service;
let startup_recovery_markers = session_service.take_startup_recovery_markers();
```

After constructing the `CodingAgentSession` with its `EventService`, emit one live product event per marker:

```rust
for marker in startup_recovery_markers {
    session.event_service.emit_operation_recovered(
        marker.operation_id,
        marker.recovery_id,
        marker.reason,
    );
}
```

These product events are live projection notifications for the current adapter; the durable recovery fact remains the `operation.recovered` session event written by `SessionService`. Do not let RPC or interactive adapters append durable recovery markers directly.

- [ ] **Step 5: Add protocol projection**

In `protocol/types.rs`, add:

```rust
#[serde(rename = "operation_recovered")]
OperationRecovered {
    #[serde(rename = "operationId")]
    operation_id: String,
    #[serde(rename = "recoveryId")]
    recovery_id: String,
    reason: String,
},
```

In `protocol/events.rs`, map the product event:

```rust
CodingAgentEvent::OperationRecovered {
    operation_id,
    recovery_id,
    reason,
} => vec![ProtocolEvent::OperationRecovered {
    operation_id: operation_id.clone(),
    recovery_id: recovery_id.clone(),
    reason: reason.clone(),
}],
```

In `interactive/event_bridge.rs`, map the product event to the existing system notice UI event:

```rust
CodingAgentEvent::OperationRecovered {
    operation_id,
    reason,
    ..
} => vec![UiEvent::SystemNotice {
    text: format!("Recovered incomplete operation {operation_id}: {reason}"),
}),
```

- [ ] **Step 6: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent recovery_markers_publish_terminal_product_events --lib
cargo test -p pi-coding-agent protocol_adapter_maps_operation_recovered_to_recovery_event --lib
cargo test -p pi-coding-agent interactive --lib
```

Expected: recovery product events map through protocol and interactive projection.

- [ ] **Step 7: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/event.rs crates/pi-coding-agent/src/coding_session/event_service.rs crates/pi-coding-agent/src/coding_session/mod.rs crates/pi-coding-agent/src/protocol/events.rs crates/pi-coding-agent/src/protocol/types.rs crates/pi-coding-agent/src/interactive/event_bridge.rs
git commit -m "feat: project startup recovery events"
```

## Task 7: Structured Idempotency Keys For RPC Root Operations

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/src/protocol/types.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/state.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/commands.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/prompt.rs`
- Modify: `crates/pi-coding-agent/tests/rpc_mode.rs`

- [ ] **Step 1: Write failing idempotency model tests**

Add these tests to `operation.rs`:

```rust
#[test]
fn idempotency_key_accepts_stable_client_keys() {
    let key = OperationIdempotencyKey::parse("client-123_prompt.retry_1").unwrap();

    assert_eq!(key.as_str(), "client-123_prompt.retry_1");
}

#[test]
fn idempotency_key_rejects_empty_or_oversized_values() {
    assert!(OperationIdempotencyKey::parse("").is_err());
    assert!(OperationIdempotencyKey::parse("x".repeat(129)).is_err());
    assert!(OperationIdempotencyKey::parse("contains space").is_err());
}
```

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent idempotency_key --lib
```

Expected: fail because `OperationIdempotencyKey` does not exist.

- [ ] **Step 3: Add key validation**

In `operation.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct OperationIdempotencyKey(String);

impl OperationIdempotencyKey {
    const MAX_LEN: usize = 128;

    pub(crate) fn parse(value: impl Into<String>) -> Result<Self, crate::coding_session::CodingSessionError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= Self::MAX_LEN
            && value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'));
        if !valid {
            return Err(crate::coding_session::CodingSessionError::Input {
                message: "idempotency key must be 1-128 ASCII letters, digits, '-', '_', '.', or ':'".into(),
            });
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}
```

In `coding_session/mod.rs`, expose the key to RPC internals:

```rust
pub(crate) use operation::OperationIdempotencyKey;
```

- [ ] **Step 4: Add optional RPC idempotency keys**

In `protocol/types.rs`, add this field to root mutating commands that can be retried by clients:

```rust
#[serde(rename = "idempotencyKey", skip_serializing_if = "Option::is_none")]
idempotency_key: Option<String>,
```

Add the field to these variants:

```rust
Prompt
SelfHealingEdit
InvokeAgent
InvokeTeam
ApproveDelegation
RejectDelegation
SetDefaultAgentProfile
```

Do not add it to pure query commands in this task. Do not add it to `Compact` while the current RPC compact branch only returns "manual compaction is not available in Rust M5"; add a compact idempotency key later with the actual compact operation implementation.

- [ ] **Step 5: Store bounded RPC idempotency records**

In `state.rs`, import `VecDeque`, `HashMap`, and `OperationIdempotencyKey`:

```rust
use std::collections::{HashMap, VecDeque};
use crate::coding_session::OperationIdempotencyKey;
```

Add:

```rust
const RPC_IDEMPOTENCY_RECORD_LIMIT: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RpcIdempotencyRecord {
    pub(super) command: &'static str,
    pub(super) operation_kind: OperationKind,
    pub(super) completed: bool,
}
```

Add the active key to `CodingRunningPrompt` so completion updates only the operation that just finished:

```rust
pub(super) idempotency_key: Option<OperationIdempotencyKey>,
```

Add fields to `RpcState`:

```rust
pub(super) idempotency_records: HashMap<OperationIdempotencyKey, RpcIdempotencyRecord>,
pub(super) idempotency_order: VecDeque<OperationIdempotencyKey>,
```

Initialize both as empty in `RpcState::new()`.

Add methods:

```rust
pub(super) fn parse_idempotency_key(
    &self,
    key: Option<String>,
) -> Result<Option<OperationIdempotencyKey>, CliError> {
    key.map(OperationIdempotencyKey::parse)
        .transpose()
        .map_err(CliError::from)
}

pub(super) fn idempotent_retry_response(
    &self,
    key: Option<&OperationIdempotencyKey>,
    command: &'static str,
) -> Result<Option<serde_json::Value>, CliError> {
    let key = key?;
    let Some(record) = self.idempotency_records.get(key) else {
        return Ok(None);
    };
    if record.command == command {
        return Ok(Some(serde_json::json!({
            "deduplicated": true,
            "operation": record.operation_kind.as_str(),
            "completed": record.completed
        })));
    }
    Err(CliError::SessionFailure(format!(
        "idempotency key was already used for {}, not {command}",
        record.command
    )))
}

pub(super) fn remember_idempotency_key(
    &mut self,
    key: Option<OperationIdempotencyKey>,
    command: &'static str,
    operation_kind: OperationKind,
) {
    let Some(key) = key else {
        return;
    };
    if !self.idempotency_records.contains_key(&key) {
        self.idempotency_order.push_back(key.clone());
    }
    self.idempotency_records.insert(
        key,
        RpcIdempotencyRecord {
            command,
            operation_kind,
            completed: false,
        },
    );
    while self.idempotency_order.len() > RPC_IDEMPOTENCY_RECORD_LIMIT {
        if let Some(expired) = self.idempotency_order.pop_front() {
            self.idempotency_records.remove(&expired);
        }
    }
}

pub(super) fn mark_idempotency_complete(&mut self, key: Option<&OperationIdempotencyKey>) {
    let Some(key) = key else {
        return;
    };
    if let Some(record) = self.idempotency_records.get_mut(key) {
        record.completed = true;
    }
}
```

- [ ] **Step 6: Apply idempotency to RPC prompt retries**

In `commands.rs`, pass `idempotency_key` from `RpcCommand::Prompt` into `handle_prompt()`.

In `prompt.rs`, add a parameter:

```rust
idempotency_key: Option<String>,
```

At the beginning of non-image prompt handling:

```rust
let idempotency_key = self.parse_idempotency_key(idempotency_key)?;
if let Some(data) = self.idempotent_retry_response(idempotency_key.as_ref(), "prompt")? {
    write_rpc_response(writer, RpcResponse::success(id, "prompt", Some(data))).await?;
    return Ok(());
}
```

Clone the key before moving it into `remember_idempotency_key()` so the running state can keep it:

```rust
let running_idempotency_key = idempotency_key.clone();
self.remember_idempotency_key(idempotency_key, "prompt", OperationKind::Prompt);
```

When constructing `CodingRunningPrompt`, store:

```rust
idempotency_key: running_idempotency_key,
```

In `finish_coding_running_prompt()`, after taking `running` and before returning, call:

```rust
self.mark_idempotency_complete(running.idempotency_key.as_ref());
```

Apply the same key parsing and retry-response logic to the remaining mutating commands, but use the command-specific completion point:

```text
invoke_agent: remember before spawning, store key in CodingRunningPrompt, complete in finish_coding_running_prompt().
invoke_team: remember before spawning, store key in CodingRunningPrompt, complete in finish_coding_running_prompt().
approve_delegation: determine OperationKind from the pending target, remember before spawning, store key in CodingRunningPrompt, complete in finish_coding_running_prompt().
self_healing_edit: remember after request validation and before calling self_healing_edit_with_options(); mark the same key complete after writing the success or structured error response.
reject_delegation: remember after the target pending confirmation is found; mark the same key complete after the response is written.
set_default_agent_profile: remember after ProfileId validation; mark the same key complete after the response is written.
```

Do not store idempotency records for validation failures before an operation has been accepted. A retry with the same key after fixing the invalid input should be treated as a fresh accepted operation.

- [ ] **Step 7: Add RPC retry coverage**

Add this test to `tests/rpc_mode.rs`:

```rust
#[tokio::test]
async fn rpc_prompt_idempotency_key_deduplicates_running_retry() {
    let api = "pi-coding-rpc-idempotent-prompt";
    let release = Arc::new(Notify::new());
    let opened = Arc::new(AtomicBool::new(false));
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(PausingProvider {
            release: Arc::clone(&release),
            opened: Arc::clone(&opened),
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(4096);
    let (output_writer, output_reader) = tokio::io::duplex(4096);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });
    let mut lines = tokio::io::BufReader::new(output_reader).lines();

    input_writer
        .write_all(
            b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\",\"idempotencyKey\":\"retry:prompt:1\"}\n",
        )
        .await
        .unwrap();
    let first = read_rpc_json_line(&mut lines, "initial prompt response").await;
    assert_eq!(first["success"], true);

    input_writer
        .write_all(
            b"{\"id\":\"p2\",\"type\":\"prompt\",\"message\":\"hello\",\"idempotencyKey\":\"retry:prompt:1\"}\n",
        )
        .await
        .unwrap();
    let retry = read_rpc_json_matching(&mut lines, "idempotent prompt retry", |value| {
        value["type"] == "response" && value["command"] == "prompt" && value["id"] == "p2"
    })
    .await;

    assert_eq!(retry["success"], true);
    assert_eq!(retry["data"]["deduplicated"], true);
    assert_eq!(retry["data"]["operation"], "prompt");

    release.notify_one();
    drop(input_writer);
    await_rpc_task_completion(task, &release, "idempotent prompt rpc task").await;
}
```

- [ ] **Step 8: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent idempotency_key --lib
cargo test -p pi-coding-agent --test rpc_mode rpc_prompt_idempotency_key_deduplicates_running_retry
cargo test -p pi-coding-agent --test rpc_mode prompt
```

Expected: idempotency model and retry behavior pass without changing existing prompt behavior.

- [ ] **Step 9: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/operation.rs crates/pi-coding-agent/src/coding_session/mod.rs crates/pi-coding-agent/src/protocol/types.rs crates/pi-coding-agent/src/protocol/rpc/state.rs crates/pi-coding-agent/src/protocol/rpc/commands.rs crates/pi-coding-agent/src/protocol/rpc/prompt.rs crates/pi-coding-agent/tests/rpc_mode.rs
git commit -m "feat: add RPC operation idempotency keys"
```

## Task 8: Stage 7 Boundary Guards

**Files:**
- Modify: `crates/pi-coding-agent/tests/event_boundary_guards.rs`
- Modify: `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs`

- [ ] **Step 1: Write failing guard tests**

Add these tests to `product_runtime_boundary_guards.rs`:

```rust
#[test]
fn rpc_running_product_events_do_not_use_unbounded_channels() {
    let prompt_rs = std::fs::read_to_string(
        workspace_path("crates/pi-coding-agent/src/protocol/rpc/prompt.rs"),
    )
    .expect("read rpc prompt source");

    assert!(
        !prompt_rs.contains("mpsc::unbounded_channel"),
        "RPC running product-event forwarding must use bounded queues"
    );
    assert!(
        prompt_rs.contains("RpcProductEventQueue::new()"),
        "RPC prompt forwarding should route through RpcProductEventQueue"
    );
    assert!(
        prompt_rs.contains("RpcQueuedProductEvent::Overflow"),
        "RPC completion drains must handle queued overflow recovery items"
    );
}

#[test]
fn event_receiver_lag_maps_to_snapshot_recovery_error() {
    let event_service_rs = std::fs::read_to_string(
        workspace_path("crates/pi-coding-agent/src/coding_session/event_service.rs"),
    )
    .expect("read event service source");

    assert!(
        event_service_rs.contains("CodingSessionError::EventStreamLag"),
        "broadcast lag must map to event_stream_lag so clients know to request a fresh snapshot"
    );
    assert!(
        !event_service_rs.contains("event receiver lagged by {skipped} events"),
        "lag should not remain a generic resource error"
    );
}
```

Add these tests to `event_boundary_guards.rs`:

```rust
#[test]
fn rpc_protocol_exposes_optional_version_negotiation_state() {
    let types_rs = std::fs::read_to_string(
        workspace_path("crates/pi-coding-agent/src/protocol/types.rs"),
    )
    .expect("read protocol types");
    let commands_rs = std::fs::read_to_string(
        workspace_path("crates/pi-coding-agent/src/protocol/rpc/commands.rs"),
    )
    .expect("read rpc commands");
    let state_rs = std::fs::read_to_string(
        workspace_path("crates/pi-coding-agent/src/protocol/rpc/state.rs"),
    )
    .expect("read rpc state");
    let stats_rs = std::fs::read_to_string(
        workspace_path("crates/pi-coding-agent/src/protocol/rpc/stats.rs"),
    )
    .expect("read rpc stats");

    assert!(types_rs.contains("Hello {"));
    assert!(commands_rs.contains("RPC_PROTOCOL_VERSION.is_compatible_with"));
    assert!(state_rs.contains("negotiated_protocol"));
    assert!(stats_rs.contains("negotiated_protocol"));
}

#[test]
fn startup_recovery_stays_session_service_owned() {
    let session_service_rs = std::fs::read_to_string(
        workspace_path("crates/pi-coding-agent/src/coding_session/session_service.rs"),
    )
    .expect("read session service source");
    let rpc_sources = [
        workspace_path("crates/pi-coding-agent/src/protocol/rpc/commands.rs"),
        workspace_path("crates/pi-coding-agent/src/protocol/rpc/prompt.rs"),
        workspace_path("crates/pi-coding-agent/src/interactive/event_bridge.rs"),
    ];

    assert!(session_service_rs.contains("apply_startup_recovery"));
    assert!(session_service_rs.contains("take_startup_recovery_markers"));
    for source in rpc_sources {
        let text = std::fs::read_to_string(&source).expect("read adapter source");
        assert!(
            !text.contains("OperationRecovered {"),
            "adapters must project recovery events but not write recovery session markers: {}",
            source.display()
        );
    }
}
```

- [ ] **Step 2: Run RED guards**

Run:

```bash
cargo test -p pi-coding-agent --test product_runtime_boundary_guards rpc_running_product_events_do_not_use_unbounded_channels
cargo test -p pi-coding-agent --test product_runtime_boundary_guards event_receiver_lag_maps_to_snapshot_recovery_error
cargo test -p pi-coding-agent --test event_boundary_guards rpc_protocol_exposes_optional_version_negotiation_state
cargo test -p pi-coding-agent --test event_boundary_guards startup_recovery_stays_session_service_owned
```

Expected: guards fail until Tasks 1-7 are implemented.

- [ ] **Step 3: Keep guards narrow and source-based**

If a guard fails for a false positive, update the searched file list or searched string only. Keep these invariants unchanged:

```text
RPC running product-event forwarding cannot use unbounded mpsc channels.
Broadcast lag must map to event_stream_lag/fresh snapshot recovery.
RPC protocol compatibility metadata is exposed through optional hello/version negotiation.
Startup recovery marker writes stay owned by SessionService.
Adapters can project recovery events but cannot write durable recovery markers.
```

- [ ] **Step 4: Run GREEN guards**

Run:

```bash
cargo test -p pi-coding-agent --test product_runtime_boundary_guards rpc_running_product_events_do_not_use_unbounded_channels
cargo test -p pi-coding-agent --test product_runtime_boundary_guards event_receiver_lag_maps_to_snapshot_recovery_error
cargo test -p pi-coding-agent --test event_boundary_guards rpc_protocol_exposes_optional_version_negotiation_state
cargo test -p pi-coding-agent --test event_boundary_guards startup_recovery_stays_session_service_owned
```

Expected: all four guards pass.

- [ ] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/tests/event_boundary_guards.rs crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs
git commit -m "test: guard runtime recovery boundaries"
```

## Task 9: Stage 7 Verification And Closure

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-09-operation-runtime-stage-7-backpressure-versioning-recovery-plan.md`

- [ ] **Step 1: Run full Stage 7 verification**

Run:

```bash
cargo fmt --check
cargo test -p pi-coding-agent event_service_reports_bounded_product_event_window --lib
cargo test -p pi-coding-agent product_event_receiver_lag_reports_snapshot_recovery --lib
cargo test -p pi-coding-agent rpc_product_event_queue --lib
cargo test -p pi-coding-agent --test rpc_mode rpc_hello_negotiates_supported_protocol_families
cargo test -p pi-coding-agent --test rpc_mode rpc_hello_rejects_unsupported_major_protocol_version
cargo test -p pi-coding-agent --test rpc_mode rpc_hello_records_negotiated_protocol_state
cargo test -p pi-coding-agent ui_snapshot_carries_projection_version --lib
cargo test -p pi-coding-agent rpc_state --lib
cargo test -p pi-coding-agent open_marks_in_doubt_operations_recovered --lib
cargo test -p pi-coding-agent startup_recovery_is_idempotent --lib
cargo test -p pi-coding-agent operation_recovered_marker_finalizes_in_doubt_operation --lib
cargo test -p pi-coding-agent recovery_markers_publish_terminal_product_events --lib
cargo test -p pi-coding-agent protocol_adapter_maps_operation_recovered_to_recovery_event --lib
cargo test -p pi-coding-agent idempotency_key --lib
cargo test -p pi-coding-agent --test rpc_mode rpc_prompt_idempotency_key_deduplicates_running_retry
cargo test -p pi-coding-agent --test rpc_mode get_state
cargo test -p pi-coding-agent --test rpc_mode prompt
cargo test -p pi-coding-agent --test event_boundary_guards
cargo test -p pi-coding-agent --test product_runtime_boundary_guards
cargo check -p pi-coding-agent
git diff --check
```

Expected: every command exits with code 0.

- [ ] **Step 2: Update this plan's verification checklist**

After the commands pass, mark these checkboxes:

```markdown
- [x] `cargo fmt --check`
- [x] `cargo test -p pi-coding-agent event_service_reports_bounded_product_event_window --lib`
- [x] `cargo test -p pi-coding-agent product_event_receiver_lag_reports_snapshot_recovery --lib`
- [x] `cargo test -p pi-coding-agent rpc_product_event_queue --lib`
- [x] `cargo test -p pi-coding-agent --test rpc_mode rpc_hello_negotiates_supported_protocol_families`
- [x] `cargo test -p pi-coding-agent --test rpc_mode rpc_hello_rejects_unsupported_major_protocol_version`
- [x] `cargo test -p pi-coding-agent --test rpc_mode rpc_hello_records_negotiated_protocol_state`
- [x] `cargo test -p pi-coding-agent ui_snapshot_carries_projection_version --lib`
- [x] `cargo test -p pi-coding-agent rpc_state --lib`
- [x] `cargo test -p pi-coding-agent open_marks_in_doubt_operations_recovered --lib`
- [x] `cargo test -p pi-coding-agent startup_recovery_is_idempotent --lib`
- [x] `cargo test -p pi-coding-agent operation_recovered_marker_finalizes_in_doubt_operation --lib`
- [x] `cargo test -p pi-coding-agent recovery_markers_publish_terminal_product_events --lib`
- [x] `cargo test -p pi-coding-agent protocol_adapter_maps_operation_recovered_to_recovery_event --lib`
- [x] `cargo test -p pi-coding-agent idempotency_key --lib`
- [x] `cargo test -p pi-coding-agent --test rpc_mode rpc_prompt_idempotency_key_deduplicates_running_retry`
- [x] `cargo test -p pi-coding-agent --test rpc_mode get_state`
- [x] `cargo test -p pi-coding-agent --test rpc_mode prompt`
- [x] `cargo test -p pi-coding-agent --test event_boundary_guards`
- [x] `cargo test -p pi-coding-agent --test product_runtime_boundary_guards`
- [x] `cargo check -p pi-coding-agent`
- [x] `git diff --check`
```

- [ ] **Step 3: Update the project checklist**

Update the active operation-runtime item in `docs/TODO.md` so the Stage 7 portion says:

```markdown
Stage 7 backpressure/versioning/recovery hardening is complete: product-event publication has explicit bounded retention and lag recovery semantics, RPC running event forwarding is bounded, clients can negotiate and observe RPC/product-event/UI-snapshot protocol families through optional `hello`, UI snapshots carry projection versions, startup recovery writes durable recovered markers for in-doubt operations, recovery events project through RPC/interactive adapters, and RPC root operations accept structured idempotency keys for retry deduplication.
```

Add a progress log entry:

```markdown
- 2026-07-09: Stage 7 backpressure/versioning/recovery hardening completed. Product-event lag now instructs clients to request a fresh `UiSnapshot`, RPC forwarding no longer uses an unbounded product-event queue, protocol families can be negotiated and observed through optional `hello`, UI snapshots carry a versioned rebuild contract, session open applies durable recovery markers for in-doubt operations, recovery projects through product events, and retried RPC root operations can use idempotency keys.
```

- [ ] **Step 4: Commit closure documentation**

```bash
git add docs/TODO.md docs/superpowers/plans/2026-07-09-operation-runtime-stage-7-backpressure-versioning-recovery-plan.md
git commit -m "docs: close runtime hardening stage"
```

## Verification Checklist

- [x] `cargo fmt --check`
- [x] `cargo test -p pi-coding-agent event_service_reports_bounded_product_event_window --lib`
- [x] `cargo test -p pi-coding-agent product_event_receiver_lag_reports_snapshot_recovery --lib`
- [x] `cargo test -p pi-coding-agent rpc_product_event_queue --lib`
- [x] `cargo test -p pi-coding-agent --test rpc_mode rpc_hello_negotiates_supported_protocol_families`
- [x] `cargo test -p pi-coding-agent --test rpc_mode rpc_hello_rejects_unsupported_major_protocol_version`
- [x] `cargo test -p pi-coding-agent --test rpc_mode rpc_hello_records_negotiated_protocol_state`
- [x] `cargo test -p pi-coding-agent ui_snapshot_carries_projection_version --lib`
- [x] `cargo test -p pi-coding-agent rpc_state --lib`
- [x] `cargo test -p pi-coding-agent open_marks_in_doubt_operations_recovered --lib`
- [x] `cargo test -p pi-coding-agent startup_recovery_is_idempotent --lib`
- [x] `cargo test -p pi-coding-agent operation_recovered_marker_finalizes_in_doubt_operation --lib`
- [x] `cargo test -p pi-coding-agent recovery_markers_publish_terminal_product_events --lib`
- [x] `cargo test -p pi-coding-agent protocol_adapter_maps_operation_recovered_to_recovery_event --lib`
- [x] `cargo test -p pi-coding-agent idempotency_key --lib`
- [x] `cargo test -p pi-coding-agent --test rpc_mode rpc_prompt_idempotency_key_deduplicates_running_retry`
- [x] `cargo test -p pi-coding-agent --test rpc_mode get_state`
- [x] `cargo test -p pi-coding-agent --test rpc_mode prompt`
- [x] `cargo test -p pi-coding-agent --test event_boundary_guards`
- [x] `cargo test -p pi-coding-agent --test product_runtime_boundary_guards`
- [x] `cargo check -p pi-coding-agent`
- [x] `git diff --check`

## Spec Coverage

- Bounded queues and explicit overflow policy: Tasks 1, 2, and 8.
- Slow subscriber disconnect/reconnect semantics: Tasks 1 and 2.
- Protocol family negotiation: Task 3.
- Startup recovery scan: Task 5.
- In-doubt commit handling: Tasks 5 and 6.
- Structured retry and idempotency keys: Task 7.
- Snapshot/version rebuild policy: Task 4.
- Storage pressure becomes operation failure/recovery instead of memory growth: Tasks 1, 2, 5, and 8.
- Unsupported protocol clients fail clearly: Task 3.
- Restart after interrupted operation produces coherent recovered state: Tasks 5 and 6.
- Terminal events and recovery markers are not displaced by live deltas: Tasks 1, 2, 6, and 8.

## Execution Notes

- Keep Stage 7 focused on production tolerance. Do not narrow public facade methods or delete compatibility APIs here; Stage 8 owns that work.
- Keep recovery marker writes in `SessionService`. Adapters project recovery product events but never append durable recovery markers.
- Keep `hello` negotiation optional in Stage 7. Supported clients can discover and record protocol-family compatibility, but existing clients that never send `hello` must continue to work unless a future public protocol version explicitly makes negotiation mandatory.
- Keep lag recovery client-driven: after `event_stream_lag` or `event_stream_gap`, the client requests a fresh `UiSnapshot` and then resumes from retained events if available.
- Keep idempotency bounded in memory for this stage. Durable cross-process idempotency can be added later only if a specific embedding client needs restart-stable retry records.
