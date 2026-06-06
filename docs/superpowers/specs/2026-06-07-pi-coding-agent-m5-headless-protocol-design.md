# Design: Rust M5 headless protocol modes

- Date: 2026-06-07
- Status: Draft (pending review)
- Scope: M5 of the Rust port ROADMAP - `--mode json` event stream and `--mode rpc` stdio JSON-RPC for `pi-coding-agent`.
- Depends on: M1 built-in tools, M3 JSONL sessions, M4 harness capabilities, and the current `pi-agent-core` event stream.

## 1. Context

The Rust coding agent can run print-mode prompts, execute built-in tools, persist JSONL sessions,
load resources, and expose several headless harness controls. It still has only one user-facing
runtime mode: `-p` / `--print`, which returns the final assistant text after the agent settles.

TypeScript has two headless protocol modes that are the M5 reference:

- `pi/packages/coding-agent/docs/json.md` and `src/modes/print-mode.ts` define a JSON event stream
  mode where stdout is JSONL: a session header followed by session/agent events.
- `pi/packages/coding-agent/docs/rpc.md`, `src/modes/rpc/jsonl.ts`,
  `src/modes/rpc/rpc-types.ts`, and `src/modes/rpc/rpc-mode.ts` define a long-running stdio
  protocol: commands arrive as strict LF-delimited JSON lines on stdin; responses and agent events
  are emitted as JSON lines on stdout.

Rust should not expose `pi-agent-core::AgentEvent` directly as the protocol. The Rust core event
enum is intentionally lower-level (`TurnStart`, `LlmEvent`, `ToolCallStart`, `ToolCallEnd`,
`AgentDone`, `AgentError`, `SessionCompacted`) and does not match TypeScript's
`agent_start/message_start/message_update/message_end/tool_execution_*/turn_end/agent_end` wire
shape. M5 should add a protocol adapter layer in `pi-coding-agent` that translates core events into
stable JSON records and can later serve interactive/TUI integration without changing the wire
contract.

## 2. Goals and success criteria

Build deterministic headless protocol modes for the Rust coding agent.

Done when:

1. `pi-coding-agent` parses `--mode print`, `--mode json`, and `--mode rpc`. Existing `-p` /
   `--print` behavior remains compatible and selects print mode.
2. JSON mode runs one prompt and writes only JSONL protocol records to stdout. Human diagnostics and
   runtime errors go to stderr.
3. JSON mode emits a session header as the first line. When session persistence is disabled, it
   emits an ephemeral in-memory header with a generated id and cwd but does not write a session
   file.
4. JSON mode emits TypeScript-compatible event names for the supported subset:
   `agent_start`, `turn_start`, `message_start`, `message_update`, `message_end`,
   `tool_execution_start`, `tool_execution_end`, `turn_end`, `queue_update`,
   `compaction_start`, `compaction_end`, and `agent_end`.
5. `pi-coding-agent` adds a protocol adapter that accumulates assistant/tool state from
   `pi-agent-core::AgentEvent` and serializes model-visible messages using the TypeScript JSON
   shapes already used by M3 sessions.
6. A Rust `AgentError` is represented as an assistant message with `stopReason: "error"` and
   `errorMessage`, followed by `message_end`, `turn_end`, and `agent_end`; it does not introduce a
   new top-level `agent_error` protocol event.
7. Tool execution events include tool call id, tool name, arguments when known, `isError`, and a
   result object containing model-visible `content` and `terminate`. Full TypeScript `details` are
   deferred until Rust tools expose structured details.
8. RPC mode owns stdout while running. It reads strict LF-delimited JSON commands from stdin, strips
   a trailing CR for CRLF clients, and never uses a generic line reader that splits on Unicode
   separators.
9. RPC mode implements the M5 command subset: `prompt`, `steer`, `follow_up`, `abort`,
   `new_session`, `get_state`, `set_thinking_level`, `set_steering_mode`, `set_follow_up_mode`,
   `compact`, `set_auto_compaction`, `get_session_stats`, `get_last_assistant_text`,
   `set_session_name`, and `get_messages`.
10. RPC commands not in the M5 subset return a structured response with
    `success: false` and `error: "unsupported command in Rust M5: <command>"`.
11. RPC prompt commands return their `response` after the prompt is accepted or rejected. Agent
    events continue streaming asynchronously after prompt acceptance.
12. Concurrent RPC `prompt` while the agent is running is rejected unless `streamingBehavior` is
    `steer` or `followUp`; with those values it is routed to the corresponding queue.
13. RPC responses preserve request `id` when present and always include `type: "response"`,
    `command`, and `success`.
14. RPC parse errors return
    `{"type":"response","command":"parse","success":false,"error":"Failed to parse command: ..."}`
    and keep the process alive.
15. Session persistence continues using the M3 JSONL v3 format. JSON/RPC modes append the same
    message and metadata entries as print mode after each accepted prompt run settles.
16. All tests are deterministic and offline. Protocol tests use the faux provider, temp session
    directories, and in-memory stdin/stdout harnesses.

Required verification:

- `cargo fmt --check`
- `cargo test -p pi-agent-core`
- `cargo test -p pi-coding-agent`
- `cargo test --workspace`
- `cargo check --workspace`

## 3. Non-goals

M5 does not implement the interactive TUI, terminal raw mode, UI components, themes, or extension UI
dialogs.

M5 does not implement the full TypeScript RPC command surface. Provider/model cycling,
`get_available_models`, auto retry, direct bash execution, HTML export, session switching, fork,
clone, fork-message browsing, and slash-command discovery are deferred unless already trivial after
the M5 adapter exists.

M5 does not add structured tool details to Rust built-in tools. The protocol reserves the result
shape for details, but the first implementation exposes only model-visible content and termination
state.

M5 does not change `pi-agent-core` into a public protocol crate. Protocol JSON types stay in
`pi-coding-agent` because the wire format belongs to the coding-agent application layer and includes
session/CLI decisions.

M5 does not require live provider keys, network access, or TypeScript test execution.

## 4. Compatibility contract

### 4.1 JSONL framing

Every stdout record in JSON and RPC mode is one JSON object followed by `\n`.

Input for RPC mode is split only on byte `\n`. If a line ends with `\r`, the trailing `\r` is
removed before JSON parsing. Blank lines are invalid commands and receive a parse-error response.

Serializer behavior:

```json
{"type":"agent_start"}
```

is written as:

```text
{"type":"agent_start"}\n
```

### 4.2 Session header

The first JSON mode line is always:

```json
{"type":"session","version":3,"id":"019de8c2-de29-73e9-ae0c-e134db34c447","timestamp":"2026-06-07T00:00:00.000Z","cwd":"/abs/project"}
```

If a persisted session is active, the header is the actual session header. If sessions are disabled,
the header is generated in memory and contains no `parentSession`.

RPC mode does not emit a session header on startup. Clients can request current state with
`get_state`. Agent/session events are emitted only as commands cause them.

### 4.3 Event shapes

Core lifecycle:

```json
{"type":"agent_start"}
{"type":"turn_start"}
{"type":"message_start","message":{"role":"assistant","content":[],"api":"faux","provider":"faux","model":"test","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0,"cost":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0}},"stopReason":"stop","timestamp":0}}
{"type":"message_update","message":{},"assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"hi","partial":{}}}
{"type":"message_end","message":{}}
{"type":"turn_end","message":{},"toolResults":[]}
{"type":"agent_end","messages":[]}
```

M5 uses the TypeScript session message shape for `message` payloads:

- user: `{"role":"user","content":[...],"timestamp":...}`
- assistant: `{"role":"assistant","content":[...],"api":"...","provider":"...","model":"...","usage":...,"stopReason":"...","timestamp":...}`
- tool result:
  `{"role":"toolResult","toolCallId":"...","toolName":"...","content":[...],"isError":false,"timestamp":...}`

For assistant usage in protocol messages, M5 uses the TypeScript session field `total`, not the
`pi-ai` internal serde field `totalTokens`.

Tool execution:

```json
{"type":"tool_execution_start","toolCallId":"tool_1","toolName":"read","args":{"path":"Cargo.toml"}}
{"type":"tool_execution_end","toolCallId":"tool_1","toolName":"read","result":{"content":[{"type":"text","text":"..."}],"terminate":false},"isError":false}
```

Queue and compaction events:

```json
{"type":"queue_update","steering":["new instruction"],"followUp":[]}
{"type":"compaction_start","reason":"threshold"}
{"type":"compaction_end","reason":"threshold","result":{"summary":"...","firstKeptMessageId":"m3","tokensBefore":40000,"details":null},"aborted":false,"willRetry":false}
```

Rust M5 may emit `compaction_start` immediately before a `SessionCompacted` result because the
current core event only reports completion. A later core-level start event can replace that
best-effort pairing without changing the wire shape.

### 4.4 Error mapping

If the core stream yields `AgentError { error }`, the adapter creates an assistant message:

```json
{
  "role": "assistant",
  "content": [],
  "api": "<model.api>",
  "provider": "<model.provider>",
  "model": "<model.id>",
  "usage": {"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0,"cost":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0}},
  "stopReason": "error",
  "errorMessage": "agent failure text",
  "timestamp": 1780588800000
}
```

The emitted protocol sequence is:

```json
{"type":"message_start","message":{...}}
{"type":"message_end","message":{...}}
{"type":"turn_end","message":{...},"toolResults":[]}
{"type":"agent_end","messages":[...]}
```

The CLI process exits non-zero in JSON mode after flushing these records. RPC mode keeps running
after prompt-level failures unless stdin ends or an explicit shutdown path is added later.

## 5. Architecture

### 5.1 Modules

Add a focused protocol subtree to `pi-coding-agent`:

```text
crates/pi-coding-agent/src/protocol/
  mod.rs
  jsonl.rs
  types.rs
  events.rs
  json_mode.rs
  rpc.rs
  session_runner.rs
```

Responsibilities:

- `jsonl.rs`: LF-only serialization and async line reader.
- `types.rs`: protocol event, RPC command, RPC response, RPC state, and stats structs.
- `events.rs`: conversion from `pi-agent-core::AgentEvent` to protocol records.
- `session_runner.rs`: shared setup for model/config/tools/resources/session hydration and
  persistence. This extracts the duplicated parts currently embedded in print mode.
- `json_mode.rs`: one-shot prompt runner that streams protocol events to a writer.
- `rpc.rs`: command loop, state management, prompt task coordination, and response writing.
- `mod.rs`: public exports used by `lib.rs` and tests.

`print_mode.rs` can either call `session_runner.rs` or remain unchanged for M5 as long as session
capture logic is not copied into a third implementation. The preferred implementation is to move
the shared session hydration/persistence helpers out of print mode first, then reuse them from
JSON/RPC mode.

### 5.2 CLI routing

Add:

```rust
pub enum CliMode {
    Print,
    Json,
    Rpc,
}
```

Parsing rules:

- `-p` / `--print` selects `CliMode::Print`.
- `--mode print` selects `CliMode::Print`.
- `--mode json` selects `CliMode::Json`.
- `--mode rpc` selects `CliMode::Rpc`.
- Combining `--mode` with `-p` is valid only if the selected mode is `print`.
- Print/json require a non-empty prompt.
- RPC must not require a prompt. If a prompt is passed with `--mode rpc`, parse accepts it only as
  a future initial prompt if that is explicitly implemented; M5 rejects it with
  `unsupported mode input: rpc does not accept positional prompt`.

### 5.3 Protocol session runner

The runner owns:

1. Build `AgentConfig` from parsed CLI options.
2. Load or create the active session.
3. Hydrate `Agent` from session context.
4. Register tools.
5. Start prompt/skill/template invocation.
6. Stream core events through an adapter callback.
7. Capture new messages and compaction/session metadata when the run settles.

The runner returns:

```rust
pub struct SessionPromptResult {
    pub final_message: AssistantMessage,
    pub messages: Vec<AgentMessage>,
}
```

JSON mode gets its exit status from the `Result` returned by the runner. RPC converts the returned
messages to protocol/session wire messages for `get_messages` and session append behavior.

### 5.4 RPC state and concurrency

RPC mode stores one mutable session state:

- current `Agent`
- selected model
- thinking level
- steering/follow-up modes
- auto compaction enabled
- active session file/id/name
- messages
- pending prompt task

Only one prompt task may run at a time. Command behavior while streaming:

- `prompt` without `streamingBehavior`: response error.
- `prompt` with `streamingBehavior: "steer"`: enqueue steering message and respond success.
- `prompt` with `streamingBehavior: "followUp"`: enqueue follow-up message and respond success.
- `steer`: enqueue steering message and respond success.
- `follow_up`: enqueue follow-up message and respond success.
- `abort`: cancel the current prompt token and respond success.

RPC does not use shell redirection or terminal raw mode. It reads from an injected `AsyncRead` and
writes to an injected `AsyncWrite` in tests; the production CLI passes stdin/stdout.

## 6. Test strategy

`pi-coding-agent` tests cover:

- CLI parsing for `--mode print|json|rpc`, `-p` compatibility, and invalid combinations.
- JSONL serializer emits exactly one trailing LF and does not escape into multiple records.
- JSONL reader splits only on LF, strips CR, preserves U+2028/U+2029 inside JSON strings, and emits
  the final unterminated line on EOF.
- JSON mode first line is a session header.
- JSON mode emits assistant message lifecycle events for a faux text response.
- JSON mode emits tool execution start/end and tool result messages for a faux tool-call response.
- JSON mode maps core errors to an assistant `stopReason: "error"` event sequence and exits
  non-zero.
- RPC parse errors return `command: "parse"` and keep reading later valid commands.
- RPC `get_state` returns model, thinking level, streaming flags, queue modes, session id/name, and
  counts.
- RPC `prompt` emits a prompt response and subsequent agent events.
- RPC `steer` and `follow_up` emit queue update events and affect the next run when M4 queues are
  available.
- RPC unsupported commands return `success: false` without terminating.
- Session persistence appends the same message shapes as print mode after JSON/RPC prompt runs.

`pi-agent-core` changes should be avoided unless the adapter cannot observe required state. If a
small core addition is needed, it must be covered by focused core tests and must not change existing
public event semantics.

## 7. Risks

- The Rust core event stream does not currently emit `agent_start`, `message_start`, or `turn_end`;
  the adapter must reconstruct them. Tests should assert event order so the wire contract does not
  drift.
- Rust tool results do not include TypeScript-style structured `details`; M5 must document the
  narrower result object and leave room for later extension.
- RPC mode can deadlock if stdout backpressure is ignored. The M5 implementation should await
  writes in the injected writer path and keep tests small enough to exercise flush behavior.
- Session persistence is currently tied to print mode. Duplicating it in JSON/RPC mode would create
  divergence; shared runner extraction is the safest path.
- M4 compaction start is not directly observable from core events. The first M5 adapter can emit
  start/end around completion, but a future core event may be needed for exact timing.
