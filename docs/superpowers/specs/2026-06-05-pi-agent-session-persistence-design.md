# Design: Rust M3 session persistence

- Date: 2026-06-05
- Status: Draft (pending review)
- Scope: M3 of the Rust port ROADMAP - persistent JSONL sessions for the headless coding agent.
- Depends on: current `pi-agent-core` loop, current `pi-coding-agent` print mode, built-in tools, and faux provider tests.

## 1. Context

The Rust print-mode agent can now run a prompt through `pi-agent-core`, call tools, and return the
final assistant text. All conversation state is still process-local: `Agent` stores messages in
memory, `run_print_mode` discards them at exit, and the CLI has no way to continue or fork a prior
conversation.

TypeScript has two relevant references:

- `pi/packages/agent/src/harness/session/*` defines a storage-oriented JSONL v3 session layer with
  `Session`, `InMemorySessionStorage`, `JsonlSessionStorage`, and repos. It supports tree paths,
  `leaf` entries, labels, compaction, branch summaries, model changes, and active tool changes.
- `pi/packages/coding-agent/src/core/session-manager.ts` defines the CLI-facing session manager.
  It also writes v3 JSONL files, but normal CLI files are a practical subset: a `session` header
  followed by append-only entries such as `message` and `session_info`. It keeps the current leaf in
  memory as the latest appended entry instead of writing `leaf` entries for ordinary linear use.

M3 should keep the Rust implementation small but interoperable. Rust should read both v3 shapes and
write the CLI-compatible v3 subset for normal print-mode runs, so TypeScript `SessionManager.open()`
can inspect Rust-produced files.

## 2. Goals and success criteria

Build persistent sessions across `pi-agent-core` and `pi-coding-agent`.

Done when:

1. `pi-agent-core` exposes a session module with typed v3 entries, in-memory storage, JSONL storage,
   and a cwd-scoped JSONL repo.
2. Rust can parse v3 files containing `session`, `message`, `session_info`, `model_change`,
   `thinking_level_change`, `active_tools_change`, `compaction`, `branch_summary`, `label`, and
   `leaf` entries without losing unknown raw JSON when appending to the file.
3. Rust writes normal print-mode sessions as a TypeScript coding-agent-compatible subset:
   header + `message` + optional `session_info` entries, one JSON object per line.
4. `Session::build_context()` rebuilds the active message path from the current leaf and converts
   only model-visible entries into `AgentMessage`.
5. `Agent` can start from hydrated messages and run a normal `prompt()` without changing provider or
   tool APIs.
6. `pi-coding-agent` supports `--no-session`, `--session <path|id>`, `--session-id <id>`,
   `--continue` / `-c`, `--resume` / `-r`, `--fork <path|id>`, `--session-dir <dir>`, and
   `--name <name>` in print mode.
7. CLI sessions are cwd-associated. Default storage resolves to a per-cwd directory compatible with
   TypeScript's `--${cwd-with-separators-replaced}--` convention.
8. A second CLI invocation can continue the first session with the previous messages included in
   the model context.
9. A forked CLI invocation creates a new session file with `parentSession` pointing at the source
   file and continues from the copied branch context.
10. All tests are deterministic and offline: no provider keys, no network, and session files under
    temp directories.

Required verification:

- `cargo fmt --check`
- `cargo test -p pi-agent-core`
- `cargo test -p pi-coding-agent`
- `cargo test --workspace`
- `cargo check --workspace`

## 3. Non-goals

M3 does not implement interactive session picking. In print mode, `--resume` is a headless alias for
opening the most recent matching session, matching the useful part of `--continue`; the future TUI
selector can replace that behavior for interactive mode.

M3 does not generate compaction summaries, branch summaries, labels, model changes, active tool
changes, custom extension entries, bash execution messages, HTML export/import, JSON/RPC event mode,
settings migration, auth storage, or interactive TUI integration. It may parse these entries so old
files remain readable, but new Rust print-mode runs only append messages and optional session names.

M3 does not stream session writes event-by-event. `run_print_mode` appends the new message batch
after the agent loop settles or errors. This matches the TypeScript coding-agent's practical
durability boundary, which delays creating the file until an assistant message exists.

## 4. Compatibility contract

### 4.1 Header

Rust writes the first JSONL line exactly as:

```json
{"type":"session","version":3,"id":"019de8c2-de29-73e9-ae0c-e134db34c447","timestamp":"2026-06-05T00:00:00.000Z","cwd":"/abs/project","parentSession":"/abs/source.jsonl"}
```

`parentSession` is omitted when there is no source session. `cwd` is absolute and syntactically
normalized. The timestamp is RFC3339 with millisecond precision and `Z`.

### 4.2 Entry envelope

Each non-header line has:

```json
{"type":"message","id":"8chars00","parentId":null,"timestamp":"2026-06-05T00:00:01.000Z","message":{}}
```

`id` is unique within the file. Use the first 8 chars of UUIDv7 when possible; if a collision occurs,
retry and fall back to the full UUIDv7 after 100 attempts. `parentId` is the current active leaf.
For linear sessions, the next entry's `parentId` is the previous entry's `id`.

### 4.3 Message JSON

Rust serializes only the messages it currently produces:

```json
{"role":"user","content":[{"type":"text","text":"hello"}],"timestamp":1780588800000}
```

```json
{"role":"assistant","content":[{"type":"text","text":"hi"}],"api":"anthropic-messages","provider":"anthropic","model":"claude-sonnet-4-5","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0,"cost":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0}},"stopReason":"stop","timestamp":1780588800000}
```

```json
{"role":"toolResult","toolCallId":"tool_1","toolName":"read","content":[{"type":"text","text":"ok"}],"isError":false,"timestamp":1780588800000}
```

`AgentMessage::SystemPrompt` is not written as a session message. System prompts remain runtime
configuration, matching current Rust behavior and TypeScript's normal CLI sessions.

`StoredAgentMessage` is the session wire shape. It may not blindly reuse every existing
`pi-ai::AssistantMessage` serde field if Rust and TypeScript differ; M3 must add fixture tests
against `pi/packages/ai/src/types.ts` and write the TypeScript session shape.

### 4.4 Reading older or richer files

When reading existing v3 files:

- `message` entries with roles `user`, `assistant`, and `toolResult` become Rust `AgentMessage`
  variants.
- `message` entries with unknown roles are retained as raw entries but ignored by
  `build_context()`.
- `compaction` entries become a synthetic user text message using the TypeScript compaction summary
  wrapper.
- `branch_summary` entries become a synthetic user text message using the TypeScript branch summary
  wrapper.
- `custom`, `custom_message`, `label`, `session_info`, `model_change`, `thinking_level_change`, and
  `active_tools_change` entries are retained for metadata or future use but do not affect the M3
  LLM context.
- `leaf` entries are honored for active leaf calculation when present. Rust does not emit `leaf`
  entries during normal print-mode runs.

## 5. Architecture

### 5.1 `pi-agent-core::session`

Add:

```text
crates/pi-agent-core/src/session/
  mod.rs
  error.rs
  id.rs
  types.rs
  context.rs
  memory.rs
  jsonl.rs
  repo.rs
```

Responsibilities:

- `types.rs`: `SessionHeader`, `SessionEntry`, `SessionMetadata`, `JsonlSessionMetadata`,
  `StoredAgentMessage`, `SessionContext`, and `SessionCreateOptions`.
- `error.rs`: `SessionError` with stable codes `not_found`, `invalid_session`, `invalid_entry`,
  `invalid_fork_target`, `storage`, and `unknown`.
- `id.rs`: UUIDv7 session ids, short entry ids, injectable clock/id generator for tests.
- `context.rs`: path traversal and conversion from session entries to `AgentMessage`.
- `memory.rs`: deterministic in-memory storage used by unit tests and agent-core consumers.
- `jsonl.rs`: strict header validation, tolerant entry loading, append-only writes.
- `repo.rs`: cwd-scoped create/open/list/delete/fork operations and path/id resolution.

The public API stays concrete at first. A trait object storage abstraction is not required for M3;
callers can use `InMemorySessionStorage`, `JsonlSessionStorage`, or `JsonlSessionRepo` directly.

### 5.2 Agent hydration

Add `Agent::with_messages(config, messages)` and `Agent::replace_messages(messages)`. The existing
`Agent::new`, `add_message`, `messages`, `prompt`, and `abort` behavior stays unchanged.

`run_print_mode` uses:

1. Load or create a session.
2. Build the session context.
3. Construct `Agent::with_messages(config, context.messages)`.
4. Run `prompt()`.
5. Append only the new messages after the hydrated baseline.

This avoids adding persistence callbacks inside the agent loop and keeps provider/tool behavior
unchanged.

### 5.3 CLI session resolution

Add a small `pi-coding-agent::session` module:

```text
crates/pi-coding-agent/src/session.rs
```

Responsibilities:

- Resolve the effective cwd.
- Resolve the effective session directory:
  - `--session-dir <dir>` wins.
  - `PI_SESSION_DIR` wins next.
  - `PI_AGENT_DIR/sessions/<encoded-cwd>` if `PI_AGENT_DIR` is set.
  - `$HOME/.pi/agent/sessions/<encoded-cwd>` otherwise.
- Resolve `--session <path|id>` and `--fork <path|id>`.
- Implement `--continue` and `--resume` as most-recent-session lookup for the effective cwd.
- Implement `--session-id <id>` as open-by-id-or-create.
- Create a new session for a normal `-p` run when sessions are enabled and no existing target is
  requested.
- Apply `--name <name>` by appending a `session_info` entry once per run before the prompt batch.

### 5.4 Error behavior

Session target errors are CLI errors with exit code 1:

- Missing `--session` path or id: `session not found: <target>`
- Multiple partial id matches: `session id is ambiguous: <target>`
- Invalid JSONL header: `invalid session: <path>: <reason>`
- Stored cwd missing: `stored session working directory does not exist: <cwd>`

`--no-session` disables all session file I/O and ignores `PI_SESSION_DIR`. Combining
`--no-session` with `--session`, `--session-id`, `--continue`, `--resume`, `--fork`,
`--session-dir`, or `--name` is rejected as invalid input.

## 6. Test strategy

Unit tests in `pi-agent-core` cover:

- Exact JSON shape for header, message entries, and session_info.
- Loading a TypeScript-style coding-agent JSONL file without `leaf`.
- Loading a storage-style JSONL file with `leaf`.
- Building context from the active path, including branch paths.
- Ignoring unknown entries while preserving append ability.
- Forking before a user message and at an arbitrary message.
- Listing sessions sorted newest first and filtering by cwd.

Integration tests in `pi-coding-agent` cover:

- Argument parsing and help text for session flags.
- `--no-session` leaves the temp session dir empty.
- Normal `-p` creates one v3 JSONL file.
- `--continue -p` sends previous user/assistant messages to the faux provider.
- `--session <path> -p` appends to that file.
- `--session-id <id> -p` creates or opens the matching session.
- `--fork <path> -p` creates a new file with `parentSession`.
- `--name <name> -p` appends `session_info`.

## 7. M6 work that can run in parallel during M3

The safe parallel M6 surface is limited to `pi-tui` internals that do not touch
`pi-coding-agent` session/runtime files.

Recommended parallel work packages:

1. **Key parser and keybindings manager**: port `keys.ts` and `keybindings.ts` into
   `pi-tui/src/input/keys.rs` and `keybindings.rs`; cover legacy escape sequences, CSI-u Kitty
   sequences, modifiers, printable Unicode, and conflict detection.
2. **Stdin buffer and bracketed paste framing**: port `stdin-buffer.ts` into
   `pi-tui/src/input/stdin_buffer.rs`; cover partial ESC chunks, OSC/DCS/APC completion, Kitty
   printable split handling, and bracketed paste boundaries.
3. **Single-line `Input` component foundation**: port the non-rendering state machine first:
   grapheme-aware cursor movement, backspace/delete, word movement, kill/yank, undo, submit/cancel,
   and paste insertion. Rendering can use the existing string component model.
4. **`SelectList` layout/input component**: port selection movement, filtering, wrapping behavior,
   and width-safe rendering. This is mostly independent once keybindings exist.
5. **Markdown rendering spike**: use `pulldown-cmark` and port the wrapped terminal output subset:
   headings, lists, block quotes, code blocks, links, and ANSI-aware wrapping.

Work to avoid in parallel with M3:

- The `pi-coding-agent` interactive mode bridge, because it will need the same session manager and
  runtime files M3 changes.
- Session selector UI, because session listing/resume semantics are being defined by M3.
- Focus/overlay/event-loop integration until the key parser and `Input` component contracts settle.
