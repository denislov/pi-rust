# Design: Rust M4 agent-core harness capabilities

- Date: 2026-06-05
- Status: Draft (pending review)
- Scope: M4 of the Rust port ROADMAP - headless harness behavior in `pi-agent-core` with focused `pi-coding-agent` print-mode integration.
- Depends on: current `pi-agent-core` loop, M1 built-in tools, M2 model metadata/provider improvements, and M3 JSONL sessions.

## 1. Context

The Rust agent can run a headless prompt, call built-in tools, and persist JSONL v3 sessions. It is
still a narrow loop: tool calls are sequential, there are no hooks, no steering/follow-up queues,
thinking level is not a first-class setting, compaction is only parsed from existing session files,
and skills / prompt templates are not loadable or invokable.

TypeScript has two relevant layers:

- `pi/packages/agent/src/agent-loop.ts` defines the low-level tool loop: per-turn hooks,
  `ToolExecutionMode`, steering and follow-up queues, `shouldStopAfterTurn`, and
  `prepareNextTurn`.
- `pi/packages/agent/src/harness/*` defines the higher-level harness: compaction, skills,
  prompt templates, thinking-level changes, active tool changes, session writes, and resource
  updates.

M4 should not port the entire TS harness. It should add the parts that materially improve the
headless Rust coding agent and create stable interfaces for later interactive/RPC modes.

## 2. Goals and success criteria

Build a usable headless harness increment for Rust.

Done when:

1. `pi-agent-core` exposes typed `ThinkingLevel`, `ToolExecutionMode`, `QueueMode`,
   tool-hook contexts/results, `AgentToolResult`, compaction settings, skills, prompt templates,
   and resource-loading APIs.
2. `AgentConfig` can configure thinking level, global tool execution mode, steering/follow-up
   drain modes, optional hooks, optional resources, and optional automatic compaction.
3. `AgentTool` supports per-tool execution mode. Existing tools default to the global mode.
4. The loop supports sequential and parallel tool batches. Parallel batches preflight calls in
   assistant-source order, execute allowed calls concurrently, emit tool-end events in completion
   order, and append tool-result messages in assistant-source order.
5. `before_tool_call` can block a tool call with model-visible error text before execution.
6. `after_tool_call` can replace tool-result content, error state, and termination hint after
   execution.
7. `should_stop_after_turn` can gracefully stop after a completed turn.
8. `prepare_next_turn` can update model, thinking level, stream options, or message context before
   the next provider request.
9. `Agent::steer()` and `Agent::follow_up()` inject queued user messages according to `QueueMode`.
10. Thinking level is forwarded through `pi-ai::StreamOptions::thinking` when enabled and omitted
    when set to `off`.
11. Skills and prompt templates can be loaded from local markdown files, formatted into the system
    prompt, and explicitly invoked through `Agent::skill()` / `Agent::prompt_from_template()`.
12. Automatic compaction can summarize old session context before the next provider request and
    preserve the summary as both in-memory context and JSONL `compaction` entries when print-mode
    sessions are enabled.
13. `pi-coding-agent` print mode wires the new headless capabilities behind small flags:
    `--thinking`, `--tool-execution`, `--skills`, `--prompt-templates`, `--skill`,
    `--prompt-template`, and `--template-arg`.
14. All tests are deterministic and offline. Compaction summary tests use the faux provider, not a
    live model.

Required verification:

- `cargo fmt --check`
- `cargo test -p pi-agent-core`
- `cargo test -p pi-coding-agent`
- `cargo test --workspace`
- `cargo check --workspace`

## 3. Non-goals

M4 does not implement interactive TUI mode, JSON event mode, JSON-RPC mode, extension loading,
slash commands, settings/auth management, OAuth, provider-specific auth refresh, HTML export, or
TUI session pickers.

M4 does not add full TypeScript-compatible `FileSystem` / `ExecutionEnv` / `Shell` abstractions.
The skills and prompt-template loaders use local filesystem APIs and remain suitable for offline
headless CLI use.

M4 does not implement full JSON Schema argument validation for tools. Rust tools already parse
`serde_json::Value` defensively. Hook contexts receive the raw JSON arguments after each tool's
existing preparation/parsing path.

M4 does not implement branch summarization. Existing `branch_summary` session entries remain
readable through the M3 session context path, but new branch summaries are deferred.

M4 does not require live compaction calls in CI. Compaction generation is tested through the faux
provider and through deterministic preparation/serialization unit tests.

## 4. Approach

### Recommended approach: evolve the existing `Agent` plus small focused modules

Extend the current `Agent` and `run_loop` rather than introducing a separate public
`AgentHarness` type. The existing Rust crate already uses `Agent` as the public headless runtime,
and `pi-coding-agent` is wired to it. Adding harness capabilities to `AgentConfig`, `AgentState`,
and helper modules keeps the milestone directly useful and avoids a second partially overlapping
agent abstraction.

Add focused modules under `pi-agent-core`:

- `hooks.rs` for hook contexts, callback types, and hook return values.
- `queues.rs` for queue modes and queue drain helpers.
- `compaction/` for token estimation, cut-point selection, summary generation, and session entry
  construction.
- `resources/` for skills, prompt templates, markdown frontmatter parsing, system prompt
  formatting, and explicit invocation formatting.

`pi-coding-agent` remains the application integration layer. It resolves CLI flags, loads resources
from disk, opts into compaction when sessions are enabled, and appends compaction/session metadata
through `pi-agent-core::session`.

### Rejected alternatives

1. Port the full TypeScript `AgentHarness` before extending `Agent`.
   This would pull in auth refresh, extension events, settings, custom messages, shell/filesystem
   abstractions, and UI concerns. M4 needs the headless behavior first.
2. Keep tool execution sequential until the interactive mode exists.
   Parallel tool execution is already part of TS core agent behavior and directly benefits print
   mode. It is also easier to test in isolation before TUI integration.
3. Implement compaction only as a CLI post-processing step.
   That would not prevent an overlarge context before the next provider request. M4 should be able
   to compact before continuing the current loop.

## 5. Scope

### In scope

- `pi-agent-core` public types:
  - `ThinkingLevel`
  - `ToolExecutionMode`
  - `QueueMode`
  - `AgentToolResult`
  - hook context/result structs
  - `AgentResources`, `Skill`, `PromptTemplate`
  - `CompactionSettings`, `CompactionPreparation`, `CompactionResult`
- `pi-agent-core` loop behavior:
  - hook invocation
  - parallel/sequential tool batches
  - ordered tool-result persistence
  - steering/follow-up queues
  - thinking options per turn
  - automatic compaction checks
- `pi-agent-core::session` additions:
  - constructors for `thinking_level_change`, `active_tools_change`, `model_change`, and
    `compaction`
  - helper conversion for `AgentMessage::CompactionSummary`
- `pi-coding-agent` print mode:
  - CLI flags for thinking/tool execution/resources
  - resource loading
  - explicit skill/template invocation
  - session writes for thinking-level changes and compaction events
- Offline tests for all new behavior.

### Out of scope

See Section 3.

## 6. Architecture

### 6.1 Core types

`crates/pi-agent-core/src/types.rs` grows harness-level types while preserving existing public
entry points:

```rust
pub enum ThinkingLevel {
    Off,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}

pub enum ToolExecutionMode {
    Sequential,
    Parallel,
}

pub enum QueueMode {
    All,
    OneAtATime,
}

pub struct AgentToolResult {
    pub content: Vec<ContentBlock>,
    pub is_error: bool,
    pub terminate: bool,
}
```

`ToolFn` continues to return `Result<Vec<ContentBlock>, String>` for compatibility. The loop
converts it into `AgentToolResult`. `AgentTool` adds:

```rust
pub execution_mode: Option<ToolExecutionMode>
```

All existing tool factories set `execution_mode: None`.

`AgentConfig` adds:

```rust
pub thinking_level: ThinkingLevel,
pub tool_execution: ToolExecutionMode,
pub steering_mode: QueueMode,
pub follow_up_mode: QueueMode,
pub hooks: AgentHooks,
pub resources: AgentResources,
pub compaction: Option<CompactionConfig>,
```

`Default` values keep the harness quiet unless configured: thinking off, queue mode
one-at-a-time, no hooks, no resources, no compaction. Tool execution defaults to `Parallel` to
match the TypeScript loop after M4; callers that need pre-M4 behavior can set `Sequential`.

### 6.2 Hook contract

Hooks are async callbacks stored in `Arc` wrappers so `AgentConfig` remains cloneable.

`before_tool_call` receives the assistant message, tool-call id/name/arguments, and a snapshot of
current messages. Returning `block: true` prevents execution and creates an error tool result.

`after_tool_call` receives the same call plus the result. It may replace:

- `content`
- `is_error`
- `terminate`

If a hook returns an error string, the loop converts that failure into an error tool result and
continues, matching the model-visible error style used by the current loop.

`should_stop_after_turn` runs after assistant and tool results are appended. Returning true emits
`AgentDone` and exits before steering/follow-up queues are drained.

`prepare_next_turn` runs after `should_stop_after_turn` returns false and before the next provider
request. It can replace context messages, model, thinking level, or stream options for subsequent
turns.

### 6.3 Parallel tool execution

The loop extracts tool calls from the final assistant message.

Execution mode selection:

1. If global mode is `Sequential`, run every call sequentially.
2. If any requested tool has `execution_mode == Some(Sequential)`, run the batch sequentially.
3. Otherwise, preflight calls sequentially, then execute prepared calls concurrently.

Preflight includes missing-tool detection and `before_tool_call`. Immediate outcomes such as
blocked or missing tools are finalized without spawning a task.

Parallel execution uses `FuturesUnordered` for end events in completion order, but stores each
finalized outcome with its original assistant-source index. The loop appends tool-result messages
to `AgentState.messages` in source order so the next model request sees the same order as the
assistant requested.

The batch terminates early only when every finalized result has `terminate == true`.

### 6.4 Steering and follow-up queues

`AgentState` gains two queues:

- steering queue: drained after each completed turn before the next provider request
- follow-up queue: drained when the agent would otherwise stop

Public methods:

```rust
pub fn steer(&self, text: impl Into<String>);
pub fn follow_up(&self, text: impl Into<String>);
pub fn clear_queues(&self);
```

Queue drain behavior follows `QueueMode`:

- `All`: drain all queued messages
- `OneAtATime`: drain only the oldest message

Queued messages become normal `AgentMessage::UserText` entries and are included in session writes.

### 6.5 Thinking level

Before every provider request, the loop builds a per-turn `StreamOptions` snapshot. If thinking is
`Off`, it clears `StreamOptions.thinking`. Otherwise it sets:

```rust
ThinkingConfig {
    enabled: true,
    budget_tokens: budget_from_model_map_or_default(model, level),
    effort: Some(level.as_str().to_string()),
}
```

`budget_from_model_map_or_default` reads `model.thinking_level_map` when it contains a numeric
budget for the requested level. If the model has no map, it uses conservative defaults:

- minimal: 1024
- low: 2048
- medium: 4096
- high: 8192
- xhigh: 16384

If `model.reasoning == false`, thinking is omitted even when requested.

### 6.6 Skills and prompt templates

`pi-agent-core::resources` adds:

```text
resources/
  mod.rs
  frontmatter.rs
  skills.rs
  prompt_templates.rs
  system_prompt.rs
```

Skill loading:

- accepts one or more directories
- recursively loads `SKILL.md`
- loads direct root `.md` files as skills
- honors `.gitignore`, `.ignore`, and `.fdignore` through the Rust `ignore` crate
- parses YAML frontmatter fields `name`, `description`, and `disable-model-invocation`
- emits warnings for invalid files instead of failing the whole load

Prompt-template loading:

- accepts files or directories
- directory inputs load direct `.md` children non-recursively
- parses YAML frontmatter `description`
- derives the template name from the filename without `.md`

System prompt formatting mirrors TS `formatSkillsForSystemPrompt`: visible skills are appended as
an XML-like `<available_skills>` block after the configured system prompt.

Explicit invocation:

- `Agent::skill(name, additional_instructions)` formats the full skill file in a model-visible
  `<skill name="..." location="...">` block.
- `Agent::prompt_from_template(name, args)` replaces `$1`, `$2`, ... and `${1}`, `${2}`, ... in
  the template content, leaving missing placeholders unchanged.

### 6.7 Compaction

`pi-agent-core::compaction` ports the TypeScript compaction shape with Rust-friendly boundaries:

```text
compaction/
  mod.rs
  error.rs
  estimate.rs
  prepare.rs
  summarize.rs
  session.rs
```

Token estimation:

- use the last successful assistant `usage.total_tokens` when available
- estimate trailing messages by characters / 4
- count image blocks as 4800 characters
- truncate tool-result text to 2000 chars when serializing for summarization

Default settings:

```rust
CompactionSettings {
    enabled: true,
    reserve_tokens: 16_384,
    keep_recent_tokens: 20_000,
}
```

The compaction threshold is:

```text
context_tokens > model.context_window - reserve_tokens
```

Preparation chooses a safe cut point that keeps recent context, avoids starting with a bare
`ToolResult`, preserves the most recent compaction summary for iterative updates, and records
read/modified file lists from assistant tool calls named `read`, `write`, and `edit`.

Summary generation calls `pi_ai::complete(pi_ai::stream_model(...))` with the same model and
thinking level. The summarization prompt follows the TS structured format. Tests use the faux
provider to return deterministic summary text.

Runtime behavior:

1. Before a provider request, estimate current context.
2. If compaction is not needed, continue normally.
3. If compaction is needed, prepare and summarize.
4. Replace compacted in-memory messages with `AgentMessage::CompactionSummary` plus retained
   recent messages.
5. Emit `AgentEvent::SessionCompacted`.
6. In `pi-coding-agent` print mode, append a JSONL `compaction` entry before writing the next
   message batch when sessions are enabled.

### 6.8 Print-mode CLI integration

Add flags:

```text
--thinking <off|minimal|low|medium|high|xhigh>
--tool-execution <parallel|sequential>
--skills <dir>              repeatable
--prompt-templates <path>   repeatable file or directory
--skill <name>              invoke a loaded skill; positional prompt becomes additional instructions
--prompt-template <name>    invoke a loaded prompt template
--template-arg <value>      repeatable positional template argument
```

Rules:

- `--skill` and `--prompt-template` are mutually exclusive.
- `--skill` requires a matching skill loaded through `--skills` or injected test resources.
- `--prompt-template` requires a matching template loaded through `--prompt-templates` or injected
  test resources.
- `--thinking off` is the default.
- `--tool-execution parallel` is the default after M4.
- `--no-session` disables compaction session writes but can still compact in memory during the run.

## 7. Test strategy

`pi-agent-core` tests:

- hook blocking produces an error tool result and does not execute the tool.
- after hook replaces content and error state.
- after hook `terminate` stops the next tool turn only when every result in the batch terminates.
- parallel execution starts multiple delayed tools and completes faster than sequential execution.
- parallel message append order follows assistant-source order.
- steering queue drains one-at-a-time and all modes.
- follow-up queue continues after a stop response.
- thinking level produces `StreamOptions.thinking` only for reasoning models.
- skill and prompt-template loaders parse frontmatter, skip invalid files with diagnostics, and
  format invocation prompts.
- compaction preparation selects safe cut points and preserves file-operation metadata.
- compaction summary generation uses faux provider output and emits `SessionCompacted`.
- session entry constructors serialize TypeScript-compatible `compaction`,
  `thinking_level_change`, and `active_tools_change` entries.

`pi-coding-agent` tests:

- parsing and help text for new flags.
- invalid flag combinations fail early.
- `--thinking high` reaches the faux provider as thinking options.
- `--tool-execution sequential` preserves sequential timing/order.
- `--skills <dir> --skill <name> -p <extra>` invokes the skill content.
- `--prompt-templates <dir> --prompt-template <name> --template-arg <value>` formats the prompt.
- print-mode sessions append thinking-level and compaction entries when enabled.
- `--no-session` leaves the session directory untouched even when in-memory compaction occurs.

## 8. M6 work that can run in parallel during M4

The safe M6 work surface is `pi-tui` internals that do not depend on the changing
`pi-agent-core` event shape or `pi-coding-agent` runtime wiring.

Recommended parallel work packages:

1. **Key parser and keybindings manager**: port `pi/packages/tui/src/keys.ts` and
   `keybindings.ts` into `crates/pi-tui/src/input/keys.rs` and `keybindings.rs`. Cover printable
   Unicode, CSI-u / Kitty sequences, modifiers, key releases, and conflict detection.
2. **Stdin buffer and bracketed paste framing**: port `stdin-buffer.ts` into
   `crates/pi-tui/src/input/stdin_buffer.rs`. Cover partial ESC chunks, OSC/DCS/APC completion,
   Kitty printable split handling, and bracketed paste start/end boundaries.
3. **Raw-mode terminal capability wrapper**: add a `RawModeGuard` and terminal input capability
   tests around crossterm without connecting it to `pi-coding-agent`.
4. **Single-line `Input` component foundation**: port the non-rendering state machine first:
   grapheme-aware cursor movement, backspace/delete, word movement, kill/yank, undo, submit,
   cancel, and paste insertion.
5. **Markdown rendering component**: use `pulldown-cmark` and existing width utilities to render
   headings, lists, block quotes, code blocks, links, and ANSI-aware wrapped text.
6. **SelectList component**: port selection movement, filtering, wrapping, disabled rows, and
   width-safe rendering once key parsing exists.
7. **Loader and cancellable loader components**: port animation state and cancellation input using
   the existing render model and virtual terminal tests.

Work to avoid in parallel with M4:

- `pi-coding-agent` interactive bridge, because M4 changes agent events, queues, thinking state,
  and compaction/session writes.
- Tool execution UI, because M4 changes tool start/end payloads and parallel ordering semantics.
- Thinking selector UI, because M4 defines the Rust thinking-level state and CLI contract.
- Compaction summary UI and session selector changes, because M4 changes when compaction entries
  are appended and how summaries enter context.
- Extension/slash-command UI, because extension loading is outside M4 and belongs to M7.

## 9. Risks

- Parallel tools can expose unsafe file mutation ordering. The M4 default allows per-tool
  `Sequential`; mutation-heavy tools can opt out of parallel execution until a file-mutation queue
  is ported.
- Compaction can damage continuity if it cuts through a tool turn. Cut-point tests must prove that
  retained context never starts with an orphan tool result.
- Hook callbacks can deadlock if they try to call mutating `Agent` methods while the loop holds a
  write lock. The loop must snapshot state before awaiting hooks.
- Skill loading can accidentally expose hidden files. The loader must skip dot directories except
  explicitly supported ignore files and must honor ignore rules.
- Event shape changes can break downstream consumers. M4 should update all existing Rust tests and
  keep old high-level `AgentDone` / `AgentError` semantics intact.
