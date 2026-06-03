# Design: Rust port of `pi-agent-core` (proof-of-concept)

- Date: 2026-06-03
- Status: Draft (pending review)
- Scope: Agent runtime crate as phase 2 of the `pi` monorepo Rust port.
- Depends on: `pi-ai` (phase 1, complete; faux provider enhancement needed)

## 1. Context

`pi-ai` is done — it provides `stream()`/`complete()` over a provider abstraction,
with Anthropic + faux providers, verified via 59 offline tests.

`pi-agent-core` (~8K LOC in TypeScript) is the agent runtime: a tool-calling loop
that manages message state, executes tools, and emits events through a stream.
It builds on `pi-ai`'s LLM streaming API.

This design covers a **proof-of-concept port** — not the full TS feature set.

## 2. Goals & success criteria

Build a `pi-agent-core` Rust crate that implements a stateful Agent with a
tool-calling loop over `pi-ai`'s streaming API.

The PoC is **done** when:

1. `cargo build -p pi-agent-core` and the whole workspace build cleanly on Rust 1.96 / edition 2024.
2. The offline test suite passes with **no network access and no credentials**:
   - Agent loop tests driven by a test provider (mock `ApiProvider`, not faux provider).
   - Tool execution tests (success, error, unknown tool → error content).
   - Message conversion tests (AgentMessage → LLM Context).
   - Multi-turn loop tests (text → tool → text, max_turns exceeded, stop on `Stop`/`Length`/`ToolUse`).
   - Agent state tests (add_tool, add_message, messages reflect mutations including assistant responses).
   - Cancellation test (abort mid-stream).
3. An optional offline example (`examples/loop_example.rs`) runs a complete agent
   conversation using a test provider (no key required).

Non-goal: live calls to the real Anthropic API, compaction, session storage,
skill loading, proxy utilities, parallel tool execution, or `continue_loop()`.

### 2.1 Prerequisite: enhance Rust faux provider

The current faux provider always yields `Done { reason: Stop }` and replays all
responses in a single call. To test the agent loop, we need:

- **Per-call response queues**: supports `Vec<Vec<FauxResponse>>` — each outer
  entry is consumed by one `stream()` call, enabling multi-turn simulation.
- **Custom stop_reason**: `FauxResponse` gains `stop_reason: StopReason` field
  (default `Stop`), so the agent loop can see `ToolUse` and execute tools.
- **Model matching**: the faux provider only responds for its registered model id.

## 3. Key decisions

- **Shared internal state:** `Agent` wraps `Arc<RwLock<AgentState>>` so the
  returned stream can hold a clone and mutate state as it runs. `abort()` uses
  a `CancellationToken` to interrupt the in-flight loop (and its underlying LLM
  stream + tool futures).
- **Single entry point:** `prompt()` adds a user message, then runs the full
  tool-calling loop until the model stops (or `max_turns` / error / cancel).
  No separate `continue_loop()` — the loop handles `ToolUse` automatically.
- **Sequential tool execution:** tools are called one at a time in the order
  the model outputs them. No parallel execution, no `beforeToolCall`/`afterToolCall`
  hooks.
- **Direct pi-ai integration:** the agent loop calls `pi_ai::stream_model()`
  directly. Tests use a dedicated test `ApiProvider` (not the faux provider) to
  supply scripted LLM responses.
- **EventStream output:** the loop yields `AgentEvent` variants through a
  stream. No internal subscription system.
- **No message queues:** the caller appends messages via `add_message()` before
  calling `prompt()`. No steering/follow-up queue injection.
- **Minimal AgentMessage:** a simple enum wrapping pi-ai types with a `message_id`
  field. No full metadata.

## 4. Scope

### In scope
- `Agent` struct (wrapping `Arc<RwLock<AgentState>>`) with `new()`, `add_tool()`,
  `add_message()`, `messages()`, `prompt()`, `abort()`.
- `AgentMessage` enum (UserText, Assistant, ToolResult, SystemPrompt) with
  `message_id`.
- `AgentTool` with `name`, `description`, `parameters` (JSON Schema), and an
  async `execute` function via `Arc<dyn Fn>`.
- `AgentConfig` (model, system_prompt, max_turns, stream_options, max_tool_turns).
- `AgentEvent` enum: `TurnStart`, `LlmEvent` (transparent passthrough),
  `ToolCallStart/End`, `AgentDone`, `AgentError`.
- `AgentState` — the shared mutable state (Vec<AgentMessage>, Vec<AgentTool>,
  AgentConfig, CancellationToken).
- `AgentStream` type alias.
- Message conversion: `AgentMessage` → `pi_ai::Message` for LLM context building.
- All code path termination branches save the assistant message to `AgentState`.
- Test `ApiProvider` for driving agent loop tests offline.
- Offline example.

### Out of scope
- `continue_loop()`, parallel tool execution, `beforeToolCall`/`afterToolCall`.
- Compaction, session storage, system prompt management, skill loading.
- Steering / follow-up message queues, proxy utilities.
- Injectable `StreamFn` trait.
- ToolResult coalescing (already handled in pi-ai's Anthropic convert layer).

## 5. Architecture

### 5.1 Crate layout

```
crates/pi-agent-core/
  Cargo.toml
  src/
    lib.rs           # public API: re-exports
    agent.rs         # Agent struct, AgentStream, AgentState
    types.rs         # AgentMessage, AgentTool, AgentConfig, AgentEvent, ToolFn
    agent_loop.rs    # core loop algorithm (internal fn run_loop)
    convert.rs       # AgentMessage -> pi_ai::Message conversion
  tests/
    agent_loop.rs    # loop tests via test provider
    convert.rs       # message conversion tests
    tool_exec.rs     # tool execution tests (sync mock tools)
  examples/
    loop_example.rs  # offline example
```

### 5.2 Types (`types.rs`)

```rust
pub enum AgentMessage {
    UserText    { message_id: String, text: String },
    Assistant   { message_id: String, message: AssistantMessage },
    ToolResult  { message_id: String, tool_call_id: String, content: Vec<ContentBlock> },
    SystemPrompt { message_id: String, text: String },
}

pub type ToolFn = Arc<
    dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = Result<Vec<ContentBlock>, String>> + Send>>
        + Send + Sync,
>;

pub struct AgentTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub execute: ToolFn,
}

pub struct AgentConfig {
    pub model: Model,
    pub system_prompt: Option<String>,
    pub max_turns: u32,
    pub stream_options: Option<StreamOptions>,
}

pub enum AgentEvent {
    TurnStart { turn: u32 },

    /// Transparent passthrough of pi-ai LLM streaming events.
    LlmEvent(AssistantMessageEvent),

    ToolCallStart { tool_call_id: String, tool_name: String },
    ToolCallEnd { tool_call_id: String, result: Result<Vec<ContentBlock>, String> },

    AgentDone { message: AssistantMessage },
    AgentError { error: String },
}

pub type AgentStream = Pin<Box<dyn Stream<Item = AgentEvent> + Send>>;
```

### 5.3 AgentState (shared mutable state)

```rust
pub struct AgentState {
    pub messages: Vec<AgentMessage>,
    pub tools: Vec<AgentTool>,
    pub config: AgentConfig,
    pub cancel_token: CancellationToken,
}
```

`Agent` holds `Arc<RwLock<AgentState>>`. The loop function clones the `Arc`
and holds a write lock during state mutations.

### 5.4 Agent struct (`agent.rs`)

```rust
pub struct Agent {
    state: Arc<RwLock<AgentState>>,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self;
    pub fn add_tool(&self, tool: AgentTool);
    pub fn add_message(&self, msg: AgentMessage);
    pub fn messages(&self) -> Vec<AgentMessage>;  // snapshot when called

    /// Adds a UserText message and runs the full tool-calling loop.
    /// Returns a stream that can be consumed by the caller.
    pub fn prompt(&self, text: &str) -> AgentStream;

    /// Cancels an in-flight loop. Safe to call from another task.
    /// After abort, the stream yields AgentError("aborted") and ends.
    pub fn abort(&self);
}
```

All methods take `&self` (not `&mut self`) because state is behind `Arc<RwLock>`.

### 5.5 Agent loop (`agent_loop.rs`)

Internal function `run_loop(state: Arc<RwLock<AgentState>>) -> AgentStream`.

The loop clones `state` into the returned stream. At `prompt()`, the caller
pushes a `UserText` message via the lock, then calls `run_loop`.

Algorithm:

```
let cancel = state.read().cancel_token.clone();
let state_clone = state.clone();

Box::pin(stream! {
  let mut turn = 0;
  loop:
    // Check cancellation
    if cancel.is_cancelled() { yield AgentError("aborted"); return; }

    turn += 1;
    { let s = state_clone.read(); if turn > s.config.max_turns { yield AgentError("max turns"); return; } }
    yield TurnStart { turn };

    // 1. Build LLM Context from current messages
    let ctx = {
        let s = state_clone.read();
        convert_to_context(&s.config.system_prompt, &s.messages, &s.tools)
    };

    // 2. Call LLM. Inject CancellationToken into StreamOptions.
    let (model, opts) = {
        let s = state_clone.read();
        let mut opts = s.config.stream_options.clone().unwrap_or_default();
        opts.cancel = Some(cancel.clone());
        (s.config.model.clone(), opts)
    };
    let mut llm_stream = pi_ai::stream_model(&model, ctx, Some(opts));

    let mut assistant_message = None;
    while let Some(event) = llm_stream.next().await {
        match &event {
            AssistantMessageEvent::Done { message, .. } => assistant_message = Some(message.clone()),
            AssistantMessageEvent::Error { .. } => { yield AgentError(...); return; }
            _ => {}
        }
        yield AgentEvent::LlmEvent(event);
    }

    let assistant = match assistant_message {
        Some(m) => m,
        None => { yield AgentError("no Done event"); return; }
    };

    // 3. Save assistant to messages (all branches)
    {
        let mut s = state_clone.write();
        s.messages.push(AgentMessage::Assistant {
            message_id: assistant.response_id.clone().unwrap_or_default(),
            message: assistant.clone(),
        });
    }

    // 4. Branch on stop reason
    match &assistant.stop_reason {
        StopReason::Stop | StopReason::Length | StopReason::Error => {
            yield AgentDone { message: assistant };
            return;
        }
        StopReason::Aborted => {
            yield AgentError { error: "aborted".into() };
            return;
        }
        StopReason::ToolUse => {
            // 5. Execute tools sequentially
            for block in &assistant.content {
                let (id, name, args) = match block {
                    ContentBlock::ToolCall { id, name, arguments, .. } =>
                        (id.clone(), name.clone(), arguments.clone()),
                    _ => continue,
                };

                let tool_result = match tools_by_name(&state_clone.read().tools, &name) {
                    Some(tool) => {
                        yield ToolCallStart { tool_call_id: id.clone(), tool_name: name.clone() };
                        tool.execute(args).await
                    }
                    None => Err(format!("unknown tool: {}", name)),
                };

                yield ToolCallEnd { tool_call_id: id.clone(), result: tool_result.clone() };

                let content = match &tool_result {
                    Ok(blocks) => blocks.clone(),
                    Err(e) => vec![ContentBlock::Text { text: e.clone(), text_signature: None }],
                };
                state_clone.write().messages.push(AgentMessage::ToolResult {
                    message_id: assistant.response_id.clone().unwrap_or_default(),
                    tool_call_id: id.clone(),
                    content,
                });
            }
            // Back to step 1 — loop continues
        }
    }
})
```

**Key points:**
- `cancel_token` is cloned into the stream and checked at each iteration start.
  It is also injected into `StreamOptions.cancel` so pi-ai's reqwest stream is
  cancelled if the agent is aborted mid-request.
- All branches (Stop, Length, Error, ToolUse) save the assistant message to
  `AgentState.messages` before yielding the terminal event.
- `ToolUse` branch executes all tool calls, saves each `ToolResult`, and loops.
- Unknown tools yield `ToolCallEnd(Err(...))` and the error content is saved
  as the tool result. The loop continues so the model can self-correct.
- Max turns is checked per iteration; exceeding it yields `AgentError`.

### 5.6 Message conversion (`convert.rs`)

```rust
pub fn convert_to_context(
    system_prompt: &Option<String>,
    messages: &[AgentMessage],
    tools: &[AgentTool],
) -> Context;
```

- `SystemPrompt` → `Context.system_prompt` (first one found wins)
- `UserText` → `Message::User { content: [Text { text }] }`
- `Assistant` → `Message::Assistant { content: assistant.message.content }` (passthrough)
- `ToolResult` → `Message::ToolResult { tool_call_id, content }`
  ToolResult messages are NOT coalesced here — coalescing is handled in the
  Anthropic provider's `convert.rs` when building the Anthropic-specific
  request JSON. The LLM `Context` uses distinct `ToolResult` messages.

Tools → `Context.tools` via `AgentTool.name/description/parameters`.

### 5.7 Test ApiProvider

For offline agent loop tests, we register a test `ApiProvider` that:

- Holds a queue of scripted responses: `Vec<(Vec<AssistantMessageEvent>, StopReason)>`
- Each `stream()` call pops the next script, replays its events, and yields
  a `Done` with the specified stop reason.
- The test provider ignores model/context/opts — it just replays its queue.
- Queue exhaustion → yields `Error("no more scripted responses")`.

This is simpler than adapting the faux provider and gives full control over
multi-turn agent scenarios.

### 5.8 Public API (`lib.rs`)

```rust
pub use agent::Agent;
pub use types::{
    AgentMessage, AgentTool, AgentConfig, AgentEvent, AgentStream, ToolFn,
};
```

## 6. Data flow

```
caller
  → agent.prompt("hello")
       → (under lock) push UserText
       → run_loop(Arc<RwLock<AgentState>>)
            → convert_to_context(...) → Context
            → pi_ai::stream_model(model, ctx, opts) → AssistantMessageEvent*
                 → yield AgentEvent::LlmEvent(event)
            → save Assistant to state.messages
            → on Done { ToolUse }:
                 for each tool_call:
                   yield ToolCallStart { id, name }
                   result = tool.execute(arguments)
                   yield ToolCallEnd { id, result }
                   save ToolResult to state.messages
                 → loop back
            → on Done { Stop|Length }:
                 yield AgentDone { message }
  → caller iterates AgentEvent stream
  → caller calls agent.messages() to inspect final state
```

## 7. Error handling

| Failure | Behavior |
|---------|----------|
| LLM returns `Error` event | `AgentError`, loop ends |
| LLM stream ends without `Done` | `AgentError("no Done event")`, loop ends |
| `max_turns` exceeded | `AgentError("max turns exceeded")`, loop ends |
| `abort()` called | `AgentError("aborted")`, loop + LLM stream end |
| `tool.execute()` returns `Err` | `ToolCallEnd(Err(...))`, error text saved as tool result, loop continues |
| Unknown tool name | `ToolCallEnd(Err("unknown tool: ..."))`, error text saved, loop continues |

## 8. Testing strategy (all offline)

1. **Message conversion** — `AgentMessage` → LLM `Context` produces correct
   `Message` variants. SystemPrompt, UserText, Assistant passthrough,
   ToolResult (no coalescing).
2. **Tool execution** — success path, error path, unknown tool name all yield
   correct `ToolCallStart/End` events and save appropriate `ToolResult` messages.
3. **Agent loop** — via test `ApiProvider`:
   - Single turn text → `AgentDone { stop: Stop }`, assistant saved to messages.
   - Turn with tool call → `ToolCallStart/End`, then final `AgentDone`, all
     intermediate messages saved.
   - Multi-turn (text → tool → text) → full event sequence.
   - Max turns exceeded → `AgentError`.
   - Abort mid-loop → `AgentError("aborted")`, check `cancel_token` propagated.
   - LLM `Error` event → `AgentError`.
4. **Agent state** — `add_tool`, `add_message`, `messages()` return correct
   snapshot. After `prompt()`, `messages()` includes the assistant response.

## 9. Dependencies

`Cargo.toml` must explicitly declare:

```toml
[dependencies]
pi-ai = { path = "../pi-ai" }
futures = "0.3"
async-stream = "0.3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

Note: `serde`/`serde_json` are not automatically available through `pi-ai` —
Rust does not expose transitive dependencies. They must be listed explicitly.

## 10. Risks

- **Test provider fidelity** — the agent loop's correctness depends on
  scripted LLM responses matching realistic event streams. Mitigation: write
  tests that mirror the exact `AssistantMessageEvent` sequences produced by
  pi-ai's Anthropic provider integration tests.
- **Arc<RwLock> contention** — the loop holds the read lock to build Context
  and the write lock to push messages. The lock is held briefly, with the LLM
  call outside the lock. Low risk.
- **ToolFn type verbosity** — acceptable for PoC. Future phase can add a
  `tool_fn!` macro or a `Tool` trait.

## 11. Future phases (not part of this PoC)

- `continue_loop()` for manual resume after aborted/interrupted turns.
- Parallel tool execution with `ToolExecutionMode`.
- `beforeToolCall`/`afterToolCall` hooks.
- Compaction, session storage, system prompt management, skill loading.
- `StreamFn` trait for injectable LLM function.
- Steering / follow-up message queues.
