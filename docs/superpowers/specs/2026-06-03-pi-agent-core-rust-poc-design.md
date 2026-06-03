# Design: Rust port of `pi-agent-core` (proof-of-concept)

- Date: 2026-06-03
- Status: Design approved
- Scope: Agent runtime crate as phase 2 of the `pi` monorepo Rust port.
- Depends on: `pi-ai` (phase 1, complete)

## 1. Context

`pi-ai` is done — it provides `stream()`/`complete()` over a provider abstraction,
with Anthropic + faux providers, verified via 59 offline tests.

`pi-agent-core` (~8K LOC in TypeScript) is the agent runtime: a tool-calling loop
that manages message state, executes tools, and emits lifecycle events through a
stream. It builds on `pi-ai`'s LLM streaming API.

This design covers a **proof-of-concept port** — not the full TS feature set.

## 2. Goals & success criteria

Build a `pi-agent-core` Rust crate that implements a stateful Agent with a
tool-calling loop over `pi-ai`'s streaming API.

The PoC is **done** when:

1. `cargo build -p pi-agent-core` and the whole workspace build cleanly on Rust 1.96 / edition 2024.
2. The offline test suite passes with **no network access and no credentials**:
   - Agent loop unit tests driven by the faux provider.
   - Tool execution tests (success, error, not-found).
   - Message conversion tests (AgentMessage → LLM Context).
   - Multi-turn loop tests (stop after max_turns, stop on `Stop`/`Length`).
   - Agent state tests (add_tool, add_message, messages snapshot).
3. An optional offline example (`examples/faux_agent.rs`) runs a complete agent
   conversation using the faux provider (no key required).

Non-goal: live calls to the real Anthropic API, compaction, session storage,
skill loading, proxy utilities, or parallel tool execution.

## 3. Key decisions

- **Agent is stateful:** an `Agent` struct owns messages, tools, and config.
  `prompt()` adds a user message and runs the loop; `continue_loop()` resumes
  after a `ToolUse` stop.
- **Sequential tool execution:** tools are called one at a time in the order
  the model outputs them. No parallel execution, no `beforeToolCall`/`afterToolCall`
  hooks. Tools are `async Fn(serde_json::Value) -> Result<Vec<ContentBlock>, String>`.
- **Direct pi-ai integration:** the agent loop calls `pi_ai::stream_model()`
  directly. No injectable `StreamFn` trait — the faux provider handles all offline
  testing through the existing pi-ai registry.
- **EventStream notifications:** the loop yields `AgentEvent` variants through a
  stream. No internal subscription system.
- **No message queues:** the caller manages messages via `add_message()`. No
  steering/follow-up queue injection.
- **Minimal AgentMessage:** a simple enum wrapping pi-ai types with a `message_id`
  field. No full metadata (agent_id, timestamp, etc.).

## 4. Scope

### In scope
- `Agent` struct with `new()`, `add_tool()`, `add_message()`, `messages()`,
  `prompt()`, `continue_loop()`, `abort()`.
- `AgentMessage` enum (UserText, Assistant, ToolResult, SystemPrompt) with
  `message_id`.
- `AgentTool` with `name`, `description`, `parameters` (JSON Schema), and an
  async `execute` function via `Arc<dyn Fn>`.
- `AgentConfig` (model, system_prompt, max_turns, stream_options).
- `AgentEvent` enum: `TurnStart`, `LlmEvent` (transparent passthrough),
  `ToolCallStart/End`, `AgentDone`, `AgentError`.
- `AgentStream` type alias (`Pin<Box<dyn Stream<Item = AgentEvent> + Send>>`).
- Message conversion: `AgentMessage` → `pi_ai::Message` for LLM context building.
- Consecutive `ToolResult` coalescing (ported from pi-ai's Anthropic convert).
- Integration tests via faux provider.
- Offline example `examples/faux_agent.rs`.

### Out of scope (explicitly)
- Parallel tool execution, `beforeToolCall`/`afterToolCall` hooks.
- Compaction (context window pruning).
- Session storage (save/load agent state).
- System prompt management / skill loading.
- Steering / follow-up message queues.
- Proxy utilities, dynamic API key resolution.
- Injectable `StreamFn` trait.

## 5. Architecture

### 5.1 Crate layout (`crates/pi-agent-core`)

```
crates/pi-agent-core/
  Cargo.toml
  src/
    lib.rs           # public API: re-exports
    agent.rs         # Agent struct + AgentStream type
    types.rs         # AgentMessage, AgentTool, AgentConfig, AgentEvent
    agent_loop.rs    # core loop algorithm (internal)
    convert.rs       # AgentMessage -> LLM Context conversion
  tests/
    agent_loop.rs    # loop tests via faux provider
    convert.rs       # message conversion tests
    tool_exec.rs     # tool execution tests
  examples/
    faux_agent.rs    # offline example
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
    LlmEvent(AssistantMessageEvent),
    ToolCallStart { tool_call_id: String, tool_name: String },
    ToolCallEnd { tool_call_id: String, result: Result<Vec<ContentBlock>, String> },
    AgentDone { message: AssistantMessage },
    AgentError { error: String },
}

pub type AgentStream = Pin<Box<dyn Stream<Item = AgentEvent> + Send>>;
```

### 5.3 Agent struct (`agent.rs`)

```rust
pub struct Agent {
    messages: Vec<AgentMessage>,
    tools: Vec<AgentTool>,
    config: AgentConfig,
    cancelled: bool,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self;
    pub fn add_tool(&mut self, tool: AgentTool);
    pub fn add_message(&mut self, msg: AgentMessage);
    pub fn messages(&self) -> &[AgentMessage];

    pub fn prompt(&mut self, text: &str) -> AgentStream;
    pub fn continue_loop(&mut self) -> AgentStream;
    pub fn abort(&mut self);
}
```

`prompt()` pushes a `UserText` message then runs the loop.
`continue_loop()` checks that the last agent message is an `Assistant` with
`ToolUse` stop reason, then continues. Both produce an `AgentStream`.

### 5.4 Agent loop (`agent_loop.rs`)

Internal function `run_loop(model, messages, tools, config, cancel) -> AgentStream`.

Algorithm:

```
turn = 0
loop:
  turn += 1
  if turn > max_turns → yield AgentError("max turns exceeded"), return
  yield TurnStart { turn }

  // 1. Convert AgentMessages → LLM Context
  ctx = convert_to_context(system_prompt, messages, tools)

  // 2. Call LLM via pi-ai
  stream = pi_ai::stream_model(&model, ctx, stream_options)
  for each event in stream:
    yield LlmEvent(event)
    if Done → extract assistant_message, break
    if Error → yield AgentError(error), return

  // 3. Check stop reason
  match assistant_message.stop_reason:
    Stop | Length → yield AgentDone(message), return
    Error        → yield AgentError(...), return
    ToolUse      →
      save Assistant message to messages list

      // 4. Execute tools sequentially
      for each tool_call in assistant_message.content:
        tool = find tool by name, or yield ToolCallEnd(Err("unknown tool"))
        yield ToolCallStart { id, name }
        result = await tool.execute(tool_call.arguments)
        yield ToolCallEnd { id, result }
        push ToolResult to messages list

      // 5. Back to step 1
```

**Cancellation:** `abort()` sets a flag. The loop checks it at iteration start
and yields `AgentError("aborted")`.

**Error handling:**
- LLM `Error` event → `AgentError`, loop ends.
- `tool.execute()` returns `Err` → `ToolCallEnd(result: Err(...))`, tool result
  is pushed as error content, loop continues (model can self-correct).
- Unknown tool name → `ToolCallEnd(result: Err("unknown tool: ..."))`.
- Stream ends without `Done` → `AgentError("stream ended without Done")`.

### 5.5 Message conversion (`convert.rs`)

`pub fn convert_to_context(system_prompt, messages, tools) -> Context`

- `AgentMessage::SystemPrompt` → `Context.system_prompt`
- `AgentMessage::UserText` → `Message::User { content: [Text { text }] }`
- `AgentMessage::Assistant` → `Message::Assistant { content: [...] }` (pass through)
- `AgentMessage::ToolResult` → `Message::ToolResult { tool_call_id, content }`
  Consecutive `ToolResult` messages are coalesced into a single `user` turn.

### 5.6 Public API (`lib.rs`)

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
       → run_loop(model, messages, tools, config)
            → convert_to_context(messages, tools) → Context
            → pi_ai::stream_model(model, ctx, opts) → AssistantMessageEvent*
                 → yield AgentEvent::LlmEvent(event)
            → on Done { ToolUse }:
                 for each tool_call:
                   yield AgentEvent::ToolCallStart { id, name }
                   result = tool.execute(arguments)
                   yield AgentEvent::ToolCallEnd { id, result }
                 → loop back to convert_to_context (with new ToolResult messages)
            → on Done { Stop }:
                 yield AgentEvent::AgentDone { message }
  → consumer iterates AgentEvent stream
```

## 7. Error handling

- All failures after the loop starts are yielded as `AgentEvent::AgentError`
  (LLM errors, max turns exceeded, cancellation, unknown tools).
- Tool execution errors become `ToolCallEnd { result: Err(...) }` — they do
  NOT terminate the loop. The error content is passed back to the model as
  tool result so it can self-correct.
- Invalid state (calling `continue_loop` without a prior `ToolUse` stop)
  produces an `AgentError` event immediately.

## 8. Testing strategy (all offline)

1. **Message conversion** — `AgentMessage` → `Context` produces correct LLM
   messages, tool-result coalescing works.
2. **Tool execution** — success path, error path, unknown tool name → error
   content; verify `ToolCallStart`/`ToolCallEnd` event sequence.
3. **Agent loop** — run complete agent conversations through the faux provider:
   - Single turn with text response → `AgentDone { stop: Stop }`
   - Turn with tool call → `ToolCallStart/End` events, then final `AgentDone`
   - Multi-turn (tool → text) → correct event sequence
   - Max turns exceeded → `AgentError`
   - Abort mid-loop → `AgentError("aborted")`
4. **Agent state** — `add_tool`, `add_message`, `messages()` reflect mutations.
5. **Faux provider** — end-to-end: create Agent with tools, `prompt()` with
   faux provider yields full event stream.

## 9. Dependencies

- Rust 1.96.0, edition 2024.
- `pi-ai` (path dependency from workspace).
- `tokio`, `futures`, `async-stream`, `serde`, `serde_json` (transitive via
  pi-ai; `tokio` + `futures` for async test runtime).

## 10. Risks

- **Agent loop complexity grows with features withheld for later phases**
  (compaction, queues). Mitigation: clean separation between `Agent` public API
  and internal `run_loop` function — easy to extend later.
- **Faux provider test fidelity** depends on pi-ai's faux provider being
  sufficiently scriptable. Mitigation: already verified in pi-ai PoC (3 faux
  e2e tests pass).
- **ToolFn type ergonomics** (`Arc<dyn Fn(...) -> Pin<Box<dyn Future<...>>>>`)
  is verbose. Mitigation: a `tool_fn!` macro in future phase. For PoC, the
  verbosity is acceptable with `async move |args| { ... }.boxed()` pattern.

## 11. Future phases (not part of this PoC)

- Parallel tool execution with `ToolExecutionMode`.
- `beforeToolCall`/`afterToolCall` hooks.
- Compaction (context window pruning via `transformContext`).
- Session storage (serialize/deserialize Agent state).
- `StreamFn` trait for injectable LLM function.
- Steering / follow-up message queues.
- Skill loading and system prompt management.
