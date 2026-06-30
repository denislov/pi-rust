# Interactive Coding Event Bridge Design

## Purpose

This design defines the Phase 3 adapter boundary that converts `CodingAgentEvent` into interactive `UiEvent` values.

Interactive mode still receives `AgentEvent` from the old `session_runner` path. Before prompt execution moves to `CodingAgentSession::prompt()`, the TUI needs a product-event bridge equivalent to the RPC product-event adapter. This slice adds that bridge without changing prompt task ownership, session actions, abort/steer/follow-up behavior, or compaction execution.

## Scope

In scope:

- add an interactive `CodingEventBridge`;
- map product prompt, assistant, tool, failure, abort, and compaction events into existing `UiEvent` values;
- keep the existing `InteractiveEventBridge` for old `AgentEvent` paths;
- add deterministic bridge tests;
- update Phase 3 TODO status.

Out of scope:

- routing interactive prompt tasks through `CodingAgentSession`;
- changing `PromptTask` event channel types;
- changing interactive session actions;
- adding new UI event variants for session persistence or capability changes;
- implementing abort, steer, follow-up, or manual compaction on the new product runtime path;
- deleting `session_runner`.

## Current State

The existing interactive adapter path is:

```text
interactive loop
  -> spawn_session_prompt()
  -> AgentEvent
  -> InteractiveEventBridge
  -> UiEvent
```

RPC has already started moving to:

```text
RpcState
  -> CodingAgentSession::prompt()
  -> CodingAgentEvent
  -> RpcCodingEventAdapter
  -> ProtocolEvent
```

Interactive needs the same product-event adapter boundary before its prompt task can migrate cleanly.

## Target State

Add a new bridge next to the old one:

```rust
pub struct CodingEventBridge { ... }

impl CodingEventBridge {
    pub fn new() -> Self;
    pub fn handle(&mut self, event: &CodingAgentEvent) -> Vec<UiEvent>;
}
```

The bridge is adapter-local. It does not own session state, inspect flow node ids, open session files, or call `SessionService`.

The old bridge remains unchanged:

```rust
pub struct InteractiveEventBridge { ... }
```

This gives the migration two explicit adapter boundaries:

```text
old prompt path: AgentEvent -> InteractiveEventBridge -> UiEvent
new prompt path: CodingAgentEvent -> CodingEventBridge -> UiEvent
```

## Event Mapping

The first bridge implementation should map only product events that have a stable existing TUI representation.

Assistant events:

- `AssistantMessageDelta` -> `UiEvent::AssistantDelta`;
- `AssistantMessageCompleted` -> `UiEvent::AssistantDone`;
- `AssistantMessageStarted` -> no UI event.

Tool events:

- `ToolCallStarted` -> `UiEvent::ToolStarted`;
- `ToolCallUpdated` -> `UiEvent::ToolUpdated`;
- `ToolCallCompleted` -> `UiEvent::ToolFinished { is_error: false }`;
- `ToolCallFailed` -> `UiEvent::ToolFinished { is_error: true }`.

Runtime and prompt lifecycle events:

- `AgentTurnStarted` -> `UiEvent::TurnStarted`;
- `RuntimeCompactionCompleted` -> `UiEvent::CompactionNotice` plus `UiEvent::UsageUpdate` with the current accumulated usage and `context_tokens: None`;
- `PromptFailed` -> `UiEvent::AgentError`;
- `PromptAborted` -> `UiEvent::AgentError`;
- `PromptStarted`, `PromptCompleted`, `ProviderRequestStarted`, and `Diagnostic` -> no UI event.

Session and capability events:

- `SessionOpened`, `SessionWritePending`, `SessionWriteCommitted`, `SessionWriteSkipped`, and `CapabilityChanged` -> no UI event in this slice.

The ignored events are still consumed by the bridge. They are intentionally not rendered until the TUI has explicit status/footer state for product session writes and capabilities.

## Tool Arguments

`CodingAgentEvent::ToolCallStarted` carries `arguments_json` as a string. The bridge should parse it into `serde_json::Value` for `UiEvent::ToolStarted`.

If parsing fails, the bridge should preserve the raw payload as `serde_json::Value::String(arguments_json.clone())`. Tool events should not be dropped because an upstream tool emitted malformed argument JSON.

## Usage Accounting

`CodingAgentEvent` does not currently carry token usage on assistant completion. Therefore `CodingEventBridge` cannot update input/output/cache/cost totals on normal assistant completion the way `InteractiveEventBridge` does when it receives `AgentEvent::AgentDone`.

For this slice:

- `AssistantMessageCompleted` emits only `AssistantDone`;
- `RuntimeCompactionCompleted` emits a `UsageUpdate` that preserves current totals and sets `context_tokens: None`;
- future usage-bearing product events can extend the bridge without changing the `UiEvent` contract.

This keeps the bridge faithful to the current product event stream instead of inventing usage data.

## Error Handling

The bridge should be total over `CodingAgentEvent`: every event returns either zero or more `UiEvent` values.

Malformed tool argument JSON is handled locally as described above. `PromptFailed` should use the product error display string. `PromptAborted` should render a clear cancellation message.

No bridge mapping should panic on missing message ids, empty text, empty summaries, or ignored session events.

## Tests

Add focused tests in `tests/interactive_event_bridge.rs` or a nearby interactive bridge test file.

Required coverage:

- assistant delta and completion mapping;
- tool start parses JSON arguments;
- malformed tool start arguments are preserved as a string value;
- tool update, completion, and failure mapping;
- prompt failure and abort mapping;
- compaction emits notice and unknown-context usage update;
- session write and capability events are ignored;
- existing `InteractiveEventBridge` tests still pass.

Suggested checks:

```text
cargo fmt --check
cargo test -p pi-coding-agent interactive_event_bridge
cargo check --workspace
```

## Acceptance

This slice is complete when:

- `CodingEventBridge` exists next to `InteractiveEventBridge`;
- interactive product-event bridge mappings are covered by deterministic tests;
- existing old `AgentEvent` interactive bridge behavior is unchanged;
- interactive prompt execution still uses the old runner until a later migration slice;
- Phase 3 TODO marks the bridge implementation complete after code lands.

