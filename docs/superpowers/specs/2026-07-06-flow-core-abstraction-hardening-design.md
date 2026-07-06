# Flow Core Abstraction Hardening Design

## Purpose

`pi-rust` already has a Flow-centered product runtime, but the core Flow abstraction is not yet as clean as the architecture name implies. The next architecture slice should harden the core abstractions after the completed Phase 1-6 migration, rather than reopening those completed phases.

This document records the expanded crate audit, the PocketFlow comparison, the proposed reform path, feasibility, and value.

## Expanded Audit

### `pi-ai`

`pi-ai` is the provider and model layer. Its core abstraction is reasonably clean:

- provider request/streaming;
- provider registry and scoped client/runtime construction;
- typed model and auth/stream options;
- no product session or UI ownership.

The remaining design pressure is public API width and legacy global-provider compatibility. That is a boundary-hardening concern, not the main Flow abstraction problem.

### `pi-agent-core`

`pi-agent-core` owns the low-level runtime, and this is the key abstraction gap.

The low-level `pi_agent_core::flow` runtime provides:

- `Action`;
- `NodeId`;
- `FlowNode<C>`;
- `Flow<C>`;
- `FlowOutcome`;
- `FlowEvent`;
- sequential async graph execution.

That is a solid minimal graph executor. It does not yet provide first-class subflows, batch flows, parallel flows, typed action enums, or reusable flow-construction helpers.

`AgentTurnFlow` is the main mismatch. The module has:

- `AgentTurnContext`;
- extracted node implementations for provider streaming, runtime compaction, stop/tool decision, and tool execution;
- tests that manually compose those nodes with `Flow`.

However, the runtime entrypoint still delegates to a monolithic loop through `AgentTurnFlow::run_state(state) -> run_loop(state)`. That means `AgentTurnFlow` is currently a node library plus compatibility entrypoint, not a true Flow graph runtime.

This mismatch matters because the docs and TODO describe Phase 4 as complete, while the core abstraction still has a name stronger than its implementation.

### `pi-coding-agent`

`pi-coding-agent` is the product runtime owner. Its boundaries are mostly correct:

- `CodingAgentSession` owns product operations;
- product operations are represented as flow contexts and flow outcomes;
- adapters consume `CodingAgentEvent`, not low-level Flow node IDs;
- session persistence is transaction-owned through `SessionService`;
- plugin APIs are capability-scoped instead of exposing raw session/runtime/provider internals.

The weak point is repetition across product flows. Many flows repeat the same pattern:

- `*_NODE_IDS`;
- linear `Flow::new(...); for pair in NODE_IDS.windows(2) { flow.edge(...) }`;
- context-local `failure_error`;
- `take_failure_error`;
- `finish_success`;
- manual nested flow execution.

This repetition is a sign that the product layer has adopted Flow, but the shared Flow composition layer is still thin.

### `pi-tui`

`pi-tui` remains a generic terminal/UI crate. Its core abstraction is clean enough for the current architecture: component, terminal, virtual terminal, and TUI application shell. It should not absorb product Flow or session semantics.

### Placeholder Crates

`pi-mom`, `pi-pods`, and `pi-web-ui` currently do not contain meaningful core abstractions. They should not be evaluated as architecture problems until real responsibilities are assigned.

## PocketFlow Mapping

PocketFlow describes the workflow model as:

```text
Graph + Shared Store
Node handles simple tasks.
Flow connects nodes through Actions.
Shared Store enables communication between nodes.
Batch nodes/flows support data-intensive tasks.
Async nodes/flows support waiting.
Parallel nodes/flows support I/O-bound work.
```

The current `pi-rust` mapping is:

| PocketFlow concept | Current `pi-rust` state | Assessment |
| --- | --- | --- |
| Graph | `Flow<C>` | Present but minimal |
| Node | `FlowNode<C>` | Present |
| Action edges | string-backed `Action` | Present, weakly typed |
| Shared Store | typed `C` context | Present in Rust form, not explicitly named |
| Async | all nodes are async futures | Present |
| Batch | product-specific loops | Not first-class |
| Parallel | ad hoc `FuturesUnordered` in tool execution | Not first-class |
| Subflow | manual nested flow calls in product flows | Present by convention, not abstraction |

The right Rust model is not to copy PocketFlow's dynamic shared store exactly. The better model is:

```text
Typed Graph + Typed Operation Context + Capability-Scoped Services
```

In this model, `C` is the shared store. Nodes communicate by mutating a flow-specific typed context. Services own durable side effects such as session writes, event publication, provider runtime construction, and plugin collection.

## Design Decisions

### 1. Make Typed Context the Explicit Shared Store

Do not add an untyped key-value shared store. Instead, document and enforce:

- every Flow has exactly one typed context;
- nodes communicate through that context;
- context fields represent operation-scoped state;
- product owner/services are not passed directly into low-level nodes unless exposed as narrow capability handles.

### 2. Make `AgentTurnFlow` a Real Graph

`AgentTurnFlow` should stop being a wrapper name over `run_loop`. The runtime entrypoint should construct and run a `Flow<AgentTurnContext>` for each turn cycle while preserving:

- `Agent::run()` public event stream;
- low-level `AgentEvent` ordering;
- runtime compaction semantics;
- transform/convert/provider hooks;
- `BeforeProviderRequest` event;
- tool execution behavior;
- abort, steer, follow-up, max-turn, and prepare-next-turn behavior.

The old `run_loop` can remain temporarily as a source reference during migration, but it should not remain the active implementation once the graph path is complete.

### 3. Add Typed Action Conventions Before Generic Runtime Complexity

The bottom `Action` type can remain string-backed for compatibility, but each non-trivial flow should define local action constants or local action enums and convert them to `Action`.

This gives compile-time locality without forcing a generic `Flow<C, A>` rewrite.

### 4. Add Product Flow Helpers After `AgentTurnFlow`

Once the lowest-level graph is honest, add small helpers for product flows:

- linear graph construction from node specs;
- common failure extraction pattern;
- common graph/outcome runner helper;
- stable node ID assertions.

These should reduce repetition without hiding flow shape.

### 5. Treat Subflow, Batch, and Parallel as Staged Additions

Do not add a large PocketFlow clone in one step.

Order:

1. `AgentTurnFlow` graph correctness.
2. Typed action and construction helpers.
3. Product subflow convention for nested `PromptTurnFlow`, `AgentInvocationFlow`, and `AgentTeamFlow`.
4. Deterministic batch/parallel helpers for product workflows and tool execution.

## Proposed Target Abstractions

### Core Flow Runtime

`pi_agent_core::flow` should remain small:

- `Flow<C>`;
- `FlowNode<C>`;
- `Action`;
- `NodeId`;
- `FlowOutcome`;
- `FlowRunOptions`;
- `FlowEvent`.

Near-term additions should be helper-level, not a type-system rewrite:

- `Flow::edge_on_str(from, action, to)` or equivalent convenience;
- `Flow::linear(start, specs)` only if it keeps node IDs visible;
- an `ActionName` or local enum conversion pattern;
- test helpers for expected path assertions.

### Low-Level Agent Runtime

`AgentTurnFlow` should expose a private/internal graph shape:

```text
start_turn
drain_queued_input
maybe_compact_runtime_context
prepare_provider_request
apply_before_provider_request_hook
provider_stream
decide_after_assistant
maybe_prepare_next_turn
execute_tools
```

The graph should loop on `continue`, finish on `done`, and finish with error events on `error` or `aborted`.

### Product Flow Layer

`pi-coding-agent` should keep product flows internal to `coding_session` and continue exposing product behavior through:

- `CodingAgentSession`;
- `CodingAgentEvent`;
- typed operation outcomes;
- capability-scoped plugin surfaces.

Product flows should not expose `FlowOutcome.last_node` to adapters or protocols.

## Feasibility

This reform is feasible because the current code already has most of the pieces:

- `Flow<C>` is stable enough for graph execution;
- `AgentTurnContext` exists;
- provider/tool/compaction nodes exist and have focused tests;
- product flows already run through `FlowService`;
- adapter tests already guard against Flow-node protocol leakage;
- `CodingAgentEvent` is already the product event boundary;
- session writes are centralized enough to avoid storage rewrites.

The highest-risk part is not building the graph. The highest risk is preserving subtle `Agent::run()` behavior while moving it from `run_loop` into nodes.

## Risks and Mitigations

### Risk: Agent Event Ordering Regresses

Mitigation:

- add behavior tests before migration;
- compare `Agent::run()` event sequences for provider success, provider error, tool use, parallel tools, runtime compaction, abort, steer, follow-up, and max-turn cases;
- keep `AgentEvent` as the low-level event stream.

### Risk: Graph Context Becomes a Hidden Global

Mitigation:

- keep `AgentTurnContext` scoped to one running turn graph;
- add explicit state import/export methods between `AgentState` and `AgentTurnContext`;
- keep product services out of `pi-agent-core`.

### Risk: Parallel Helpers Create Nondeterminism

Mitigation:

- keep parallel execution result aggregation ordered by original tool/member index;
- test event ordering separately from result ordering;
- add deterministic virtual-time tests for delayed tools.

### Risk: Product Flows Become Over-Abstracted

Mitigation:

- keep node IDs and graph construction visible;
- add helpers only where repetition is already proven;
- avoid hiding business flow shape behind macros.

## Value

### Architecture Honesty

The code will match the documented architecture. `AgentTurnFlow` will be a real graph runtime, not a wrapper over a monolithic loop.

### Better Local Reasoning

Each turn phase becomes a node with an explicit input/output contract through `AgentTurnContext`. Debugging provider, tool, compaction, and control behavior becomes easier.

### Safer Product Growth

Delegation, teams, self-healing edit, plugin load, export, and compaction already use product flows. Strengthening the core Flow layer gives those workflows a consistent composition model.

### Better Plugin Boundary

Capability-scoped plugins can eventually attach to defined flow extension points without receiving raw session, provider, storage, or runtime internals.

### Lower Regression Risk Over Time

Graph-level tests can lock paths and transitions. Node-level tests can lock individual behavior. Product adapter tests can continue ignoring Flow internals.

## Success Criteria

The hardening effort is complete when:

- `AgentTurnFlow::run_state` no longer delegates to an active monolithic `run_loop`;
- `AgentTurnFlow` has a real `Flow<AgentTurnContext>` graph path;
- existing `Agent::run()` behavior is preserved by focused tests;
- product flows use common construction/outcome helpers where repetition is high;
- nested product flows use an explicit subflow convention;
- batch/parallel helpers are deterministic and covered by tests;
- adapters and protocols still do not expose Flow node IDs.

