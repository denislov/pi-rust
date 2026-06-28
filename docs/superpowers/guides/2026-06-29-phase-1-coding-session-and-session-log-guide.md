# Phase 1 Guide: CodingAgentSession Skeleton and Rust-Native Session Log

## Phase Goal

Create the new product runtime anchor without changing prompt execution behavior yet.

Phase 1 establishes:

- `CodingAgentSession` as the future stable product owner.
- `CodingAgentEvent` as the product event boundary.
- A Rust-native typed session event log.
- `SessionService` and `TurnTransaction` as the only path to canonical session writes.

This phase must not route print/json/RPC/interactive prompts through the new runtime yet. That happens in Phase 2 and Phase 3.

## Non-Negotiable Constraints

- Do not rewrite `pi-agent-core/src/agent_loop.rs`.
- Do not change provider request/stream behavior.
- Do not preserve TypeScript session JSONL compatibility.
- Do not implement TypeScript session import/export.
- Do not remove the existing `JsonlSessionStorage` path.
- Do not expose internal services through the stable facade.
- Do not make `SessionEventData` a generic `serde_json::Value` model.

## Target Module Layout

Add this internal module tree:

```text
crates/pi-coding-agent/src/coding_session/
  mod.rs
  capability_service.rs
  context.rs
  error.rs
  event.rs
  event_service.rs
  flow_service.rs
  plugin_service.rs
  runtime_service.rs
  session_service.rs
  session_log/
    mod.rs
    event.rs
    id.rs
    manifest.rs
    replay.rs
    store.rs
    transaction.rs
```

Update `crates/pi-coding-agent/src/lib.rs`:

```rust
mod coding_session;

pub mod api {
    pub use crate::coding_session::{CodingAgentEvent, CodingAgentSession};
    // keep existing api exports during migration
}
```

Keep `coding_session` private or migration-private. The stable surface starts in `api`.

## Public API Surface

Phase 1 should expose only:

```rust
pub struct CodingAgentSession;
pub enum CodingAgentEvent;
pub enum CodingSessionError;
pub struct CodingAgentSessionOptions;
pub struct CodingAgentSessionView;
pub struct CodingAgentCapabilities;
```

The exact option/view field names can evolve during implementation, but the external concept should be stable:

- open/create a product session runtime;
- subscribe to product events;
- inspect current session view;
- inspect capabilities.

Do not expose:

- `SessionService`;
- `RuntimeService`;
- `FlowService`;
- `EventService`;
- `PromptTurnContext`;
- `TurnTransaction`;
- `SessionLogStore`;
- concrete flow nodes.

## Product Error Boundary

Add `CodingSessionError` in `coding_session/error.rs`.

Recommended shape:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodingSessionError {
    Config { message: String },
    Auth { message: String },
    Input { message: String },
    Resource { message: String },
    Session { message: String },
    Provider { message: String },
    Tool { message: String },
    Flow { message: String },
    Plugin { message: String },
    Cancelled,
    UnsupportedCapability { capability: String },
    Busy { operation: String },
}
```

Implement:

- `Display`;
- `Error`;
- `code() -> &'static str`;
- conversions from internal session log errors;
- optional conversion to `CliError` for adapter integration.

Do not replace `CliError` globally in this phase.

## CodingAgentEvent

Add `CodingAgentEvent` in `coding_session/event.rs`.

Phase 1 minimum:

```rust
pub enum CodingAgentEvent {
    SessionOpened { session_id: String },
    SessionWritePending { operation_id: String },
    SessionWriteCommitted { operation_id: String, session_id: String },
    SessionWriteSkipped { operation_id: String, reason: String },
    PromptStarted { operation_id: String, turn_id: String },
    PromptCompleted { operation_id: String, turn_id: String },
    PromptFailed { operation_id: String, error: CodingSessionError },
    PromptAborted { operation_id: String, reason: String },
    Diagnostic { operation_id: Option<String>, message: String },
    CapabilityChanged,
}
```

Phase 2 will add agent/message/tool variants.

Use explicit string IDs initially. Strong ID newtypes can be introduced if they reduce mistakes without bloating call sites.

## EventService

Phase 1 `EventService` should provide:

```rust
pub(crate) struct EventService { ... }

impl EventService {
    pub(crate) fn new() -> Self;
    pub(crate) fn emit(&self, event: CodingAgentEvent);
    pub(crate) fn subscribe(&self) -> EventReceiver;
}
```

Implementation options:

- `tokio::sync::broadcast` if multiple subscribers are needed immediately.
- `mpsc` if a single subscriber is sufficient.
- In-memory capture helper for tests.

Prefer a simple implementation. Backpressure policy can be refined when RPC/TUI move to product events.

## Session Log Layout

Canonical runtime format:

```text
<session_dir>/
  session.json
  events.jsonl
  blobs/
  index/
```

`events.jsonl` is canonical. `index/` is rebuildable.

The old `pi-agent-core::session::JsonlSessionStorage` remains available for existing paths until adapters migrate.

## Manifest

Add `SessionManifest`:

```rust
pub struct SessionManifest {
    pub schema: String,
    pub version: u32,
    pub session_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub active_branch_id: Option<String>,
    pub active_leaf_id: Option<String>,
    pub event_log: String,
}
```

Constants:

```rust
pub const SESSION_SCHEMA: &str = "pi-rust.session";
pub const SESSION_VERSION: u32 = 1;
pub const EVENT_SCHEMA: &str = "pi-rust.session.event";
pub const EVENT_VERSION: u32 = 1;
```

Use stable relative paths inside the manifest, not absolute paths.

## Session Event Envelope

Add `SessionEventEnvelope`:

```rust
pub struct SessionEventEnvelope {
    pub schema: String,
    pub version: u32,
    pub session_id: String,
    pub event_id: String,
    pub operation_id: Option<String>,
    pub turn_id: Option<String>,
    pub branch_id: Option<String>,
    pub leaf_id: Option<String>,
    pub parent_event_id: Option<String>,
    pub created_at: String,
    pub data: SessionEventData,
}
```

Use serde tagging:

```rust
#[serde(tag = "kind", content = "data")]
pub enum SessionEventData { ... }
```

Expected JSON shape:

```json
{
  "schema": "pi-rust.session.event",
  "version": 1,
  "session_id": "sess_1",
  "event_id": "evt_1",
  "operation_id": "op_1",
  "turn_id": "turn_1",
  "created_at": "2026-06-29T00:00:00Z",
  "kind": "turn.started",
  "data": {}
}
```

If serde tagging cannot produce the exact top-level `kind` shape cleanly, prefer a small custom wrapper over storing arbitrary maps.

## Minimum SessionEventData Variants

Phase 1 variants:

```rust
pub enum SessionEventData {
    SessionCreated { cwd: Option<String> },
    OperationStarted { operation: OperationKind },
    OperationCommitted { new_leaf_id: Option<String> },
    OperationAborted { reason: String },
    OperationFailed { error_code: String, message: String },
    TurnStarted,
    TurnInputRecorded { content: Vec<PersistedContentBlock> },
    MessageStarted { message_id: String, role: PersistedRole },
    MessageDelta { message_id: String, text: String },
    MessageCompleted { message_id: String, finish_reason: Option<String> },
    MessageCancelled { message_id: String, reason: String },
    ToolCallStarted { tool_call_id: String, name: String },
    ToolCallUpdated { tool_call_id: String, message: String },
    ToolCallCompleted { tool_call_id: String, result: PersistedToolResult },
    ToolCallFailed { tool_call_id: String, message: String },
    ToolCallCancelled { tool_call_id: String, reason: String },
    DiagnosticEmitted { level: DiagnosticLevel, message: String },
    MetadataUpdated { key: String, value: serde_json::Value },
    ActiveLeafChanged { leaf_id: String },
}
```

`serde_json::Value` is acceptable for metadata values and provider/tool details. It is not acceptable as the primary event model.

## ID Generation

Add an ID helper in `session_log/id.rs`:

```rust
pub(crate) trait IdGenerator {
    fn next_session_id(&mut self) -> String;
    fn next_event_id(&mut self) -> String;
    fn next_operation_id(&mut self) -> String;
    fn next_turn_id(&mut self) -> String;
    fn next_message_id(&mut self) -> String;
    fn next_tool_call_id(&mut self) -> String;
    fn next_leaf_id(&mut self) -> String;
}
```

Implement:

- production generator based on existing ID helpers or UUID-like strings;
- deterministic test generator.

Avoid using wall-clock time in assertions.

## Clock Boundary

Add a small clock trait if timestamps are required:

```rust
pub(crate) trait Clock {
    fn now_rfc3339(&self) -> String;
}
```

Use deterministic clock in tests.

## SessionLogStore

`SessionLogStore` should own filesystem I/O:

```rust
pub(crate) struct SessionLogStore {
    root: PathBuf,
}

impl SessionLogStore {
    pub(crate) fn create_session(&self, options: CreateSessionOptions) -> Result<SessionHandle, CodingSessionError>;
    pub(crate) fn open_session(&self, path: &Path) -> Result<SessionHandle, CodingSessionError>;
    pub(crate) fn append_events(&self, handle: &SessionHandle, events: &[SessionEventEnvelope]) -> Result<(), CodingSessionError>;
    pub(crate) fn read_events(&self, handle: &SessionHandle) -> Result<Vec<SessionEventEnvelope>, CodingSessionError>;
    pub(crate) fn update_manifest(&self, handle: &SessionHandle, patch: ManifestPatch) -> Result<(), CodingSessionError>;
}
```

Use temp dirs in tests. Do not write outside configured root.

## Transaction Finalization

Add `TurnTransaction` in `session_log/transaction.rs` or `session_service.rs`.

Transaction states:

```rust
enum TransactionState {
    Open,
    Committed,
    Aborted,
    Failed,
}
```

Minimum methods:

```rust
record_user_input(...)
start_assistant_message(...)
append_assistant_delta(...)
complete_assistant_message(...)
cancel_assistant_message(...)
record_tool_started(...)
record_tool_completed(...)
emit_diagnostic(...)
commit(...)
abort(...)
fail(...)
```

Rules:

- Once finalized, further mutation returns an error.
- `commit` appends pending events and `operation.committed`.
- `abort` appends lifecycle cancellation events where needed and `operation.aborted`.
- `fail` appends diagnostic/failure events and `operation.failed`.
- Manifest active leaf updates only after committed operation final marker is appended.

Phase 1 can keep crash recovery simple, but replay must treat operations without final markers as incomplete.

## Replay

`session_log/replay.rs` should fold events into:

```rust
pub struct SessionReplay {
    pub session_id: String,
    pub active_leaf_id: Option<String>,
    pub transcript: Vec<TranscriptItem>,
    pub diagnostics: Vec<ReplayDiagnostic>,
}
```

Minimum transcript items:

```rust
UserInput { turn_id, text }
AssistantMessage { message_id, text, status }
ToolCall { tool_call_id, name, status, summary }
Diagnostic { level, message }
```

Replay must:

- preserve event order;
- ignore or mark incomplete operations without final markers;
- close cancelled messages as cancelled, not completed;
- not require old TS session entries.

## CodingAgentSession Skeleton

Phase 1 owner:

```rust
pub struct CodingAgentSession {
    session_service: SessionService,
    runtime_service: RuntimeService,
    flow_service: FlowService,
    event_service: EventService,
    capability_service: CapabilityService,
    plugin_service: PluginService,
}
```

Public methods:

```rust
impl CodingAgentSession {
    pub async fn create(options: CodingAgentSessionOptions) -> Result<Self, CodingSessionError>;
    pub async fn open(options: CodingAgentSessionOptions) -> Result<Self, CodingSessionError>;
    pub fn subscribe(&self) -> CodingAgentEventReceiver;
    pub fn capabilities(&self) -> CodingAgentCapabilities;
    pub fn view(&self) -> CodingAgentSessionView;
}
```

Do not add `prompt()` until Phase 2 unless it returns `UnsupportedCapability`.

## Test Files

Recommended tests:

```text
crates/pi-coding-agent/tests/coding_session_public_api.rs
crates/pi-coding-agent/tests/session_event_log.rs
crates/pi-coding-agent/tests/session_event_replay.rs
crates/pi-coding-agent/tests/session_transaction.rs
```

Coverage:

- API facade imports.
- Create/open Rust-native session.
- Manifest round trip.
- Event JSON round trip.
- Transaction commit/abort/fail.
- Replay transcript from events.
- No old TS session reader involved.

## Phase 1 Handoff to Phase 2

Phase 1 must leave these stable internal outputs:

- `CodingAgentSession` owner can be constructed.
- `EventService` can emit and subscribe to `CodingAgentEvent`.
- `SessionService` can create/open a Rust-native session.
- `TurnTransaction` can commit/abort/fail typed session events.
- Replay can fold event log into transcript.
- `RuntimeService` and `FlowService` exist, even if mostly skeletal.

Phase 2 depends on these names and responsibilities.

## Stop Conditions

Stop and redesign before continuing if:

- `CodingAgentSession` exposes mutable service references publicly.
- `SessionEventData` becomes mostly dynamic JSON.
- transaction finalization updates active leaf before append succeeds.
- old `JsonlSessionStorage` is removed before adapters migrate.
- timestamps make tests nondeterministic.

## Suggested Checks

Run focused checks first:

```text
cargo fmt --check
cargo test -p pi-coding-agent coding_session
cargo test -p pi-coding-agent session_event
cargo test -p pi-coding-agent public_api
cargo check -p pi-coding-agent
```

Before merging Phase 1:

```text
cargo test -p pi-coding-agent
cargo check --workspace
```
