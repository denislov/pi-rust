# M4 Agent-Core Harness Capabilities Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Do not commit unless the user explicitly requests a commit.

**Goal:** Add M4 headless harness behavior to Rust `pi-agent-core` and wire the useful parts into `pi-coding-agent` print mode.

**Architecture:** Extend the existing `Agent` runtime instead of introducing a second harness type. `pi-agent-core` owns types, hooks, queues, resources, compaction, and loop behavior; `pi-coding-agent` owns CLI flag parsing, resource loading, and JSONL session writes for thinking/compaction metadata.

**Tech Stack:** Rust edition 2024, futures, async-stream, tokio, tokio-util cancellation, serde/serde_json, serde_yaml, ignore, tempfile, existing faux provider. Behavioral references: `pi/packages/agent/src/agent-loop.ts`, `pi/packages/agent/src/types.ts`, `pi/packages/agent/src/harness/compaction/*`, `pi/packages/agent/src/harness/skills.ts`, `pi/packages/agent/src/harness/prompt-templates.ts`, `pi/packages/agent/src/harness/system-prompt.ts`.

**Spec:** `docs/superpowers/specs/2026-06-05-pi-agent-core-m4-harness-capabilities-design.md`

---

## File Structure

- Modify `crates/pi-agent-core/Cargo.toml` - add `serde_yaml`, `ignore`; extend dev tests as needed.
- Modify `crates/pi-agent-core/src/lib.rs` - export new modules and public types.
- Modify `crates/pi-agent-core/src/types.rs` - add harness enums, hook config, resources, compaction config, tool result, and event payloads.
- Modify `crates/pi-agent-core/src/agent.rs` - add queue/resource helpers and invocation methods.
- Modify `crates/pi-agent-core/src/agent_loop.rs` - implement thinking snapshots, hooks, parallel tools, queues, turn hooks, and compaction checks.
- Modify `crates/pi-agent-core/src/convert.rs` - map compaction summaries and resource-augmented system prompts to `pi-ai::Context`.
- Modify `crates/pi-agent-core/src/session/types.rs` - add session entry constructors for M4 metadata.
- Create `crates/pi-agent-core/src/hooks.rs` - hook context/result aliases and helpers.
- Create `crates/pi-agent-core/src/queues.rs` - queue drain helpers.
- Create `crates/pi-agent-core/src/resources/{mod.rs,frontmatter.rs,skills.rs,prompt_templates.rs,system_prompt.rs}`.
- Create `crates/pi-agent-core/src/compaction/{mod.rs,error.rs,estimate.rs,prepare.rs,summarize.rs,session.rs}`.
- Create tests:
  - `crates/pi-agent-core/tests/harness_types.rs`
  - `crates/pi-agent-core/tests/hooks.rs`
  - `crates/pi-agent-core/tests/parallel_tools.rs`
  - `crates/pi-agent-core/tests/queues_thinking.rs`
  - `crates/pi-agent-core/tests/resources.rs`
  - `crates/pi-agent-core/tests/compaction.rs`
  - `crates/pi-agent-core/tests/session_harness_entries.rs`
- Modify `crates/pi-coding-agent/src/args.rs` - add M4 flags and validation.
- Modify `crates/pi-coding-agent/src/runtime.rs` - pass thinking/tool execution/resource options into `AgentConfig`.
- Modify `crates/pi-coding-agent/src/print_mode.rs` - support skill/template invocation and session compaction event recording.
- Modify `crates/pi-coding-agent/src/lib.rs` - wire parsed flags into print mode.
- Create `crates/pi-coding-agent/src/resources.rs` - CLI-facing resource path resolution/loading.
- Create tests:
  - `crates/pi-coding-agent/tests/harness_args.rs`
  - `crates/pi-coding-agent/tests/harness_print_mode.rs`
  - `crates/pi-coding-agent/tests/harness_sessions.rs`

---

## Task 1: Core Harness Types and Defaults

**Files:**
- Modify: `crates/pi-agent-core/Cargo.toml`
- Modify: `crates/pi-agent-core/src/lib.rs`
- Modify: `crates/pi-agent-core/src/types.rs`
- Test: `crates/pi-agent-core/tests/harness_types.rs`

- [ ] **Step 1: Add dependencies**

Add to `crates/pi-agent-core/Cargo.toml`:

```toml
serde_yaml = "0.9"
ignore = "0.4"
```

Keep existing dependencies intact.

- [ ] **Step 2: Write failing harness type tests**

Create `crates/pi-agent-core/tests/harness_types.rs`:

```rust
use pi_agent_core::{
    AgentConfig, AgentTool, QueueMode, ThinkingLevel, ToolExecutionMode,
};

#[test]
fn thinking_level_parses_cli_values() {
    assert_eq!("off".parse::<ThinkingLevel>().unwrap(), ThinkingLevel::Off);
    assert_eq!("minimal".parse::<ThinkingLevel>().unwrap(), ThinkingLevel::Minimal);
    assert_eq!("low".parse::<ThinkingLevel>().unwrap(), ThinkingLevel::Low);
    assert_eq!("medium".parse::<ThinkingLevel>().unwrap(), ThinkingLevel::Medium);
    assert_eq!("high".parse::<ThinkingLevel>().unwrap(), ThinkingLevel::High);
    assert_eq!("xhigh".parse::<ThinkingLevel>().unwrap(), ThinkingLevel::XHigh);
    assert!("extreme".parse::<ThinkingLevel>().is_err());
}

#[test]
fn tool_execution_mode_parses_cli_values() {
    assert_eq!("parallel".parse::<ToolExecutionMode>().unwrap(), ToolExecutionMode::Parallel);
    assert_eq!("sequential".parse::<ToolExecutionMode>().unwrap(), ToolExecutionMode::Sequential);
    assert!("serial".parse::<ToolExecutionMode>().is_err());
}

#[test]
fn queue_mode_parses_cli_values() {
    assert_eq!("all".parse::<QueueMode>().unwrap(), QueueMode::All);
    assert_eq!("one-at-a-time".parse::<QueueMode>().unwrap(), QueueMode::OneAtATime);
    assert!("one".parse::<QueueMode>().is_err());
}

#[test]
fn agent_config_defaults_match_m4_baseline() {
    let model = pi_ai::Model {
        id: "test".into(),
        name: "Test".into(),
        api: "test-api".into(),
        provider: "test-provider".into(),
        base_url: "https://example.invalid".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![pi_ai::ModelInput::Text],
        cost: pi_ai::ModelCost::default(),
        context_window: 8_000,
        max_tokens: 1_024,
        headers: None,
        compat: None,
    };
    let config = AgentConfig::new(model);
    assert_eq!(config.thinking_level, ThinkingLevel::Off);
    assert_eq!(config.tool_execution, ToolExecutionMode::Parallel);
    assert_eq!(config.steering_mode, QueueMode::OneAtATime);
    assert_eq!(config.follow_up_mode, QueueMode::OneAtATime);
    assert!(config.hooks.is_empty());
    assert!(config.resources.is_empty());
    assert!(config.compaction.is_none());
}

#[test]
fn agent_tool_defaults_to_global_execution_mode() {
    let tool = AgentTool::new_text("echo", "echo input", serde_json::json!({"type": "object"}), |_| async {
        Ok("ok".to_string())
    });
    assert_eq!(tool.execution_mode, None);
}
```

- [ ] **Step 3: Run the failing test**

Run:

```bash
cargo test -p pi-agent-core --test harness_types
```

Expected: FAIL because the new types and helpers do not exist.

- [ ] **Step 4: Implement public enums and config defaults**

In `types.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingLevel {
    Off,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolExecutionMode {
    Sequential,
    Parallel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueMode {
    All,
    OneAtATime,
}
```

Implement `Display` and `FromStr` for each enum using the exact lowercase strings in the tests.

Add:

```rust
#[derive(Debug, Clone)]
pub struct AgentToolResult {
    pub content: Vec<ContentBlock>,
    pub is_error: bool,
    pub terminate: bool,
}

impl AgentToolResult {
    pub fn ok(content: Vec<ContentBlock>) -> Self {
        Self { content, is_error: false, terminate: false }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::Text { text: message.into(), text_signature: None }],
            is_error: true,
            terminate: false,
        }
    }
}
```

Extend `AgentTool`:

```rust
pub execution_mode: Option<ToolExecutionMode>,
```

Add this convenience constructor for text-only test tools:

```rust
use pi_ai::ContentBlock;
use std::{future::Future, sync::Arc};

impl AgentTool {
    pub fn new_text<F, Fut>(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
        f: F,
    ) -> Self
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String, String>> + Send + 'static,
    {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            execution_mode: None,
            execute: Arc::new(move |args| {
                let fut = f(args);
                Box::pin(async move {
                    fut.await.map(|text| vec![ContentBlock::Text {
                        text,
                        text_signature: None,
                    }])
                })
            }),
        }
    }
}
```

Existing explicit struct initializers must be updated with `execution_mode: None`.

Extend `AgentConfig` with M4 fields and add `AgentConfig::new(model)` so tests and callers can
construct a config without repeating defaults.

- [ ] **Step 5: Export the new types**

Update `lib.rs` public exports:

```rust
pub use types::{
    AgentConfig, AgentEvent, AgentMessage, AgentStream, AgentTool, AgentToolResult,
    QueueMode, ThinkingLevel, ToolExecutionMode,
};
```

- [ ] **Step 6: Run the type tests**

Run:

```bash
cargo test -p pi-agent-core --test harness_types
```

Expected: PASS.

---

## Task 2: Hook Types and Hook-Driven Tool Results

**Files:**
- Create: `crates/pi-agent-core/src/hooks.rs`
- Modify: `crates/pi-agent-core/src/types.rs`
- Modify: `crates/pi-agent-core/src/lib.rs`
- Modify: `crates/pi-agent-core/src/agent_loop.rs`
- Test: `crates/pi-agent-core/tests/hooks.rs`

- [ ] **Step 1: Write failing hook tests**

Create tests that use the faux provider to emit one tool call followed by a final text response:

```rust
use futures::StreamExt;
use pi_agent_core::{
    Agent, AgentConfig, AgentEvent, AgentTool, AgentToolResult,
    BeforeToolCallResult, AfterToolCallResult,
};
use pi_ai::{ContentBlock, StopReason};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[tokio::test]
async fn before_hook_blocks_tool_execution() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut config = AgentConfig::new(crate_test_model("hooks-before"));
    config.hooks.before_tool_call = Some(Arc::new(|ctx| {
        assert_eq!(ctx.tool_name, "echo");
        Box::pin(async {
            Ok(Some(BeforeToolCallResult {
                block: true,
                reason: Some("blocked by test".into()),
            }))
        })
    }));

    let agent = Agent::new(config);
    let calls_for_tool = calls.clone();
    agent.add_tool(AgentTool {
        name: "echo".into(),
        description: "echo".into(),
        parameters: serde_json::json!({"type": "object"}),
        execution_mode: None,
        execute: Arc::new(move |_| {
            calls_for_tool.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(vec![ContentBlock::Text { text: "executed".into(), text_signature: None }]) })
        }),
    });

    script_tool_then_stop("hooks-before", "echo", serde_json::json!({}));
    let mut stream = agent.prompt("run");
    let mut saw_blocked_result = false;
    while let Some(event) = stream.next().await {
        if let AgentEvent::ToolCallEnd { result, .. } = event {
            saw_blocked_result = result.is_error
                && result.content.iter().any(|block| matches!(block, ContentBlock::Text { text, .. } if text == "blocked by test"));
        }
    }

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(saw_blocked_result);
}

#[tokio::test]
async fn after_hook_replaces_tool_result() {
    let mut config = AgentConfig::new(crate_test_model("hooks-after"));
    config.hooks.after_tool_call = Some(Arc::new(|ctx| {
        assert_eq!(ctx.tool_name, "echo");
        assert!(!ctx.result.is_error);
        Box::pin(async {
            Ok(Some(AfterToolCallResult {
                content: Some(vec![ContentBlock::Text { text: "rewritten".into(), text_signature: None }]),
                is_error: Some(true),
                terminate: Some(false),
            }))
        })
    }));

    let agent = Agent::new(config);
    agent.add_tool(simple_text_tool("echo", "original"));
    script_tool_then_stop("hooks-after", "echo", serde_json::json!({}));

    let mut stream = agent.prompt("run");
    while stream.next().await.is_some() {}

    let messages = agent.messages();
    assert!(messages.iter().any(|msg| matches!(
        msg,
        pi_agent_core::AgentMessage::ToolResult { is_error: true, content, .. }
            if content.iter().any(|block| matches!(block, ContentBlock::Text { text, .. } if text == "rewritten"))
    )));
}
```

Add `crate_test_model`, `script_tool_then_stop`, and `simple_text_tool` to
`tests/common/mod.rs`. `crate_test_model(api)` returns a model bound to a unique faux-provider api,
`script_tool_then_stop(api, tool_name, args)` registers a faux script that emits one tool call then
a final stop message, and `simple_text_tool(name, text)` returns an `AgentTool` with
`execution_mode: None`.

- [ ] **Step 2: Run the failing hook tests**

Run:

```bash
cargo test -p pi-agent-core --test hooks
```

Expected: FAIL because hook APIs and `AgentEvent::ToolCallEnd { result: AgentToolResult }` do not exist.

- [ ] **Step 3: Implement hook structs and callback aliases**

Create `hooks.rs` with:

```rust
use crate::{AgentMessage, AgentToolResult};
use pi_ai::AssistantMessage;
use serde_json::Value;
use std::{future::Future, pin::Pin, sync::Arc};

pub type HookFuture<T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send>>;

#[derive(Clone, Default)]
pub struct AgentHooks {
    pub before_tool_call: Option<BeforeToolCallHook>,
    pub after_tool_call: Option<AfterToolCallHook>,
    pub should_stop_after_turn: Option<ShouldStopAfterTurnHook>,
    pub prepare_next_turn: Option<PrepareNextTurnHook>,
}

impl AgentHooks {
    pub fn is_empty(&self) -> bool {
        self.before_tool_call.is_none()
            && self.after_tool_call.is_none()
            && self.should_stop_after_turn.is_none()
            && self.prepare_next_turn.is_none()
    }
}

pub type BeforeToolCallHook = Arc<dyn Fn(BeforeToolCallContext) -> HookFuture<Option<BeforeToolCallResult>> + Send + Sync>;
pub type AfterToolCallHook = Arc<dyn Fn(AfterToolCallContext) -> HookFuture<Option<AfterToolCallResult>> + Send + Sync>;
pub type ShouldStopAfterTurnHook = Arc<dyn Fn(ShouldStopAfterTurnContext) -> HookFuture<bool> + Send + Sync>;
pub type PrepareNextTurnHook = Arc<dyn Fn(PrepareNextTurnContext) -> HookFuture<Option<AgentLoopTurnUpdate>> + Send + Sync>;

#[derive(Clone)]
pub struct BeforeToolCallContext {
    pub assistant_message: AssistantMessage,
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: Value,
    pub messages: Vec<AgentMessage>,
}

#[derive(Clone)]
pub struct BeforeToolCallResult {
    pub block: bool,
    pub reason: Option<String>,
}

#[derive(Clone)]
pub struct AfterToolCallContext {
    pub assistant_message: AssistantMessage,
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: Value,
    pub result: AgentToolResult,
    pub messages: Vec<AgentMessage>,
}

#[derive(Clone, Default)]
pub struct AfterToolCallResult {
    pub content: Option<Vec<pi_ai::ContentBlock>>,
    pub is_error: Option<bool>,
    pub terminate: Option<bool>,
}
```

Add `ShouldStopAfterTurnContext`, `PrepareNextTurnContext`, and `AgentLoopTurnUpdate` with model,
thinking-level, stream-options, and message replacements.

- [ ] **Step 4: Update loop tool finalization**

In `agent_loop.rs`, introduce helper functions:

- `prepare_tool_call(...) -> PreparedToolCall | AgentToolResult`
- `execute_prepared_tool_call(...) -> AgentToolResult`
- `finalize_tool_call(...) -> AgentToolResult`

The current `Err(String)` path becomes `AgentToolResult::error(error)`. Hook errors also become
`AgentToolResult::error(error)`.

Update `AgentEvent::ToolCallEnd` to carry:

```rust
ToolCallEnd {
    tool_call_id: String,
    tool_name: String,
    result: AgentToolResult,
}
```

Update tests and callers that matched the old `Result<Vec<ContentBlock>, String>` shape.

- [ ] **Step 5: Run hook and existing loop tests**

Run:

```bash
cargo test -p pi-agent-core --test hooks
cargo test -p pi-agent-core --test agent_loop
```

Expected: PASS.

---

## Task 3: Parallel Tool Execution

**Files:**
- Modify: `crates/pi-agent-core/src/agent_loop.rs`
- Modify: `crates/pi-agent-core/src/types.rs`
- Modify: `crates/pi-coding-agent/src/tools/*.rs` - add `execution_mode: None` to tool initializers.
- Test: `crates/pi-agent-core/tests/parallel_tools.rs`

- [ ] **Step 1: Write failing parallel execution tests**

Create tests with two delayed tools:

```rust
#[tokio::test]
async fn parallel_tools_finish_faster_than_sequential_tools() {
    let parallel_ms = run_two_delayed_tools(ToolExecutionMode::Parallel).await;
    let sequential_ms = run_two_delayed_tools(ToolExecutionMode::Sequential).await;

    assert!(parallel_ms < 180, "parallel took {parallel_ms}ms");
    assert!(sequential_ms >= 190, "sequential took {sequential_ms}ms");
}

#[tokio::test]
async fn parallel_tool_results_are_appended_in_assistant_order() {
    let agent = run_two_tools_with_different_delays(ToolExecutionMode::Parallel).await;
    let results: Vec<_> = agent
        .messages()
        .into_iter()
        .filter_map(|msg| match msg {
            AgentMessage::ToolResult { tool_name, .. } => Some(tool_name),
            _ => None,
        })
        .collect();
    assert_eq!(results, vec!["slow", "fast"]);
}

#[tokio::test]
async fn per_tool_sequential_override_forces_batch_sequential() {
    let elapsed = run_batch_with_one_sequential_tool().await;
    assert!(elapsed >= 190, "sequential override elapsed {elapsed}ms");
}
```

Use `tokio::time::Instant` and `tokio::time::sleep(Duration::from_millis(100))`.

- [ ] **Step 2: Run the failing tests**

Run:

```bash
cargo test -p pi-agent-core --test parallel_tools
```

Expected: FAIL because tools still execute sequentially or because source-order append is not implemented.

- [ ] **Step 3: Implement batch execution selection**

In `agent_loop.rs`, after collecting tool calls:

1. Read global `tool_execution` from state.
2. Check whether any matched tool has `execution_mode == Some(ToolExecutionMode::Sequential)`.
3. Use sequential execution if either condition requires it.
4. Otherwise use a parallel path.

The parallel path must:

- run missing-tool and `before_tool_call` preflight in source order
- store each prepared call with its original index
- execute prepared calls through `FuturesUnordered`
- yield `ToolCallEnd` as calls complete
- sort finalized results by original index before appending `AgentMessage::ToolResult`

- [ ] **Step 4: Ensure mutation-heavy built-in tools can opt out later**

Leave M1 tools with `execution_mode: None` for this task. Add a comment in `tools/mod.rs` explaining
that write/edit/bash can be moved to `Some(ToolExecutionMode::Sequential)` when the file mutation
queue is ported.

- [ ] **Step 5: Run the parallel tests**

Run:

```bash
cargo test -p pi-agent-core --test parallel_tools
```

Expected: PASS.

---

## Task 4: Steering, Follow-Up Queues, and Thinking Level

**Files:**
- Create: `crates/pi-agent-core/src/queues.rs`
- Modify: `crates/pi-agent-core/src/agent.rs`
- Modify: `crates/pi-agent-core/src/agent_loop.rs`
- Modify: `crates/pi-agent-core/src/types.rs`
- Test: `crates/pi-agent-core/tests/queues_thinking.rs`

- [ ] **Step 1: Write failing queue tests**

Cover these behaviors:

- `Agent::steer("...")` called while a tool is running injects a user message before the next model call.
- `QueueMode::OneAtATime` drains one steering message per drain point.
- `QueueMode::All` drains every queued steering message.
- `Agent::follow_up("...")` continues after a stop response instead of ending.
- `Agent::clear_queues()` clears both queues when aborting or before reuse.

Use a faux provider script where the first response calls a delayed tool, the second response
captures injected user messages, and the final response stops.

- [ ] **Step 2: Write failing thinking tests**

Add tests that configure reasoning and non-reasoning models:

```rust
#[tokio::test]
async fn thinking_level_sets_stream_options_for_reasoning_model() {
    let mut config = AgentConfig::new(reasoning_model("thinking-high"));
    config.thinking_level = ThinkingLevel::High;
    let options = pi_agent_core::thinking::stream_options_for_turn(
        &config.model,
        config.stream_options.clone().unwrap_or_default(),
        config.thinking_level,
    );
    assert!(options.thinking.as_ref().unwrap().enabled);
    assert_eq!(options.thinking.as_ref().unwrap().effort.as_deref(), Some("high"));
}

#[tokio::test]
async fn thinking_level_is_omitted_for_non_reasoning_model() {
    let mut config = AgentConfig::new(non_reasoning_model("thinking-off"));
    config.thinking_level = ThinkingLevel::High;
    let options = pi_agent_core::thinking::stream_options_for_turn(
        &config.model,
        config.stream_options.clone().unwrap_or_default(),
        config.thinking_level,
    );
    assert!(options.thinking.is_none());
}
```

Place the helper in `agent_loop.rs` or a small `thinking.rs` module and export it only if tests need
direct access.

- [ ] **Step 3: Run failing tests**

Run:

```bash
cargo test -p pi-agent-core --test queues_thinking
```

Expected: FAIL because queue APIs and thinking option snapshots do not exist.

- [ ] **Step 4: Implement queues**

Add `steering_queue` and `follow_up_queue` to `AgentState`.

Add public methods on `Agent`:

```rust
pub fn steer(&self, text: impl Into<String>);
pub fn follow_up(&self, text: impl Into<String>);
pub fn clear_queues(&self);
```

Add `queues.rs`:

```rust
pub fn drain_queue(queue: &mut VecDeque<AgentMessage>, mode: QueueMode) -> Vec<AgentMessage> {
    match mode {
        QueueMode::All => queue.drain(..).collect(),
        QueueMode::OneAtATime => queue.pop_front().into_iter().collect(),
    }
}
```

The loop drains steering after each completed turn and drains follow-up when it would otherwise
return `AgentDone`.

- [ ] **Step 5: Implement thinking snapshots**

Before each `stream_model` call:

1. Clone `state.config.stream_options.unwrap_or_default()`.
2. Attach cancellation token.
3. Apply `ThinkingLevel` to `StreamOptions.thinking`.
4. Use the updated options for the request only.

The helper should not mutate the stored config.

- [ ] **Step 6: Run queue/thinking tests**

Run:

```bash
cargo test -p pi-agent-core --test queues_thinking
```

Expected: PASS.

---

## Task 5: Skills, Prompt Templates, and System Prompt Formatting

**Files:**
- Create: `crates/pi-agent-core/src/resources/{mod.rs,frontmatter.rs,skills.rs,prompt_templates.rs,system_prompt.rs}`
- Modify: `crates/pi-agent-core/src/lib.rs`
- Modify: `crates/pi-agent-core/src/types.rs`
- Modify: `crates/pi-agent-core/src/convert.rs`
- Modify: `crates/pi-agent-core/src/agent.rs`
- Test: `crates/pi-agent-core/tests/resources.rs`

- [ ] **Step 1: Write failing resource tests**

Create temp-dir fixtures:

```text
skills/
  rust/SKILL.md
  hidden/.ignored/SKILL.md
templates/
  review.md
```

Test cases:

- `load_skills([skills])` loads `rust/SKILL.md`.
- frontmatter `name`, `description`, and `disable-model-invocation` are parsed.
- ignored directories are skipped when `.gitignore` contains `hidden/`.
- invalid YAML returns a warning diagnostic without failing valid skills.
- `format_skills_for_system_prompt` emits `<available_skills>` and excludes disabled skills.
- `format_skill_invocation` includes the skill location and content.
- `load_prompt_templates([templates])` loads direct `.md` children.
- `format_prompt_template_invocation` replaces `$1` and `${2}` with supplied args.

- [ ] **Step 2: Run the failing resource tests**

Run:

```bash
cargo test -p pi-agent-core --test resources
```

Expected: FAIL because the resources module does not exist.

- [ ] **Step 3: Implement frontmatter parser**

In `frontmatter.rs`, normalize CRLF to LF. If content starts with `---`, parse the first matching
closing `---` line as YAML using `serde_yaml`; otherwise return empty metadata and the full body.

Warnings should contain:

```rust
pub struct ResourceDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub path: PathBuf,
}
```

Use severity `Warning`.

- [ ] **Step 4: Implement skills loader**

Use `ignore::WalkBuilder` for recursive traversal. For each root:

- skip missing roots without diagnostics
- load a directory-level `SKILL.md` before recursing into child entries
- load direct root `.md` files as skills
- skip dot directories except ignore files handled by `ignore`
- derive fallback name from directory or filename
- derive fallback description from first non-empty body line, capped at 1024 chars
- cap explicit skill names at 64 chars and descriptions at 1024 chars

- [ ] **Step 5: Implement prompt template loader**

For each input path:

- if file and `.md`, load it
- if directory, load direct `.md` children sorted by filename
- derive name from filename without `.md`
- derive description from frontmatter or first non-empty body line capped at 60 chars plus `...`

- [ ] **Step 6: Wire resources into system prompt and explicit invocation**

Add `AgentResources` to `types.rs`:

```rust
#[derive(Debug, Clone, Default)]
pub struct AgentResources {
    pub skills: Vec<Skill>,
    pub prompt_templates: Vec<PromptTemplate>,
}

impl AgentResources {
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty() && self.prompt_templates.is_empty()
    }
}
```

Update `convert_to_context` to accept `AgentResources` or a preformatted system prompt. The final
system prompt is:

1. configured system prompt, if any
2. blank line
3. skill list block, if there are visible skills

Add methods:

```rust
pub fn set_resources(&self, resources: AgentResources);
pub fn skill(&self, name: &str, additional_instructions: Option<&str>) -> Result<AgentStream, String>;
pub fn prompt_from_template(&self, name: &str, args: &[String]) -> Result<AgentStream, String>;
```

These methods should use the same run guard as `prompt()`.

- [ ] **Step 7: Run resource tests and convert tests**

Run:

```bash
cargo test -p pi-agent-core --test resources
cargo test -p pi-agent-core convert
```

Expected: PASS.

---

## Task 6: Compaction Preparation, Summary Generation, and Session Entries

**Files:**
- Create: `crates/pi-agent-core/src/compaction/{mod.rs,error.rs,estimate.rs,prepare.rs,summarize.rs,session.rs}`
- Modify: `crates/pi-agent-core/src/lib.rs`
- Modify: `crates/pi-agent-core/src/types.rs`
- Modify: `crates/pi-agent-core/src/convert.rs`
- Modify: `crates/pi-agent-core/src/session/types.rs`
- Modify: `crates/pi-agent-core/src/session/context.rs`
- Test: `crates/pi-agent-core/tests/compaction.rs`
- Test: `crates/pi-agent-core/tests/session_harness_entries.rs`

- [ ] **Step 1: Write failing compaction tests**

Cover:

- `estimate_tokens` counts text, thinking, tool calls, and tool results.
- `estimate_context_tokens` uses the last successful assistant usage plus trailing estimated tokens.
- `should_compact` applies `context_window - reserve_tokens`.
- `prepare_compaction` avoids cut points that would retain a bare tool result.
- `prepare_compaction` carries previous compaction summary into the next summary request.
- `extract_file_ops` records assistant tool calls named `read`, `write`, and `edit`.
- `serialize_conversation` truncates tool results to 2000 chars.
- `summarize_compaction` returns deterministic faux provider text.

- [ ] **Step 2: Write failing session entry tests**

Add JSON shape tests:

```rust
#[test]
fn compaction_entry_serializes_as_typescript_shape() {
    let entry = SessionEntry::compaction(
        "cmp00001".into(),
        Some("msg00001".into()),
        "2026-06-05T00:00:00.000Z".into(),
        "summary".into(),
        "msg00010".into(),
        12345,
        Some(serde_json::json!({"readFiles":["README.md"],"modifiedFiles":["src/lib.rs"]})),
        false,
    );
    let json = serde_json::to_value(entry).unwrap();
    assert_eq!(json["type"], "compaction");
    assert_eq!(json["summary"], "summary");
    assert_eq!(json["firstKeptEntryId"], "msg00010");
    assert_eq!(json["tokensBefore"], 12345);
    assert_eq!(json["fromHook"], false);
}

#[test]
fn thinking_level_change_serializes_as_typescript_shape() {
    let entry = SessionEntry::thinking_level_change(
        "think001".into(),
        None,
        "2026-06-05T00:00:00.000Z".into(),
        ThinkingLevel::High,
    );
    let json = serde_json::to_value(entry).unwrap();
    assert_eq!(json["type"], "thinking_level_change");
    assert_eq!(json["thinkingLevel"], "high");
}
```

- [ ] **Step 3: Run failing tests**

Run:

```bash
cargo test -p pi-agent-core --test compaction
cargo test -p pi-agent-core --test session_harness_entries
```

Expected: FAIL because compaction modules and constructors do not exist.

- [ ] **Step 4: Implement compaction error and settings**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactionSettings {
    pub enabled: bool,
    pub reserve_tokens: u32,
    pub keep_recent_tokens: u32,
}

impl Default for CompactionSettings {
    fn default() -> Self {
        Self { enabled: true, reserve_tokens: 16_384, keep_recent_tokens: 20_000 }
    }
}
```

Add `CompactionError` with codes `aborted`, `summarization_failed`, `invalid_session`, and
`unknown`.

- [ ] **Step 5: Implement estimation and preparation**

Port the TS algorithms at the behavior level:

- characters / 4 token heuristic
- image block estimate of 4800 chars
- last assistant usage shortcut
- safe cut points at user, assistant, compaction summary, branch summary, custom-message equivalents
- avoid tool-result-only retained prefixes
- previous compaction summary detection
- file-operation metadata extraction from assistant tool-call arguments

The Rust preparation input can accept `&[SessionEntry]` and convert through
`build_session_context()`.

- [ ] **Step 6: Implement summarization**

Use:

```rust
let stream = pi_ai::stream_model(&model, context, Some(options));
let message = pi_ai::complete(stream).await?;
```

The summarization system prompt and structured user prompt should match the spec. Extract text
blocks from the returned assistant message and join them with `\n`.

- [ ] **Step 7: Implement compaction summary message conversion**

Add `AgentMessage::CompactionSummary { message_id, summary, tokens_before }`.

Update `convert_to_context` to map it to:

```text
The conversation history before this point was compacted into the following summary:

<summary>
...
</summary>
```

- [ ] **Step 8: Implement session entry constructors**

Add `SessionEntry::compaction`, `SessionEntry::thinking_level_change`,
`SessionEntry::active_tools_change`, and `SessionEntry::model_change`. Ensure field names are
camelCase where TypeScript uses camelCase.

- [ ] **Step 9: Run compaction/session tests**

Run:

```bash
cargo test -p pi-agent-core --test compaction
cargo test -p pi-agent-core --test session_harness_entries
```

Expected: PASS.

---

## Task 7: Runtime Compaction, Turn Hooks, and Event Recording

**Files:**
- Modify: `crates/pi-agent-core/src/types.rs`
- Modify: `crates/pi-agent-core/src/agent.rs`
- Modify: `crates/pi-agent-core/src/agent_loop.rs`
- Modify: `crates/pi-coding-agent/src/print_mode.rs`
- Modify: `crates/pi-coding-agent/src/session.rs`
- Test: `crates/pi-agent-core/tests/compaction.rs`
- Test: `crates/pi-coding-agent/tests/harness_sessions.rs`

- [ ] **Step 1: Add runtime compaction tests**

Add a core test where:

1. The model has a tiny `context_window`.
2. Existing messages exceed the threshold.
3. The faux provider returns a summary during compaction.
4. The next normal model request sees a `CompactionSummary` plus retained messages.
5. The stream emits `AgentEvent::SessionCompacted`.

- [ ] **Step 2: Add print-mode session compaction tests**

Use a temp session directory and faux provider. Assert the JSONL file contains:

- original header
- normal message entries
- a `compaction` entry
- later message entries parented after the compaction point

- [ ] **Step 3: Run failing runtime tests**

Run:

```bash
cargo test -p pi-agent-core --test compaction
cargo test -p pi-coding-agent --test harness_sessions
```

Expected: FAIL because runtime compaction and print-mode recording do not exist.

- [ ] **Step 4: Add compaction config and event**

Add to `types.rs`:

```rust
#[derive(Clone)]
pub struct CompactionConfig {
    pub settings: CompactionSettings,
    pub custom_instructions: Option<String>,
}

pub enum AgentEvent {
    SessionCompacted {
        summary: String,
        first_kept_message_id: String,
        tokens_before: u32,
        details: Option<serde_json::Value>,
    },
    // existing variants
}
```

Keep existing `TurnStart`, `LlmEvent`, `ToolCallStart`, `ToolCallEnd`, `AgentDone`, and
`AgentError` semantics intact.

- [ ] **Step 5: Compact before provider requests**

In `agent_loop.rs`, before `convert_to_context`:

1. Snapshot messages, model, thinking level, and compaction config without holding a write lock
   across await points.
2. If compaction is disabled or not needed, continue.
3. Generate the summary.
4. Acquire write lock and replace compacted messages with `AgentMessage::CompactionSummary` plus
   retained messages.
5. Yield `AgentEvent::SessionCompacted`.

Do not compact when the last message is already a compaction summary.

- [ ] **Step 6: Update print-mode session recording**

Replace the current end-of-run-only `capture_session_messages` path with a `SessionRecorder` that:

- keeps `existing_ids`
- tracks current parent id
- appends user/assistant/tool-result entries as they become durable
- appends a `compaction` entry when `AgentEvent::SessionCompacted` arrives
- appends `session_info` once per run before the first new message

The recorder should still append final messages on error before returning `CliError::AgentFailure`.

- [ ] **Step 7: Run runtime/session tests**

Run:

```bash
cargo test -p pi-agent-core --test compaction
cargo test -p pi-coding-agent --test harness_sessions
```

Expected: PASS.

---

## Task 8: CLI Flags for Thinking, Tool Execution, Skills, and Templates

**Files:**
- Modify: `crates/pi-coding-agent/src/args.rs`
- Modify: `crates/pi-coding-agent/src/error.rs`
- Modify: `crates/pi-coding-agent/src/runtime.rs`
- Modify: `crates/pi-coding-agent/src/print_mode.rs`
- Modify: `crates/pi-coding-agent/src/lib.rs`
- Create: `crates/pi-coding-agent/src/resources.rs`
- Test: `crates/pi-coding-agent/tests/harness_args.rs`
- Test: `crates/pi-coding-agent/tests/harness_print_mode.rs`

- [ ] **Step 1: Write failing CLI argument tests**

Cover:

- `--thinking high`
- `--tool-execution sequential`
- repeated `--skills <dir>`
- repeated `--prompt-templates <path>`
- `--skill rust`
- `--prompt-template review`
- repeated `--template-arg value`
- `--skill` and `--prompt-template` rejected together
- invalid thinking/tool-execution values rejected

- [ ] **Step 2: Run failing argument tests**

Run:

```bash
cargo test -p pi-coding-agent --test harness_args
```

Expected: FAIL because flags do not exist.

- [ ] **Step 3: Extend `CliArgs` and parser**

Add:

```rust
pub thinking: Option<ThinkingLevel>,
pub tool_execution: Option<ToolExecutionMode>,
pub skills: Vec<String>,
pub prompt_templates: Vec<String>,
pub skill: Option<String>,
pub prompt_template: Option<String>,
pub template_args: Vec<String>,
```

Update `help_text()` with the new flags. Reject mutually exclusive invocation modes before
returning parsed args.

- [ ] **Step 4: Add CLI resource loader**

Create `resources.rs` that resolves `--skills` and `--prompt-templates` paths relative to runtime
cwd and calls `pi_agent_core::resources::load_skills` and `load_prompt_templates`.

Return diagnostics as CLI stderr warnings only when loading succeeds. Loading errors that prevent
the requested `--skill` or `--prompt-template` from being resolved are `CliError::InvalidInput`.

- [ ] **Step 5: Wire runtime options**

Extend `PrintModeOptions` with:

```rust
pub thinking_level: Option<ThinkingLevel>,
pub tool_execution: Option<ToolExecutionMode>,
pub resources: AgentResources,
pub invocation: PromptInvocation,
```

`PromptInvocation` variants:

```rust
Text(String)
Skill { name: String, additional_instructions: Option<String> }
PromptTemplate { name: String, args: Vec<String> }
```

Build `AgentConfig` with thinking/tool-execution/resources. In `run_print_mode`, call:

- `agent.prompt(&prompt)` for text
- `agent.skill(&name, additional.as_deref())?` for skill
- `agent.prompt_from_template(&name, &args)?` for template

- [ ] **Step 6: Add print-mode tests**

Use injected faux provider/resources to assert:

- skill invocation prompt includes `<skill name="rust" location="...">`
- prompt template replaces args before reaching provider
- thinking flag reaches `AgentConfig`
- sequential flag changes elapsed behavior for delayed tools

- [ ] **Step 7: Run CLI M4 tests**

Run:

```bash
cargo test -p pi-coding-agent --test harness_args
cargo test -p pi-coding-agent --test harness_print_mode
```

Expected: PASS.

---

## Task 9: Existing Tests, Public API Cleanup, and Verification

**Files:**
- Modify existing tests in `crates/pi-agent-core/tests/`
- Modify existing tests in `crates/pi-coding-agent/tests/`
- Modify public re-exports in `crates/pi-agent-core/src/lib.rs`
- Modify public re-exports in `crates/pi-coding-agent/src/lib.rs`

- [ ] **Step 1: Update existing tool initializers**

Search:

```bash
rg -n "AgentTool \\{" crates/pi-agent-core crates/pi-coding-agent
```

Every explicit initializer must set:

```rust
execution_mode: None,
```

- [ ] **Step 2: Update old `ToolCallEnd` matches**

Search:

```bash
rg -n "ToolCallEnd|result: Result|AgentEvent::ToolCallEnd" crates/pi-agent-core crates/pi-coding-agent
```

Update tests and code to inspect `AgentToolResult { content, is_error, terminate }`.

- [ ] **Step 3: Run focused package tests**

Run:

```bash
cargo test -p pi-agent-core
cargo test -p pi-coding-agent
```

Expected: PASS.

- [ ] **Step 4: Run workspace verification**

Run:

```bash
cargo fmt --check
cargo test --workspace
cargo check --workspace
```

Expected: all PASS.

- [ ] **Step 5: Update ROADMAP status only after implementation**

After code is implemented and verified, update `ROADMAP.md` M4 status with actual results. Do not
mark M4 complete in the roadmap while only this plan exists.

---

## Parallel M6 Work During M4

M6 work is not part of the M4 implementation checklist above. It can run in a separate branch or
worktree as long as it stays inside `crates/pi-tui` and avoids `pi-coding-agent` interactive wiring.

Safe parallel packages:

- `crates/pi-tui/src/input/keys.rs` and `keybindings.rs`
- `crates/pi-tui/src/input/stdin_buffer.rs`
- `crates/pi-tui/src/terminal/raw_mode.rs`
- `crates/pi-tui/src/components/input.rs`
- `crates/pi-tui/src/components/markdown.rs`
- `crates/pi-tui/src/components/select_list.rs`
- `crates/pi-tui/src/components/loader.rs` and `cancellable_loader.rs`

Avoid during M4:

- `pi-coding-agent` interactive mode bridge
- tool execution UI
- thinking selector UI
- compaction/session selector UI
- extension/slash-command UI
