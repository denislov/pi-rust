# pi-agent-core Rust PoC Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a `pi-agent-core` Rust crate with a stateful Agent that runs a tool-calling loop over `pi-ai`'s streaming API, verified via offline tests.

**Architecture:** `Agent` wraps `Arc<RwLock<AgentState>>` for shared mutable state. `prompt()` adds user text and runs the full loop — LLM call → tool execution → repeat — until the model stops. Events streamed via `AgentStream`. Tests use a dedicated test `ApiProvider` with scripted responses.

**Tech Stack:** Rust 1.96.0, edition 2024, pi-ai (workspace dep), tokio, futures, async-stream, serde, serde_json.

**Prerequisite:** Enhance pi-ai's faux provider to support per-call response queues and custom stop reasons. Task 1 below handles this before pi-agent-core work begins.

---

### Task 1: Enhance pi-ai faux provider for multi-turn agent testing

**Files:**
- Modify: `crates/pi-ai/src/providers/faux.rs`

**What to build:** The current faux provider always yields `Done { reason: Stop }` and replays all responses in a single call. We need per-call response queues and custom `stop_reason` so the agent loop can see `ToolUse`.

- [ ] **Step 1: Replace faux.rs**

```rust
use std::sync::Mutex;
use async_stream::stream;
use crate::registry::ApiProvider;
use crate::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Model,
    StopReason, StreamOptions, Usage,
};
use crate::stream::EventStream;

pub struct FauxProvider {
    pub responses: Mutex<FauxState>,
}

pub struct FauxState {
    /// Queue of per-call responses. Each call to stream() pops the first entry.
    pub call_queue: Vec<FauxCall>,
    /// Default responses used when call_queue is empty (backward compat).
    pub default_responses: Vec<FauxResponse>,
}

pub struct FauxCall {
    pub responses: Vec<FauxResponse>,
    pub stop_reason: StopReason,
}

pub struct FauxResponse {
    pub text_deltas: Vec<String>,
    pub thinking_deltas: Vec<String>,
    pub tool_calls: Vec<FauxToolCall>,
}

pub struct FauxToolCall {
    pub id: String,
    pub name: String,
    pub deltas: Vec<String>,
    pub final_arguments: serde_json::Value,
}

impl FauxProvider {
    pub fn new(responses: Vec<FauxResponse>) -> Self {
        Self {
            responses: Mutex::new(FauxState {
                call_queue: vec![],
                default_responses: responses,
            }),
        }
    }

    /// Create a provider with a queue of per-call responses.
    /// Each stream() call pops the next FauxCall and replays it.
    pub fn with_call_queue(calls: Vec<FauxCall>) -> Self {
        Self {
            responses: Mutex::new(FauxState {
                call_queue: calls,
                default_responses: vec![],
            }),
        }
    }

    pub fn simple_text(text: &str) -> Self {
        Self::new(vec![FauxResponse {
            text_deltas: vec![text.to_string()],
            thinking_deltas: vec![],
            tool_calls: vec![],
        }])
    }

    /// Create a single faux call with the given responses and stop reason.
    pub fn single_call(responses: Vec<FauxResponse>, stop_reason: StopReason) -> FauxCall {
        FauxCall { responses, stop_reason }
    }

    /// Create a text-only faux call with the given stop reason.
    pub fn text_call(text: &str, stop_reason: StopReason) -> FauxCall {
        FauxCall {
            responses: vec![FauxResponse {
                text_deltas: vec![text.to_string()],
                thinking_deltas: vec![],
                tool_calls: vec![],
            }],
            stop_reason,
        }
    }
}

impl ApiProvider for FauxProvider {
    fn stream(&self, model: &Model, _ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        let (responses, stop_reason) = {
            let mut state = self.responses.lock().unwrap();
            if let Some(call) = state.call_queue.first().cloned() {
                state.call_queue.remove(0);
                (call.responses, call.stop_reason)
            } else {
                (state.default_responses.clone(), StopReason::Stop)
            }
        };
        let model_id = model.id.clone();
        Box::pin(stream! {
            let mut partial = AssistantMessage::empty("faux", &model_id);
            partial.provider = Some("faux".into());
            partial.timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            yield AssistantMessageEvent::Start { partial: partial.clone() };

            for resp in &responses {
                if !resp.text_deltas.is_empty() {
                    let mut p = partial.clone();
                    p.content.push(ContentBlock::Text {
                        text: resp.text_deltas.join(""),
                        text_signature: None,
                    });
                    yield AssistantMessageEvent::TextStart { partial: p };
                    for delta in &resp.text_deltas {
                        if let Some(ContentBlock::Text { text, .. }) = partial.content.last_mut() {
                            text.push_str(delta);
                        }
                        yield AssistantMessageEvent::TextDelta {
                            delta: delta.clone(),
                            partial: partial.clone(),
                        };
                    }
                    yield AssistantMessageEvent::TextEnd { partial: partial.clone() };
                }

                if !resp.thinking_deltas.is_empty() {
                    let mut p = partial.clone();
                    p.content.push(ContentBlock::Thinking {
                        thinking: resp.thinking_deltas.join(""),
                        thinking_signature: None,
                        redacted: None,
                    });
                    yield AssistantMessageEvent::ThinkingStart { partial: p };
                    for delta in &resp.thinking_deltas {
                        yield AssistantMessageEvent::ThinkingDelta {
                            delta: delta.clone(),
                            partial: partial.clone(),
                        };
                    }
                    yield AssistantMessageEvent::ThinkingEnd { partial: partial.clone() };
                }

                for tc in &resp.tool_calls {
                    let mut p = partial.clone();
                    p.content.push(ContentBlock::ToolCall {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        arguments: tc.final_arguments.clone(),
                        thought_signature: None,
                    });
                    yield AssistantMessageEvent::ToolcallStart { partial: p };
                    let mut accumulated = String::new();
                    for delta in &tc.deltas {
                        accumulated.push_str(delta);
                        if let Some(ContentBlock::ToolCall { arguments, .. }) =
                            partial.content.last_mut()
                        {
                            *arguments = serde_json::json!(accumulated);
                        }
                        yield AssistantMessageEvent::ToolcallDelta {
                            delta: delta.clone(),
                            partial: partial.clone(),
                        };
                    }
                    yield AssistantMessageEvent::ToolcallEnd { partial: partial.clone() };
                }
            }

            partial.usage = Usage {
                input: 10, output: 20, total_tokens: 30,
                ..Default::default()
            };
            partial.stop_reason = stop_reason.clone();

            yield AssistantMessageEvent::Done {
                reason: stop_reason,
                message: partial,
            };
        })
    }
}
```

Note: the `FauxToolCall` struct and streaming logic remain unchanged from the existing implementation — only `FauxState`, `FauxCall`, and `ApiProvider::stream()` are modified.

- [ ] **Step 2: Run existing tests to verify no regression**

Run: `cargo test -p pi-ai -- --nocapture`
Expected: all 59 tests still pass. The existing faux tests (`tests/faux.rs`) must continue to work because the default path (empty `call_queue`) falls back to `default_responses` with `StopReason::Stop`.

- [ ] **Step 3: Add a quick smoke test for the new call queue**

Append to `crates/pi-ai/tests/faux.rs`:

```rust
#[tokio::test]
async fn faux_call_queue_with_tool_use() {
    use pi_ai::providers::faux::{FauxProvider, FauxCall, FauxToolCall};
    use std::sync::Arc;

    let provider = Arc::new(FauxProvider::with_call_queue(vec![
        FauxCall {
            responses: vec![FauxResponse {
                text_deltas: vec![],
                thinking_deltas: vec![],
                tool_calls: vec![FauxToolCall {
                    id: "toolu_01".into(),
                    name: "search".into(),
                    deltas: vec!["{\"q\":".into(), "\"rust\"}".into()],
                    final_arguments: serde_json::json!({"q": "rust"}),
                }],
            }],
            stop_reason: pi_ai::types::StopReason::ToolUse,
        },
    ]));
    pi_ai::registry::register("faux-call-queue", provider);

    let model = Model {
        id: "faux-model".into(), name: "Faux".into(),
        api: "faux-call-queue".into(), provider: "faux".into(),
        base_url: "".into(), reasoning: false,
        input: 0.0, output: 0.0, cache_read: None, cache_write: None,
        context_window: 0, max_tokens: None, headers: None,
    };
    let ctx = Context { system_prompt: None, messages: vec![], tools: None };

    use futures::StreamExt;
    let mut stream = pi_ai::registry::stream_model(&model, ctx, None);
    let events: Vec<_> = stream.collect().await;

    let last = events.last().unwrap();
    match last {
        AssistantMessageEvent::Done { reason, .. } => {
            assert_eq!(*reason, StopReason::ToolUse);
        }
        other => panic!("expected Done with ToolUse, got {:?}", other),
    }

    pi_ai::registry::unregister("faux-call-queue");
}
```

Note: You'll need to import `FauxResponse` and `FauxToolCall` at the top of the file if not already imported, and add `use pi_ai::types::{Model, Context, AssistantMessageEvent, StopReason};`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p pi-ai -- faux --nocapture`
Expected: 4 faux tests pass (3 existing + 1 new call_queue test).

- [ ] **Step 5: Commit**

```bash
git add crates/pi-ai/src/providers/faux.rs crates/pi-ai/tests/faux.rs
git commit -m "feat(pi-ai): enhance faux provider with per-call queues and custom stop_reason"
```

---

### Task 2: pi-agent-core Cargo.toml and lib.rs stub

**Files:**
- Modify: `crates/pi-agent-core/Cargo.toml`
- Modify: `crates/pi-agent-core/src/lib.rs`

- [ ] **Step 1: Replace Cargo.toml**

```toml
[package]
name = "pi-agent-core"
version = "0.1.0"
edition = "2024"

[dependencies]
pi-ai = { path = "../pi-ai" }
futures = "0.3"
async-stream = "0.3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

- [ ] **Step 2: Replace lib.rs**

```rust
pub mod types;
pub mod agent;
pub mod convert;
pub mod agent_loop;

pub use agent::Agent;
pub use types::{
    AgentMessage, AgentTool, AgentConfig, AgentEvent, AgentStream, ToolFn,
};
```

- [ ] **Step 3: Verify dependency resolution**

Run: `cargo check -p pi-agent-core 2>&1`
Expected: fails on missing modules (types.rs, agent.rs, etc.) — that's fine, they're coming in later tasks.

- [ ] **Step 4: Verify workspace still builds**

Run: `cargo build -p pi-ai`
Expected: pi-ai still builds cleanly (no breakage from Task 1 changes).

- [ ] **Step 5: Commit**

```bash
git add crates/pi-agent-core/
git commit -m "feat(pi-agent-core): add dependencies and lib.rs skeleton"
```

---

### Task 3: Agent types

**Files:**
- Create: `crates/pi-agent-core/src/types.rs`

**What to build:** `AgentMessage`, `AgentTool`, `AgentConfig`, `AgentEvent`, `AgentStream`, `ToolFn`.

- [ ] **Step 1: Write types.rs**

```rust
use std::pin::Pin;
use std::sync::Arc;
use futures::Stream;
use pi_ai::types::{AssistantMessage, AssistantMessageEvent, ContentBlock, Model, StreamOptions};

// ── AgentMessage ───────────────────────────────────

#[derive(Debug, Clone)]
pub enum AgentMessage {
    UserText {
        message_id: String,
        text: String,
    },
    Assistant {
        message_id: String,
        message: AssistantMessage,
    },
    ToolResult {
        message_id: String,
        tool_call_id: String,
        content: Vec<ContentBlock>,
    },
    SystemPrompt {
        message_id: String,
        text: String,
    },
}

// ── AgentTool ──────────────────────────────────────

pub type ToolFn = Arc<
    dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = Result<Vec<ContentBlock>, String>> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub struct AgentTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub execute: ToolFn,
}

// ── AgentConfig ────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub model: Model,
    pub system_prompt: Option<String>,
    pub max_turns: u32,
    pub stream_options: Option<StreamOptions>,
}

// ── AgentEvent ─────────────────────────────────────

#[derive(Debug)]
pub enum AgentEvent {
    TurnStart { turn: u32 },
    LlmEvent(AssistantMessageEvent),
    ToolCallStart { tool_call_id: String, tool_name: String },
    ToolCallEnd { tool_call_id: String, result: Result<Vec<ContentBlock>, String> },
    AgentDone { message: AssistantMessage },
    AgentError { error: String },
}

// ── AgentStream ────────────────────────────────────

pub type AgentStream = Pin<Box<dyn Stream<Item = AgentEvent> + Send>>;

// ── Unit tests ─────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;

    fn make_text_tool() -> AgentTool {
        AgentTool {
            name: "echo".into(),
            description: "echoes input".into(),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
            execute: Arc::new(|args| {
                let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("no text");
                let result: Vec<ContentBlock> = vec![ContentBlock::Text {
                    text: text.to_string(),
                    text_signature: None,
                }];
                Box::pin(async move { Ok(result) })
            }),
        }
    }

    #[test]
    fn agent_message_user_text_constructs() {
        let msg = AgentMessage::UserText {
            message_id: "1".into(),
            text: "hello".into(),
        };
        match &msg {
            AgentMessage::UserText { text, .. } => assert_eq!(text, "hello"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn agent_tool_has_correct_fields() {
        let tool = make_text_tool();
        assert_eq!(tool.name, "echo");
        assert!(tool.description.contains("echoes"));
    }

    #[tokio::test]
    async fn tool_fn_executes() {
        let tool = make_text_tool();
        let result = (tool.execute)(serde_json::json!({"text": "hi"})).await;
        assert!(result.is_ok());
        let blocks = result.unwrap();
        assert_eq!(blocks.len(), 1);
    }
}
```

- [ ] **Step 2: Verify compilation and tests**

Run: `cargo test -p pi-agent-core -- --nocapture`
Expected: 3 tests pass (agent_message, agent_tool, tool_fn). Compilation errors from missing modules (agent.rs, convert.rs, agent_loop.rs) — these are expected; `cargo test` will only compile types.

- [ ] **Step 3: Commit**

```bash
git add crates/pi-agent-core/src/types.rs
git commit -m "feat(pi-agent-core): add agent types (AgentMessage, AgentTool, AgentConfig, AgentEvent)"
```

---

### Task 4: Agent struct with shared state

**Files:**
- Create: `crates/pi-agent-core/src/agent.rs`

**What to build:** `AgentState` and `Agent` struct wrapping `Arc<RwLock<AgentState>>`. All methods take `&self`.

- [ ] **Step 1: Write agent.rs**

```rust
use std::sync::{Arc, RwLock};
use tokio_util::sync::CancellationToken;
use crate::types::{AgentMessage, AgentTool, AgentConfig, AgentEvent, AgentStream};
use crate::agent_loop;

pub struct AgentState {
    pub messages: Vec<AgentMessage>,
    pub tools: Vec<AgentTool>,
    pub config: AgentConfig,
    pub cancel_token: CancellationToken,
}

pub struct Agent {
    state: Arc<RwLock<AgentState>>,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(AgentState {
                messages: Vec::new(),
                tools: Vec::new(),
                cancel_token: CancellationToken::new(),
                config,
            })),
        }
    }

    pub fn add_tool(&self, tool: AgentTool) {
        self.state.write().unwrap().tools.push(tool);
    }

    pub fn add_message(&self, msg: AgentMessage) {
        self.state.write().unwrap().messages.push(msg);
    }

    pub fn messages(&self) -> Vec<AgentMessage> {
        self.state.read().unwrap().messages.clone()
    }

    /// Adds a UserText message and runs the full tool-calling loop.
    /// Returns an AgentStream that yields events until the model stops
    /// or an error occurs.
    pub fn prompt(&self, text: &str) -> AgentStream {
        self.state.write().unwrap().messages.push(AgentMessage::UserText {
            message_id: format!("user_{}", self.state.read().unwrap().messages.len()),
            text: text.to_string(),
        });
        agent_loop::run_loop(self.state.clone())
    }

    /// Cancels an in-flight loop. Safe to call from another task.
    pub fn abort(&self) {
        self.state.read().unwrap().cancel_token.cancel();
    }
}
```

- [ ] **Step 2: Verify compilation (agent_loop.rs is next task, so expect errors from missing module)**

Run: `cargo check -p pi-agent-core 2>&1 | head -10`
Expected: error about missing `agent_loop` module. This is fine — agent_loop.rs is Task 6. But agent.rs itself must have correct syntax.

Actually, this won't compile because `agent_loop` module doesn't exist. Let's stub it temporarily so we can verify agent.rs compiles.

- [ ] **Step 2a: Create minimal agent_loop.rs stub**

Create `crates/pi-agent-core/src/agent_loop.rs` with:

```rust
use std::sync::{Arc, RwLock};
use crate::agent::AgentState;
use crate::types::AgentStream;

pub fn run_loop(_state: Arc<RwLock<AgentState>>) -> AgentStream {
    unimplemented!()
}
```

- [ ] **Step 2b: Verify compilation**

Run: `cargo check -p pi-agent-core 2>&1`
Expected: compiles (agent.rs + agent_loop stub). Still may have error about missing `convert` module — if so, create `crates/pi-agent-core/src/convert.rs` with `pub fn convert_to_context() -> pi_ai::types::Context { unimplemented!() }` (minimal stub).

- [ ] **Step 3: Commit**

```bash
git add crates/pi-agent-core/src/agent.rs crates/pi-agent-core/src/agent_loop.rs
git commit -m "feat(pi-agent-core): add Agent struct with Arc<RwLock<AgentState>>"
```

---

### Task 5: Message conversion

**Files:**
- Create: `crates/pi-agent-core/src/convert.rs`

**What to build:** `convert_to_context()` — transforms `AgentMessage`s into pi-ai's `Context` for LLM calls. No ToolResult coalescing (that's in Anthropic's convert layer).

- [ ] **Step 1: Write convert.rs**

```rust
use pi_ai::types::{ContentBlock, Context, Message, Tool};
use crate::types::{AgentMessage, AgentTool};

/// Convert agent messages and tools into an LLM Context.
/// ToolResult messages are converted individually (no coalescing).
pub fn convert_to_context(
    system_prompt: &Option<String>,
    messages: &[AgentMessage],
    tools: &[AgentTool],
) -> Context {
    let llm_messages: Vec<Message> = messages
        .iter()
        .filter_map(|msg| match msg {
            AgentMessage::UserText { text, .. } => Some(Message::User {
                content: vec![ContentBlock::Text {
                    text: text.clone(),
                    text_signature: None,
                }],
            }),
            AgentMessage::Assistant { message, .. } => Some(Message::Assistant {
                content: message.content.clone(),
            }),
            AgentMessage::ToolResult { tool_call_id, content, .. } => Some(Message::ToolResult {
                tool_call_id: tool_call_id.clone(),
                content: content.clone(),
            }),
            AgentMessage::SystemPrompt { .. } => None, // handled as system_prompt below
        })
        .collect();

    let system = {
        let configured = system_prompt.clone();
        // Also check messages for SystemPrompt (first one wins)
        let from_messages = messages.iter().find_map(|m| match m {
            AgentMessage::SystemPrompt { text, .. } => Some(text.clone()),
            _ => None,
        });
        configured.or(from_messages)
    };

    let llm_tools: Option<Vec<Tool>> = if tools.is_empty() {
        None
    } else {
        Some(
            tools
                .iter()
                .map(|t| Tool {
                    name: t.name.clone(),
                    description: Some(t.description.clone()),
                    parameters: t.parameters.clone(),
                })
                .collect(),
        )
    };

    Context {
        system_prompt: system,
        messages: llm_messages,
        tools: llm_tools,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assistant_msg() -> pi_ai::types::AssistantMessage {
        pi_ai::types::AssistantMessage::empty("test", "test-model")
    }

    #[test]
    fn user_text_becomes_user_message() {
        let msgs = vec![AgentMessage::UserText {
            message_id: "1".into(),
            text: "hello".into(),
        }];
        let ctx = convert_to_context(&None, &msgs, &[]);
        assert_eq!(ctx.messages.len(), 1);
        match &ctx.messages[0] {
            Message::User { content } => {
                match &content[0] {
                    ContentBlock::Text { text, .. } => assert_eq!(text, "hello"),
                    _ => panic!("expected text block"),
                }
            }
            _ => panic!("expected user message"),
        }
    }

    #[test]
    fn assistant_passthrough() {
        let am = assistant_msg();
        let msgs = vec![AgentMessage::Assistant {
            message_id: "2".into(),
            message: am.clone(),
        }];
        let ctx = convert_to_context(&None, &msgs, &[]);
        assert_eq!(ctx.messages.len(), 1);
        match &ctx.messages[0] {
            Message::Assistant { content } => {
                assert_eq!(*content, am.content);
            }
            _ => panic!("expected assistant message"),
        }
    }

    #[test]
    fn tool_result_becomes_tool_result_message() {
        let msgs = vec![AgentMessage::ToolResult {
            message_id: "3".into(),
            tool_call_id: "call_1".into(),
            content: vec![ContentBlock::Text {
                text: "result".into(),
                text_signature: None,
            }],
        }];
        let ctx = convert_to_context(&None, &msgs, &[]);
        assert_eq!(ctx.messages.len(), 1);
        match &ctx.messages[0] {
            Message::ToolResult { tool_call_id, content } => {
                assert_eq!(tool_call_id, "call_1");
                assert_eq!(content.len(), 1);
            }
            _ => panic!("expected tool result message"),
        }
    }

    #[test]
    fn system_prompt_from_config() {
        let ctx = convert_to_context(&Some("be helpful".into()), &[], &[]);
        assert_eq!(ctx.system_prompt, Some("be helpful".into()));
    }

    #[test]
    fn system_prompt_from_messages() {
        let msgs = vec![AgentMessage::SystemPrompt {
            message_id: "4".into(),
            text: "be concise".into(),
        }];
        let ctx = convert_to_context(&None, &msgs, &[]);
        assert_eq!(ctx.system_prompt, Some("be concise".into()));
    }

    #[test]
    fn config_system_prompt_wins_over_messages() {
        let msgs = vec![AgentMessage::SystemPrompt {
            message_id: "4".into(),
            text: "from messages".into(),
        }];
        let ctx = convert_to_context(&Some("from config".into()), &msgs, &[]);
        assert_eq!(ctx.system_prompt, Some("from config".into()));
    }

    #[test]
    fn tools_converted_to_llm_tools() {
        let tools = vec![AgentTool {
            name: "search".into(),
            description: "search the web".into(),
            parameters: serde_json::json!({"type": "object"}),
            execute: std::sync::Arc::new(|_| Box::pin(async { Ok(vec![]) })),
        }];
        let ctx = convert_to_context(&None, &[], &tools);
        let llm_tools = ctx.tools.unwrap();
        assert_eq!(llm_tools.len(), 1);
        assert_eq!(llm_tools[0].name, "search");
        assert_eq!(llm_tools[0].description, Some("search the web".into()));
    }

    #[test]
    fn empty_tools_produce_none() {
        let ctx = convert_to_context(&None, &[], &[]);
        assert!(ctx.tools.is_none());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p pi-agent-core -- --nocapture`
Expected: 11 tests pass (3 from types.rs + 8 from convert.rs).

- [ ] **Step 3: Commit**

```bash
git add crates/pi-agent-core/src/convert.rs
git commit -m "feat(pi-agent-core): add AgentMessage -> LLM Context conversion"
```

---

### Task 6: Agent loop core

**Files:**
- Replace: `crates/pi-agent-core/src/agent_loop.rs` (currently a stub)

**What to build:** `run_loop()` — the tool-calling loop algorithm. Takes `Arc<RwLock<AgentState>>`, produces `AgentStream`.

- [ ] **Step 1: Replace agent_loop.rs**

```rust
use std::sync::{Arc, RwLock};
use async_stream::stream;
use futures::StreamExt;

use pi_ai::types::{AssistantMessageEvent, ContentBlock, StopReason};
use crate::agent::AgentState;
use crate::convert::convert_to_context;
use crate::types::{AgentEvent, AgentMessage, AgentStream};

/// Run the agent tool-calling loop. Clones the state Arc so the
/// returned stream holds an independent reference.
pub fn run_loop(state: Arc<RwLock<AgentState>>) -> AgentStream {
    Box::pin(stream! {
        let cancel = {
            let s = state.read().unwrap();
            s.cancel_token.clone()
        };

        let mut turn: u32 = 0;

        loop {
            // Check cancellation before each turn
            if cancel.is_cancelled() {
                yield AgentEvent::AgentError { error: "aborted".into() };
                return;
            }

            turn += 1;

            // Check max_turns
            {
                let s = state.read().unwrap();
                if turn > s.config.max_turns {
                    yield AgentEvent::AgentError {
                        error: format!("max turns ({}) exceeded", s.config.max_turns),
                    };
                    return;
                }
            }

            yield AgentEvent::TurnStart { turn };

            // 1. Build LLM Context
            let (ctx, model, mut opts) = {
                let s = state.read().unwrap();
                let ctx = convert_to_context(
                    &s.config.system_prompt,
                    &s.messages,
                    &s.tools,
                );
                let mut opts = s.config.stream_options.clone().unwrap_or_default();
                opts.cancel = Some(cancel.clone());
                (ctx, s.config.model.clone(), opts)
            };

            // 2. Call LLM
            let mut llm_stream = pi_ai::stream_model(&model, ctx, Some(opts));
            let mut assistant_message: Option<pi_ai::types::AssistantMessage> = None;

            while let Some(event) = llm_stream.next().await {
                let is_terminal = matches!(
                    event,
                    AssistantMessageEvent::Done { .. } | AssistantMessageEvent::Error { .. }
                );
                if let AssistantMessageEvent::Done { ref message, .. } = &event {
                    assistant_message = Some(message.clone());
                }
                yield AgentEvent::LlmEvent(event);
                if is_terminal {
                    break;
                }
            }

            let assistant = match assistant_message {
                Some(m) => m,
                None => {
                    yield AgentEvent::AgentError {
                        error: "LLM stream ended without Done event".into(),
                    };
                    return;
                }
            };

            // 3. Save assistant to messages (all branches)
            {
                let mut s = state.write().unwrap();
                s.messages.push(AgentMessage::Assistant {
                    message_id: assistant.response_id.clone().unwrap_or_default(),
                    message: assistant.clone(),
                });
            }

            // 4. Branch on stop reason
            match &assistant.stop_reason {
                StopReason::Stop | StopReason::Length => {
                    yield AgentEvent::AgentDone { message: assistant };
                    return;
                }
                StopReason::Error => {
                    yield AgentEvent::AgentError {
                        error: assistant
                            .error_message
                            .clone()
                            .unwrap_or_else(|| "LLM error".into()),
                    };
                    return;
                }
                StopReason::Aborted => {
                    yield AgentEvent::AgentError { error: "aborted".into() };
                    return;
                }
                StopReason::ToolUse => {
                    // 5. Execute tools sequentially
                    for block in &assistant.content {
                        let (tool_id, tool_name, tool_args) = match block {
                            ContentBlock::ToolCall { id, name, arguments, .. } => {
                                (id.clone(), name.clone(), arguments.clone())
                            }
                            _ => continue,
                        };

                        // Find tool by name
                        let tool = {
                            let s = state.read().unwrap();
                            s.tools.iter().find(|t| t.name == tool_name).cloned()
                        };

                        yield AgentEvent::ToolCallStart {
                            tool_call_id: tool_id.clone(),
                            tool_name: tool_name.clone(),
                        };

                        let result = match &tool {
                            Some(t) => (t.execute)(tool_args).await,
                            None => Err(format!("unknown tool: {}", tool_name)),
                        };

                        yield AgentEvent::ToolCallEnd {
                            tool_call_id: tool_id.clone(),
                            result: result.clone(),
                        };

                        // Save tool result to state
                        let content = match &result {
                            Ok(blocks) => blocks.clone(),
                            Err(e) => vec![ContentBlock::Text {
                                text: e.clone(),
                                text_signature: None,
                            }],
                        };
                        {
                            let mut s = state.write().unwrap();
                            s.messages.push(AgentMessage::ToolResult {
                                message_id: tool_id.clone(),
                                tool_call_id: tool_id.clone(),
                                content,
                            });
                        }
                    }
                    // Loop continues — next iteration will include tool results in context
                }
            }
        }
    })
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p pi-agent-core 2>&1`
Expected: compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add crates/pi-agent-core/src/agent_loop.rs
git commit -m "feat(pi-agent-core): add run_loop() tool-calling algorithm"
```

---

### Task 7: Test ApiProvider for offline agent loop tests

**Files:**
- Create directory: `crates/pi-agent-core/tests/common/`
- Create: `crates/pi-agent-core/tests/common/mod.rs`

**What to build:** A test `ApiProvider` that replays scripted `AssistantMessageEvent` sequences with configurable stop reasons. Used by all agent loop tests.

- [ ] **Step 1: Write tests/common/mod.rs**

```rust
use std::sync::Mutex;
use async_stream::stream;
use futures::StreamExt;
use pi_ai::registry::ApiProvider;
use pi_ai::types::{AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Model, StopReason, StreamOptions};
use pi_ai::stream::EventStream;

/// A scripted LLM response for one turn.
pub struct ScriptedTurn {
    pub events: Vec<AssistantMessageEvent>,
    pub stop_reason: StopReason,
    pub response_id: String,
    pub model_name: String,
}

/// Test ApiProvider that replays scripted turns from a queue.
pub struct TestProvider {
    pub turns: Mutex<Vec<ScriptedTurn>>,
}

impl TestProvider {
    pub fn new(turns: Vec<ScriptedTurn>) -> Self {
        Self { turns: Mutex::new(turns) }
    }
}

impl ApiProvider for TestProvider {
    fn stream(&self, _model: &Model, _ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        let turn = {
            let mut turns = self.turns.lock().unwrap();
            if turns.is_empty() {
                return Box::pin(stream! {
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        error: "no more scripted turns".into(),
                    };
                });
            }
            turns.remove(0)
        };

        Box::pin(stream! {
            // Replay all the turn's events
            for event in &turn.events {
                yield event.clone();
            }

            // Build a Done event with correct stop reason
            let msg = AssistantMessage {
                content: vec![], // content already delivered via events above
                api: "test".into(),
                provider: Some("test".into()),
                model: turn.model_name.clone(),
                response_model: None,
                response_id: Some(turn.response_id.clone()),
                usage: Default::default(),
                stop_reason: turn.stop_reason.clone(),
                error_message: None,
                timestamp: 0,
            };
            yield AssistantMessageEvent::Done {
                reason: turn.stop_reason,
                message: msg,
            };
        })
    }
}

/// Helper: create a simple text-response turn.
pub fn text_turn(text: &str) -> ScriptedTurn {
    let text_block = ContentBlock::Text { text: text.into(), text_signature: None };
    let mut msg = AssistantMessage::empty("test", "test-model");
    msg.content.push(text_block.clone());
    let partial = msg.clone();

    ScriptedTurn {
        events: vec![
            AssistantMessageEvent::Start { partial: partial.clone() },
            AssistantMessageEvent::TextStart {
                partial: {
                    let mut p = partial.clone();
                    p.content = vec![text_block.clone()];
                    p
                },
            },
            AssistantMessageEvent::TextDelta { delta: text.into(), partial: partial.clone() },
            AssistantMessageEvent::TextEnd { partial: partial.clone() },
        ],
        stop_reason: StopReason::Stop,
        response_id: "resp_1".into(),
        model_name: "test-model".into(),
    }
}

/// Helper: create a tool-use turn.
pub fn tool_use_turn(tool_id: &str, tool_name: &str, arguments: serde_json::Value) -> ScriptedTurn {
    let tool_block = ContentBlock::ToolCall {
        id: tool_id.into(),
        name: tool_name.into(),
        arguments: arguments.clone(),
        thought_signature: None,
    };
    let mut msg = AssistantMessage::empty("test", "test-model");
    msg.content.push(tool_block.clone());
    let partial = msg.clone();

    let json_str = arguments.to_string();

    ScriptedTurn {
        events: vec![
            AssistantMessageEvent::Start { partial: partial.clone() },
            AssistantMessageEvent::ToolcallStart {
                partial: {
                    let mut p = partial.clone();
                    p.content = vec![ContentBlock::ToolCall {
                        id: tool_id.into(),
                        name: tool_name.into(),
                        arguments: serde_json::json!({}),
                        thought_signature: None,
                    }];
                    p
                },
            },
            AssistantMessageEvent::ToolcallDelta {
                delta: json_str,
                partial: partial.clone(),
            },
            AssistantMessageEvent::ToolcallEnd { partial: partial.clone() },
        ],
        stop_reason: StopReason::ToolUse,
        response_id: "resp_tool".into(),
        model_name: "test-model".into(),
    }
}
```

- [ ] **Step 2: Verify compilation in test context**

Run: `cargo test -p pi-agent-core --no-run 2>&1 | tail -5`
Expected: compiles (the test_provider module is not a test yet, just utility code). Tests may fail since no `#[test]` functions yet.

- [ ] **Step 3: Commit**

```bash
git add crates/pi-agent-core/tests/
git commit -m "test(pi-agent-core): add test ApiProvider with scripted turns"
```

---

### Task 8: Integration tests — agent loop

**Files:**
- Create: `crates/pi-agent-core/tests/agent_loop.rs`

**What to build:** Agent loop integration tests using the test provider from `tests/common/mod.rs`.

- [ ] **Step 1: Write tests/agent_loop.rs**

```rust
mod common;
use common::{TestProvider, ScriptedTurn, text_turn, tool_use_turn};
use std::sync::Arc;
use futures::StreamExt;
use pi_ai::registry;
use pi_ai::types::{
    AssistantMessageEvent, ContentBlock, Model, StopReason,
};
use pi_agent_core::{Agent, AgentConfig, AgentEvent, AgentMessage, AgentTool};

fn test_model() -> Model {
    Model {
        id: "test-model".into(),
        name: "Test Model".into(),
        api: "test-api".into(),
        provider: "test".into(),
        base_url: "".into(),
        reasoning: false,
        input: 0.0, output: 0.0,
        cache_read: None, cache_write: None,
        context_window: 0, max_tokens: None, headers: None,
    }
}

fn test_config(model: Model) -> AgentConfig {
    AgentConfig {
        model,
        system_prompt: Some("Be helpful.".into()),
        max_turns: 5,
        stream_options: None,
    }
}

#[tokio::test]
async fn single_turn_text_response() {
    let provider = Arc::new(TestProvider::new(vec![
        text_turn("Hello, world!"),
    ]));
    registry::register("test-api", provider);

    let agent = Agent::new(test_config(test_model()));

    let mut stream = agent.prompt("hi");
    let events: Vec<_> = stream.collect().await;

    // Should have AgentDone at the end
    let has_done = events.iter().any(|e| matches!(e, AgentEvent::AgentDone { .. }));
    assert!(has_done, "should have AgentDone event");

    // Should have text delta
    let has_text = events.iter().any(|e| matches!(e, AgentEvent::LlmEvent(
        AssistantMessageEvent::TextDelta { .. }
    )));
    assert!(has_text, "should have text delta event");

    // Agent state should contain assistant response
    let msgs = agent.messages();
    assert_eq!(msgs.len(), 2); // UserText + Assistant
    assert!(matches!(&msgs[0], AgentMessage::UserText { .. }));
    assert!(matches!(&msgs[1], AgentMessage::Assistant { .. }));

    registry::unregister("test-api");
}

#[tokio::test]
async fn tool_use_turn_executes_tool() {
    // Turn 1: tool call → ToolUse
    // Turn 2: text response → Stop
    let provider = Arc::new(TestProvider::new(vec![
        tool_use_turn("tool_1", "echo", serde_json::json!({"text": "hi"})),
        text_turn("Tool executed successfully."),
    ]));
    registry::register("test-api", provider);

    let agent = Agent::new(test_config(test_model()));

    let tool = AgentTool {
        name: "echo".into(),
        description: "echoes input".into(),
        parameters: serde_json::json!({"type": "object", "properties": {"text": {"type": "string"}}}),
        execute: Arc::new(|args| {
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("no text");
            let result = vec![ContentBlock::Text {
                text: format!("echo: {}", text),
                text_signature: None,
            }];
            Box::pin(async move { Ok(result) })
        }),
    };
    agent.add_tool(tool);

    let mut stream = agent.prompt("echo hi");
    let events: Vec<_> = stream.collect().await;

    // Should have tool events
    let has_tool_start = events.iter().any(|e| matches!(e, AgentEvent::ToolCallStart { .. }));
    let has_tool_end = events.iter().any(|e| matches!(e, AgentEvent::ToolCallEnd { .. }));
    assert!(has_tool_start, "should have ToolCallStart");
    assert!(has_tool_end, "should have ToolCallEnd");

    // Should end with AgentDone (from second turn)
    let has_done = events.iter().any(|e| matches!(e, AgentEvent::AgentDone { .. }));
    assert!(has_done, "should have AgentDone");

    // State should have: UserText, Assistant (tool_use), ToolResult, Assistant (text)
    let msgs = agent.messages();
    assert_eq!(msgs.len(), 4);
    assert!(matches!(&msgs[2], AgentMessage::ToolResult { .. }));

    registry::unregister("test-api");
}

#[tokio::test]
async fn unknown_tool_yields_error_content_and_continues() {
    let provider = Arc::new(TestProvider::new(vec![
        tool_use_turn("tool_1", "nonexistent", serde_json::json!({})),
        text_turn("I tried but the tool was not found."),
    ]));
    registry::register("test-api", provider);

    let agent = Agent::new(test_config(test_model()));
    // No tools registered — so "nonexistent" will be unknown

    let mut stream = agent.prompt("use nonexistent tool");
    let events: Vec<_> = stream.collect().await;

    // Should have ToolCallEnd with Err
    let tool_end = events.iter().find_map(|e| match e {
        AgentEvent::ToolCallEnd { result, .. } => Some(result.clone()),
        _ => None,
    }).unwrap();
    assert!(tool_end.is_err());
    assert!(tool_end.unwrap_err().contains("unknown tool"));

    // Should still end with AgentDone (second turn with text)
    let has_done = events.iter().any(|e| matches!(e, AgentEvent::AgentDone { .. }));
    assert!(has_done);

    registry::unregister("test-api");
}

#[tokio::test]
async fn max_turns_exceeded_yields_error() {
    // Provider returns tool_use forever, but max_turns limits it
    let mut turns = Vec::new();
    for _ in 0..10 {
        turns.push(tool_use_turn("tool_1", "echo", serde_json::json!({"text": "x"})));
    }
    let provider = Arc::new(TestProvider::new(turns));
    registry::register("test-api", provider);

    let mut config = test_config(test_model());
    config.max_turns = 2;

    let agent = Agent::new(config);
    let tool = AgentTool {
        name: "echo".into(),
        description: "echo".into(),
        parameters: serde_json::json!({"type": "object"}),
        execute: Arc::new(|_| {
            Box::pin(async {
                Ok(vec![ContentBlock::Text {
                    text: "ok".into(),
                    text_signature: None,
                }])
            })
        }),
    };
    agent.add_tool(tool);

    let mut stream = agent.prompt("go");
    let events: Vec<_> = stream.collect().await;

    let has_error = events.iter().any(|e| matches!(e, AgentEvent::AgentError { error } if error.contains("max turns")));
    assert!(has_error, "should have max turns error");

    registry::unregister("test-api");
}

#[tokio::test]
async fn abort_mid_turn_yields_error() {
    let provider = Arc::new(TestProvider::new(vec![
        text_turn("Hello"), // won't be used, aborted earlier
    ]));
    registry::register("test-api", provider);

    let agent = Agent::new(test_config(test_model()));

    let mut stream = agent.prompt("hi");
    // Abort immediately
    agent.abort();

    let events: Vec<_> = stream.collect().await;
    let has_abort_error = events.iter().any(|e| matches!(e, AgentEvent::AgentError { error } if error.contains("aborted")));
    assert!(has_abort_error, "should have aborted error, got: {:?}", events);

    registry::unregister("test-api");
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p pi-agent-core -- --nocapture`
Expected: ~16 tests pass (3 types + 8 convert + 5 agent_loop).

- [ ] **Step 4: Commit**

```bash
git add crates/pi-agent-core/tests/
git commit -m "test(pi-agent-core): add agent loop integration tests"
```

---

### Task 9: Example — loop_example.rs

**Files:**
- Create directory: `crates/pi-agent-core/examples/`
- Create: `crates/pi-agent-core/examples/loop_example.rs`

- [ ] **Step 1: Write loop_example.rs**

```rust
use std::sync::Arc;
use futures::StreamExt;
use pi_ai::registry;
use pi_ai::providers::faux::FauxProvider;
use pi_ai::types::{ContentBlock, Model, StopReason};
use pi_agent_core::{Agent, AgentConfig, AgentEvent, AgentTool};

#[tokio::main]
async fn main() {
    // Register a faux provider with two turns:
    // Turn 1: tool call → ToolUse
    // Turn 2: text → Stop
    let provider = Arc::new(FauxProvider::with_call_queue(vec![
        FauxProvider::text_call("I'll search for that.", StopReason::ToolUse),
        FauxProvider::text_call("Done searching. The answer is 42.", StopReason::Stop),
    ]));
    registry::register("faux-api", provider);

    let model = Model {
        id: "faux-model".into(),
        name: "Faux Model".into(),
        api: "faux-api".into(),
        provider: "faux".into(),
        base_url: String::new(),
        reasoning: false,
        input: 0.0, output: 0.0,
        cache_read: None, cache_write: None,
        context_window: 0, max_tokens: None, headers: None,
    };

    let agent = Agent::new(AgentConfig {
        model,
        system_prompt: Some("You are a helpful assistant.".into()),
        max_turns: 5,
        stream_options: None,
    });

    // Add a dummy search tool
    agent.add_tool(AgentTool {
        name: "search".into(),
        description: "Search the web".into(),
        parameters: serde_json::json!({"type": "object", "properties": {"query": {"type": "string"}}}),
        execute: Arc::new(|_args| {
            Box::pin(async move {
                Ok(vec![ContentBlock::Text {
                    text: "search results: 42 is the answer".into(),
                    text_signature: None,
                }])
            })
        }),
    });

    println!("=== pi-agent-core loop example ===\n");

    let mut stream = agent.prompt("What is the meaning of life?");
    while let Some(event) = stream.next().await {
        match event {
            AgentEvent::TurnStart { turn } => {
                println!("--- Turn {} ---", turn);
            }
            AgentEvent::LlmEvent(e) => {
                if let pi_ai::types::AssistantMessageEvent::TextDelta { delta, .. } = &e {
                    print!("{}", delta);
                }
            }
            AgentEvent::ToolCallStart { tool_name, .. } => {
                println!("\n[tool call: {}]", tool_name);
            }
            AgentEvent::ToolCallEnd { result, .. } => {
                match result {
                    Ok(blocks) => println!("[tool result: {:?}]", blocks),
                    Err(e) => println!("[tool error: {}]", e),
                }
            }
            AgentEvent::AgentDone { message } => {
                println!("\n\nDone — stop reason: {:?}", message.stop_reason);
            }
            AgentEvent::AgentError { error } => {
                eprintln!("\nError: {}", error);
            }
        }
    }

    println!("\n=== Final messages ({}) ===", agent.messages().len());
    for msg in agent.messages() {
        match msg {
            pi_agent_core::AgentMessage::UserText { text, .. } => println!("  User: {}", text),
            pi_agent_core::AgentMessage::Assistant { .. } => println!("  Assistant (response)"),
            pi_agent_core::AgentMessage::ToolResult { tool_call_id, .. } => println!("  ToolResult: {}", tool_call_id),
            pi_agent_core::AgentMessage::SystemPrompt { text, .. } => println!("  System: {}", text),
        }
    }
}
```

- [ ] **Step 2: Verify example compiles and runs**

Run: `cargo run -p pi-agent-core --example loop_example`
Expected: prints turn events, tool calls, and final messages list.

- [ ] **Step 3: Commit**

```bash
git add crates/pi-agent-core/examples/
git commit -m "feat(pi-agent-core): add loop_example offline example"
```

---

### Task 10: Final verification

- [ ] **Step 1: Run all pi-agent-core tests**

Run: `cargo test -p pi-agent-core -- --nocapture`
Expected: all tests pass (3 types + 8 convert + 5 agent_loop = 16 tests).

- [ ] **Step 2: Verify workspace builds**

Run: `cargo build`
Expected: entire workspace builds cleanly.

- [ ] **Step 3: Verify workspace tests**

Run: `cargo test`
Expected: all workspace tests pass (pi-ai 60 tests + pi-agent-core 16 tests).

- [ ] **Step 4: Run example smoke test**

Run: `cargo run -p pi-agent-core --example loop_example`
Expected: prints streaming output and exits cleanly.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: final verification - all tests pass, workspace builds, example runs"
```
