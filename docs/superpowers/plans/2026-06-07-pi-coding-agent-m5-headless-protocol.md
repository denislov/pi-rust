# M5 Headless Protocol Modes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Do not commit unless the user explicitly requests a commit.

**Goal:** Add Rust `pi-coding-agent` headless protocol modes: one-shot `--mode json` JSONL event streaming and long-running `--mode rpc` stdio JSON-RPC.

**Architecture:** Keep protocol wire types in `pi-coding-agent`, not `pi-agent-core`. Add a small protocol subtree that owns strict JSONL framing, TypeScript-compatible event/response serialization, core-event adaptation, shared session-backed prompt running, JSON mode, and RPC mode. Existing print mode remains compatible and can reuse the shared runner after extraction.

**Tech Stack:** Rust edition 2024, tokio async IO, futures, serde/serde_json, tempfile, existing faux provider, existing M3 session module, existing M4 queues/thinking/compaction APIs. Behavioral references: `pi/packages/coding-agent/docs/json.md`, `pi/packages/coding-agent/docs/rpc.md`, `pi/packages/coding-agent/src/modes/rpc/{jsonl,rpc-types,rpc-mode}.ts`, `pi/packages/agent/src/types.ts`.

**Spec:** `docs/superpowers/specs/2026-06-07-pi-coding-agent-m5-headless-protocol-design.md`

---

## File Structure

- Modify `crates/pi-coding-agent/src/args.rs` - add `CliMode`, `--mode`, validation, help text.
- Modify `crates/pi-coding-agent/src/error.rs` - add protocol/session-mode errors when needed.
- Modify `crates/pi-coding-agent/src/lib.rs` - route print/json/rpc modes and export protocol helpers for tests.
- Modify `crates/pi-coding-agent/src/main.rs` - keep stdout/stderr printing for print/json and call a streaming entry point for rpc if `CliOutput` is not enough.
- Modify `crates/pi-coding-agent/src/print_mode.rs` - move shared session hydration/capture helpers into protocol runner.
- Modify `crates/pi-coding-agent/src/runtime.rs` - keep common runtime option construction usable by all modes.
- Create `crates/pi-coding-agent/src/protocol/mod.rs` - module exports.
- Create `crates/pi-coding-agent/src/protocol/jsonl.rs` - strict JSONL serialization and LF-only reader.
- Create `crates/pi-coding-agent/src/protocol/types.rs` - protocol event, RPC command, RPC response, state, stats types.
- Create `crates/pi-coding-agent/src/protocol/events.rs` - `pi-agent-core::AgentEvent` to protocol-event adapter.
- Create `crates/pi-coding-agent/src/protocol/session_runner.rs` - shared session-backed agent runner.
- Create `crates/pi-coding-agent/src/protocol/json_mode.rs` - one-shot JSON event stream mode.
- Create `crates/pi-coding-agent/src/protocol/rpc.rs` - stdio command loop and RPC state.
- Create tests:
  - `crates/pi-coding-agent/tests/protocol_args.rs`
  - `crates/pi-coding-agent/tests/protocol_jsonl.rs`
  - `crates/pi-coding-agent/tests/protocol_events.rs`
  - `crates/pi-coding-agent/tests/json_mode.rs`
  - `crates/pi-coding-agent/tests/rpc_mode.rs`
  - `crates/pi-coding-agent/tests/protocol_sessions.rs`

---

## Task 1: CLI Mode Parsing

**Files:**
- Modify: `crates/pi-coding-agent/src/args.rs`
- Modify: `crates/pi-coding-agent/src/error.rs`
- Modify: `crates/pi-coding-agent/src/lib.rs`
- Test: `crates/pi-coding-agent/tests/protocol_args.rs`

- [ ] **Step 1: Write failing mode parsing tests**

Create `crates/pi-coding-agent/tests/protocol_args.rs`:

```rust
use pi_coding_agent::{parse_args, CliError, CliMode};

#[test]
fn print_flag_selects_print_mode() {
    let args = parse_args(vec!["-p".to_string(), "hello".to_string()]).unwrap();
    assert_eq!(args.mode, CliMode::Print);
    assert_eq!(args.prompt.as_deref(), Some("hello"));
}

#[test]
fn explicit_json_mode_accepts_positional_prompt() {
    let args = parse_args(vec![
        "--mode".to_string(),
        "json".to_string(),
        "hello world".to_string(),
    ])
    .unwrap();
    assert_eq!(args.mode, CliMode::Json);
    assert_eq!(args.prompt.as_deref(), Some("hello world"));
    assert!(!args.print);
}

#[test]
fn explicit_rpc_mode_accepts_without_prompt() {
    let args = parse_args(vec!["--mode".to_string(), "rpc".to_string()]).unwrap();
    assert_eq!(args.mode, CliMode::Rpc);
    assert_eq!(args.prompt, None);
}

#[test]
fn print_flag_cannot_select_json_mode() {
    let err = parse_args(vec![
        "--mode".to_string(),
        "json".to_string(),
        "-p".to_string(),
        "hello".to_string(),
    ])
    .unwrap_err();
    assert_eq!(
        err,
        CliError::InvalidInput("--print can only be combined with --mode print".into())
    );
}

#[test]
fn rpc_mode_rejects_positional_prompt_for_m5() {
    let err = parse_args(vec![
        "--mode".to_string(),
        "rpc".to_string(),
        "hello".to_string(),
    ])
    .unwrap_err();
    assert_eq!(
        err,
        CliError::InvalidInput(
            "unsupported mode input: rpc does not accept positional prompt".into()
        )
    );
}

#[test]
fn unknown_mode_is_rejected() {
    let err = parse_args(vec!["--mode".to_string(), "xml".to_string()]).unwrap_err();
    assert_eq!(err, CliError::InvalidInput("unknown mode: xml".into()));
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test -p pi-coding-agent --test protocol_args
```

Expected: FAIL because `CliMode` and `--mode` do not exist.

- [ ] **Step 3: Add `CliMode` and parser fields**

In `crates/pi-coding-agent/src/args.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliMode {
    Print,
    Json,
    Rpc,
}

impl std::str::FromStr for CliMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "print" => Ok(Self::Print),
            "json" => Ok(Self::Json),
            "rpc" => Ok(Self::Rpc),
            other => Err(format!("unknown mode: {other}")),
        }
    }
}
```

Extend `CliArgs`:

```rust
pub mode: CliMode,
pub mode_explicit: bool,
```

Set defaults:

```rust
mode: CliMode::Print,
mode_explicit: false,
```

Add parsing:

```rust
"--mode" => {
    let value = take_value(&raw, &mut i, "--mode")?;
    parsed.mode = value.parse().map_err(CliError::InvalidInput)?;
    parsed.mode_explicit = true;
}
```

Keep `-p` / `--print` setting `parsed.print = true`; mode validation runs after the loop.

- [ ] **Step 4: Add mode validation**

At the end of `parse_args`, after prompt assembly and existing session validation, add:

```rust
if parsed.print {
    if parsed.mode_explicit && parsed.mode != CliMode::Print {
        return Err(CliError::InvalidInput(
            "--print can only be combined with --mode print".into(),
        ));
    }
    parsed.mode = CliMode::Print;
}

if parsed.mode == CliMode::Rpc && parsed.prompt.is_some() {
    return Err(CliError::InvalidInput(
        "unsupported mode input: rpc does not accept positional prompt".into(),
    ));
}
```

Update help text with:

```text
  --mode <mode>            Headless mode: print|json|rpc
```

- [ ] **Step 5: Export `CliMode`**

In `crates/pi-coding-agent/src/lib.rs`, update the args export:

```rust
pub use args::{CliArgs, CliMode, DEFAULT_MAX_TURNS, help_text, parse_args};
```

- [ ] **Step 6: Run mode parsing tests**

Run:

```bash
cargo test -p pi-coding-agent --test protocol_args
```

Expected: PASS.

---

## Task 2: JSONL Framing and Protocol Wire Types

**Files:**
- Modify: `crates/pi-coding-agent/src/lib.rs`
- Create: `crates/pi-coding-agent/src/protocol/mod.rs`
- Create: `crates/pi-coding-agent/src/protocol/jsonl.rs`
- Create: `crates/pi-coding-agent/src/protocol/types.rs`
- Test: `crates/pi-coding-agent/tests/protocol_jsonl.rs`

- [ ] **Step 1: Write failing JSONL tests**

Create `crates/pi-coding-agent/tests/protocol_jsonl.rs`:

```rust
use pi_coding_agent::protocol::jsonl::{read_jsonl_lines, serialize_json_line};
use serde_json::json;
use tokio::io::AsyncWriteExt;

#[test]
fn serialize_json_line_appends_exactly_one_lf() {
    let line = serialize_json_line(&json!({"type": "agent_start"})).unwrap();
    assert_eq!(line, "{\"type\":\"agent_start\"}\n");
}

#[tokio::test]
async fn jsonl_reader_splits_only_on_lf_and_strips_cr() {
    let input = b"{\"type\":\"a\"}\r\n{\"message\":\"line\\u2028inside\"}\n{\"type\":\"c\"}";
    let lines = read_jsonl_lines(&input[..]).await.unwrap();
    assert_eq!(
        lines,
        vec![
            "{\"type\":\"a\"}".to_string(),
            "{\"message\":\"line\\u2028inside\"}".to_string(),
            "{\"type\":\"c\"}".to_string(),
        ]
    );
}

#[tokio::test]
async fn jsonl_reader_handles_chunk_boundaries() {
    let (mut writer, reader) = tokio::io::duplex(8);
    let task = tokio::spawn(async move { read_jsonl_lines(reader).await.unwrap() });
    writer.write_all(b"{\"type\"").await.unwrap();
    writer.write_all(b":\"a\"}\n{\"type\":\"b\"}").await.unwrap();
    drop(writer);
    let lines = task.await.unwrap();
    assert_eq!(
        lines,
        vec!["{\"type\":\"a\"}".to_string(), "{\"type\":\"b\"}".to_string()]
    );
}
```

- [ ] **Step 2: Run the failing JSONL tests**

Run:

```bash
cargo test -p pi-coding-agent --test protocol_jsonl
```

Expected: FAIL because `protocol::jsonl` does not exist.

- [ ] **Step 3: Create the protocol module**

Create `crates/pi-coding-agent/src/protocol/mod.rs`:

```rust
pub mod jsonl;
pub mod types;
```

Add to `crates/pi-coding-agent/src/lib.rs`:

```rust
pub mod protocol;
```

- [ ] **Step 4: Implement JSONL helpers**

Create `crates/pi-coding-agent/src/protocol/jsonl.rs`:

```rust
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncReadExt};

pub fn serialize_json_line<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let mut line = serde_json::to_string(value)?;
    line.push('\n');
    Ok(line)
}

pub async fn read_jsonl_lines<R>(mut reader: R) -> std::io::Result<Vec<String>>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;

    let mut lines = Vec::new();
    let mut start = 0;
    for (idx, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            let mut line = String::from_utf8_lossy(&bytes[start..idx]).to_string();
            if line.ends_with('\r') {
                line.pop();
            }
            lines.push(line);
            start = idx + 1;
        }
    }

    if start < bytes.len() {
        let mut line = String::from_utf8_lossy(&bytes[start..]).to_string();
        if line.ends_with('\r') {
            line.pop();
        }
        lines.push(line);
    }

    Ok(lines)
}
```

The first implementation returns all lines for tests. Task 6 adds streaming command processing for
RPC mode.

- [ ] **Step 5: Add protocol wire structs**

Create `crates/pi-coding-agent/src/protocol/types.rs`:

```rust
use pi_agent_core::session::StoredAgentMessage;
use pi_agent_core::{QueueMode, ThinkingLevel};
use pi_ai::types::{AssistantMessageEvent, ContentBlock, Model};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type")]
pub enum ProtocolEvent {
    #[serde(rename = "agent_start")]
    AgentStart,
    #[serde(rename = "turn_start")]
    TurnStart,
    #[serde(rename = "message_start")]
    MessageStart { message: StoredAgentMessage },
    #[serde(rename = "message_update")]
    MessageUpdate {
        message: StoredAgentMessage,
        #[serde(rename = "assistantMessageEvent")]
        assistant_message_event: AssistantMessageEvent,
    },
    #[serde(rename = "message_end")]
    MessageEnd { message: StoredAgentMessage },
    #[serde(rename = "tool_execution_start")]
    ToolExecutionStart {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        args: serde_json::Value,
    },
    #[serde(rename = "tool_execution_end")]
    ToolExecutionEnd {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        result: ToolExecutionResult,
        #[serde(rename = "isError")]
        is_error: bool,
    },
    #[serde(rename = "turn_end")]
    TurnEnd {
        message: StoredAgentMessage,
        #[serde(rename = "toolResults")]
        tool_results: Vec<StoredAgentMessage>,
    },
    #[serde(rename = "queue_update")]
    QueueUpdate {
        steering: Vec<String>,
        #[serde(rename = "followUp")]
        follow_up: Vec<String>,
    },
    #[serde(rename = "compaction_start")]
    CompactionStart { reason: CompactionReason },
    #[serde(rename = "compaction_end")]
    CompactionEnd {
        reason: CompactionReason,
        result: Option<CompactionProtocolResult>,
        aborted: bool,
        #[serde(rename = "willRetry")]
        will_retry: bool,
        #[serde(rename = "errorMessage", skip_serializing_if = "Option::is_none")]
        error_message: Option<String>,
    },
    #[serde(rename = "agent_end")]
    AgentEnd { messages: Vec<StoredAgentMessage> },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ToolExecutionResult {
    pub content: Vec<ContentBlock>,
    pub terminate: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CompactionReason {
    Manual,
    Threshold,
    Overflow,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CompactionProtocolResult {
    pub summary: String,
    #[serde(rename = "firstKeptMessageId")]
    pub first_kept_message_id: String,
    #[serde(rename = "tokensBefore")]
    pub tokens_before: u32,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum RpcCommand {
    #[serde(rename = "prompt")]
    Prompt {
        id: Option<String>,
        message: String,
        images: Option<Vec<ContentBlock>>,
        #[serde(rename = "streamingBehavior")]
        streaming_behavior: Option<StreamingBehavior>,
    },
    #[serde(rename = "steer")]
    Steer { id: Option<String>, message: String, images: Option<Vec<ContentBlock>> },
    #[serde(rename = "follow_up")]
    FollowUp { id: Option<String>, message: String, images: Option<Vec<ContentBlock>> },
    #[serde(rename = "abort")]
    Abort { id: Option<String> },
    #[serde(rename = "new_session")]
    NewSession {
        id: Option<String>,
        #[serde(rename = "parentSession")]
        parent_session: Option<String>,
    },
    #[serde(rename = "get_state")]
    GetState { id: Option<String> },
    #[serde(rename = "set_thinking_level")]
    SetThinkingLevel { id: Option<String>, level: ThinkingLevel },
    #[serde(rename = "set_steering_mode")]
    SetSteeringMode { id: Option<String>, mode: QueueMode },
    #[serde(rename = "set_follow_up_mode")]
    SetFollowUpMode { id: Option<String>, mode: QueueMode },
    #[serde(rename = "compact")]
    Compact {
        id: Option<String>,
        #[serde(rename = "customInstructions")]
        custom_instructions: Option<String>,
    },
    #[serde(rename = "set_auto_compaction")]
    SetAutoCompaction { id: Option<String>, enabled: bool },
    #[serde(rename = "get_session_stats")]
    GetSessionStats { id: Option<String> },
    #[serde(rename = "get_last_assistant_text")]
    GetLastAssistantText { id: Option<String> },
    #[serde(rename = "set_session_name")]
    SetSessionName { id: Option<String>, name: String },
    #[serde(rename = "get_messages")]
    GetMessages { id: Option<String> },
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum StreamingBehavior {
    #[serde(rename = "steer")]
    Steer,
    #[serde(rename = "followUp")]
    FollowUp,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RpcSessionState {
    pub model: Option<Model>,
    #[serde(rename = "thinkingLevel")]
    pub thinking_level: ThinkingLevel,
    #[serde(rename = "isStreaming")]
    pub is_streaming: bool,
    #[serde(rename = "isCompacting")]
    pub is_compacting: bool,
    #[serde(rename = "steeringMode")]
    pub steering_mode: QueueMode,
    #[serde(rename = "followUpMode")]
    pub follow_up_mode: QueueMode,
    #[serde(rename = "sessionFile", skip_serializing_if = "Option::is_none")]
    pub session_file: Option<String>,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "sessionName", skip_serializing_if = "Option::is_none")]
    pub session_name: Option<String>,
    #[serde(rename = "autoCompactionEnabled")]
    pub auto_compaction_enabled: bool,
    #[serde(rename = "messageCount")]
    pub message_count: usize,
    #[serde(rename = "pendingMessageCount")]
    pub pending_message_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RpcResponse {
    #[serde(rename = "type")]
    pub response_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub command: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl RpcResponse {
    pub fn success(id: Option<String>, command: impl Into<String>, data: Option<serde_json::Value>) -> Self {
        Self { response_type: "response", id, command: command.into(), success: true, data, error: None }
    }

    pub fn error(id: Option<String>, command: impl Into<String>, error: impl Into<String>) -> Self {
        Self { response_type: "response", id, command: command.into(), success: false, data: None, error: Some(error.into()) }
    }
}
```

If deriving `Deserialize` for `ThinkingLevel` and `QueueMode` is not available yet, add serde impls
or deserialize through helper string types in this task.

- [ ] **Step 6: Run JSONL tests**

Run:

```bash
cargo test -p pi-coding-agent --test protocol_jsonl
```

Expected: PASS.

---

## Task 3: Shared Session-Backed Runner

**Files:**
- Modify: `crates/pi-coding-agent/src/print_mode.rs`
- Create: `crates/pi-coding-agent/src/protocol/session_runner.rs`
- Modify: `crates/pi-coding-agent/src/protocol/mod.rs`
- Test: existing `crates/pi-coding-agent/tests/print_mode.rs`
- Test: existing `crates/pi-coding-agent/tests/session_print_mode.rs`

- [ ] **Step 1: Run current print/session tests before refactor**

Run:

```bash
cargo test -p pi-coding-agent --test print_mode
cargo test -p pi-coding-agent --test session_print_mode
```

Expected: PASS before editing. If either fails from unrelated local work, record the failing test
name in the implementation notes and keep the refactor minimal.

- [ ] **Step 2: Extract shared options and result structs**

Create `crates/pi-coding-agent/src/protocol/session_runner.rs`:

```rust
use crate::runtime::{PromptInvocation, SessionRunOptions};
use crate::session::ResolvedSessionTarget;
use crate::CliError;
use pi_agent_core::{AgentMessage, AgentResources, AgentTool, ThinkingLevel, ToolExecutionMode};
use pi_ai::types::{AssistantMessage, Model};

pub struct SessionPromptOptions {
    pub prompt: String,
    pub model: Model,
    pub api_key: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: u32,
    pub tools: Vec<AgentTool>,
    pub register_builtins: bool,
    pub session: Option<SessionRunOptions>,
    pub session_target: Option<ResolvedSessionTarget>,
    pub session_name: Option<String>,
    pub thinking_level: Option<ThinkingLevel>,
    pub tool_execution: Option<ToolExecutionMode>,
    pub resources: AgentResources,
    pub invocation: PromptInvocation,
}

pub struct SessionPromptResult {
    pub final_message: AssistantMessage,
    pub messages: Vec<AgentMessage>,
}

pub type CoreEventSink<'a> =
    &'a mut (dyn FnMut(&pi_agent_core::AgentEvent) -> Result<(), CliError> + Send);
```

Export it in `protocol/mod.rs`:

```rust
pub mod session_runner;
```

- [ ] **Step 3: Move session capture helpers**

Move these responsibilities from `print_mode.rs` into `session_runner.rs` without changing behavior:

```rust
pub async fn run_session_prompt(
    options: SessionPromptOptions,
    mut on_event: Option<CoreEventSink<'_>>,
) -> Result<SessionPromptResult, CliError>
```

The function must:

1. Register built-in providers when requested.
2. Build `AgentConfig` through `build_agent_config`.
3. Enable default compaction when sessions are enabled.
4. Open/hydrate the active session using existing `open_active_session`.
5. Add tools to the agent.
6. Start `PromptInvocation::Text`, `PromptInvocation::Skill`, or `PromptInvocation::PromptTemplate`.
7. Call `on_event` for every core event before matching it.
8. Preserve current `AgentDone`, `AgentError`, and `SessionCompacted` handling.
9. Capture new messages, session info, and compaction entries using the existing append logic.
10. Return `SessionPromptResult` with the final assistant message and final agent messages.

Keep `PendingCompaction`, `assistant_text`, `capture_session_messages`, and `agent_message_id` in
`session_runner.rs` if they become shared. Re-export `assistant_text` only inside the crate:

```rust
pub(crate) fn assistant_text(message: &AssistantMessage) -> String
```

- [ ] **Step 4: Rebuild print mode on the shared runner**

Replace the body of `run_print_mode` with:

```rust
pub async fn run_print_mode(options: PrintModeOptions) -> Result<String, CliError> {
    let result = crate::protocol::session_runner::run_session_prompt(
        crate::protocol::session_runner::SessionPromptOptions {
            prompt: options.prompt,
            model: options.model,
            api_key: options.api_key,
            system_prompt: options.system_prompt,
            max_turns: options.max_turns,
            tools: options.tools,
            register_builtins: options.register_builtins,
            session: options.session,
            session_target: options.session_target,
            session_name: options.session_name,
            thinking_level: options.thinking_level,
            tool_execution: options.tool_execution,
            resources: options.resources,
            invocation: options.invocation,
        },
        None,
    )
    .await?;
    Ok(crate::protocol::session_runner::assistant_text(&result.final_message))
}
```

Keep `PrintModeOptions` public fields unchanged so existing tests and callers compile.

- [ ] **Step 5: Run print/session tests after refactor**

Run:

```bash
cargo test -p pi-coding-agent --test print_mode
cargo test -p pi-coding-agent --test session_print_mode
```

Expected: PASS with no observable print-mode output changes.

---

## Task 4: Event Adapter and JSON Mode

**Files:**
- Create: `crates/pi-coding-agent/src/protocol/events.rs`
- Create: `crates/pi-coding-agent/src/protocol/json_mode.rs`
- Modify: `crates/pi-coding-agent/src/protocol/mod.rs`
- Modify: `crates/pi-coding-agent/src/lib.rs`
- Test: `crates/pi-coding-agent/tests/protocol_events.rs`
- Test: `crates/pi-coding-agent/tests/json_mode.rs`

- [ ] **Step 1: Write failing event adapter tests**

Create `crates/pi-coding-agent/tests/protocol_events.rs`:

```rust
use pi_agent_core::AgentToolResult;
use pi_ai::types::{AssistantMessage, AssistantMessageEvent, ContentBlock, StopReason};
use pi_coding_agent::protocol::events::ProtocolEventAdapter;
use pi_coding_agent::protocol::types::ProtocolEvent;

fn assistant(text: &str) -> AssistantMessage {
    let mut msg = AssistantMessage::empty("faux", "faux-model");
    msg.provider = Some("faux".into());
    msg.content.push(ContentBlock::Text {
        text: text.into(),
        text_signature: None,
    });
    msg.stop_reason = StopReason::Stop;
    msg
}

#[test]
fn adapter_maps_text_stream_to_message_lifecycle() {
    let mut adapter = ProtocolEventAdapter::new("faux".into(), "faux-model".into());
    let msg = assistant("hi");
    let events = adapter.push(&pi_agent_core::AgentEvent::LlmEvent(
        AssistantMessageEvent::Start {
            content_index: None,
            partial: AssistantMessage::empty("faux", "faux-model"),
        },
    ));
    assert!(matches!(events[0], ProtocolEvent::MessageStart { .. }));

    let events = adapter.push(&pi_agent_core::AgentEvent::LlmEvent(
        AssistantMessageEvent::TextDelta {
            content_index: 0,
            delta: "hi".into(),
            partial: msg.clone(),
        },
    ));
    assert!(matches!(events[0], ProtocolEvent::MessageUpdate { .. }));

    let events = adapter.push(&pi_agent_core::AgentEvent::AgentDone { message: msg });
    assert!(events.iter().any(|event| matches!(event, ProtocolEvent::MessageEnd { .. })));
    assert!(events.iter().any(|event| matches!(event, ProtocolEvent::TurnEnd { .. })));
    assert!(events.iter().any(|event| matches!(event, ProtocolEvent::AgentEnd { .. })));
}

#[test]
fn adapter_maps_tool_events_with_content_result() {
    let mut adapter = ProtocolEventAdapter::new("faux".into(), "faux-model".into());
    let mut msg = AssistantMessage::empty("faux", "faux-model");
    msg.provider = Some("faux".into());
    msg.content.push(ContentBlock::ToolCall {
        id: "tool_1".into(),
        name: "read".into(),
        arguments: serde_json::json!({"path": "Cargo.toml"}),
        thought_signature: None,
    });
    adapter.push(&pi_agent_core::AgentEvent::LlmEvent(AssistantMessageEvent::Done {
        reason: StopReason::ToolUse,
        message: msg,
    }));

    let events = adapter.push(&pi_agent_core::AgentEvent::ToolCallStart {
        tool_call_id: "tool_1".into(),
        tool_name: "read".into(),
    });
    assert!(matches!(events[0], ProtocolEvent::ToolExecutionStart { .. }));

    let events = adapter.push(&pi_agent_core::AgentEvent::ToolCallEnd {
        tool_call_id: "tool_1".into(),
        tool_name: "read".into(),
        result: AgentToolResult {
            content: vec![ContentBlock::Text {
                text: "file".into(),
                text_signature: None,
            }],
            is_error: false,
            terminate: false,
        },
    });
    assert!(matches!(events[0], ProtocolEvent::ToolExecutionEnd { is_error: false, .. }));
}
```

- [ ] **Step 2: Run failing event adapter tests**

Run:

```bash
cargo test -p pi-coding-agent --test protocol_events
```

Expected: FAIL because `ProtocolEventAdapter` does not exist.

- [ ] **Step 3: Implement event adapter state**

Create `crates/pi-coding-agent/src/protocol/events.rs`:

```rust
use crate::protocol::types::{
    CompactionProtocolResult, CompactionReason, ProtocolEvent, ToolExecutionResult,
};
use pi_agent_core::session::{agent_message_to_stored, StoredAgentMessage, StoredUsage, StoredUsageCost};
use pi_agent_core::{AgentEvent, AgentMessage};
use pi_ai::types::{AssistantMessage, AssistantMessageEvent, ContentBlock, StopReason};
use std::collections::HashMap;

pub struct ProtocolEventAdapter {
    api: String,
    model: String,
    messages: Vec<StoredAgentMessage>,
    current_assistant: Option<AssistantMessage>,
    current_tool_results: Vec<StoredAgentMessage>,
    tool_args: HashMap<String, serde_json::Value>,
}
```

Add helper conversion:

```rust
fn stored_assistant(message: &AssistantMessage) -> StoredAgentMessage {
    StoredAgentMessage::Assistant {
        content: message.content.clone(),
        api: message.api.clone(),
        provider: message.provider.clone().unwrap_or_default(),
        model: message.model.clone(),
        response_model: message.response_model.clone(),
        response_id: message.response_id.clone(),
        usage: StoredUsage {
            input: message.usage.input,
            output: message.usage.output,
            cache_read: message.usage.cache_read,
            cache_write: message.usage.cache_write,
            total: message.usage.total_tokens,
            cost: StoredUsageCost {
                input: message.usage.cost.input,
                output: message.usage.cost.output,
                cache_read: message.usage.cost.cache_read,
                cache_write: message.usage.cost.cache_write,
            },
        },
        stop_reason: message.stop_reason.clone(),
        error_message: message.error_message.clone(),
        timestamp: message.timestamp,
    }
}

fn stored_error_assistant(api: &str, model: &str, error: &str) -> StoredAgentMessage {
    StoredAgentMessage::Assistant {
        content: Vec::new(),
        api: api.to_string(),
        provider: String::new(),
        model: model.to_string(),
        response_model: None,
        response_id: None,
        usage: StoredUsage::default(),
        stop_reason: StopReason::Error,
        error_message: Some(error.to_string()),
        timestamp: 0,
    }
}
```

Implement `ProtocolEventAdapter::push(&mut self, event: &AgentEvent) -> Vec<ProtocolEvent>`:

- `TurnStart` maps to `TurnStart` and clears `current_tool_results`.
- `LlmEvent(Start { partial, .. })` stores `partial` and emits `MessageStart`.
- Streaming LLM events update `current_assistant` from their `partial` and emit `MessageUpdate`.
- `LlmEvent(Done | Error)` stores the final assistant and records tool-call arguments by id.
- `ToolCallStart` emits `ToolExecutionStart` with args from `tool_args` or `serde_json::Value::Null`.
- `ToolCallEnd` emits `ToolExecutionEnd` and stores a `StoredAgentMessage::ToolResult`.
- `SessionCompacted` emits `CompactionStart { reason: Threshold }` then `CompactionEnd`.
- `AgentDone` emits final `MessageEnd`, `TurnEnd`, and `AgentEnd`.
- `AgentError` emits the error assistant sequence described in the spec.

- [ ] **Step 4: Write failing JSON mode tests**

Create `crates/pi-coding-agent/tests/json_mode.rs`:

```rust
use pi_ai::providers::faux::{FauxCall, FauxProvider, FauxResponse, FauxToolCall};
use pi_ai::registry;
use pi_ai::types::{Model, ModelCost, ModelInput, StopReason};
use pi_coding_agent::{run_cli_with_options, CliRunOptions};
use std::sync::Arc;

fn faux_model(api: &str) -> Model {
    Model {
        id: "faux-model".into(),
        name: "Faux Model".into(),
        api: api.into(),
        provider: "faux".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 8_000,
        max_tokens: 1_024,
        headers: None,
        compat: None,
    }
}

fn json_lines(stdout: &str) -> Vec<serde_json::Value> {
    stdout
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[tokio::test]
async fn json_mode_emits_session_header_and_lifecycle_events() {
    let api = "pi-coding-json-lifecycle";
    registry::register(api, Arc::new(FauxProvider::simple_text("Hello")));

    let output = run_cli_with_options(
        vec!["--mode".to_string(), "json".to_string(), "hello".to_string()],
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert!(output.stderr.is_empty());
    let lines = json_lines(&output.stdout);
    assert_eq!(lines[0]["type"], "session");
    assert!(lines.iter().any(|line| line["type"] == "agent_start"));
    assert!(lines.iter().any(|line| line["type"] == "turn_start"));
    assert!(lines.iter().any(|line| line["type"] == "message_update"));
    assert!(lines.iter().any(|line| line["type"] == "agent_end"));
    registry::unregister(api);
}

#[tokio::test]
async fn json_mode_emits_tool_execution_events() {
    let api = "pi-coding-json-tool";
    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxCall {
                responses: vec![FauxResponse {
                    text_deltas: vec![],
                    thinking_deltas: vec![],
                    tool_calls: vec![FauxToolCall {
                        id: "tool_1".into(),
                        name: "echo".into(),
                        deltas: vec!["{\"text\":\"hi\"}".into()],
                        final_arguments: serde_json::json!({"text": "hi"}),
                    }],
                }],
                stop_reason: StopReason::ToolUse,
            },
            FauxProvider::text_call("done", StopReason::Stop),
        ])),
    );

    let tool = pi_agent_core::AgentTool::new_text(
        "echo",
        "echo input",
        serde_json::json!({"type":"object","properties":{"text":{"type":"string"}}}),
        |args| async move {
            Ok(format!("echo: {}", args["text"].as_str().unwrap_or("")))
        },
    );

    let output = run_cli_with_options(
        vec!["--mode".to_string(), "json".to_string(), "echo hi".to_string()],
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: vec![tool],
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
    let lines = json_lines(&output.stdout);
    assert!(lines.iter().any(|line| line["type"] == "tool_execution_start"));
    assert!(lines.iter().any(|line| line["type"] == "tool_execution_end"));
    registry::unregister(api);
}
```

- [ ] **Step 5: Run failing JSON mode tests**

Run:

```bash
cargo test -p pi-coding-agent --test json_mode
```

Expected: FAIL because JSON mode is not routed.

- [ ] **Step 6: Implement JSON mode runner**

Create `crates/pi-coding-agent/src/protocol/json_mode.rs`:

```rust
use crate::protocol::events::ProtocolEventAdapter;
use crate::protocol::jsonl::serialize_json_line;
use crate::protocol::session_runner::{run_session_prompt, SessionPromptOptions};
use crate::{CliError, CliOutput};
use pi_agent_core::session::{create_session_id, create_timestamp, SessionHeader};

pub async fn run_json_mode(options: SessionPromptOptions) -> CliOutput {
    let header = SessionHeader {
        entry_type: "session".into(),
        version: 3,
        id: create_session_id(),
        timestamp: create_timestamp(),
        cwd: std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .display()
            .to_string(),
        parent_session: None,
    };

    let mut stdout = match serialize_json_line(&header) {
        Ok(line) => line,
        Err(error) => return CliOutput::failure(CliError::AgentFailure(error.to_string())),
    };
    let mut adapter = ProtocolEventAdapter::new(options.model.api.clone(), options.model.id.clone());
    stdout.push_str("{\"type\":\"agent_start\"}\n");

    let run = run_session_prompt(
        options,
        Some(&mut |event| {
            for protocol_event in adapter.push(event) {
                stdout.push_str(
                    &serialize_json_line(&protocol_event)
                        .map_err(|e| CliError::AgentFailure(e.to_string()))?,
                );
            }
            Ok(())
        }),
    )
    .await;

    match run {
        Ok(_) => CliOutput::success(stdout),
        Err(error) => CliOutput {
            exit_code: 1,
            stdout,
            stderr: format!("{error}\n"),
        },
    }
}
```

If borrow checker constraints make the closure awkward, use an internal `Vec<ProtocolEvent>` and
serialize after each event callback. Keep the output order identical to callback order.

Update `protocol/mod.rs`:

```rust
pub mod events;
pub mod json_mode;
```

- [ ] **Step 7: Route `CliMode::Json`**

In `run_cli_with_options`, after selecting model/resources/session target, build
`SessionPromptOptions` through a local helper and route:

```rust
match parsed.mode {
    CliMode::Print => { /* existing print route */ }
    CliMode::Json => {
        return protocol::json_mode::run_json_mode(session_prompt_options).await;
    }
    CliMode::Rpc => {
        return CliOutput::failure(CliError::UnsupportedMode("rpc".into()));
    }
}
```

Keep print mode behavior unchanged.

- [ ] **Step 8: Run JSON mode tests**

Run:

```bash
cargo test -p pi-coding-agent --test protocol_events
cargo test -p pi-coding-agent --test json_mode
cargo test -p pi-coding-agent --test cli
cargo test -p pi-coding-agent --test print_mode
```

Expected: PASS.

---

## Task 5: RPC Command Types and Basic Command Loop

**Files:**
- Create: `crates/pi-coding-agent/src/protocol/rpc.rs`
- Modify: `crates/pi-coding-agent/src/protocol/mod.rs`
- Modify: `crates/pi-coding-agent/src/lib.rs`
- Test: `crates/pi-coding-agent/tests/rpc_mode.rs`

- [ ] **Step 1: Write failing RPC parse and unsupported-command tests**

Create `crates/pi-coding-agent/tests/rpc_mode.rs`:

```rust
use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::{Model, ModelCost, ModelInput};
use pi_coding_agent::{protocol::rpc::run_rpc_mode_for_io, CliRunOptions};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

fn faux_model(api: &str) -> Model {
    Model {
        id: "faux-model".into(),
        name: "Faux Model".into(),
        api: api.into(),
        provider: "faux".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 8_000,
        max_tokens: 1_024,
        headers: None,
        compat: None,
    }
}

fn parse_lines(bytes: &[u8]) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[tokio::test]
async fn rpc_parse_error_keeps_process_alive_for_next_command() {
    let api = "pi-coding-rpc-parse";
    registry::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let input = b"{bad json}\n{\"id\":\"s1\",\"type\":\"get_state\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["type"], "response");
    assert_eq!(lines[0]["command"], "parse");
    assert_eq!(lines[0]["success"], false);
    assert_eq!(lines[1]["id"], "s1");
    assert_eq!(lines[1]["command"], "get_state");
    assert_eq!(lines[1]["success"], true);
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_unsupported_command_returns_error_response() {
    let api = "pi-coding-rpc-unsupported";
    registry::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let input = b"{\"id\":\"m1\",\"type\":\"set_model\",\"provider\":\"faux\",\"modelId\":\"x\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "m1");
    assert_eq!(lines[0]["command"], "set_model");
    assert_eq!(lines[0]["success"], false);
    assert_eq!(
        lines[0]["error"],
        "unsupported command in Rust M5: set_model"
    );
    registry::unregister(api);
}
```

- [ ] **Step 2: Run failing RPC tests**

Run:

```bash
cargo test -p pi-coding-agent --test rpc_mode
```

Expected: FAIL because `protocol::rpc` does not exist.

- [ ] **Step 3: Add RPC module and output writer**

Create `crates/pi-coding-agent/src/protocol/rpc.rs`:

```rust
use crate::protocol::jsonl::{read_jsonl_lines, serialize_json_line};
use crate::protocol::types::{RpcCommand, RpcResponse};
use crate::{CliError, CliRunOptions};
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

pub async fn write_rpc_response<W>(
    writer: &mut W,
    response: RpcResponse,
) -> Result<(), CliError>
where
    W: AsyncWrite + Unpin,
{
    let line = serialize_json_line(&response)
        .map_err(|e| CliError::AgentFailure(e.to_string()))?;
    writer
        .write_all(line.as_bytes())
        .await
        .map_err(|e| CliError::AgentFailure(e.to_string()))
}
```

Update `protocol/mod.rs`:

```rust
pub mod rpc;
```

- [ ] **Step 4: Implement command-name extraction for unsupported commands**

In `rpc.rs`, add:

```rust
fn command_type(value: &Value) -> String {
    value
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string()
}

fn command_id(value: &Value) -> Option<String> {
    value
        .get("id")
        .and_then(|v| v.as_str())
        .map(ToString::to_string)
}
```

Before deserializing into `RpcCommand`, parse into `serde_json::Value`. If the command type is not
in the M5 subset, emit:

```rust
RpcResponse::error(
    command_id(&value),
    command_type(&value).clone(),
    format!("unsupported command in Rust M5: {}", command_type(&value)),
)
```

Use this M5 subset set:

```rust
matches!(
    command_type.as_str(),
    "prompt"
        | "steer"
        | "follow_up"
        | "abort"
        | "new_session"
        | "get_state"
        | "set_thinking_level"
        | "set_steering_mode"
        | "set_follow_up_mode"
        | "compact"
        | "set_auto_compaction"
        | "get_session_stats"
        | "get_last_assistant_text"
        | "set_session_name"
        | "get_messages"
)
```

- [ ] **Step 5: Implement basic `run_rpc_mode_for_io`**

Add:

```rust
pub async fn run_rpc_mode_for_io<R, W>(
    reader: R,
    writer: &mut W,
    options: CliRunOptions,
) -> Result<(), CliError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut state = RpcState::new(options)?;
    let lines = read_jsonl_lines(reader)
        .await
        .map_err(|e| CliError::AgentFailure(e.to_string()))?;

    for line in lines {
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(
                        None,
                        "parse",
                        format!("Failed to parse command: {error}"),
                    ),
                )
                .await?;
                continue;
            }
        };

        let command_name = command_type(&value);
        if !is_supported_m5_command(&command_name) {
            write_rpc_response(
                writer,
                RpcResponse::error(
                    command_id(&value),
                    command_name.clone(),
                    format!("unsupported command in Rust M5: {command_name}"),
                ),
            )
            .await?;
            continue;
        }

        let command: RpcCommand = match serde_json::from_value(value) {
            Ok(command) => command,
            Err(error) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error(None, command_name, format!("Invalid command: {error}")),
                )
                .await?;
                continue;
            }
        };

        let response = state.handle_command(command).await;
        write_rpc_response(writer, response).await?;
    }

    Ok(())
}
```

Add a minimal `RpcState` in this task that supports only `get_state` and returns success. Other
supported commands can return a structured M5-internal message until Task 6:

```rust
struct RpcState {
    model: Option<pi_ai::types::Model>,
    thinking_level: pi_agent_core::ThinkingLevel,
    steering_mode: pi_agent_core::QueueMode,
    follow_up_mode: pi_agent_core::QueueMode,
}
```

- [ ] **Step 6: Run RPC basic tests**

Run:

```bash
cargo test -p pi-coding-agent --test rpc_mode
```

Expected: PASS for parse error, unsupported command, and `get_state`.

---

## Task 6: RPC Prompting, Queues, State, and Session Commands

**Files:**
- Modify: `crates/pi-coding-agent/src/protocol/rpc.rs`
- Modify: `crates/pi-coding-agent/src/protocol/types.rs`
- Test: `crates/pi-coding-agent/tests/rpc_mode.rs`
- Test: `crates/pi-coding-agent/tests/protocol_sessions.rs`

- [ ] **Step 1: Add failing RPC prompt/state tests**

Append to `crates/pi-coding-agent/tests/rpc_mode.rs`:

```rust
#[tokio::test]
async fn rpc_prompt_returns_response_then_agent_events() {
    let api = "pi-coding-rpc-prompt";
    registry::register(api, Arc::new(FauxProvider::simple_text("Hello")));

    let input = b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "p1");
    assert_eq!(lines[0]["command"], "prompt");
    assert_eq!(lines[0]["success"], true);
    assert!(lines.iter().any(|line| line["type"] == "agent_start"));
    assert!(lines.iter().any(|line| line["type"] == "agent_end"));
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_state_commands_update_get_state() {
    let api = "pi-coding-rpc-state";
    registry::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let input = b"{\"id\":\"t1\",\"type\":\"set_thinking_level\",\"level\":\"high\"}\n\
                  {\"id\":\"q1\",\"type\":\"set_steering_mode\",\"mode\":\"one-at-a-time\"}\n\
                  {\"id\":\"s1\",\"type\":\"get_state\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    let state = lines
        .iter()
        .find(|line| line["command"] == "get_state")
        .unwrap();
    assert_eq!(state["data"]["thinkingLevel"], "high");
    assert_eq!(state["data"]["steeringMode"], "one-at-a-time");
    registry::unregister(api);
}
```

- [ ] **Step 2: Add failing RPC session persistence test**

Create `crates/pi-coding-agent/tests/protocol_sessions.rs`:

```rust
use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::{Model, ModelCost, ModelInput};
use pi_coding_agent::{protocol::rpc::run_rpc_mode_for_io, CliRunOptions, SessionRunOptions};
use std::sync::Arc;
use tempfile::tempdir;

fn faux_model(api: &str) -> Model {
    Model {
        id: "faux-model".into(),
        name: "Faux Model".into(),
        api: api.into(),
        provider: "faux".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 8_000,
        max_tokens: 1_024,
        headers: None,
        compat: None,
    }
}

#[tokio::test]
async fn rpc_prompt_persists_session_messages() {
    let dir = tempdir().unwrap();
    let api = "pi-coding-rpc-session";
    registry::register(api, Arc::new(FauxProvider::simple_text("Hello")));

    let input = b"{\"id\":\"n1\",\"type\":\"set_session_name\",\"name\":\"rpc work\"}\n\
                  {\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            session: SessionRunOptions::enabled(dir.path().to_path_buf()),
        },
    )
    .await
    .unwrap();

    let session_files: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .flat_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some("jsonl"))
        .collect();
    assert_eq!(session_files.len(), 1);
    let contents = std::fs::read_to_string(session_files[0].path()).unwrap();
    assert!(contents.contains("\"type\":\"session\""));
    assert!(contents.contains("\"type\":\"session_info\""));
    assert!(contents.contains("\"role\":\"user\""));
    assert!(contents.contains("\"role\":\"assistant\""));
    registry::unregister(api);
}
```

- [ ] **Step 3: Run failing RPC behavior tests**

Run:

```bash
cargo test -p pi-coding-agent --test rpc_mode
cargo test -p pi-coding-agent --test protocol_sessions
```

Expected: FAIL because prompt execution and persistence are not wired.

- [ ] **Step 4: Implement RPC state**

In `rpc.rs`, expand `RpcState`:

```rust
struct RpcState {
    options: CliRunOptions,
    model: pi_ai::types::Model,
    thinking_level: pi_agent_core::ThinkingLevel,
    steering_mode: pi_agent_core::QueueMode,
    follow_up_mode: pi_agent_core::QueueMode,
    auto_compaction_enabled: bool,
    session_name: Option<String>,
    messages: Vec<pi_agent_core::session::StoredAgentMessage>,
    is_streaming: bool,
    is_compacting: bool,
    steering: Vec<String>,
    follow_up: Vec<String>,
}
```

Initialize `model` with `options.model_override.clone()` when present, otherwise use
`pi_ai::lookup_model(DEFAULT_MODEL_ID)` and return `CliError::UnknownModel` on failure.

- [ ] **Step 5: Implement state and setter responses**

`get_state` returns:

```rust
RpcResponse::success(
    id,
    "get_state",
    Some(serde_json::to_value(RpcSessionState {
        model: Some(self.model.clone()),
        thinking_level: self.thinking_level,
        is_streaming: self.is_streaming,
        is_compacting: self.is_compacting,
        steering_mode: self.steering_mode,
        follow_up_mode: self.follow_up_mode,
        session_file: None,
        session_id: "in-memory".into(),
        session_name: self.session_name.clone(),
        auto_compaction_enabled: self.auto_compaction_enabled,
        message_count: self.messages.len(),
        pending_message_count: self.steering.len() + self.follow_up.len(),
    }).unwrap()),
)
```

Setters mutate state and return success:

- `set_thinking_level`: `self.thinking_level = level`
- `set_steering_mode`: `self.steering_mode = mode`
- `set_follow_up_mode`: `self.follow_up_mode = mode`
- `set_auto_compaction`: `self.auto_compaction_enabled = enabled`
- `set_session_name`: reject empty trimmed names with `Session name cannot be empty`

- [ ] **Step 6: Implement prompt command**

For `RpcCommand::Prompt`, if `self.is_streaming` is true:

```rust
match streaming_behavior {
    Some(StreamingBehavior::Steer) => {
        self.steering.push(message);
        emit_queue_update(writer).await?;
        return Ok(RpcResponse::success(id, "prompt", None));
    }
    Some(StreamingBehavior::FollowUp) => {
        self.follow_up.push(message);
        emit_queue_update(writer).await?;
        return Ok(RpcResponse::success(id, "prompt", None));
    }
    None => {
        return Ok(RpcResponse::error(
            id,
            "prompt",
            "agent is streaming; set streamingBehavior to steer or followUp",
        ));
    }
}
```

When not streaming:

1. Write success response first.
2. Write `agent_start`.
3. Run `run_session_prompt` with a `ProtocolEventAdapter`.
4. Write adapter events as they arrive.
5. Store final `StoredAgentMessage`s in `self.messages`.
6. Set `self.is_streaming` true before the run and false after it settles.

Because `handle_command` now needs to write events as well as return responses, change its shape to:

```rust
async fn handle_command<W>(&mut self, command: RpcCommand, writer: &mut W) -> Result<(), CliError>
where
    W: AsyncWrite + Unpin
```

and let it write all responses/events directly.

- [ ] **Step 7: Implement queue commands**

For `steer` and `follow_up`, push the message text, write success, then emit:

```json
{"type":"queue_update","steering":[...],"followUp":[...]}
```

In M5, image payloads may be accepted and ignored only if the model does not support images. If
image handling is not wired through `AgentMessage` yet, reject non-empty `images` with:

```text
image prompt payloads are not supported in Rust M5 RPC mode
```

- [ ] **Step 8: Implement remaining M5 commands**

Implement:

- `abort`: cancel current run if a cancellation token is available; otherwise respond success and
  clear queued prompt state.
- `new_session`: clear `messages`, `steering`, `follow_up`, `session_name`, and respond
  `{"cancelled":false}`.
- `compact`: respond success with the latest available compaction result if `run_session_prompt`
  produced one; if manual compaction is not callable through current core APIs, return
  `success:false` with `manual compaction is not available in Rust M5`.
- `get_session_stats`: return counts derived from `self.messages`.
- `get_last_assistant_text`: return the joined text blocks from the last assistant message.
- `get_messages`: return `{"messages": self.messages}`.

Use exact command names from `rpc-types.ts` in every response.

- [ ] **Step 9: Run RPC behavior tests**

Run:

```bash
cargo test -p pi-coding-agent --test rpc_mode
cargo test -p pi-coding-agent --test protocol_sessions
```

Expected: PASS.

---

## Task 7: CLI Routing for RPC and Final Verification

**Files:**
- Modify: `crates/pi-coding-agent/src/lib.rs`
- Modify: `crates/pi-coding-agent/src/main.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc.rs`
- Test: `crates/pi-coding-agent/tests/cli.rs`
- Test: all protocol tests

- [ ] **Step 1: Add CLI-level JSON/RPC tests**

Append to `crates/pi-coding-agent/tests/cli.rs`:

```rust
#[tokio::test]
async fn json_mode_uses_injected_model_and_returns_jsonl() {
    let api = "pi-coding-cli-json";
    registry::register(api, Arc::new(FauxProvider::simple_text("Hello JSON")));

    let output = run_cli_with_options(
        vec!["--mode".to_string(), "json".to_string(), "hello".to_string()],
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert!(output.stderr.is_empty());
    assert!(output.stdout.lines().all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok()));
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_mode_is_not_run_through_buffered_cli_output() {
    let output = run_cli_with_options(
        vec!["--mode".to_string(), "rpc".to_string()],
        CliRunOptions {
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 1);
    assert_eq!(
        output.stderr,
        "unsupported mode: rpc requires the streaming binary entry point\n"
    );
}
```

- [ ] **Step 2: Run failing CLI tests**

Run:

```bash
cargo test -p pi-coding-agent --test cli
```

Expected: FAIL until final routing is explicit.

- [ ] **Step 3: Keep buffered `run_cli_with_options` honest**

In `run_cli_with_options`, keep JSON mode routed through `CliOutput` and return the explicit RPC
error:

```rust
CliMode::Rpc => {
    return CliOutput::failure(CliError::UnsupportedMode(
        "rpc requires the streaming binary entry point".into(),
    ));
}
```

This preserves the test-facing API. Production `main.rs` should parse args and call the streaming
RPC runner directly before falling back to `run_cli`.

- [ ] **Step 4: Add production RPC entry point**

In `protocol/rpc.rs`, add:

```rust
pub async fn run_rpc_mode_stdio(options: CliRunOptions) -> Result<(), CliError> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    run_rpc_mode_for_io(stdin, &mut stdout, options).await
}
```

In `main.rs`, parse early:

```rust
#[tokio::main]
async fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if let Ok(parsed) = pi_coding_agent::parse_args(raw.clone()) {
        if parsed.mode == pi_coding_agent::CliMode::Rpc {
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let options = pi_coding_agent::CliRunOptions {
                model_override: None,
                tools: pi_coding_agent::builtin_tools(cwd.clone()),
                register_builtins: true,
                session: pi_coding_agent::SessionRunOptions::enabled(cwd),
            };
            match pi_coding_agent::protocol::rpc::run_rpc_mode_stdio(options).await {
                Ok(()) => std::process::exit(0),
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            }
        }
    }

    let output = pi_coding_agent::run_cli(raw).await;
    if !output.stdout.is_empty() {
        print!("{}", output.stdout);
    }
    if !output.stderr.is_empty() {
        eprint!("{}", output.stderr);
    }
    std::process::exit(output.exit_code);
}
```

If early parsing fails, fall through to `run_cli` so existing error formatting stays centralized.

- [ ] **Step 5: Run all pi-coding-agent tests**

Run:

```bash
cargo test -p pi-coding-agent
```

Expected: PASS.

- [ ] **Step 6: Run workspace verification**

Run:

```bash
cargo fmt --check
cargo test -p pi-agent-core
cargo test -p pi-coding-agent
cargo test --workspace
cargo check --workspace
```

Expected: PASS. If the existing unrelated DeepSeek test from ROADMAP M0 is still failing, record it
as pre-existing and include the exact failing test name in the implementation summary.

---

## Self-Review Checklist

- Spec coverage: Tasks cover CLI mode parsing, JSONL framing, protocol event types, core-event
  adaptation, JSON mode, RPC mode, state commands, queue commands, session persistence, and
  verification.
- Placeholder scan: This plan contains no intentionally blank implementation sections.
- Type consistency: `CliMode`, `ProtocolEvent`, `RpcCommand`, `RpcResponse`, `RpcSessionState`, and
  `SessionPromptOptions` names are consistent across tasks.
- Scope control: Full TypeScript RPC parity remains outside M5; unsupported commands return
  structured errors.
