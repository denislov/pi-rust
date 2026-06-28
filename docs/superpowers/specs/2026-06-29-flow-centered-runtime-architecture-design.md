# Flow-Centered Runtime Architecture Design

## Purpose

`pi-rust` should use Flow as the next architecture spine, not only as a small orchestration helper. The goal is to move from "multiple migrated slices can run" to a clearer product runtime:

- one owner for long-lived coding-agent session state;
- explicit Flow graphs for product operations;
- operation-scoped context instead of global mutable access;
- transactional session persistence;
- one canonical product event stream for CLI, RPC, and interactive UI;
- a plugin boundary that can grow into Flow extensions without exposing the whole runtime.

This document fixes the design constraints and phased plan. It does not implement code.

## Goals

- Establish `CodingAgentSession` as the product runtime owner in `pi-coding-agent`.
- Use `PromptTurnFlow` as the first high-value product Flow.
- Keep `RunAgentTurn` as a node that initially calls the existing `Agent::run()`.
- Later replace `Agent::run()` internals with `AgentTurnFlow` in `pi-agent-core`.
- Use operation-scoped contexts such as `PromptTurnContext` and `AgentTurnContext`.
- Add a canonical `CodingAgentEvent` stream above low-level `FlowEvent` and `AgentEvent`.
- Replace TS-compatible session JSONL with a Rust-native typed session event log.
- Reserve `FlowExtension` for the plugin system without exposing arbitrary node/subflow registration to Lua in the first phase.
- Keep the stable product API focused around `pi_coding_agent::api::CodingAgentSession` and `CodingAgentEvent`.

## Non-Goals

- Do not rewrite `agent_loop.rs` in the first phase.
- Do not migrate every CLI, RPC, and interactive command at once.
- Do not expose Lua plugins to arbitrary Flow node/subflow registration in the first plugin phase.
- Do not keep TypeScript `pi` session JSONL compatibility.
- Do not implement TypeScript session import/export.
- Do not change provider request or stream protocols as part of this design.
- Do not make `FlowEvent` the main product/RPC/TUI event protocol.
- Do not make operation contexts, internal services, or concrete flow nodes stable public API.
- Do not move coding-agent product ownership into `pi-agent-core`.

## Core Constraints

The architecture is based on these fixed constraints:

1. **Layered Flow**
   - Internal Rust Flow APIs remain strongly typed.
   - Product workflows use Flow for orchestration.
   - Plugins and Lua get restricted capability views, not full runtime access.

2. **Thick owner with internal services**
   - `CodingAgentSession` is the product runtime owner.
   - Internals are split into services so the owner does not become a monolithic class.

3. **Operation-scoped contexts**
   - Flow nodes do not receive `&mut CodingAgentSession`.
   - Each operation has a scoped context, such as `PromptTurnContext`, `AgentTurnContext`, or `CompactionContext`.

4. **Two Flow layers**
   - `PromptTurnFlow` belongs to `pi-coding-agent`.
   - `AgentTurnFlow` belongs to `pi-agent-core`.
   - `PromptTurnFlow` initially calls existing `Agent::run()` through a `RunAgentTurn` node.

5. **Canonical product events**
   - `CodingAgentSession` exposes `CodingAgentEvent`.
   - `FlowEvent` and `AgentEvent` are internal inputs or debug payloads.

6. **Transactional session persistence**
   - Flow nodes append pending session events to an operation transaction.
   - `SessionService` finalizes success, abort, and failure.
   - Flow nodes never write final session storage directly.

7. **Plugin Flow extension is reserved**
   - M12 should reserve `FlowExtension`.
   - First public/Lua plugin phase exposes tool, command, hook, keybind, and limited UI capabilities, not arbitrary node/subflow registration.

8. **Public API is narrowed**
   - Product consumers should prefer `pi_coding_agent::api::CodingAgentSession` and `CodingAgentEvent`.
   - Internal services, operation contexts, concrete flow nodes, and event mapping internals are migration-private unless explicitly promoted later.

## Crate Boundaries

### `pi-ai`

`pi-ai` remains the model/provider layer:

- provider request, response, and streaming;
- model catalog, provider registry, and future `Models`/auth runtime;
- API keys, bearer tokens, provider headers, and transport hooks;
- LLM stream events such as `AssistantMessageEvent`.

It must not know about coding-agent sessions, product Flows, CLI/RPC/TUI, tools as product concepts, or session persistence.

### `pi-agent-core`

`pi-agent-core` remains the low-level agent runtime layer:

- `Agent`, `AgentTool`, `AgentMessage`, `AgentConfig`;
- existing `Agent::run()` and `Agent::prompt()`;
- `AgentHarness` and low-level hooks;
- `pi_agent_core::flow`;
- session storage primitives where appropriate;
- `ExecutionEnv`, `FileSystem`, and `Shell`;
- low-level `AgentEvent`.

Future `AgentTurnFlow` belongs here. Coding-agent product session ownership does not.

### `pi-coding-agent`

`pi-coding-agent` owns the product runtime:

- `CodingAgentSession`;
- `PromptTurnFlow`;
- product services;
- `CodingAgentEvent`;
- CLI/RPC/interactive adapters;
- plugin host integration.

CLI, RPC, and interactive mode should become adapters over the same product runtime instead of independent business runtimes.

## `CodingAgentSession`

`CodingAgentSession` is the long-lived product owner. It coordinates services and maintains cross-service invariants. It should live behind the `pi_coding_agent::api` facade.

Conceptual use:

```rust
let mut session = CodingAgentSession::open(options).await?;
let mut events = session.subscribe();
let outcome = session.prompt(prompt_options).await?;
```

The exact method names can change during implementation. The architectural role should not.

### Internal Services

`CodingAgentSession` should internally coordinate focused services:

- `SessionService`
- `RuntimeService`
- `FlowService`
- `EventService`
- `CapabilityService`
- `PluginService`

These services are implementation units. They should not become the ordinary public API.

### `SessionService`

`SessionService` owns session lifecycle and persistence policy:

- create/open/continue/fork/clone/adopt local Rust-native sessions;
- active session identity;
- active branch and leaf;
- session metadata;
- operation transaction creation;
- pending session events;
- success, abort, and failure finalization;
- derived transcript/tree/stat rebuild.

Only `SessionService` may commit canonical session writes.

### `RuntimeService`

`RuntimeService` builds the runtime snapshot for an operation:

- provider/model/thinking selection;
- settings/auth/request override resolution;
- `AgentConfig`;
- stream options, retry, and compaction settings;
- built-in and plugin tools;
- tool filter;
- `ExecutionEnv`;
- resources snapshot.

A running prompt should use a snapshot, not live mutable configuration.

### `FlowService`

`FlowService` builds and runs product Flows:

- `PromptTurnFlow`;
- future `ManualCompactionFlow`;
- future `SessionSwitchFlow`;
- future `ExportFlow`;
- future `PluginLoadFlow`.

It does not own session state. It runs operation contexts.

### `EventService`

`EventService` maps internal events into product events:

- `FlowEvent`;
- `AgentEvent`;
- provider stream events;
- session transaction events;
- plugin diagnostics.

Consumers should subscribe to `CodingAgentEvent`, not reconstruct product state from low-level events.

### `CapabilityService`

`CapabilityService` declares what the current runtime can do:

- provider/model feature support;
- tools/images/thinking support;
- session commands such as fork, clone, export, compact;
- filesystem/shell permission availability;
- plugin state;
- RPC and TUI command availability.

This avoids protocols declaring commands that handlers later report as ad hoc unsupported strings.

### `PluginService`

`PluginService` hosts first-party and future Lua plugins:

- tool providers;
- command providers;
- hook providers;
- UI providers;
- keybind providers;
- reserved/first-party `FlowExtension`.

Plugins never receive raw `CodingAgentSession`.

## Operation Contexts

Flow nodes use operation-scoped contexts. They do not mutate the session owner directly.

### `PromptTurnContext`

`PromptTurnContext` owns one prompt operation's temporary state. It is created by `CodingAgentSession`.

It should carry:

- raw prompt and structured content;
- stdin/file/image attachments;
- request mode and request overrides;
- resolved runtime snapshot;
- selected provider/model/thinking;
- stream options;
- `AgentConfig`;
- tool registry snapshot;
- resources snapshot;
- execution environment handle;
- cancellation token;
- active session identity;
- active leaf before the turn;
- `TurnTransaction`;
- constructed `Agent` or harness handle;
- accumulated assistant/tool observations;
- event sink into `EventService`;
- final `PromptTurnOutcome`;
- diagnostics.

It should expose capability handles, not unrestricted owner access.

### Future Contexts

Future flows should use separate contexts:

- `AgentTurnContext`;
- `CompactionContext`;
- `SessionSwitchContext`;
- `PluginLoadContext`;
- `ExportContext`.

Each context should represent one operation and one set of allowed capabilities.

## `PromptTurnFlow`

`PromptTurnFlow` is the first real product-level Flow. It should be implemented before `AgentTurnFlow`.

Conceptual nodes:

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

The initial `RunAgentTurn` node calls existing `Agent::run()`. Later it can delegate to `AgentTurnFlow` without changing the surrounding product flow.

### Node Roles

- `StartPromptTurn`: initialize operation/turn IDs, create `TurnTransaction`, emit `PromptStarted`.
- `ResolveRequest`: normalize mode, overrides, tools, thinking, and session flags.
- `PrepareInput`: process stdin, `@file`, `@image`, prompt templates, and structured content.
- `ResolveRuntime`: merge settings/auth/request and select provider/model/thinking/retry/compaction.
- `LoadResources`: load context files, skills, prompt templates, themes, and attachments.
- `OpenSession`: open or create the Rust-native session and locate the base leaf.
- `BuildAgentRuntime`: assemble `AgentConfig`, tools, resources, stream options, and execution environment.
- `RecordUserInput`: add pending session events for user input.
- `RunAgentTurn`: run current `Agent::run()`, map events, and collect assistant/tool results.
- `FinalizeTurn`: commit, abort, or fail the transaction through `SessionService`.
- `EmitCompletion`: emit final product event and build command output.

### Actions

The first version can keep graph actions simple:

- `default`
- `abort`
- `error`
- `no_session`

Typed product errors should replace string errors at product boundaries as the design is implemented.

### `PromptTurnOutcome`

The prompt operation must return a clear outcome:

- `Success`: final assistant output, committed session identity, usage/cost if available, diagnostics.
- `Aborted`: abort reason, retained/committed policy result, session identity if affected.
- `Failed`: typed error, finalize result, diagnostics.

CLI, RPC, and interactive mode should not infer operation outcome by scraping low-level events.

## Future `AgentTurnFlow`

`AgentTurnFlow` is a later `pi-agent-core` migration. It should eventually replace the monolithic agent loop internals while preserving `Agent::run()` and `Agent::prompt()` as compatibility wrappers.

Conceptual nodes:

```text
DrainQueuedInput
PrepareContext
MaybeCompactRuntimeContext
BeforeProviderRequest
StreamProvider
AccumulateAssistantMessage
DecideStopOrTools
ExecuteTools
AppendToolResults
PrepareNextTurn
```

Migration order:

1. `PromptTurnFlow` calls existing `Agent::run()`.
2. `pi-agent-core` adds `AgentTurnFlow`.
3. `RunAgentTurn` delegates to `AgentTurnFlow`.
4. `Agent::run()` becomes a wrapper over `AgentTurnFlow`.

## `CodingAgentEvent`

`CodingAgentEvent` is the canonical product event stream.

Low-level event sources:

- `FlowEvent`;
- `AgentEvent`;
- provider stream events;
- session transaction events;
- plugin diagnostics.

Product consumers:

- CLI print renderer;
- JSON mode writer;
- RPC wire adapter;
- interactive transcript/TUI adapter.

### Event Families

The full event enum can evolve, but it should cover these families:

- lifecycle: session opened, prompt started, prompt completed, prompt failed, prompt aborted;
- input/context: prompt input prepared, resources loaded, runtime resolved;
- agent turn: agent turn started/completed, provider request started/completed;
- assistant message: message started, delta, completed, cancelled;
- tool execution: tool call started, updated, completed, failed, cancelled;
- compaction: runtime compaction and session compaction;
- session persistence: write pending, write committed, write skipped, active leaf changed;
- capabilities: capability changed;
- diagnostics: diagnostic, warning, plugin error.

### Ordering Invariants

The event stream should preserve these product-level orderings:

- `PromptStarted` before provider/tool/session write events for the turn.
- runtime/resource events before `AgentTurnStarted`.
- `AgentTurnStarted` before assistant/tool events.
- session write pending before session write committed.
- final prompt event after finalize policy has run.
- successful prompt completion includes committed session identity when persistence is enabled.

### Correlation IDs

Events should carry stable IDs as needed:

- `operation_id`;
- `turn_id`;
- `session_id`;
- `message_id`;
- `tool_call_id`;
- optional debug `flow_run_id`.

Timestamps should be injectable or optional so tests remain deterministic.

### Error Categories

Product errors should be typed at least by category:

- config;
- auth;
- resource;
- input;
- session;
- provider;
- tool;
- flow;
- plugin;
- cancellation;
- unsupported capability;
- busy/conflict.

Debug source messages can be retained, but public behavior should not depend on raw string matching.

## Rust-Native Session Format

The TypeScript session JSONL compatibility requirement is removed. `pi-rust` session persistence should be Rust-native and typed.

### Storage Layout

Recommended layout:

```text
session_dir/
  session.json
  events.jsonl
  blobs/
  index/
```

- `session.json`: manifest containing schema version, session ID, created/updated metadata, active leaf, and event log path.
- `events.jsonl`: append-only canonical event log.
- `blobs/`: content-addressed large attachments, images, long tool output, full shell output.
- `index/`: rebuildable derived indexes such as transcript, branch tree, and stats.

`events.jsonl` is the canonical source of truth. Indexes may be deleted and rebuilt.

### Session Event Envelope

Every JSONL line should be a typed event envelope:

```json
{
  "schema": "pi-rust.session.event",
  "version": 1,
  "session_id": "sess_...",
  "event_id": "evt_...",
  "operation_id": "op_...",
  "turn_id": "turn_...",
  "branch_id": "br_...",
  "leaf_id": "leaf_...",
  "parent_event_id": "evt_...",
  "created_at": "2026-06-29T12:00:00Z",
  "kind": "message.delta",
  "data": {}
}
```

`kind + data` should be represented by Rust enum variants, not by an untyped map as the primary model.

### Session Event Families

The schema should support:

- `session.created`;
- `session.metadata.updated`;
- `operation.started`;
- `operation.committed`;
- `operation.aborted`;
- `operation.failed`;
- `turn.started`;
- `turn.input.recorded`;
- `turn.completed`;
- `turn.aborted`;
- `turn.failed`;
- `message.started`;
- `message.delta`;
- `message.completed`;
- `message.cancelled`;
- `tool.call.started`;
- `tool.call.updated`;
- `tool.call.completed`;
- `tool.call.failed`;
- `tool.call.cancelled`;
- `runtime.compaction.started`;
- `runtime.compaction.completed`;
- `session.compaction.started`;
- `session.compaction.completed`;
- `branch.created`;
- `branch.summary.created`;
- `attachment.added`;
- `diagnostic.emitted`;
- `active_leaf.changed`.

The first implementation can start with a smaller set, but the schema should be designed around these families.

### Transaction Semantics

`TurnTransaction` collects pending session events and staged blobs:

```text
TurnTransaction
  operation_id
  turn_id
  base_leaf
  target_branch
  pending SessionEvent[]
  staged blobs
  diagnostics
```

Finalize should:

1. stage blobs;
2. append pending events;
3. append `operation.committed`, `operation.aborted`, or `operation.failed`;
4. update `session.json` active leaf and metadata only after successful commit;
5. emit `CodingAgentEvent` session persistence events.

An operation without a committed/aborted/failed marker is incomplete and must be handled by recovery policy during replay.

### Abort and Error

Abort/error are first-class facts:

- record `operation.aborted` or `operation.failed`;
- record `turn.aborted` or `turn.failed` when a turn exists;
- close partial assistant messages with `message.cancelled`;
- close running tools with `tool.call.cancelled`;
- emit diagnostics for provider/tool/session failures.

The session log must never contain a normal-looking half-complete lifecycle as accepted final state.

### Branch and Leaf

Branch and leaf are first-class identifiers:

- `branch_id` describes a branch lineage;
- `leaf_id` describes a committed point;
- operations start from `base_leaf` and commit to `new_leaf`;
- active leaf is stored in the manifest and can also be tracked through `active_leaf.changed`.

Fork, clone, switch, branch summary, and compaction should be represented as typed events.

### Attachments and Blobs

Files, images, long tool outputs, and full shell captures should not be forced into message text.

Use `attachment.added` with inline or blob storage:

```json
{
  "kind": "attachment.added",
  "data": {
    "attachment_id": "att_1",
    "source": "input_reference",
    "media_type": "text/rust",
    "content_hash": "sha256:...",
    "storage": {
      "type": "blob",
      "path": "blobs/sha256..."
    }
  }
}
```

Prompt content can reference attachments by ID.

### Runtime vs Session Compaction

Separate two concepts:

- runtime compaction: affects provider context for a turn;
- session compaction: changes long-term session history and creates session events.

They should have separate event families.

## Plugin and Flow Extension Strategy

The plugin system should be designed around capability-scoped access.

### Extension Types

Keep the M12 categories:

- `ToolProvider`;
- `CommandProvider`;
- `HookProvider`;
- `UiProvider`;
- `KeybindProvider`.

Reserve:

- `FlowExtension`.

### Phase A: Rust Trait Kernel

Phase A should define the Rust trait registry and first-party extension path. `FlowExtension` may exist for internal/first-party use only.

Allowed early hook points may include:

- before prompt prepare;
- after input prepared;
- after resources loaded;
- before agent turn;
- after agent turn;
- before session commit.

### Phase B: Lua Bridge

Lua should initially support stable capabilities:

- register tool;
- register command;
- register hook;
- register keybind;
- limited UI action.

Lua should not initially support:

- arbitrary Flow node registration;
- replacing `PromptTurnFlow`;
- replacing `AgentTurnFlow`;
- raw session owner access;
- raw storage/auth/provider access.

### Later Restricted Flow Extensions

Only after operation contexts, transaction semantics, events, and capability views have stabilized should Lua be allowed to register restricted Flow extensions.

Even then:

- plugins get capability-scoped views;
- insertions happen only at defined extension points;
- plugins cannot directly commit sessions;
- plugins cannot access auth secrets unless explicitly granted;
- plugin APIs are versioned.

## Migration Plan

### Phase 0: Design Fixed

This document is the output. No code changes are required by Phase 0.

### Phase 1: `CodingAgentSession` Skeleton and Rust-Native Session Format

Build:

- `pi_coding_agent::api::CodingAgentSession`;
- draft `CodingAgentEvent`;
- `SessionService`;
- `TurnTransaction`;
- Rust-native `session.json` + `events.jsonl`;
- replay/fold transcript support;
- `PromptTurnContext` skeleton;
- `FlowService` skeleton.

Minimum session events:

- `session.created`;
- `operation.started`;
- `turn.started`;
- `turn.input.recorded`;
- `message.started`;
- `message.completed`;
- `operation.committed`;
- `operation.aborted`;
- `operation.failed`;
- metadata/active leaf events if required.

Acceptance:

- create/open Rust-native sessions;
- append and replay event log into transcript;
- test success/abort/error transaction finalization;
- expose `CodingAgentSession` and `CodingAgentEvent` through `pi_coding_agent::api`;
- keep services and contexts out of the stable facade.

### Phase 2: `PromptTurnFlow` on Headless/JSON Path

Build:

- `CodingAgentSession::prompt()`;
- `PromptTurnFlow`;
- real `PromptTurnContext`;
- `RunAgentTurn` node calling existing `Agent::run()`;
- `EventService` mapping from low-level events to `CodingAgentEvent`;
- session commit into Rust-native event log.

Acceptance:

- faux provider runs a full prompt through `CodingAgentSession`;
- assistant output appears in the event log;
- tool calls/results appear in the event log;
- `CodingAgentEvent` ordering is stable;
- headless output does not regress;
- JSON mode can be adapted from `CodingAgentEvent`;
- abort/error finalize tests pass.

### Phase 3: CLI/RPC/Interactive Adapters Converge

Move frontends onto `CodingAgentSession`:

- CLI print/json;
- RPC prompt/session commands;
- interactive prompt and session operations;
- capability queries for RPC/TUI.

Acceptance:

- print/json/RPC/interactive prompt use the same prompt API;
- session operations share `SessionService`;
- RPC can report capability availability instead of ad hoc unsupported strings;
- TUI consumes `CodingAgentEvent`;
- old `session_runner` shrinks into wrappers or is removed.

### Phase 4: `AgentTurnFlow`

Move the agent loop internals into `pi-agent-core` Flow:

- `AgentTurnContext`;
- `AgentTurnFlow`;
- provider streaming node;
- tool execution node;
- runtime compaction node;
- next-turn preparation node;
- `Agent::run()` wrapper.

Acceptance:

- existing `pi-agent-core` behavior does not regress;
- current tests pass or are intentionally replaced with equivalent coverage;
- tool execution modes, hooks, queues, abort, and compaction semantics hold;
- `RunAgentTurn` no longer calls the old monolithic loop.

### Phase 5: M12 Plugin Kernel

Build the plugin kernel on stable session/flow boundaries:

- tool provider;
- command provider;
- hook provider;
- UI provider;
- keybind provider;
- first-party/reserved `FlowExtension`;
- capability-scoped plugin host.

Acceptance:

- first-party plugin can register tool/command/hook;
- plugin failures become diagnostics, not panics;
- plugin cannot direct-commit session;
- plugin tools use `RuntimeService`/`ExecutionEnv`;
- Lua does not expose arbitrary node/subflow in the first phase.

### Phase 6: Advanced Flow Workflows

Use the architecture for higher-level workflows:

- `ManualCompactionFlow`;
- `SessionCompactionFlow`;
- `BranchSummaryFlow`;
- `ExportFlow`;
- `PluginLoadFlow`;
- subagent/supervisor flows;
- self-healing edit workflows.

## Compatibility Invariants

Removed invariants:

- TypeScript session JSONL compatibility;
- reading TypeScript session fixtures;
- TypeScript session import/export.

New invariants:

- provider request and stream behavior should not regress;
- headless/json user-visible behavior should not regress except for the intentional session format change;
- `pi-agent-core` low-level APIs remain usable during migration;
- Rust-native session schema has versions and migration paths;
- `events.jsonl` is canonical;
- indexes/caches are rebuildable;
- `CodingAgentEvent` is the product event boundary;
- operation transactions do not half-commit as normal state;
- internal services and contexts are not accidentally promoted into stable public API.

## Risks

### `CodingAgentSession` Becomes Too Large

Mitigation: keep it as owner/coordinator. Push implementation into services and flows.

### Operation Context Becomes a Hidden Global

Mitigation: keep one context type per operation. Expose capability handles instead of raw service access.

### Event Model Becomes Too Broad Too Early

Mitigation: Phase 2 only implements prompt-path events required by headless/json. Add more families when adapters need them.

### Session Event Log Is Overdesigned

Mitigation: start with a minimal event set, but keep the envelope and event-family model from day one.

### Migration Has Split Behavior

Mitigation: migrate one real path first, then turn old paths into wrappers. Tests should lock behavior at adapter boundaries.

### Plugin API Freezes Too Early

Mitigation: do not expose arbitrary Lua node/subflow APIs until operation contexts and transaction semantics have proven stable.

## Success Criteria

The architecture is proven after Phase 1 and Phase 2 if:

- `CodingAgentSession` can run a real prompt;
- `PromptTurnFlow` is the real headless/json path, not a demo;
- `RunAgentTurn` calls existing `Agent::run()`;
- session events can be replayed into a transcript;
- session writes are committed through transactions;
- abort/error finalization is deterministic;
- `CodingAgentEvent` can drive headless/json output;
- old agent loop internals have not been prematurely rewritten.
