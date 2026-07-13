# Phase 7: Adapter Migration and Compatibility Deletion - Pattern Map

**Mapped:** 2026-07-13
**Phase:** Adapter Migration and Compatibility Deletion
**Files analyzed:** 18 (16 direct migration targets; 2 conditional fixture/verification targets)
**Analogs found:** 5 strong analog groups / 18 files

The phase-level `06-SUMMARY.md` named by the task does not exist. The three existing
`06-01-SUMMARY.md`, `06-02-SUMMARY.md`, and `06-03-SUMMARY.md` files were read as the
Phase 6 summary evidence. All findings below are from current source plus
`07-RESEARCH.md`, `07-VALIDATION.md`, and `docs/product-event-contract.md`.

## File Classification

| File | Role | Event data flow | Closest analog | Match |
|---|---|---|---|---|
| `src/coding_session/event.rs` | internal event model/transform | event-driven transform; compatibility payload currently stored | `src/coding_session/public_event.rs` typed exhaustive projection | role + transform |
| `src/coding_session/event_service.rs` | event service/pub-sub | publish, retain, replay, broadcast | `src/coding_session/public_projection.rs` receiver projection | exact transport boundary |
| `src/coding_session/public_event.rs` | public model/transform | event-driven transform to owned payload | Phase 6 typed hierarchy itself | exact |
| `src/coding_session/public_projection.rs` | public adapter | streaming receiver -> one typed projection | `event_service.rs:987-1007` typed receiver | exact |
| `src/coding_session/mod.rs` | session facade/controller | request/response subscription and startup-recovery events | `public_api.rs:133-148` stable facade signature checks | role match |
| `src/lib.rs` | public API facade | transform/re-export; no event side effects | `coding_session/mod.rs:74-89` curated re-exports | exact facade |
| `src/protocol/events.rs` | stateful protocol adapter | event-driven transform to ordered `ProtocolEvent` stream | `tests/protocol_events.rs:40-118` | exact role + flow |
| `src/protocol/rpc/events.rs` | RPC adapter wrapper | request-response wrapper over streaming protocol events | `protocol/events.rs:22-38` | exact wrapper |
| `src/protocol/json_mode.rs` | JSON/print adapter entry | streaming product events -> JSONL wire output | `protocol/rpc/events.rs:16-18` | role match |
| `src/interactive/event_bridge.rs` | interactive projection adapter | event-driven transform to UI deltas | `tests/interactive_event_bridge.rs:319-370` | exact role + flow |
| `src/interactive/loop.rs` | interactive controller/test harness | prompt-task stream -> UI projection/state | `interactive/event_bridge.rs:149-158` | role + flow |
| `tests/protocol_events.rs` | protocol behavior test | fixture events -> ordered wire events and payload assertions | `protocol/events.rs:40-118` | exact |
| `tests/interactive_event_bridge.rs` | interactive behavior test | fixture events -> UI/delegation/usage assertions | `interactive/event_bridge.rs:158-475` | exact |
| `tests/event_boundary_guards.rs` | source-audit test | batch source scan / fail-closed guard | existing guard at lines 513-566 | exact guard pattern |
| `tests/public_api.rs` | public contract test | compile-time facade and typed family matching | `public_projection.rs:48-64` | exact API contract |
| `tests/product_event_contract.rs` | integration contract test | live receiver -> typed payload/metadata/Serde | `public_projection.rs:53-64` | exact |
| `src/protocol/rpc/event_queue.rs` | conditional test fixture | bounded queue, ordering, overflow marker | `event_service.rs:130-170` retained replay | verification-only unless constructor changes |
| `src/protocol/rpc.rs` | conditional verification target | queue overflow -> `event_stream_lag`/fresh snapshot response | `src/coding_session/error.rs:51-52` | verification-only |

`src/print_mode.rs`, `tests/print_mode.rs`, and `tests/json_mode.rs` have no direct
`ProductEvent`/compatibility symbol; print/JSON behavior enters through the protocol
adapter and should be regression-tested there rather than given a new event path.

## Pattern Assignments

### 1. Typed event ownership and projection

**Files:** `event.rs`, `public_event.rs`, `public_projection.rs`, `event_service.rs`

**Analog A: `public_event.rs` typed hierarchy and metadata accessors**

The stable target is the family/payload hierarchy, not a second raw-event taxonomy:

```rust
// public_event.rs:451-479
pub enum CodingAgentProductEventKind {
    Session(CodingAgentSessionProductEvent),
    Profile(CodingAgentProfileProductEvent),
    Agent(CodingAgentAgentProductEvent),
    Team(CodingAgentTeamProductEvent),
    Message(CodingAgentMessageProductEvent),
    Tool(CodingAgentToolProductEvent),
    Runtime(CodingAgentRuntimeProductEvent),
    Delegation(CodingAgentDelegationProductEvent),
    Workflow(CodingAgentWorkflowProductEvent),
    Diagnostic(CodingAgentDiagnosticProductEvent),
    Capability(CodingAgentCapabilityProductEvent),
}
```

`family()` is an exhaustive, non-string classification (`public_event.rs:465-480`),
and each payload is owned and Serde-tagged. For example, session fields preserve
operation/session IDs and write durability (`public_event.rs:151-175`):

```rust
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CodingAgentSessionProductEvent {
    Opened { session_id: String },
    WritePending { operation_id: String },
    WriteCommitted { operation_id: String, session_id: String },
    WriteSkipped { operation_id: String, reason: String },
    CompactionCompleted { operation_id: String, turn_id: String,
        summary: String, first_kept_message_id: String, tokens_before: u32 },
}
```

The current compatibility storage is the migration source and must be inverted only
after all consumers move:

```rust
// event.rs:310-339
pub(crate) struct ProductEvent {
    sequence: ProductEventSequence,
    kind: ProductEventKind,
    operation_id: Option<String>,
    terminal_status: Option<ProductEventTerminalStatus>,
    durability: ProductEventDurability,
    compatibility_event: CodingAgentEvent,
}
```

Its five root-terminal mappings currently match `compatibility_event()` (`event.rs:367-384`);
preserve those mappings as typed identity/payload metadata before removing the field.
`ProductEventKind::from_compat_event` is the existing exhaustive classification pattern
(`event.rs:48-70` and continuing through all 45 variants), but it does not own payload
fields and therefore cannot be used as the adapter input by itself.

**Analog B: typed receiver projection**

`public_projection.rs:43-64` keeps transport private and projects exactly once per receive:

```rust
pub struct CodingAgentProductEventReceiver { inner: ProductEventReceiver }

pub async fn recv(&mut self) -> Result<CodingAgentProductEvent, CodingSessionError> {
    self.inner.recv().await.map(CodingAgentProductEvent::from_internal)
}
pub fn try_recv(&mut self) -> Result<Option<CodingAgentProductEvent>, CodingSessionError> {
    self.inner.try_recv().map(|event| event.map(CodingAgentProductEvent::from_internal))
}
```

`public_event.rs:714-736` is the conversion to replace: it currently derives metadata,
then calls `CodingAgentProductEventKind::from(source.compatibility_event())`. The planner
should move the owned typed payload into `ProductEvent` (or a private equivalent) and keep
sequence, operation ID, terminal status/association, and durability independent.

**Analog C: EventService ordering/replay owner**

`event_service.rs:156-185` assigns sequence, retains, and broadcasts the typed product event
before the legacy sender. Keep this order and `ProductEventReceiver` (`event_service.rs:987-1007`)
unchanged while adapters migrate. Delete the legacy `sender` only after all `subscribe()`
callers and raw projections are gone; do not introduce adapter-local counters or queues.

### 2. Protocol, JSON, and RPC adapters

**Files:** `protocol/events.rs`, `protocol/rpc/events.rs`, `protocol/json_mode.rs`,
`tests/protocol_events.rs`, conditional `protocol/rpc/event_queue.rs`/`rpc.rs`.

**Analog D: stateful protocol matcher**

`protocol/events.rs:12-38` owns provider/message/tool state and currently delegates product
events to the raw matcher:

```rust
pub(crate) fn push_product_event(&mut self, event: &ProductEvent) -> Vec<ProtocolEvent> {
    self.push(event.compatibility_event())
}
```

The existing `push` matcher (`protocol/events.rs:40-208`, with delegation and agent/team arms
through ~537) is the behavior to preserve. Its representative state transitions are:

```rust
CodingAgentEvent::AssistantMessageDelta { text, .. } => {
    let (content_index, message) = self.append_assistant_text(text);
    // open assistant if needed, then emit MessageUpdate/TextDelta
}
CodingAgentEvent::PromptCompleted { .. } => {
    let mut events = self.finish_current_turn();
    events.push(ProtocolEvent::AgentEnd { messages: self.messages.clone() });
    events
}
```

Replace the input match with `CodingAgentProductEventKind` family/payload arms and retain
provider updates, JSON argument fallback to `Value::Null`, delegation folded blocks,
compaction reason, prompt failure text, and `AgentEnd` ordering. Do not recreate a
`CodingAgentEvent` merely to call `push`.

`rpc/events.rs:5-18` is intentionally a thin wrapper; it should continue forwarding to the
stateful adapter. Its tests (`rpc/events.rs:38-156`) already assert TurnStart, MessageUpdate,
TurnEnd, AgentEnd, and provider failure text; migrate fixtures to typed product events while
retaining those assertions.

`tests/protocol_events.rs:40-118` is the closest external behavior fixture. It asserts thinking
delta payload content, partial transcript, TurnEnd, and AgentEnd, not only event counts. Extend
the same fixture with `ProductEvent`/typed payload construction and retain delegation, tool,
self-healing, compaction, capability, and provider-error tests listed at lines 223, 271, 345,
654, 750, 795, 830, and 869.

`json_mode.rs:224-237` and RPC prompt/command call sites only consume the adapter output; keep
wire serialization and queue sequencing unchanged. `rpc/event_queue.rs:4-55` and its tests
(`:72-99`) are conditional fixture targets: preserve bounded ordering and explicit Overflow.
`rpc.rs` must continue mapping overflow to `event_stream_lag` and `fresh_snapshot`; no lifecycle
or reconnect behavior belongs in this phase.

### 3. Interactive projection and loop

**Files:** `interactive/event_bridge.rs`, `interactive/loop.rs`,
`tests/interactive_event_bridge.rs`.

**Analog E: UI event bridge**

`event_bridge.rs:149-158` is the migration seam:

```rust
pub(crate) fn push_product_event(&mut self, event: &ProductEvent) -> Vec<UiEvent> {
    self.handle(event.compatibility_event())
}
pub fn handle(&mut self, event: &CodingAgentEvent) -> Vec<UiEvent> { match event { ... } }
```

The `handle` arms (`:158-475`) preserve usage deltas/context tokens, tool argument parsing,
delegation blocks, compaction reset notices, partial error text, self-healing notices, and
recovery notices. Match typed family payloads directly and retain the same `UiEvent` shapes.
The fallback list at `:454-474` is important: event-terminal events with no UI projection must
remain no-ops, and root terminal association must not be inferred from them.

`tests/interactive_event_bridge.rs:319-370` asserts delegation confirmation IDs, target,
task, status, and command summary; the lifecycle test at `:400` folds a complete delegation
into one transcript item. Keep these field assertions and add typed payload setup.

`interactive/loop.rs:2892-2903` currently checks typed behavior through
`event.compatibility_event()` for `PartialCommit`; `:2987-3005` checks failed fork does not
publish `SessionOpened`; `:3042-3051` checks typed profile change. Rewrite these to
`event.event()` family/payload matches while preserving durable operation IDs, recovery,
navigation, and projection cursor behavior. `event_boundary_guards.rs:480-501` already requires
the loop to pass `ProductEvent` through `UiProjection` and reset from `UiSnapshot`; retain this
boundary.

### 4. Facade, API, and source guards

**Files:** `coding_session/mod.rs`, `lib.rs`, `tests/public_api.rs`,
`tests/event_boundary_guards.rs`.

`coding_session/mod.rs:392-408` shows deletion order: `subscribe_product_events_public()`
delegates to the internal typed receiver and emits startup recovery markers; the deprecated
`subscribe()` must be removed only after all callers migrate. `lib.rs:64-86` is the curated
facade; keep typed product-event types and remove any legacy receiver/raw event exposure that
remains after migration.

`tests/public_api.rs:133-148` is the stable method-signature analog: it binds `run`, snapshot,
capability, and typed subscription methods without depending on implementation modules.
`tests/public_api.rs:443-490` exhaustively matches all 11 typed families and binds each payload
type; preserve this compile-time privacy/coverage pattern. `tests/product_event_contract.rs:23-105`
checks monotonic order, pending/committed/skipped durability, terminal association, and exact
Serde identity; `:115-145` runs persistent and non-persistent Prompt operations through the
typed public receiver. These are deletion gates, not disposable compatibility tests.

`tests/event_boundary_guards.rs:513-566` is the source-guard analog. It scans protocol,
interactive, and tests for `.subscribe()`/`CodingAgentEventReceiver`, with only named guard/API
files allowed, and currently accepts deprecated/test-gated methods. Flip it fail-closed after
consumer migration to reject `compatibility_event()`, the legacy sender/receiver/type, and
local deprecated suppressions except explicitly named `cfg(test)` fixtures.

## Deletion Order Dependencies

1. Migrate `protocol/events.rs` and `interactive/event_bridge.rs` matchers to typed payloads;
   keep `ProductEvent`, retained replay, and both receivers compiling.
2. Migrate RPC/JSON entry points and all adapter fixtures (`protocol_events.rs`, RPC adapter
   tests, `interactive_event_bridge.rs`) without changing output ordering or field assertions.
3. Migrate co-located `coding_session/mod.rs`, `event_service.rs`, `event.rs`, and
   `interactive/loop.rs` tests, including startup recovery, partial commit, profile/session
   navigation, durability, and fork behavior.
4. Update `public_api.rs`, `product_event_contract.rs`, and source guards; compile after each
   wave. The new guard must fail on any production compatibility consumer.
5. Remove `CodingAgentSession::subscribe()` and `CodingAgentEventReceiver` export/method, then
   remove `EventService::subscribe()` and its `sender` field/broadcast. Keep
   `ProductEventReceiver` and `subscribe_product_events_public()`.
6. Remove `ProductEvent.compatibility_event`, `compatibility_event()` accessor, and raw
   conversion helpers only after all remaining `from_compat_event` fixtures are either typed
   constructors or explicitly documented `cfg(test)` migration fixtures. Re-run public event
   inventory and contract tests after this final storage deletion.

## Shared Patterns

- **Identity:** match `CodingAgentProductEventKind` and its payload enums; do not parse legacy
  strings or use `Debug` formatting. The authoritative 45-row table is in
  `docs/product-event-contract.md`.
- **Ordering:** `EventService` assigns sequence before retention/broadcast; adapters never
  synthesize sequence values.
- **Errors:** retain typed `CodingSessionError` code/message and existing provider/tool/partial-
  commit formatting; do not collapse payload errors to event counts.
- **Durability and terminals:** preserve `live_only`, `pending_session_write`, and `durable`
  independently from event terminal status and the five current root-operation associations.
- **Testing:** deterministic offline fixtures, exact payload fields, Serde shape, monotonic
  order, bounded overflow recovery, replay/cursor behavior, and source guards are all required.

## No Analog Found

No new architecture is needed. The only intentionally unusual migration is ownership of the
typed payload inside the private `ProductEvent` after raw compatibility storage is removed;
use the Phase 6 public hierarchy and `public_projection.rs` as the design source rather than
inventing a parallel event representation.

## Metadata

**Analog search scope:** `crates/pi-coding-agent/src/coding_session`, `src/protocol`,
`src/interactive`, and `crates/pi-coding-agent/tests`; CodeGraph index plus targeted source
audits.
**Files scanned:** 18 classified targets; additional call sites in RPC prompt/command/state
were verified as consumers/fixtures, not separate architectural patterns.
**Pattern extraction date:** 2026-07-13
