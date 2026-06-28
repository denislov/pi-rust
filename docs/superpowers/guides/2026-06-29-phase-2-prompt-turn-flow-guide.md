# Phase 2 Guide: PromptTurnFlow on Headless and JSON Paths

## Phase Goal

Route a real prompt path through the new product runtime:

```text
CodingAgentSession::prompt()
  -> PromptTurnContext
  -> PromptTurnFlow
  -> RunAgentTurn
  -> existing Agent::run()
  -> TurnTransaction
  -> CodingAgentEvent
```

Phase 2 proves that Flow is part of the runtime, not a demo. It should migrate print/headless first, then JSON mode. RPC and interactive remain transitional until Phase 3.

## Non-Negotiable Constraints

- Do not rewrite `agent_loop.rs`.
- Do not introduce `AgentTurnFlow` yet.
- Do not write final session storage from a Flow node.
- Do not let adapters consume raw `FlowEvent`.
- Do not remove old `run_session_prompt` while RPC/interactive still need it.
- Do not change JSON mode wire shape unless deliberately documented.

## Main New Types

Phase 2 should add:

```text
PromptTurnOptions
PromptTurnOutcome
PromptTurnContext
RuntimeSnapshot
PromptTurnFlow
PromptTurnNode
AgentRunObservation
```

Keep these internal except outcome/options if `CodingAgentSession::prompt()` needs a public signature.

## PromptTurnOptions

Prefer a new product option type instead of reusing `SessionPromptOptions` directly.

Recommended shape:

```rust
pub struct PromptTurnOptions {
    pub invocation: PromptInvocation,
    pub mode: PromptMode,
    pub session_target: Option<ResolvedSessionTarget>,
    pub request_overrides: PromptRequestOverrides,
    pub cancellation: Option<CancellationToken>,
}
```

`PromptInvocation` may reuse the existing runtime type:

- text prompt;
- structured content;
- skill;
- prompt template;
- compact can remain unsupported in Phase 2 and move later.

Provide conversion from existing resolved request output:

```rust
impl TryFrom<ResolvedPromptRequest> for PromptTurnOptions { ... }
```

Do not require CLI/RPC/interactive to construct internal contexts directly.

## PromptTurnOutcome

Recommended shape:

```rust
pub enum PromptTurnOutcome {
    Success {
        operation_id: String,
        turn_id: String,
        session_id: Option<String>,
        leaf_id: Option<String>,
        final_text: String,
        final_message: AssistantMessage,
        diagnostics: Vec<CodingDiagnostic>,
    },
    Aborted {
        operation_id: String,
        turn_id: Option<String>,
        reason: String,
        session_id: Option<String>,
    },
    Failed {
        operation_id: String,
        turn_id: Option<String>,
        error: CodingSessionError,
        diagnostics: Vec<CodingDiagnostic>,
    },
}
```

Adapters should use this outcome for final output, not infer completion from low-level stream endings.

## PromptTurnContext

`PromptTurnContext` is the operation state bag, but it should be narrow and method-driven.

Recommended fields:

```rust
pub(crate) struct PromptTurnContext {
    ids: PromptTurnIds,
    options: PromptTurnOptions,
    runtime: Option<RuntimeSnapshot>,
    session: Option<PromptSessionState>,
    transaction: Option<TurnTransaction>,
    agent: Option<Agent>,
    final_message: Option<AssistantMessage>,
    diagnostics: Vec<CodingDiagnostic>,
    event_sink: CodingEventSink,
}
```

Recommended methods:

```rust
operation_id()
turn_id()
emit(event)
set_runtime(snapshot)
runtime()
set_session_state(state)
begin_transaction(...)
transaction_mut()
set_agent(agent)
agent_mut()
record_final_message(message)
finish_success()
finish_abort(reason)
finish_failure(error)
```

Avoid public mutable fields. Nodes should call methods so invariants stay local.

## RuntimeSnapshot

`RuntimeSnapshot` is the output of `RuntimeService` for one operation.

It should be equivalent to the current data passed into `SessionPromptOptions`:

```rust
pub(crate) struct RuntimeSnapshot {
    pub model: Model,
    pub api_key: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: Option<u32>,
    pub tools: Vec<AgentTool>,
    pub register_builtins: bool,
    pub resources: AgentResources,
    pub settings: Option<Settings>,
    pub thinking_level: Option<ThinkingLevel>,
    pub tool_execution: Option<ToolExecutionMode>,
    pub session_run_options: Option<SessionRunOptions>,
}
```

This type can be internal. If fields are many, use private fields and accessors.

Do not duplicate request resolution. Initially adapt from:

- `request::resolve_prompt_request`;
- `runtime::build_agent_config`;
- `resources::load_resources`;
- `models::parse_model_rotation`;
- `runtime::select_model`.

## Flow Graph

Use `pi_agent_core::flow::Flow<PromptTurnContext>`.

Node IDs:

```text
start_prompt_turn
resolve_request
prepare_input
resolve_runtime
load_resources
open_session
build_agent_runtime
record_user_input
run_agent_turn
finalize_turn
emit_completion
```

Actions:

```text
default
abort
error
no_session
```

Keep graph construction in `FlowService`:

```rust
impl FlowService {
    pub(crate) fn prompt_turn_flow(&self) -> Result<Flow<PromptTurnContext>, CodingSessionError>;
    pub(crate) async fn run_prompt_turn(&self, ctx: &mut PromptTurnContext) -> Result<PromptTurnOutcome, CodingSessionError>;
}
```

## Node Contracts

### StartPromptTurn

Inputs:

- operation IDs from context;
- event sink;
- session service capability.

Outputs:

- `PromptStarted`;
- operation started pending event if a session transaction can already be created.

Errors:

- busy/cancelled.

### ResolveRequest

Inputs:

- `PromptTurnOptions`.

Outputs:

- normalized invocation;
- request override fields.

Notes:

- Do not parse CLI args here. CLI args should already have resolved into prompt options.

### PrepareInput

Inputs:

- prompt invocation;
- existing input processing helpers.

Outputs:

- normalized content blocks;
- attachment references when session log supports them;
- `PromptInputPrepared`.

Notes:

- Phase 2 may store inline file/image details simply if blob storage is not yet fully used.

### ResolveRuntime

Inputs:

- options;
- request context;
- settings/auth paths.

Outputs:

- `RuntimeSnapshot`;
- `RuntimeResolved`.

Notes:

- Match old print/json behavior first.

### LoadResources

Inputs:

- runtime snapshot;
- resource paths/settings.

Outputs:

- resources snapshot;
- `ResourcesLoaded`.

Notes:

- If existing request resolution already loaded resources, this node can validate/attach the snapshot rather than reload.

### OpenSession

Inputs:

- session target;
- session options.

Outputs:

- Rust-native session handle;
- base leaf;
- transaction started.

Notes:

- Existing old session target types may need adapter conversion.
- Do not open old `JsonlSessionStorage` for new prompt path.

### BuildAgentRuntime

Inputs:

- runtime snapshot;
- replayed transcript from Rust-native session.

Outputs:

- configured `Agent`;
- tools registered;
- previous messages hydrated.

Notes:

- Phase 2 may support only new sessions first if replay-to-AgentMessage is not ready. If so, document the limitation and keep old path for continue/resume until Phase 3.

### RecordUserInput

Inputs:

- normalized prompt content.

Outputs:

- `turn.input.recorded` pending session event.

Notes:

- Do not commit yet.

### RunAgentTurn

Inputs:

- configured `Agent`;
- invocation.

Outputs:

- final assistant message;
- message/tool pending session events;
- `CodingAgentEvent` stream.

Notes:

- This node calls existing `Agent::run()` or current prompt helpers.
- It is the future replacement seam for `AgentTurnFlow`.

### FinalizeTurn

Inputs:

- final message or error/abort state;
- transaction.

Outputs:

- committed/aborted/failed session event log;
- manifest active leaf update on success;
- session persistence product events.

### EmitCompletion

Inputs:

- finalized outcome.

Outputs:

- `PromptCompleted`, `PromptFailed`, or `PromptAborted`;
- final `PromptTurnOutcome`.

## AgentEvent to CodingAgentEvent Mapping

Add mapping in `EventService`.

Minimum mapping:

```text
AgentEvent::TurnStart
  -> AgentTurnStarted

AgentEvent::BeforeProviderRequest
  -> ProviderRequestStarted

AgentEvent::LlmEvent(text delta)
  -> AssistantMessageDelta

AgentEvent::ToolCallStart
  -> ToolCallStarted

AgentEvent::ToolCallUpdate
  -> ToolCallUpdated

AgentEvent::ToolCallEnd success
  -> ToolCallCompleted

AgentEvent::ToolCallEnd error
  -> ToolCallFailed

AgentEvent::AgentDone
  -> AssistantMessageCompleted

AgentEvent::AgentError
  -> PromptFailed

AgentEvent::SessionCompacted
  -> RuntimeCompactionCompleted
```

Do not expose concrete Flow node names through product events.

## Recording Agent Output to Session Events

The `RunAgentTurn` node must translate observations into pending `SessionEventData`.

Minimum:

- user input was already recorded by `RecordUserInput`;
- assistant message started before first delta or at completion if no deltas were captured;
- assistant deltas may be coalesced if exact streaming persistence is too expensive;
- assistant message completed at `AgentDone`;
- tool call lifecycle events from tool events.

Important:

- It is acceptable for Phase 2 persistence to store final assistant text rather than every delta, as long as the event model supports delta and the runtime event stream preserves live deltas.
- The session log should not claim a message completed if `AgentError` or abort occurred.

## CodingAgentSession::prompt()

Add:

```rust
impl CodingAgentSession {
    pub async fn prompt(&mut self, options: PromptTurnOptions) -> Result<PromptTurnOutcome, CodingSessionError>;
}
```

Concurrency rule:

- one active mutating operation at a time;
- concurrent prompt returns `CodingSessionError::Busy`;
- `abort()` can be added if cancellation is wired in Phase 2.

## Print Mode Migration

Current anchor:

- `crates/pi-coding-agent/src/print_mode.rs`

Migration:

1. Add conversion from `PrintModeOptions` to `CodingAgentSession` + `PromptTurnOptions`.
2. Call `CodingAgentSession::prompt()`.
3. Return final text from `PromptTurnOutcome::Success`.
4. Preserve existing output newline behavior in `lib.rs`.

Tests:

- existing `print_mode` tests.
- new test: print mode writes Rust-native `events.jsonl` when session enabled.
- new test: `--no-session` still works if supported.

## JSON Mode Migration

Current anchors:

- `protocol/json_mode.rs`;
- `protocol/events.rs`;
- `protocol/types.rs`.

Migration:

1. Build JSON mode adapter from `CodingAgentEvent`.
2. Preserve existing JSON wire fields unless intentionally changed.
3. Stop constructing protocol events directly from raw `AgentEvent` for the migrated path.
4. Keep old `AgentEvent` adapter for RPC/interactive until Phase 3.

Tests:

- existing `json_mode` tests.
- event sequence from `CodingAgentEvent`.
- provider failure maps to error output.

## Transitional Old Runner Policy

`protocol/session_runner.rs` remains for:

- RPC prompt until Phase 3;
- interactive prompt until Phase 3;
- manual compaction until a dedicated flow exists.

Do not delete:

- `SessionPromptOptions`;
- `run_session_prompt`;
- `spawn_session_prompt`;
- old session append helpers.

Phase 2 may add comments identifying transitional status.

## Test Files

Recommended additions:

```text
crates/pi-coding-agent/tests/coding_session_prompt.rs
crates/pi-coding-agent/tests/prompt_turn_flow.rs
crates/pi-coding-agent/tests/coding_session_events.rs
crates/pi-coding-agent/tests/session_event_prompt_replay.rs
```

Expected coverage:

- flow graph runs with faux provider;
- success prompt commits operation;
- provider error records failed operation;
- abort records aborted operation;
- final output matches old print mode for same faux provider;
- JSON mode consumes `CodingAgentEvent`.

## Phase 2 Handoff to Phase 3

Phase 2 must leave:

- `CodingAgentSession::prompt()` working for print/headless.
- JSON mode adapter using `CodingAgentEvent`.
- session event log containing prompt/assistant/tool facts.
- old runner still available for RPC/interactive.
- `RunAgentTurn` isolated enough to replace with `AgentTurnFlow` later.

## Stop Conditions

Stop and reassess if:

- print/json migration requires editing `agent_loop.rs`;
- `PromptTurnContext` starts holding all services directly;
- a node writes `events.jsonl` without transaction finalization;
- JSON mode protocol starts depending on flow node names;
- continue/resume semantics cannot be represented from Rust-native replay.

## Suggested Checks

Focused:

```text
cargo fmt --check
cargo test -p pi-coding-agent coding_session_prompt
cargo test -p pi-coding-agent prompt_turn_flow
cargo test -p pi-coding-agent print_mode
cargo test -p pi-coding-agent json_mode
```

Broader:

```text
cargo test -p pi-coding-agent
cargo test -p pi-agent-core
cargo check --workspace
```
