# Flow Runtime Design

## Purpose

`pi-rust` should absorb PocketFlow's core idea: agent behavior is a graph of small executable nodes, not a hard-coded monolithic loop. The Rust version should keep the existing strengths of `pi-agent-core` (`pi-ai` providers, `AgentTool`, hooks, sessions, resources, compaction, and harness events) while adding a first-class workflow runtime that can later host agent loops, product workflows, plugins, delegation-first agent collaboration, and multi-agent orchestration.

This spec covers the first implementation step: add a minimal, typed `pi_agent_core::flow` module and tests. It does not migrate the existing agent loop.

## Goals

- Add a small Rust-native Flow runtime in `pi-agent-core`.
- Model control flow explicitly with nodes, actions, and transitions.
- Use caller-owned typed context instead of a global dynamic shared map.
- Emit structured runtime events suitable for future harness/UI observability.
- Keep the module independent from provider APIs, tools, sessions, TUI, and plugins.
- Prove semantics with deterministic offline unit tests.

## Non-Goals

- Do not import PocketFlow or copy its Python API.
- Do not rewrite `Agent::prompt`, `Agent::run`, or `agent_loop.rs`.
- Do not migrate manual compaction, edit tools, or session orchestration in this step.
- Do not add Lua/plugin integration.
- Do not add parallel/batch flow execution yet.
- Do not expose `serde_json::Value` as the primary context model.

## Design Principles

PocketFlow's useful insight is that `workflow`, `agent loop`, `RAG`, `tool routing`, and `multi-agent coordination` are all graph-shaped. The Rust runtime should keep that insight while adapting the API to Rust:

- Prefer typed context `C` over dynamic dictionaries.
- Prefer an async trait object boundary that is easy to use from current `pi-agent-core`.
- Keep the first public surface small.
- Add helper traits/builders later only after real call sites reveal friction.
- Preserve existing `pi-agent-core` observability instead of replacing it with print-style logging.

## Module Boundary

Add a new module:

```rust
pub mod flow;
```

The first implementation should live in `crates/pi-agent-core/src/flow.rs`. It is a pure orchestration module. It does not depend on `pi_ai`, `Agent`, `AgentTool`, session storage, resources, or TUI code. If the module grows after the first real migrations, it can be split into `src/flow/`.

`pi-agent-core/src/lib.rs` should export `pub mod flow;` and may re-export stable public types after the API settles. For the first step, module-level exports are sufficient:

```rust
use pi_agent_core::flow::{Action, Flow, FlowError, FlowEvent, FlowNode, NodeId};
```

## Core Types

### `Action`

An action is the label returned by a node to select the next transition.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Action(String);
```

Required behavior:

- `Action::default()` returns `"default"`.
- `Action::new(value)` rejects empty strings.
- `Action::as_str()` exposes the label.
- `From<&str>` or `TryFrom<&str>` may be provided if validation remains explicit.

The default transition mirrors PocketFlow's default edge while keeping Rust validation stricter.

### `NodeId`

`NodeId` is a stable node identifier used in transitions, events, and errors.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(String);
```

Required behavior:

- `NodeId::new(value)` rejects empty strings.
- `NodeId::as_str()` exposes the identifier.

Node IDs are intentionally not indexes. Stable string IDs make future event streams, graph export, diagnostics, and plugin integration easier.

### `FlowNode<C>`

`FlowNode<C>` is the executable node boundary.

```rust
use std::future::Future;
use std::pin::Pin;

pub trait FlowNode<C>: Send + Sync {
    fn name(&self) -> &str;

    fn run<'a>(
        &'a self,
        ctx: &'a mut C,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>>;
}
```

The first version deliberately does not expose PocketFlow's `prep -> exec -> post` as a trait. That three-part lifecycle is valuable as a design pattern, but a direct Rust trait would create avoidable generic and object-safety complexity before there are real call sites. A later helper can wrap three closures or a `StepNode` type around this core trait.

Node-local failures return `String`; the runner wraps them as `FlowError::NodeFailed { node, message }`. Runtime errors such as missing transitions, cancellation, and max-step protection remain `FlowError`.

### `Flow<C>`

`Flow<C>` owns nodes and directed transitions.

Conceptual shape:

```rust
pub struct Flow<C> {
    start: NodeId,
    nodes: HashMap<NodeId, Box<dyn FlowNode<C>>>,
    transitions: HashMap<(NodeId, Action), NodeId>,
}
```

Required builder behavior:

- A flow requires a valid start node.
- Adding the same node ID twice returns `FlowError::DuplicateNode`.
- Adding an edge from or to an unknown node returns `FlowError::UnknownNode`.
- `edge(from, to)` is shorthand for the default action.
- `edge_on(from, action, to)` registers a conditional transition.

The first implementation can use a builder-style API or mutable methods. It should keep construction errors explicit rather than panicking.

### `FlowRunOptions`

Runtime options should remain small:

```rust
pub struct FlowRunOptions {
    pub max_steps: usize,
    pub strict_missing_transition: bool,
    pub cancel: Option<tokio_util::sync::CancellationToken>,
    pub on_event: Option<FlowEventCallback>,
}
```

Defaults:

- `max_steps`: `1024`.
- `strict_missing_transition`: `true`.
- `cancel`: `None`.
- `on_event`: `None`.

The strict default is intentional. In Python PocketFlow, a missing action can warn and end. In Rust core, a graph definition mistake should be visible during tests unless the caller explicitly opts into lenient completion.

### `FlowOutcome`

`Flow::run` should return an outcome when execution completes normally.

```rust
pub struct FlowOutcome {
    pub last_node: NodeId,
    pub last_action: Action,
    pub steps: usize,
    pub path: Vec<NodeId>,
}
```

The path is useful for tests and future debugging. It is acceptable for the first version to clone node IDs; graph execution is not currently performance critical.

### `FlowEvent`

The runtime should emit structured events through an optional callback.

```rust
pub enum FlowEvent {
    Started { start: NodeId },
    NodeStart { node: NodeId, name: String, step: usize },
    NodeEnd { node: NodeId, name: String, action: Action, step: usize },
    MissingTransition { node: NodeId, action: Action },
    Completed { outcome: FlowOutcome },
    Error { error: FlowError },
}
```

The event API is a callback on run options:

```rust
pub type FlowEventCallback = Arc<dyn Fn(FlowEvent) + Send + Sync>;
```

The callback receives cloned event values. That keeps the initial API simple and independent from stream lifetimes. A future `Stream<Item = FlowEvent>` runner can be added if a caller needs backpressure or async observation.

## Execution Semantics

The runner executes this loop:

1. Validate that the start node exists.
2. If cancel token is cancelled, return `FlowError::Cancelled`.
3. Emit `Started`.
4. Set current node to `start`.
5. Before each node, check cancellation and max steps.
6. Emit `NodeStart`.
7. Call `node.run(ctx).await`.
8. On node failure, wrap the message in `FlowError::NodeFailed`, emit `Error`, and return it.
9. On success, emit `NodeEnd`.
10. Look up `(current_node, returned_action)`.
11. If a transition exists, move to that node and continue.
12. If no transition exists and the current node has no outgoing transitions, complete normally.
13. If no transition exists and outgoing transitions exist:
    - strict mode: return `FlowError::MissingTransition`;
    - lenient mode: emit `MissingTransition` and complete normally.

This preserves PocketFlow's graph semantics while making invalid actions fail loudly in normal Rust tests.

## Errors

`FlowError` should be a typed enum:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowError {
    EmptyAction,
    EmptyNodeId,
    DuplicateNode { node: NodeId },
    UnknownNode { node: NodeId },
    MissingStartNode { node: NodeId },
    MissingTransition { node: NodeId, action: Action },
    MaxStepsExceeded { max_steps: usize },
    Cancelled,
    NodeFailed { node: NodeId, message: String },
}
```

The first version can store node failure as `String`. This matches much of the current `pi-agent-core` and `pi-coding-agent` error boundary, where tools and hooks often return `Result<_, String>`. A richer source chain can be added later if real call sites need it.

Implement:

- `std::fmt::Display`
- `std::error::Error`

## Tests

Add focused tests under `crates/pi-agent-core/tests/flow.rs` or inside the flow module.

Required tests:

- Linear execution: `start -> second -> done` mutates typed context and returns expected path.
- Conditional transition: one node returns `"retry"` or `"done"` and routes correctly.
- Duplicate node: construction fails.
- Unknown edge endpoint: construction fails.
- Missing transition: strict mode returns `FlowError::MissingTransition`.
- Lenient missing transition: run completes and records the last action.
- Max steps: self-loop returns `FlowError::MaxStepsExceeded`.
- Node failure: node error returns `FlowError::NodeFailed`.
- Cancellation: a cancelled token before execution returns `FlowError::Cancelled`.
- Public API smoke: integration test imports `pi_agent_core::flow::*`.

All tests must be deterministic and offline.

## Integration Plan

This spec's implementation step only adds the module and tests. Later work should proceed in separate specs/plans:

1. Use Flow to model one low-risk product workflow, such as manual compaction or edit-tool read/validate/apply.
2. Extract internal agent-loop phases into nodes while preserving `Agent::prompt` and `Agent::run`.
3. Add graph export for debugging and documentation, likely Mermaid and JSON.
4. Extend the plugin system roadmap with a `FlowProvider` or `FlowExtension` trait.
5. Model delegation-first child-agent and multi-agent workflows as nested flows once the basic runtime has proven stable.

## Compatibility

The first implementation must be backwards compatible:

- No existing public agent API changes.
- No session JSONL format changes.
- No provider request or stream event changes.
- No CLI behavior changes.
- No TUI behavior changes.

## Open Decisions Resolved

- Location: start inside `pi-agent-core` as `pi_agent_core::flow`.
- Context: typed caller-owned `C`, not `serde_json::Value`.
- Missing transition default: strict error.
- `prep/exec/post`: documented as a pattern, not exposed as first public trait.
- Parallel/batch execution: deferred.
- Plugin integration: deferred.
