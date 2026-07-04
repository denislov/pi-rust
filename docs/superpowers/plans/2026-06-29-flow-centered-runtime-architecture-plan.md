# Flow-Centered Runtime Architecture Implementation Plan

## Source Design

Design spec:

- `docs/superpowers/specs/2026-06-29-flow-centered-runtime-architecture-design.md`

This plan implements the design in slices. The first implementation target is not a full rewrite. It is a real path:

```text
headless/json prompt path
  -> CodingAgentSession
  -> PromptTurnFlow
  -> RunAgentTurn node
  -> existing Agent::run()
  -> Rust-native session event log
  -> CodingAgentEvent stream
```

## Current Code Anchors

The current product prompt path is centered on:

- `crates/pi-coding-agent/src/lib.rs`
- `crates/pi-coding-agent/src/protocol/session_runner.rs`
- `crates/pi-coding-agent/src/print_mode.rs`
- `crates/pi-coding-agent/src/protocol/json_mode.rs`
- `crates/pi-coding-agent/src/session.rs`
- `crates/pi-coding-agent/src/runtime.rs`
- `crates/pi-coding-agent/src/request.rs`

The existing low-level Flow runtime is:

- `crates/pi-agent-core/src/flow.rs`

The existing low-level agent loop stays in place during Phase 1 and Phase 2:

- `crates/pi-agent-core/src/agent_loop.rs`
- `crates/pi-agent-core/src/agent.rs`

## Implementation Rules

- Do not rewrite `agent_loop.rs` in Phase 1 or Phase 2.
- Do not preserve TypeScript session JSONL compatibility.
- Do not add TypeScript session import/export.
- Keep old session code available until adapters migrate.
- Add new Rust-native session event log beside existing code first.
- Keep `CodingAgentSession` and `CodingAgentEvent` behind `pi_coding_agent::api`.
- Keep services, contexts, and concrete nodes internal unless a later step intentionally promotes them.
- Every slice should have focused tests before moving to the next slice.
- Prefer deterministic faux-provider and fixture tests.

## Phase 1: Rust-Native Session Log and `CodingAgentSession` Skeleton

Phase 1 creates the product owner and session persistence foundation. It does not yet replace the existing prompt path.

### Step 1. Add Runtime Module Shell

Add internal modules under `pi-coding-agent`:

```text
crates/pi-coding-agent/src/coding_session/mod.rs
crates/pi-coding-agent/src/coding_session/event.rs
crates/pi-coding-agent/src/coding_session/session_service.rs
crates/pi-coding-agent/src/coding_session/runtime_service.rs
crates/pi-coding-agent/src/coding_session/flow_service.rs
crates/pi-coding-agent/src/coding_session/capability_service.rs
crates/pi-coding-agent/src/coding_session/plugin_service.rs
crates/pi-coding-agent/src/coding_session/context.rs
crates/pi-coding-agent/src/coding_session/error.rs
```

Expose only stable-facing types through `api`:

```rust
pub use crate::coding_session::{CodingAgentEvent, CodingAgentSession};
```

Keep the module itself `pub(crate)` or migration-private where possible.

Tests:

- Extend `crates/pi-coding-agent/tests/public_api.rs` to import `CodingAgentSession` and `CodingAgentEvent` from `pi_coding_agent::api`.
- Add a minimal constructor test that does not run a model.

Acceptance:

- The crate compiles with the new shell.
- `api` facade exports the intended stable entry points.
- Existing public API remains available.

Suggested commit:

```text
feat(coding-agent): add coding session runtime shell
```

### Step 2. Define Product Errors

Add a typed product error boundary:

```text
CodingSessionError
  Config
  Auth
  Input
  Resource
  Session
  Provider
  Tool
  Flow
  Plugin
  Cancelled
  UnsupportedCapability
  Busy
```

Map to/from existing `CliError` where needed, but do not replace `CliError` everywhere yet.

Files:

- `crates/pi-coding-agent/src/coding_session/error.rs`
- `crates/pi-coding-agent/src/error.rs` only if conversion helpers are needed.

Tests:

- Unit tests for display/code mapping.
- Verify existing CLI error output does not change unless deliberately routed through the new API.

Acceptance:

- New runtime has typed errors.
- Existing CLI paths still return existing `CliError`.

### Step 3. Define `CodingAgentEvent`

Create the product event enum with the minimum Phase 1/2 surface:

```text
SessionOpened
PromptStarted
PromptInputPrepared
RuntimeResolved
ResourcesLoaded
AgentTurnStarted
AssistantMessageStarted
AssistantMessageDelta
AssistantMessageCompleted
ToolCallStarted
ToolCallUpdated
ToolCallCompleted
ToolCallFailed
SessionWritePending
SessionWriteCommitted
SessionWriteSkipped
PromptCompleted
PromptFailed
PromptAborted
Diagnostic
CapabilityChanged
```

Include correlation IDs where available:

```text
operation_id
turn_id
session_id
message_id
tool_call_id
```

Use plain deterministic IDs in tests. Do not require timestamps for Phase 1 unless a clock abstraction is added.

Files:

- `crates/pi-coding-agent/src/coding_session/event.rs`

Tests:

- Serialization tests if events are serialized in Phase 1.
- Basic construction/import smoke in `public_api.rs`.

Acceptance:

- Event type exists as canonical product event.
- No adapter consumes it yet.

### Step 4. Add Rust-Native Session Event Types

Add session event model separate from `CodingAgentEvent`:

```text
crates/pi-coding-agent/src/coding_session/session_log/mod.rs
crates/pi-coding-agent/src/coding_session/session_log/event.rs
crates/pi-coding-agent/src/coding_session/session_log/manifest.rs
crates/pi-coding-agent/src/coding_session/session_log/store.rs
crates/pi-coding-agent/src/coding_session/session_log/replay.rs
crates/pi-coding-agent/src/coding_session/session_log/id.rs
```

Minimum event set:

```text
session.created
operation.started
operation.committed
operation.aborted
operation.failed
turn.started
turn.input.recorded
message.started
message.delta
message.completed
message.cancelled
tool.call.started
tool.call.updated
tool.call.completed
tool.call.failed
tool.call.cancelled
diagnostic.emitted
active_leaf.changed
metadata.updated
```

Represent as Rust enum with serde tagging:

```rust
#[serde(tag = "kind", content = "data")]
pub enum SessionEventData { ... }
```

Envelope fields:

```text
schema
version
session_id
event_id
operation_id
turn_id
branch_id
leaf_id
parent_event_id
created_at
data
```

Use optional fields where not every event has every ID.

Files:

- New `session_log` module under `coding_session`.

Tests:

- JSON round-trip for each minimum event variant.
- Assert `kind` strings are stable and snake/dot style as designed.
- Reject unsupported schema version if the store reads a future incompatible major version.

Acceptance:

- Session events are strongly typed.
- No dynamic `serde_json::Map` primary model.
- Event fixtures are readable JSONL lines.

Suggested commit:

```text
feat(coding-agent): add rust-native session event model
```

### Step 5. Implement Session Manifest and Store

Implement canonical layout:

```text
session_dir/
  session.json
  events.jsonl
  blobs/
  index/
```

Store responsibilities:

- create session directory;
- write `session.json`;
- append event envelopes to `events.jsonl`;
- read manifest;
- read event log;
- update active leaf only after operation finalization;
- leave `blobs/` and `index/` present but minimally used.

Do not implement SQLite or derived indexes in Phase 1.

Files:

- `session_log/manifest.rs`
- `session_log/store.rs`
- `session_log/id.rs`

Tests:

- create session writes manifest and empty/initial event log;
- append events and read back in order;
- update manifest active leaf after commit;
- temp directory tests only;
- no dependency on home directory unless env is explicitly injected.

Acceptance:

- A Rust-native session can be created and reopened.
- `events.jsonl` is canonical.
- Manifest active leaf can be read and updated.

### Step 6. Implement Replay/Fold to Transcript

Add a minimal replay view:

```text
SessionReplay
  session_id
  active_leaf
  operations
  turns
  transcript messages
  diagnostics
```

Phase 1 transcript only needs:

- user input text;
- assistant message text from started/delta/completed events;
- cancelled/incomplete message markers;
- tool call summaries if present.

Files:

- `session_log/replay.rs`

Tests:

- replay user input + assistant message;
- replay cancelled assistant message;
- ignore incomplete operation without final marker or mark it as incomplete according to recovery policy;
- replay tool call start/completion into a readable transcript item.

Acceptance:

- Event log can produce a transcript without reading old TS-compatible JSONL.

### Step 7. Add `TurnTransaction`

`TurnTransaction` collects pending `SessionEventEnvelope`s and staged blob references.

Minimum API:

```text
begin_turn(...)
record_user_input(...)
start_assistant_message(...)
append_assistant_delta(...)
complete_assistant_message(...)
cancel_assistant_message(...)
record_tool_call_started(...)
record_tool_call_completed(...)
emit_diagnostic(...)
commit(...)
abort(...)
fail(...)
```

Commit policy:

- append pending events;
- append operation final marker;
- update manifest active leaf only after final marker;
- emit session persistence events through an event sink if supplied.

Files:

- `coding_session/session_service.rs`
- `coding_session/session_log/*`

Tests:

- success commit appends operation.started through operation.committed;
- abort appends operation.aborted and does not write normal completion;
- fail appends operation.failed;
- manifest active leaf updates only after commit;
- failed append does not update in-memory active leaf.

Acceptance:

- Flow nodes can later record pending events without writing storage directly.

Suggested commit:

```text
feat(coding-agent): add turn transaction for rust-native sessions
```

### Step 8. Implement `CodingAgentSession` Skeleton

Add owner structure:

```text
CodingAgentSession
  session_service
  runtime_service
  flow_service
  event_service
  capability_service
  plugin_service
```

Phase 1 methods:

```text
open(options)
create(options)
subscribe()
capabilities()
session_view()
```

Do not implement `prompt()` yet except possibly as an unimplemented typed error if needed.

Files:

- `coding_session/mod.rs`
- service modules

Tests:

- create/open session through `CodingAgentSession`;
- subscribe receives `SessionOpened` or equivalent if event dispatch exists in Phase 1;
- capabilities can be queried.

Acceptance:

- The owner exists and owns services.
- The owner does not expose mutable service references in public API.

### Step 9. Phase 1 Integration Check

Run:

```text
cargo fmt --check
cargo test -p pi-coding-agent session
cargo test -p pi-coding-agent public_api
cargo check -p pi-coding-agent
```

If changes touch shared agent-core types, also run:

```text
cargo test -p pi-agent-core
```

Acceptance:

- Phase 1 tests pass.
- Existing old session tests are not removed unless intentionally replaced.
- New docs may note that old session storage is transitional.

## Phase 2: `PromptTurnFlow` on Headless/JSON Path

Phase 2 connects the new owner to a real prompt path. It still calls existing `Agent::run()`.

### Step 10. Define `PromptTurnContext`

Add operation context:

```text
PromptTurnContext
  input
  request mode
  request overrides
  runtime snapshot
  resources snapshot
  tool snapshot
  active session info
  transaction
  agent handle
  agent observations
  event sink
  cancellation token
  output
  diagnostics
```

Keep fields private where possible. Expose methods for nodes.

Files:

- `coding_session/context.rs`
- possibly `coding_session/prompt.rs`

Tests:

- context creation from a minimal prompt request;
- context records diagnostics and transaction events through methods.

Acceptance:

- Flow nodes can operate on context without receiving `&mut CodingAgentSession`.

### Step 11. Add `RuntimeSnapshot`

Extract the reusable output of current request/runtime resolution:

```text
RuntimeSnapshot
  model
  api_key
  system_prompt
  max_turns
  tools
  register_builtins
  resources
  settings
  thinking_level
  tool_execution
  invocation
  session options
```

This should reuse existing `request::resolve_prompt_request()` and `runtime::build_agent_config()` initially. Do not duplicate model/resource resolution logic.

Files:

- `coding_session/runtime_service.rs`
- existing `request.rs` and `runtime.rs` only if small helpers need to be factored out.

Tests:

- resolving the same CLI request yields equivalent model/session/tool choices as old path.
- test with faux model/options from existing request/runtime tests.

Acceptance:

- `RuntimeService` can build the same inputs currently passed into `SessionPromptOptions`.

### Step 12. Add `PromptTurnFlow`

Use `pi_agent_core::flow::Flow<PromptTurnContext>`.

Add internal node structs or closures for:

```text
StartPromptTurn
ResolveRequest
PrepareInput
ResolveRuntime
LoadResources
OpenSession
BuildAgentRuntime
RecordUserInput
RunAgentTurn
FinalizeTurn
EmitCompletion
```

Implementation can combine adjacent nodes if current code structure makes a one-to-one split premature, but the graph should preserve the conceptual boundaries in names/events.

Files:

- `coding_session/flow_service.rs`
- `coding_session/prompt_flow.rs`
- `coding_session/prompt_nodes.rs` if needed.

Tests:

- graph construction contains expected node IDs;
- strict missing transition still fails in test-only misconfigured graph;
- successful no-tool faux provider run reaches `PromptCompleted`.

Acceptance:

- `PromptTurnFlow` is a real flow, not a normal function named flow.
- Flow node names are stable enough for debug logs but not public protocol.

### Step 13. Implement Event Mapping

Map:

```text
FlowEvent -> optional debug CodingAgentEvent
AgentEvent -> CodingAgentEvent
Session transaction event -> CodingAgentEvent
```

Minimum mapping:

- `AgentEvent::AgentDone` -> `AssistantMessageCompleted` + later `PromptCompleted`;
- LLM stream deltas -> `AssistantMessageDelta` where available;
- tool start/done -> tool events;
- agent error -> `PromptFailed`;
- transaction commit -> `SessionWriteCommitted`.

Files:

- `coding_session/event.rs`
- `coding_session/event_service.rs` if split out.

Tests:

- table-driven mapping from representative `AgentEvent` values;
- event ordering in a faux prompt turn.

Acceptance:

- JSON/headless adapters can eventually consume `CodingAgentEvent`.
- No adapter needs to understand `FlowEvent` directly.

### Step 14. Add `RunAgentTurn` Node

`RunAgentTurn` should:

- build or receive an `Agent`;
- call existing `Agent::run()`/prompt equivalent;
- forward `AgentEvent`s to `EventService`;
- collect final assistant message;
- record assistant/tool lifecycle into `TurnTransaction`.

It should not:

- write final session storage directly;
- update active leaf directly;
- assume future `AgentTurnFlow` internals.

Files:

- `coding_session/prompt_nodes.rs`
- possibly helper extraction from `protocol/session_runner.rs`.

Tests:

- faux provider success records assistant message events;
- tool call records tool lifecycle events;
- provider error records `operation.failed`;
- cancellation records abort/cancel events.

Acceptance:

- Existing `Agent::run()` is used.
- Session writes happen through transaction.

### Step 15. Add `CodingAgentSession::prompt()`

Public product API:

```text
CodingAgentSession::prompt(prompt_options) -> PromptTurnOutcome
```

Use a new option type rather than reusing `SessionPromptOptions` directly if the latter is too tied to old runner internals. Provide conversion from existing resolved request types.

Outcome:

```text
Success
Aborted
Failed
```

Files:

- `coding_session/mod.rs`
- `coding_session/prompt.rs`
- `api` facade export.

Tests:

- public API smoke imports and calls prompt with faux provider/options;
- prompt creates Rust-native session event log;
- replay transcript contains user and assistant text.

Acceptance:

- A real prompt can run through `CodingAgentSession`.

Suggested commit:

```text
feat(coding-agent): run prompt turn through coding session flow
```

### Step 16. Route Print Mode Through `CodingAgentSession`

Update print mode first because it is the simplest adapter.

Current anchor:

- `crates/pi-coding-agent/src/print_mode.rs`

Plan:

- Convert `PrintModeOptions` into `CodingAgentSession` open/prompt options.
- Subscribe or collect events if needed.
- Return final assistant text from `PromptTurnOutcome`.
- Keep old `run_session_prompt` available for paths not yet migrated.

Tests:

- existing `print_mode` tests still pass;
- add a test proving print mode writes Rust-native session events when sessions are enabled;
- no regression for `--no-session` if that mode exists.

Acceptance:

- Print/headless path uses new runtime.
- User-visible print output does not regress.

### Step 17. Route JSON Mode Through `CodingAgentEvent`

Current anchor:

- `crates/pi-coding-agent/src/protocol/json_mode.rs`
- `crates/pi-coding-agent/src/protocol/events.rs`

Plan:

- Adapt `CodingAgentEvent` to existing JSON mode wire events.
- Keep wire compatibility for JSON mode unless intentionally changed.
- Do not expose raw `FlowEvent` as JSON mode protocol.

Tests:

- existing `json_mode` tests still pass or are deliberately updated;
- event ordering test: prompt started, assistant delta/completed, session committed, prompt completed;
- error event maps to existing JSON failure shape.

Acceptance:

- JSON mode is driven by product events.
- JSON mode no longer needs to infer state from raw `AgentEvent`.

### Step 18. Keep Old Runner as Transitional Wrapper

`protocol/session_runner.rs` should not be deleted in Phase 2. It can:

- remain used by RPC/interactive until Phase 3;
- call `CodingAgentSession` for print/json if convenient;
- be gradually reduced as adapters migrate.

Document transitional status in code comments only where needed.

Tests:

- existing RPC/interactive tests continue using old path until migrated.

Acceptance:

- Migration does not break unconverted adapters.

### Step 19. Phase 2 Integration Check

Run:

```text
cargo fmt --check
cargo test -p pi-coding-agent print_mode
cargo test -p pi-coding-agent json_mode
cargo test -p pi-coding-agent public_api
cargo test -p pi-coding-agent session
cargo check -p pi-coding-agent
```

Then run broader checks:

```text
cargo test -p pi-coding-agent
cargo test -p pi-agent-core
cargo check --workspace
```

Acceptance:

- Headless/json path uses the new owner and flow.
- Old interactive/RPC paths still pass.

## Phase 3: Adapter Convergence

Phase 3 moves existing frontends onto `CodingAgentSession`.

### Step 20. RPC Capabilities and Prompt Path

Current anchors:

- `crates/pi-coding-agent/src/protocol/rpc.rs`
- `crates/pi-coding-agent/src/protocol/rpc/commands.rs`
- `crates/pi-coding-agent/src/protocol/rpc/prompt.rs`
- `crates/pi-coding-agent/src/protocol/rpc/state.rs`
- `crates/pi-coding-agent/src/protocol/rpc/wire.rs`

Plan:

- Add `get_capabilities` response from `CapabilityService`.
- Route RPC prompt through `CodingAgentSession::prompt()`.
- Adapt `CodingAgentEvent` to RPC wire events.
- Replace M5 unsupported strings where capability information is available.

Tests:

- RPC prompt uses Rust-native session event log.
- unsupported commands are visible through capabilities.
- existing RPC mode tests still pass.

### Step 21. Interactive Prompt Path

Current anchors:

- `crates/pi-coding-agent/src/interactive/prompt_task.rs`
- `crates/pi-coding-agent/src/interactive/event_bridge.rs`
- `crates/pi-coding-agent/src/interactive/root.rs`
- `crates/pi-coding-agent/src/interactive/session_actions.rs`

Plan:

- Drive prompt tasks through `CodingAgentSession`.
- Adapt `CodingAgentEvent` to transcript updates.
- Keep UI rendering independent from raw `AgentEvent`.
- Move session commands toward owner methods.

Tests:

- existing interactive transcript/event bridge tests updated to consume product events.
- session actions use owner/service instead of direct old storage where migrated.

### Step 22. Session Operations

Move shared session operations into `SessionService`:

- new session;
- open/switch;
- fork/clone;
- name/metadata update;
- export if retained as Rust-native export;
- stats/tree from replay/index.

Tests:

- operations are shared by RPC and interactive;
- active leaf update goes through transaction/service rules.

## Phase 4: `AgentTurnFlow`

Do this only after product owner and prompt flow are stable.

Target files:

- `crates/pi-agent-core/src/agent_turn_flow.rs` or `src/agent_turn_flow/mod.rs`
- `crates/pi-agent-core/src/agent.rs`
- `crates/pi-agent-core/src/agent_loop.rs`
- `crates/pi-agent-core/src/loop_runtime/*`

Plan:

1. Define `AgentTurnContext`.
2. Extract provider stream phase into a node.
3. Extract tool execution phase into a node.
4. Extract runtime compaction phase into a node.
5. Preserve `Agent::run()` API.
6. Make `Agent::run()` delegate to `AgentTurnFlow`.

Tests:

- existing `pi-agent-core` tests pass.
- add flow path tests for provider/tool/compaction transitions.
- abort/steer/follow-up semantics remain.

Acceptance:

- Product `RunAgentTurn` node no longer depends on monolithic loop internals.
- `AgentEvent` remains available as low-level event output.

## Phase 5: Plugin Kernel

Target files are not fixed yet. Likely modules:

```text
crates/pi-coding-agent/src/plugins/mod.rs
crates/pi-coding-agent/src/plugins/registry.rs
crates/pi-coding-agent/src/plugins/host.rs
crates/pi-coding-agent/src/plugins/tool.rs
crates/pi-coding-agent/src/plugins/command.rs
crates/pi-coding-agent/src/plugins/hook.rs
```

Plan:

- implement Rust trait registry first;
- register first-party sample tool/command/hook;
- integrate tools through `RuntimeService`;
- integrate hooks through `PromptTurnFlow` extension points;
- keep `FlowExtension` first-party/reserved;
- do not expose arbitrary Lua node/subflow.

Tests:

- plugin tool can be called by faux provider;
- plugin command appears in capability list;
- plugin hook diagnostic becomes `CodingAgentEvent::Diagnostic`;
- plugin failure does not panic or half-commit session.

## Documentation Updates During Implementation

Update docs as behavior changes:

- `docs/archive/roadmap/M12-plugin-system.md` when consulting the archived plugin-kernel background.
- `docs/roadmap/cross-cutting.md` to remove TS session compatibility risk and add Rust-native session schema risk.
- Possibly add `docs/session-format.md` once event schema stabilizes.
- Update existing migration comparison docs only if they become materially stale.

## Test Strategy Summary

Use these test layers:

1. Unit tests for event types, manifest, store, replay, transaction.
2. Integration tests for `CodingAgentSession` public API.
3. Faux provider prompt-turn tests for `PromptTurnFlow`.
4. Adapter tests for print/json output.
5. Existing CLI/RPC/interactive regression tests during migration.
6. Workspace checks after adapter convergence.

Avoid:

- real provider keys;
- network;
- filesystem writes outside temp dirs;
- timestamp-dependent assertions without injected clocks.

## Expected Commit Sequence

Recommended commit slices:

1. `feat(coding-agent): add coding session runtime shell`
2. `feat(coding-agent): add rust-native session event model`
3. `feat(coding-agent): add session event store and replay`
4. `feat(coding-agent): add turn transaction finalization`
5. `feat(coding-agent): expose coding session owner`
6. `feat(coding-agent): add prompt turn flow`
7. `feat(coding-agent): map agent events to coding session events`
8. `feat(coding-agent): route print mode through coding session`
9. `feat(coding-agent): route json mode through coding session events`
10. `docs: update roadmap for rust-native session runtime`

Each commit should keep tests passing for the touched surface.

## Stop Points

Stop and reassess if any of these happen:

- `CodingAgentSession` starts exposing mutable services publicly.
- `PromptTurnContext` grows into a generic all-runtime bag.
- `SessionEventData` falls back to untyped `serde_json::Value` for common events.
- print/json migration requires rewriting `agent_loop.rs`.
- plugin/Lua API pressure appears before Phase 2 proves the main path.
- event mapping forces RPC/TUI to depend on concrete Flow node names.

## First Implementation Start

Start with Phase 1 Step 1:

1. Add `coding_session` module shell.
2. Define `CodingAgentSession` as an owner with no prompt execution yet.
3. Define `CodingAgentEvent` minimum enum.
4. Export both from `pi_coding_agent::api`.
5. Add public API smoke tests.

This gives the architecture a concrete anchor while keeping behavior unchanged.
