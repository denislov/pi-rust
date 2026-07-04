# Phase 3 Guide: CLI, RPC, and Interactive Adapter Convergence

> Historical status, 2026-07-05: Phase 3 is complete. CLI, JSON, RPC, and interactive primary paths consume `CodingAgentSession`/`CodingAgentEvent`; old JSONL product paths are rejected instead of maintained as compatibility paths. This guide is retained as implementation history.

## Phase Goal

Move frontends onto `CodingAgentSession` so CLI, JSON, RPC, and interactive mode consume the same product runtime and `CodingAgentEvent` stream.

Phase 3 is where the old runner starts shrinking into compatibility wrappers.

## Preconditions

Phase 1 complete:

- Rust-native session event log exists.
- `CodingAgentSession` exists.
- transaction finalization works.

Phase 2 complete:

- `CodingAgentSession::prompt()` works for print/headless.
- JSON mode can be driven from `CodingAgentEvent`.
- `PromptTurnFlow` calls existing `Agent::run()` through `RunAgentTurn`.

## Non-Negotiable Constraints

- Do not rewrite `agent_loop.rs`.
- Do not make RPC or TUI depend on concrete Flow node IDs.
- Do not let RPC/interactive write session files directly for migrated operations.
- Do not expose internal services publicly just to ease adapter migration.
- Do not delete old runner until all adapter paths are migrated.

## Adapter Ownership Model

After Phase 3:

```text
CLI print/json
  -> CodingAgentSession

RPC
  -> CodingAgentSession
  -> CapabilityService
  -> CodingAgentEvent adapter

Interactive
  -> CodingAgentSession
  -> CapabilityService
  -> CodingAgentEvent -> UiEvent adapter
```

Adapters should translate user input and render output. They should not own core session mutation policy.

## CapabilityService Contract

Add concrete capability model:

```rust
pub struct CodingAgentCapabilities {
    pub prompt: CapabilityStatus,
    pub abort: CapabilityStatus,
    pub steer: CapabilityStatus,
    pub follow_up: CapabilityStatus,
    pub compact: CapabilityStatus,
    pub fork: CapabilityStatus,
    pub clone_session: CapabilityStatus,
    pub switch_session: CapabilityStatus,
    pub export: CapabilityStatus,
    pub tools: CapabilityStatus,
    pub shell: CapabilityStatus,
    pub plugins: CapabilityStatus,
}

pub enum CapabilityStatus {
    Available,
    Disabled { reason: String },
    Unsupported { reason: String },
    Busy { operation: String },
}
```

RPC and TUI should prefer this over ad hoc "not supported in Rust M5" strings.

## RPC Migration

Current anchors:

```text
crates/pi-coding-agent/src/protocol/rpc.rs
crates/pi-coding-agent/src/protocol/rpc/commands.rs
crates/pi-coding-agent/src/protocol/rpc/prompt.rs
crates/pi-coding-agent/src/protocol/rpc/state.rs
crates/pi-coding-agent/src/protocol/rpc/wire.rs
crates/pi-coding-agent/src/protocol/types.rs
```

### RPC State

Current RPC state owns:

- running prompt handle;
- `AgentEvent` receiver;
- mode flags;
- session metadata.

Move toward:

```rust
pub(crate) struct RpcSessionState {
    session: CodingAgentSession,
    running: Option<RunningPrompt>,
    event_adapter: RpcEventAdapter,
}
```

`RunningPrompt` should track:

- operation ID;
- cancellation handle;
- product event receiver;
- done handle/outcome.

### RPC Prompt Command

Current `prompt.rs` uses `spawn_session_prompt(SessionPromptOptions)`.

Migration:

1. Convert RPC prompt command to `PromptTurnOptions`.
2. Call `CodingAgentSession::prompt()` in a task.
3. Stream `CodingAgentEvent` through RPC event adapter.
4. Store cancellation handle from prompt operation.
5. Return final response from `PromptTurnOutcome`.

Do not stream raw `AgentEvent` in the migrated prompt path.

### RPC Event Adapter

Add:

```rust
pub(crate) struct RpcCodingEventAdapter { ... }

impl RpcCodingEventAdapter {
    pub(crate) fn push(&mut self, event: &CodingAgentEvent) -> Vec<ProtocolEvent>;
}
```

Initially this can reuse logic from `protocol/events.rs`, but the input should become product events.

Mapping examples:

```text
AssistantMessageDelta -> transcript/message delta protocol event
ToolCallStarted -> tool execution start protocol event
ToolCallCompleted -> tool execution end protocol event
PromptFailed -> error protocol event
SessionWriteCommitted -> session state update
CapabilityChanged -> capability update
```

### RPC Capability Command

Add or extend a command:

```text
get_capabilities
```

If the wire protocol cannot change immediately, include capabilities in existing state response.

Tests:

- RPC reports prompt available when idle.
- RPC reports prompt busy during active prompt.
- unsupported commands reflect capability status.
- prompt command streams product-event-derived protocol events.

## Interactive Migration

Current anchors:

```text
crates/pi-coding-agent/src/interactive/prompt_task.rs
crates/pi-coding-agent/src/interactive/event_bridge.rs
crates/pi-coding-agent/src/interactive/loop.rs
crates/pi-coding-agent/src/interactive/root.rs
crates/pi-coding-agent/src/interactive/session_actions.rs
crates/pi-coding-agent/src/interactive/commands.rs
```

### Prompt Task

Current prompt task uses `spawn_session_prompt()` and receives `AgentEvent`.

Migration:

1. Replace spawned old runner with `CodingAgentSession::prompt()` task.
2. Give task a product event receiver.
3. Bridge `CodingAgentEvent` to `UiEvent`.
4. Keep abort/steer/follow-up behavior through session methods or operation handles.

If steer/follow-up cannot move immediately, leave those commands on old path until `AgentTurnFlow` support is available. Mark capability as unavailable for new path if necessary.

### Interactive Event Bridge

Current `interactive/event_bridge.rs` maps `AgentEvent` to `UiEvent`.

Add a product bridge:

```rust
pub struct CodingEventBridge { ... }

impl CodingEventBridge {
    pub fn handle(&mut self, event: &CodingAgentEvent) -> Vec<UiEvent>;
}
```

Keep the old bridge until all prompt paths migrate.

Mapping examples:

```text
PromptStarted -> UiEvent::TurnStarted
AssistantMessageDelta -> transcript append/update
ToolCallStarted -> tool panel entry
ToolCallCompleted -> tool panel completion
PromptFailed -> error banner
PromptAborted -> cancellation state
SessionWriteCommitted -> active leaf update
CapabilityChanged -> footer/key hint update
```

### Session Actions

Current `interactive/session_actions.rs` reads/writes `JsonlSessionStorage`.

Migration path:

1. Add equivalent methods on `CodingAgentSession`/`SessionService`:
   - new session;
   - switch/open session;
   - fork;
   - clone;
   - name;
   - stats;
   - tree;
   - export Rust-native view if retained.
2. Move interactive actions one by one to these methods.
3. Keep old actions only for old sessions during transition if necessary.

Do not make UI code open `events.jsonl` directly.

## CLI Root Integration

Current anchor:

- `crates/pi-coding-agent/src/lib.rs`

After Phase 2, print/json should already use new runtime. Phase 3 should:

- keep command dispatch simple;
- avoid duplicate prompt resolution paths;
- ensure `ResolvedPromptRequest` conversion to `PromptTurnOptions` is shared by CLI/RPC/interactive.

## Session Format Transition

Old session format remains for old paths until migration completes.

Phase 3 policy:

- New `CodingAgentSession` paths create Rust-native session directories.
- Old runner paths can still create old JSONL files until migrated.
- Do not attempt automatic conversion.
- Once RPC/interactive prompt paths migrate, stop creating old session JSONL from product prompt operations.

Update user-visible session listing carefully:

- either list Rust-native sessions only in migrated UI;
- or label old sessions as legacy/internal and do not open them through new runtime.

## Tests

Recommended RPC tests:

```text
rpc_capabilities.rs
rpc_coding_session_prompt.rs
rpc_coding_events.rs
```

Recommended interactive tests:

```text
interactive_coding_event_bridge.rs
interactive_coding_session_prompt.rs
interactive_rust_session_actions.rs
```

Regression tests to keep:

- existing `rpc_mode.rs`;
- existing `interactive_event_bridge.rs` until old bridge is removed;
- existing `interactive_sessions.rs` updated when Rust-native session actions land.

## Phase 3 Handoff to Phase 4

Phase 3 must leave:

- all primary prompt frontends route through `CodingAgentSession`;
- product events drive JSON/RPC/TUI adapters;
- session commands use `SessionService` where migrated;
- old `session_runner` either unused by primary frontends or clearly limited;
- `RunAgentTurn` remains the only old-agent-loop bridge.

Phase 4 can then focus on `pi-agent-core` without also solving product owner migration.

## Stop Conditions

Stop and redesign if:

- RPC or TUI needs direct `SessionService` mutable access;
- RPC wire starts exposing `FlowEvent`;
- TUI transcript depends on concrete prompt node IDs;
- old and new session roots become indistinguishable in UI;
- capability status cannot represent current unsupported commands cleanly.

## Suggested Checks

Focused:

```text
cargo fmt --check
cargo test -p pi-coding-agent rpc
cargo test -p pi-coding-agent interactive_event_bridge
cargo test -p pi-coding-agent interactive_sessions
cargo test -p pi-coding-agent json_mode
```

Broader:

```text
cargo test -p pi-coding-agent
cargo check --workspace
```
