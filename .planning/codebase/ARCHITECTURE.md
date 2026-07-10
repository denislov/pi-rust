<!-- refreshed: 2026-07-10 -->
# Architecture

**Analysis Date:** 2026-07-10

## System Overview

```text
+-----------------------------------------------------------------------+
| Product entry points and adapters                                     |
+----------------------+----------------------+-------------------------+
| Interactive terminal | Print / JSON         | JSONL RPC               |
| `crates/pi-coding-   | `crates/pi-coding-  | `crates/pi-coding-      |
| agent/src/interactive`| agent/src/print_mode.rs` | agent/src/protocol` |
+-----------+----------+-----------+----------+-------------+-----------+
            |                      |                        |
            +----------------------+------------------------+
                                   v
+-----------------------------------------------------------------------+
| Product operation runtime                                             |
| `crates/pi-coding-agent/src/coding_session`                            |
| typed Operation -> admission/capability snapshot -> product Flow       |
| -> services -> ProductEvent -> session transaction                     |
+--------------------------+-----------------------+--------------------+
                           |                       |
                           v                       v
+------------------------------------+  +-------------------------------+
| Low-level agent runtime            |  | Generic terminal UI           |
| `crates/pi-agent-core/src`          |  | `crates/pi-tui/src`           |
| AgentTurnFlow, tools, hooks, queues |  | components, input, rendering  |
+-------------------+----------------+  +-------------------------------+
                    |
                    v
+-----------------------------------------------------------------------+
| Model/provider and transport runtime                                  |
| `crates/pi-ai/src`                                                     |
| model registry -> scoped provider registry -> provider adapter -> HTTP |
+--------------------------+--------------------------------------------+
                           |
                           v
+-----------------------------------------------------------------------+
| External model APIs and local durable session storage                  |
| provider HTTPS/SSE; `session.json` + `events.jsonl` via `session_log`   |
+-----------------------------------------------------------------------+
```

The active product is the `pi-coding-agent` binary in `crates/pi-coding-agent/src/main.rs`. The workspace root binary in `src/main.rs` only prints `Hello, world!`; it is not the coding-agent entry point.

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| Binary bootstrap | Parse process arguments, special-case streaming RPC, collect piped stdin, install builtin tools, and convert `CliOutput` into process output/exit status | `crates/pi-coding-agent/src/main.rs` |
| CLI facade | Parse modes, resolve prompt requests, and dispatch interactive, print, JSON, or RPC-compatible work | `crates/pi-coding-agent/src/lib.rs` |
| Stable product API | Re-export intended embedding contracts while root modules and deprecated exports remain available during migration | `crates/pi-coding-agent/src/lib.rs` |
| Product runtime owner | Own session state, operation admission, services, profiles, plugins, capability generations, and public operation execution | `crates/pi-coding-agent/src/coding_session/mod.rs` |
| Operation contract | Classify each product action by kind, origin, class, and async/sync dispatch mode | `crates/pi-coding-agent/src/coding_session/operation.rs` |
| Public operation facade | Convert public `CodingAgentOperation` values to internal operations and convert internal outcomes back to stable results | `crates/pi-coding-agent/src/coding_session/public_operation.rs` |
| Intent admission | Resolve client intents into typed operations and enforce operation metadata/capability rules | `crates/pi-coding-agent/src/coding_session/intent_router.rs` |
| Operation control | Enforce active-operation exclusivity and carry abort/follow-up/steering control handles | `crates/pi-coding-agent/src/coding_session/operation_control.rs` |
| Product flow factory | Construct and run prompt, compaction, export, plugin load, branch summary, agent/team, and self-healing-edit graphs | `crates/pi-coding-agent/src/coding_session/flow_service.rs` |
| Prompt orchestration | Execute the ordered product prompt graph from request resolution through session commit and completion events | `crates/pi-coding-agent/src/coding_session/prompt_flow.rs` |
| Runtime assembly | Build and hydrate low-level `Agent` instances from immutable runtime snapshots, tools, plugins, auth, and resources | `crates/pi-coding-agent/src/coding_session/runtime_service.rs` |
| Capability snapshots | Derive operation-local model, tool, filesystem, shell, session, UI, and plugin permissions | `crates/pi-coding-agent/src/coding_session/capability_snapshot.rs` |
| Session persistence | Create/open/list/replay/fork/clone sessions and commit typed transactions to the Rust-native log | `crates/pi-coding-agent/src/coding_session/session_service.rs` |
| Durable log primitives | Store manifests and JSONL envelopes, replay/fold state, and stage transactional operation facts | `crates/pi-coding-agent/src/coding_session/session_log` |
| Event publication | Map low-level agent events into product events, assign sequences, retain a replay window, and broadcast to adapters | `crates/pi-coding-agent/src/coding_session/event_service.rs` |
| Plugin host | Register capability-scoped tools, commands, hooks, keybinds, dialogs, UI actions, and Lua-backed providers | `crates/pi-coding-agent/src/plugins` |
| Builtin coding tools | Implement read/write/edit/bash/grep/find/ls behind filesystem or shell capabilities | `crates/pi-coding-agent/src/tools` |
| Low-level flow engine | Run typed node graphs with actions, transitions, cancellation, step limits, and flow events | `crates/pi-agent-core/src/flow.rs` |
| Agent turn runtime | Orchestrate context preparation, provider streaming, stop/tool decisions, tool execution, and subsequent turns | `crates/pi-agent-core/src/agent_turn_flow` |
| Provider runtime | Resolve model API identifiers and auth into scoped providers, then return normalized assistant event streams | `crates/pi-ai/src/registry.rs` |
| Provider adapters | Convert shared request types to provider wire formats and process provider-specific streaming responses | `crates/pi-ai/src/providers` |
| Transport | Own HTTP headers, retry/error classification, and request execution shared by providers | `crates/pi-ai/src/transport` |
| Generic TUI | Provide terminal lifecycle, normalized input, component trees, overlays, differential rendering, themes, and image protocols | `crates/pi-tui/src` |
| Product TUI adapter | Project product events/snapshots into coding-agent-specific transcript, menus, commands, and footer state | `crates/pi-coding-agent/src/interactive` |

## Pattern Overview

**Overall:** Flow-centered, layered operation runtime with event-sourced durable session state and adapter projections.

**Key Characteristics:**
- Product actions enter as typed operations at `crates/pi-coding-agent/src/coding_session/public_operation.rs` and are classified before execution by `crates/pi-coding-agent/src/coding_session/operation.rs`.
- Workflows are explicit graphs built on the generic `Flow<C>` engine in `crates/pi-agent-core/src/flow.rs`; product graphs stay in `pi-coding-agent`, while the reusable agent loop stays in `pi-agent-core`.
- Side effects are service-owned: `SessionService` persists facts, `RuntimeService` assembles agents, `EventService` publishes semantic events, and `PluginService` mediates extensions under `crates/pi-coding-agent/src/coding_session`.
- Persistent session truth is reconstructed from typed `SessionEventEnvelope` records under `crates/pi-coding-agent/src/coding_session/session_log`; adapters consume product events and snapshots instead of reading the log directly.
- Crate dependencies form a one-way DAG: `pi-coding-agent` depends on `pi-agent-core`, `pi-ai`, and `pi-tui`; `pi-agent-core` depends on `pi-ai`; `pi-ai` and `pi-tui` remain product-neutral.
- Compatibility surfaces remain visible but are marked deprecated in `crates/pi-coding-agent/src/lib.rs` and `crates/pi-ai/src/lib.rs`; new consumers should use each crate's `api` module.

## Layers

**Process and Adapter Layer:**
- Purpose: Translate process I/O, CLI commands, TTY input, JSON, and JSONL RPC messages into product operations; project product events back to users or clients.
- Location: `crates/pi-coding-agent/src/main.rs`, `crates/pi-coding-agent/src/lib.rs`, `crates/pi-coding-agent/src/interactive`, `crates/pi-coding-agent/src/print_mode.rs`, `crates/pi-coding-agent/src/protocol`
- Contains: Mode dispatch, request resolution, interactive event loop, UI projection, JSON serialization, and RPC command state.
- Depends on: The stable/product facade in `crates/pi-coding-agent/src/coding_session` and generic terminal primitives from `crates/pi-tui/src`.
- Used by: Shell users, embedding callers, scripted JSON consumers, and long-lived RPC clients.

**Product Operation Layer:**
- Purpose: Own coding-agent semantics, operation admission, workflow composition, profiles/teams/delegation, plugins, session navigation, and product events.
- Location: `crates/pi-coding-agent/src/coding_session`
- Contains: `CodingAgentSession`, typed operations, Flow contexts, product flows, service boundaries, capability snapshots, event publication, projections, and session logging.
- Depends on: `pi-agent-core` for agent/flow primitives, `pi-ai` types/providers, local config/resources, plugins, and builtin tools.
- Used by: All adapters in `crates/pi-coding-agent/src/interactive`, `crates/pi-coding-agent/src/print_mode.rs`, and `crates/pi-coding-agent/src/protocol`.

**Low-Level Agent Layer:**
- Purpose: Execute a provider/tool loop without knowing coding-agent sessions, UI protocols, profile policy, or persistence rules.
- Location: `crates/pi-agent-core/src`
- Contains: `Agent`, `AgentTurnFlow`, `Flow<C>`, tool contracts, hooks, queues, context conversion, compaction, resources, execution environments, and low-level agent events.
- Depends on: Shared model/message/event contracts and streaming from `crates/pi-ai/src`.
- Used by: Product runtime construction in `crates/pi-coding-agent/src/coding_session/runtime_service.rs` and agent invocations in product flows.

**Provider and Transport Layer:**
- Purpose: Normalize model metadata, authentication, request conversion, streaming event parsing, cost/usage, and HTTP behavior across providers.
- Location: `crates/pi-ai/src`
- Contains: `ProviderRegistry`, `AiClient`, `ApiProvider`, model registry, provider-specific `convert`/`wire`/`process` modules, transport and retry helpers, compatibility adapters, and image APIs.
- Depends on: Network/runtime libraries only; it does not depend on product or TUI crates.
- Used by: `pi-agent-core` provider streaming and product model selection/configuration.

**Generic Presentation Layer:**
- Purpose: Provide reusable terminal mechanics without coding-agent business semantics.
- Location: `crates/pi-tui/src`
- Contains: `Terminal`, `ProcessTerminal`, `VirtualTerminal`, `Tui<T>`, `Component`, editor/input/list/dialog components, overlay/focus management, render scheduling, ANSI styles, and terminal image support.
- Depends on: Terminal and rendering libraries; it does not depend on `pi-coding-agent`.
- Used by: Product-specific interactive code under `crates/pi-coding-agent/src/interactive`.

**Durable Storage Layer:**
- Purpose: Persist session identity, active leaf, operation boundaries, messages, compaction, delegation, runtime generations, and navigation as typed facts.
- Location: `crates/pi-coding-agent/src/coding_session/session_log` and `crates/pi-coding-agent/src/coding_session/session_service.rs`
- Contains: `session.json` manifest handling, `events.jsonl` append/read, transaction staging, replay/fold, recovery markers, and fork/clone/export helpers.
- Depends on: Local filesystem and typed product/session contracts.
- Used by: `CodingAgentSession` only through `SessionService`; adapters receive derived views and snapshots.

## Data Flow

### Primary Request Path

1. The product binary gathers process arguments/stdin and selects the RPC fast path or common CLI path (`crates/pi-coding-agent/src/main.rs:5`, `crates/pi-coding-agent/src/main.rs:39`).
2. `run_cli_with_options_and_stdin` parses arguments, resolves config/request context, then dispatches interactive, print, or JSON mode (`crates/pi-coding-agent/src/lib.rs:289`, `crates/pi-coding-agent/src/lib.rs:319`, `crates/pi-coding-agent/src/lib.rs:335`).
3. The adapter creates, opens, or resumes a `CodingAgentSession` and submits a typed prompt operation (`crates/pi-coding-agent/src/print_mode.rs:109`, `crates/pi-coding-agent/src/print_mode.rs:159`, `crates/pi-coding-agent/src/coding_session/mod.rs:248`).
4. `CodingAgentSession::run` converts the public operation, selects async/read-only/mutable dispatch, admits it with operation metadata and a capability snapshot, and delegates to the relevant product flow under `crates/pi-coding-agent/src/coding_session` (`crates/pi-coding-agent/src/coding_session/operation.rs:72`).
5. `PromptTurnFlow` runs request resolution, resources, session/transaction setup, runtime assembly, user-input recording, the low-level turn, finalization, and completion emission (`crates/pi-coding-agent/src/coding_session/prompt_flow.rs:19`, `crates/pi-coding-agent/src/coding_session/prompt_flow.rs:118`).
6. `RuntimeService` constructs an `Agent`; `AgentTurnFlow` prepares provider context, streams the model, decides whether to stop or run tools, executes tools, and loops as needed (`crates/pi-coding-agent/src/coding_session/prompt_flow.rs:257`, `crates/pi-agent-core/src/agent_turn_flow/runtime.rs:49`).
7. Provider calls resolve through `ProviderRegistry::stream_model_with_auth`, a provider adapter, and the shared HTTP/stream processing layer (`crates/pi-agent-core/src/agent_turn_flow/nodes.rs:464`, `crates/pi-ai/src/registry.rs:231`, `crates/pi-ai/src/providers`).
8. Low-level `AgentEvent` values are recorded by the prompt context, mapped into sequenced product events by `EventService`, committed as durable session facts where applicable, and projected by the selected adapter (`crates/pi-coding-agent/src/coding_session/prompt_flow.rs:300`, `crates/pi-coding-agent/src/coding_session/event_service.rs:174`).

### Tool Call Loop

1. `AgentTurnFlow` receives an assistant message whose stop reason is `ToolUse` and extracts pending calls in `crates/pi-agent-core/src/agent_turn_flow/nodes.rs`.
2. Before/after hooks and operation-local capabilities mediate tool execution; tools run sequentially or in parallel according to `ToolExecutionMode` in `crates/pi-agent-core/src/loop_runtime/tools.rs`.
3. Tool start/update/end events are emitted, tool result messages are appended to agent history, and the flow transitions back through context preparation for another provider request in `crates/pi-agent-core/src/agent_turn_flow/runtime.rs`.

### Session Commit and Replay

1. `SessionService` creates or opens a session handle backed by `session.json` and `events.jsonl` under the configured session root in `crates/pi-coding-agent/src/coding_session/session_service.rs`.
2. A `TurnTransaction` stages operation-started, input, assistant/tool, runtime-generation, and operation-terminal facts in `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`.
3. Finalization appends typed envelopes through `crates/pi-coding-agent/src/coding_session/session_log/store.rs`, updates the manifest, and emits pending/committed/skipped product events through `crates/pi-coding-agent/src/coding_session/event_service.rs`.
4. Open, hydration, navigation, snapshot, export, and recovery paths fold the event log through `crates/pi-coding-agent/src/coding_session/session_log/replay.rs` rather than treating adapter state as durable truth.

### Interactive Projection

1. `run_interactive_mode` creates a `ProcessTerminal` and enters the product event loop (`crates/pi-coding-agent/src/interactive/app.rs:63`, `crates/pi-coding-agent/src/interactive/loop.rs:191`).
2. The loop subscribes to sequenced product events and requests snapshots for recovery (`crates/pi-coding-agent/src/interactive/loop.rs:1329`).
3. `UiProjection` and `CodingEventBridge` translate product events into UI-specific deltas in `crates/pi-coding-agent/src/interactive/event_bridge.rs`.
4. `InteractiveRoot` mutates transcript/menu/footer state, while `Tui<T>` in `crates/pi-tui/src/tui.rs` owns focus, overlays, input dispatch, and render strategy.

**State Management:**
- Product ownership is explicit mutable state on `CodingAgentSession` in `crates/pi-coding-agent/src/coding_session/mod.rs`; services are fields rather than process-wide service locators.
- Each operation receives an immutable `OperationCapabilitySnapshot` from `crates/pi-coding-agent/src/coding_session/capability_snapshot.rs`; runtime-affecting permission state is generation-tracked.
- Low-level agent state is shared as `Arc<RwLock<AgentState>>` while a turn copies state into `AgentTurnContext` and applies it back after graph execution in `crates/pi-agent-core/src/agent_turn_flow/runtime.rs`.
- Live adapter events use Tokio broadcast channels plus a bounded retained deque in `crates/pi-coding-agent/src/coding_session/event_service.rs`; sequence gaps require snapshot recovery.
- Durable state is the Rust-native manifest/event log under `crates/pi-coding-agent/src/coding_session/session_log`; interactive and protocol state are projections.

## Key Abstractions

**`Flow<C>`:**
- Purpose: Represent workflow nodes and action-selected transitions with cancellation, maximum-step protection, and lifecycle events.
- Examples: `crates/pi-agent-core/src/flow.rs`, `crates/pi-coding-agent/src/coding_session/prompt_flow.rs`, `crates/pi-coding-agent/src/coding_session/agent_team_flow.rs`
- Pattern: Generic state-machine/graph runner; operation-specific context carries temporary state.

**`CodingAgentOperation` / `Operation`:**
- Purpose: Define the complete product action vocabulary and attach admission/dispatch metadata.
- Examples: `crates/pi-coding-agent/src/coding_session/public_operation.rs`, `crates/pi-coding-agent/src/coding_session/operation.rs`
- Pattern: Stable public command mapped to an internal discriminated operation and typed outcome.

**`CodingAgentSession`:**
- Purpose: Act as the product runtime owner and stable coordination boundary for sessions, services, operations, clients, events, and capabilities.
- Examples: `crates/pi-coding-agent/src/coding_session/mod.rs`, re-exported from `crates/pi-coding-agent/src/lib.rs`
- Pattern: Facade/service container during the operation-runtime convergence; new callers use `run` rather than adding operation-specific public methods.

**Operation Contexts:**
- Purpose: Carry temporary workflow state, diagnostics, transactions, capability handles, and outcomes between Flow nodes.
- Examples: `PromptTurnContext` in `crates/pi-coding-agent/src/coding_session/prompt.rs`, `AgentTeamContext` in `crates/pi-coding-agent/src/coding_session/agent_team_flow.rs`, `AgentTurnContext` in `crates/pi-agent-core/src/agent_turn_flow/context.rs`
- Pattern: Mutable context object scoped to one graph execution; durable facts leave through services/transactions.

**`SessionEventEnvelope` and `TurnTransaction`:**
- Purpose: Preserve typed durable facts and atomic operation boundaries independent of UI/protocol formats.
- Examples: `crates/pi-coding-agent/src/coding_session/session_log/event.rs`, `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`
- Pattern: Append-only event log plus replay/fold; manifest stores session-level index state.

**`ProductEvent`:**
- Purpose: Provide an adapter-facing, sequenced semantic stream with family classification, operation IDs, terminal status, durability, and compatibility events.
- Examples: `crates/pi-coding-agent/src/coding_session/event.rs`, `crates/pi-coding-agent/src/coding_session/event_service.rs`
- Pattern: Publish/subscribe event bus with bounded replay and snapshot recovery.

**`ProviderRegistry` / `ApiProvider`:**
- Purpose: Select a provider by `Model.api`, inject scoped auth, and normalize all providers to `EventStream`.
- Examples: `crates/pi-ai/src/registry.rs`, `crates/pi-ai/src/providers/mod.rs`
- Pattern: Registry/strategy; prefer scoped `AiClient` or `ProviderRegistry` over deprecated global registration.

**`Component` / `Terminal`:**
- Purpose: Separate terminal I/O and generic renderable/input-aware UI components from coding-agent semantics.
- Examples: `crates/pi-tui/src/component.rs`, `crates/pi-tui/src/terminal.rs`, `crates/pi-tui/src/tui.rs`
- Pattern: Trait-based generic presentation runtime with a virtual terminal test double.

**Plugin Provider Traits:**
- Purpose: Restrict extensions to declared tools, commands, hooks, keybinds, dialogs, UI actions, and registered flow extension points.
- Examples: `crates/pi-coding-agent/src/plugins/tool.rs`, `crates/pi-coding-agent/src/plugins/hook.rs`, `crates/pi-coding-agent/src/plugins/registry.rs`
- Pattern: Capability-scoped provider registry; Lua plugins are adapted into the same host contracts by `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs`.

## Entry Points

**Coding Agent Binary:**
- Location: `crates/pi-coding-agent/src/main.rs`
- Triggers: Running the `pi-coding-agent` package binary.
- Responsibilities: Process-level stdin/TTY handling, RPC fast path, builtin tool setup, CLI facade invocation, and exit codes.

**Library CLI Facade:**
- Location: `crates/pi-coding-agent/src/lib.rs`
- Triggers: Embedders call `run_cli`, `run_cli_with_options`, or `run_cli_with_options_and_stdin`.
- Responsibilities: Parse/resolve/dispatch without taking direct ownership of process exit.

**Stable Embedding API:**
- Location: `crates/pi-coding-agent/src/lib.rs` (`api` module)
- Triggers: Rust callers embedding product sessions or typed operations.
- Responsibilities: Expose the intended stable product surface, including `CodingAgentSession`, operations, outcomes, events, snapshots, resources, and tools.

**Interactive Adapter:**
- Location: `crates/pi-coding-agent/src/interactive/app.rs`
- Triggers: No explicit print/JSON/RPC mode and stdin/stdout are TTYs.
- Responsibilities: Start/stop terminal mode, create the product UI, run input/event/render scheduling, and project product state.

**Print Adapter:**
- Location: `crates/pi-coding-agent/src/print_mode.rs`
- Triggers: `--print` or explicit print mode.
- Responsibilities: Create/open a persistent or transient coding session, run one prompt, and return final text.

**JSON Adapter:**
- Location: `crates/pi-coding-agent/src/protocol/json_mode.rs`
- Triggers: Explicit JSON mode.
- Responsibilities: Run a prompt and serialize protocol events/outcome for one-shot machine consumption.

**RPC Adapter:**
- Location: `crates/pi-coding-agent/src/protocol/rpc.rs`
- Triggers: Explicit RPC mode from the binary fast path.
- Responsibilities: Process JSONL commands, manage running operation/event subscriptions, report stream lag, and serve snapshots/responses over stdio.

**Workspace Root Placeholder:**
- Location: `src/main.rs`
- Triggers: Running the root `pi-rust` package.
- Responsibilities: Prints `Hello, world!` only; do not treat it as the product runtime.

## Architectural Constraints

- **Threading:** Tokio's multi-thread runtime drives product binaries and async provider/tool workflows; `AgentTurnFlow` streams events while operating on `Arc<RwLock<AgentState>>` in `crates/pi-agent-core/src/agent_turn_flow/runtime.rs`. The interactive adapter remains a single coordinating event loop over terminal input, prompt tasks, product events, and render deadlines in `crates/pi-coding-agent/src/interactive/loop.rs`.
- **Global state:** Scoped registries/services are preferred. Deprecated compatibility globals remain in `crates/pi-ai/src/registry.rs`; environment-backed auth and terminal/color detection also read process state. Tests serialize environment/provider registry mutation through guards in `crates/pi-coding-agent/src/lib.rs`.
- **Circular imports:** No crate-level circular dependency is possible in the current Cargo DAG. Preserve `pi-coding-agent -> pi-agent-core -> pi-ai` and `pi-coding-agent -> pi-tui`; never add reverse dependencies from `pi-ai`, `pi-agent-core`, or `pi-tui` to product code.
- **Durability:** `SessionEvent` is the durable fact boundary in `crates/pi-coding-agent/src/coding_session/session_log/event.rs`; `ProductEvent`, raw `FlowEvent`, raw `AgentEvent`, and TUI state are not substitutes for session persistence.
- **Capabilities:** Filesystem, shell, tool, model, session, UI, and plugin access must come from `OperationCapabilitySnapshot` in `crates/pi-coding-agent/src/coding_session/capability_snapshot.rs`, not from unscoped paths/services passed into arbitrary nodes or plugins.
- **Event recovery:** Product event channels and retained replay are bounded in `crates/pi-coding-agent/src/coding_session/event_service.rs`; adapters must detect lag/gaps and recover from `UiSnapshot`/fresh state rather than assuming lossless broadcast delivery.
- **Extension boundary:** Plugins may register through the explicit contracts in `crates/pi-coding-agent/src/plugins`; generic Flow internals, session storage, adapter state, and raw services are not plugin-owned extension surfaces.
- **Root package:** `Cargo.toml` includes all crates in one workspace but the root package has no dependencies and `src/main.rs` is a scaffold. Product work belongs in an owned crate under `crates/`.

## Anti-Patterns

### Expanding Compatibility Surfaces

**What happens:** Root modules and operation-specific methods remain public/deprecated during migration in `crates/pi-coding-agent/src/lib.rs` and `crates/pi-coding-agent/src/coding_session/mod.rs`; adding new consumers to them prolongs the broad facade.
**Why it's wrong:** It couples callers to internal module layout and duplicates the typed `CodingAgentSession::run` operation boundary, making operation admission and future narrowing harder.
**Do this instead:** Import stable contracts from `pi_coding_agent::api` and submit `CodingAgentOperation` through `CodingAgentSession::run` in `crates/pi-coding-agent/src/coding_session/mod.rs`.

### Bypassing Services or Transactions

**What happens:** A Flow node or adapter directly writes session files, publishes wire events, or mutates durable owner state instead of using `SessionService`, `EventService`, and an operation transaction under `crates/pi-coding-agent/src/coding_session`.
**Why it's wrong:** Direct side effects break replay invariants, pending/committed event ordering, recovery markers, capability enforcement, and adapter convergence.
**Do this instead:** Keep nodes focused on context progression, stage durable facts through `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`, commit through `crates/pi-coding-agent/src/coding_session/session_service.rs`, and publish semantics through `crates/pi-coding-agent/src/coding_session/event_service.rs`.

### Leaking Product Semantics Downward

**What happens:** Coding-session, profile/team, plugin policy, or protocol concepts are added to `crates/pi-agent-core/src` or `crates/pi-tui/src`.
**Why it's wrong:** It reverses the dependency direction and prevents the low-level agent runtime and TUI from remaining reusable and independently testable.
**Do this instead:** Keep product workflows/projections in `crates/pi-coding-agent/src/coding_session` or `crates/pi-coding-agent/src/interactive`; expose only generic hooks, events, components, and capability-neutral primitives from lower crates.

### Implementing Product Work in the Root Scaffold

**What happens:** New features are added to `src/main.rs` because it appears to be the workspace entry point.
**Why it's wrong:** The root package has no product dependencies and is disconnected from the tested `pi-coding-agent` runtime.
**Do this instead:** Extend `crates/pi-coding-agent/src/main.rs` for process bootstrap or the appropriate owned module under `crates/pi-coding-agent/src` for product behavior.

## Error Handling

**Strategy:** Use typed errors within ownership boundaries, map them at layer edges, and represent streaming failures both as terminal stream events and returned errors.

**Patterns:**
- `FlowError` in `crates/pi-agent-core/src/flow.rs` covers graph construction, cancellation, missing transitions, step limits, and node failure; product `FlowService` maps it to `CodingSessionError::Flow`.
- `CodingSessionError` in `crates/pi-coding-agent/src/coding_session/error.rs` groups config, auth, input, resource, session, partial commit, provider, tool, flow, plugin, capability, busy, event-gap/lag, protocol, and cancellation failures.
- `CliError` in `crates/pi-coding-agent/src/error.rs` is the adapter/process boundary; `crates/pi-coding-agent/src/coding_session/error.rs` maps product errors to user-facing CLI categories.
- Provider failures are normalized by `ProviderError` under `crates/pi-ai/src/transport/error.rs` and by terminal `AssistantMessageEvent::Error` values consumed in `crates/pi-agent-core/src/agent_turn_flow/nodes.rs`.
- Persistent commit uncertainty is not collapsed into a generic error; `CodingSessionError::PartialCommit` and startup recovery markers in `crates/pi-coding-agent/src/coding_session/session_service.rs` preserve the ambiguous operation state.
- Adapters treat event-stream lag/gaps as recoverable protocol conditions requiring a fresh snapshot, as implemented in `crates/pi-coding-agent/src/protocol/rpc.rs` and `crates/pi-coding-agent/src/interactive/loop.rs`.

## Cross-Cutting Concerns

**Logging:** There is no general logging framework. Process diagnostics and startup/resource notices use stderr in `crates/pi-coding-agent/src/main.rs` and `crates/pi-coding-agent/src/interactive/loop.rs`; runtime observability primarily travels as typed diagnostics, agent events, product events, and durable session events under `crates/pi-coding-agent/src/coding_session`.
**Validation:** CLI values are parsed in `crates/pi-coding-agent/src/args.rs`; request/config resolution lives in `crates/pi-coding-agent/src/request.rs`; tool definitions validate schemas in `crates/pi-agent-core/src/types.rs`; operation metadata/admission and capability snapshots validate execution authority in `crates/pi-coding-agent/src/coding_session/operation.rs` and `crates/pi-coding-agent/src/coding_session/capability_snapshot.rs`; boundary guard tests under each crate's `tests/` enforce dependency/API rules.
**Authentication:** Provider auth is resolved into scoped runtime options by `ProviderAuthResolver`/`EnvProviderAuthResolver` in `crates/pi-ai/src/registry.rs`; product config/auth sources are assembled in `crates/pi-coding-agent/src/config/auth.rs` and passed through request/runtime snapshots. Secrets are not persisted into codebase documentation or adapter events.

---

*Architecture analysis: 2026-07-10*
