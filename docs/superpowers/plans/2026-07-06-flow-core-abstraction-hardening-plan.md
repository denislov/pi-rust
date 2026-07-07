# Flow Core Abstraction Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `pi-rust`'s Flow abstraction match the documented Flow-centered architecture by turning `AgentTurnFlow` into a real graph runtime and then reducing repeated product-flow composition patterns.

**Architecture:** Keep `Flow<C>` as the small typed graph executor. Treat `C` as the typed shared store, keep product side effects in services, and introduce helpers only after behavior-preserving tests are in place.

**Tech Stack:** Rust 2024, Tokio, futures, `pi_agent_core::flow`, `pi-coding-agent` product flows, deterministic faux-provider tests.

---

## Current Execution Status

This status block reconciles the original plan with the current repository state before continuing execution.

- [x] Task 1: graph-shape and runtime-entrypoint gap tests exist.
- [x] Task 2: `AgentTurnFlow` builds a real `Flow<AgentTurnContext>` graph.
- [x] Task 3: missing turn graph nodes and focused node behavior tests are in place.
- [x] Task 4: `Agent::run()` executes through the graph-backed `AgentTurnFlow::run_state` path.
- [x] Task 5: shared product Flow construction helpers are in place for export, manual compaction, and branch summary flows.
- [x] Task 6: nested invocation/team subflow execution is routed through explicit `FlowService` subflow runners.
- [x] Task 7: deterministic parallel tool aggregation is extracted into a named helper while preserving completion-order events and assistant-order transcript results.
- [x] Task 8: final verification and documentation closeout are complete.

## File Structure

- Modify: `crates/pi-agent-core/src/agent_turn_flow/runtime.rs`
  - Own the real `AgentTurnFlow` graph builder and the `Agent::run()` runtime bridge.
- Modify: `crates/pi-agent-core/src/agent_turn_flow/context.rs`
  - Add explicit import/export helpers between `AgentState` and `AgentTurnContext`.
- Modify: `crates/pi-agent-core/src/agent_turn_flow/nodes.rs`
  - Add missing graph nodes for queue drainage, full provider request preparation, provider hook application, stop hooks, and next-turn preparation.
- Modify: `crates/pi-agent-core/src/agent_turn_flow/mod.rs`
  - Re-export only the intended low-level test/API surface.
- Modify: `crates/pi-agent-core/tests/agent_turn_flow.rs`
  - Add graph-shape and behavior-preservation coverage before switching the runtime path.
- Modify: `crates/pi-agent-core/tests/agent_runtime_boundary.rs`
  - Replace the current wrapper-source guard with a guard that requires the graph runtime path.
- Modify: `crates/pi-coding-agent/src/coding_session/flow_service.rs`
  - Add product-flow runner helpers after `AgentTurnFlow` is graph-backed.
- Modify: selected product flow files under `crates/pi-coding-agent/src/coding_session/`
  - Convert only high-repetition graph construction and outcome-finalization code.
- Modify: `crates/pi-coding-agent/tests/event_boundary_guards.rs`
  - Keep adapter/product event independence from Flow node IDs locked.
- Modify: `docs/TODO.md`
  - Track this work as a post-Phase 6 hardening stream.

## Task 1: Lock the Current `AgentTurnFlow` Gap With Failing Tests

**Files:**
- Modify: `crates/pi-agent-core/tests/agent_turn_flow.rs`
- Modify: `crates/pi-agent-core/tests/agent_runtime_boundary.rs`

- [ ] **Step 1: Add graph-shape assertions**

Add this test near the existing `agent_turn_flow` tests:

```rust
#[test]
fn agent_turn_flow_exposes_real_graph_shape() {
    assert_eq!(
        pi_agent_core::agent_turn_flow::AgentTurnFlow::node_ids(),
        &[
            "start_turn",
            "drain_queued_input",
            "maybe_compact_runtime_context",
            "prepare_provider_request",
            "apply_before_provider_request_hook",
            "provider_stream",
            "decide_after_assistant",
            "maybe_prepare_next_turn",
            "execute_tools",
        ]
    );
}
```

- [ ] **Step 2: Add a source guard against the monolithic runtime bridge**

Replace the current `AgentTurnFlow::run_state(state)` source guard in `crates/pi-agent-core/tests/agent_runtime_boundary.rs` with:

```rust
#[test]
fn agent_turn_flow_runtime_entrypoint_does_not_delegate_to_monolithic_loop() {
    let runtime_source = include_str!("../src/agent_turn_flow/runtime.rs");

    assert!(
        !runtime_source.contains("run_loop(state)"),
        "AgentTurnFlow::run_state should drive the graph runtime instead of delegating to run_loop"
    );
    assert!(
        runtime_source.contains("AgentTurnFlow::new()"),
        "AgentTurnFlow::run_state should construct the graph runtime"
    );
    assert!(
        runtime_source.contains(".run_with_options("),
        "AgentTurnFlow::run_state should execute Flow<AgentTurnContext>"
    );
}
```

- [ ] **Step 3: Run the focused tests and confirm they fail**

Run:

```text
cargo test -p pi-agent-core --test agent_turn_flow agent_turn_flow_exposes_real_graph_shape
cargo test -p pi-agent-core --test agent_runtime_boundary agent_turn_flow_runtime_entrypoint_does_not_delegate_to_monolithic_loop
```

Expected:

```text
FAILED agent_turn_flow_exposes_real_graph_shape
FAILED agent_turn_flow_runtime_entrypoint_does_not_delegate_to_monolithic_loop
```

- [ ] **Step 4: Commit**

```text
git add crates/pi-agent-core/tests/agent_turn_flow.rs crates/pi-agent-core/tests/agent_runtime_boundary.rs
git commit -m "test(agent-core): expose agent turn flow graph gap"
```

## Task 2: Add the Real `AgentTurnFlow` Graph Builder

**Files:**
- Modify: `crates/pi-agent-core/src/agent_turn_flow/runtime.rs`
- Modify: `crates/pi-agent-core/src/agent_turn_flow/mod.rs`
- Modify: `crates/pi-agent-core/tests/agent_turn_flow.rs`

- [ ] **Step 1: Add node IDs and action constants**

Insert near the top of `runtime.rs`:

```rust
use crate::flow::{Action, Flow, FlowError, FlowOutcome, FlowRunOptions};

use super::context::AgentTurnContext;
use super::nodes::{
    ApplyBeforeProviderRequestHookNode, DecideAfterAssistantNode, DrainQueuedInputNode,
    ExecuteToolsNode, MaybeCompactRuntimeContextNode, MaybePrepareNextTurnNode,
    PrepareProviderRequestNode, ProviderStreamNode, StartTurnNode,
};

pub const AGENT_TURN_NODE_IDS: &[&str] = &[
    "start_turn",
    "drain_queued_input",
    "maybe_compact_runtime_context",
    "prepare_provider_request",
    "apply_before_provider_request_hook",
    "provider_stream",
    "decide_after_assistant",
    "maybe_prepare_next_turn",
    "execute_tools",
];

const ACTION_DEFAULT: &str = "default";
const ACTION_CONTINUE: &str = "continue";
const ACTION_TOOLS: &str = "tools";
const ACTION_DONE: &str = "done";
const ACTION_ERROR: &str = "error";
const ACTION_ABORTED: &str = "aborted";
```

- [ ] **Step 2: Change `AgentTurnFlow` from unit struct to graph owner**

Replace:

```rust
pub struct AgentTurnFlow;
```

with:

```rust
pub struct AgentTurnFlow {
    flow: Flow<AgentTurnContext>,
}
```

- [ ] **Step 3: Add graph construction**

Add this implementation block before `run_state`:

```rust
impl AgentTurnFlow {
    pub fn new() -> Result<Self, FlowError> {
        let mut flow = Flow::new(AGENT_TURN_NODE_IDS[0])?;
        flow.add_node("start_turn", StartTurnNode)?;
        flow.add_node("drain_queued_input", DrainQueuedInputNode)?;
        flow.add_node("maybe_compact_runtime_context", MaybeCompactRuntimeContextNode)?;
        flow.add_node("prepare_provider_request", PrepareProviderRequestNode)?;
        flow.add_node("apply_before_provider_request_hook", ApplyBeforeProviderRequestHookNode)?;
        flow.add_node("provider_stream", ProviderStreamNode)?;
        flow.add_node("decide_after_assistant", DecideAfterAssistantNode)?;
        flow.add_node("maybe_prepare_next_turn", MaybePrepareNextTurnNode)?;
        flow.add_node("execute_tools", ExecuteToolsNode)?;

        flow.edge("start_turn", "drain_queued_input")?;
        flow.edge("drain_queued_input", "maybe_compact_runtime_context")?;
        flow.edge("maybe_compact_runtime_context", "prepare_provider_request")?;
        flow.edge("prepare_provider_request", "apply_before_provider_request_hook")?;
        flow.edge("apply_before_provider_request_hook", "provider_stream")?;
        flow.edge("provider_stream", "decide_after_assistant")?;
        flow.edge_on("decide_after_assistant", Action::new(ACTION_TOOLS)?, "execute_tools")?;
        flow.edge_on(
            "decide_after_assistant",
            Action::new(ACTION_CONTINUE)?,
            "maybe_prepare_next_turn",
        )?;
        flow.edge_on("execute_tools", Action::new(ACTION_CONTINUE)?, "maybe_prepare_next_turn")?;

        Ok(Self { flow })
    }

    pub fn node_ids() -> &'static [&'static str] {
        AGENT_TURN_NODE_IDS
    }

    pub async fn run(
        &self,
        ctx: &mut AgentTurnContext,
    ) -> Result<FlowOutcome, FlowError> {
        self.flow.run(ctx).await
    }

    pub async fn run_with_options(
        &self,
        ctx: &mut AgentTurnContext,
        options: FlowRunOptions,
    ) -> Result<FlowOutcome, FlowError> {
        self.flow.run_with_options(ctx, options).await
    }
}
```

- [ ] **Step 4: Update `agent_turn_flow/mod.rs` unit-struct smoke test**

Replace the unit struct smoke assertion with:

```rust
#[test]
fn agent_turn_flow_builds_graph() {
    let flow = AgentTurnFlow::new().expect("agent turn flow graph should build");
    assert_eq!(AgentTurnFlow::node_ids()[0], "start_turn");
    drop(flow);
}
```

- [ ] **Step 5: Run the graph-shape test**

Run:

```text
cargo test -p pi-agent-core --test agent_turn_flow agent_turn_flow_exposes_real_graph_shape
```

Expected:

```text
test agent_turn_flow_exposes_real_graph_shape ... ok
```

- [ ] **Step 6: Commit**

```text
git add crates/pi-agent-core/src/agent_turn_flow/runtime.rs crates/pi-agent-core/src/agent_turn_flow/mod.rs crates/pi-agent-core/tests/agent_turn_flow.rs
git commit -m "feat(agent-core): build agent turn flow graph"
```

## Task 3: Fill the Missing Agent Runtime Nodes

**Files:**
- Modify: `crates/pi-agent-core/src/agent_turn_flow/context.rs`
- Modify: `crates/pi-agent-core/src/agent_turn_flow/nodes.rs`
- Modify: `crates/pi-agent-core/tests/agent_turn_flow.rs`

- [ ] **Step 1: Add context fields required by the full loop behavior**

Extend `AgentTurnContext` with:

```rust
pub max_turns_exceeded: Option<u32>,
pub should_finish: bool,
pub has_more_queued_input: bool,
```

Initialize them in `from_state`:

```rust
max_turns_exceeded: None,
should_finish: false,
has_more_queued_input: false,
```

- [ ] **Step 2: Add `StartTurnNode`**

Add to `nodes.rs`:

```rust
pub struct StartTurnNode;

impl FlowNode<AgentTurnContext> for StartTurnNode {
    fn name(&self) -> &str {
        "start_turn"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            if ctx.cancel_token.is_cancelled() {
                ctx.events.push(AgentEvent::AgentError { error: "aborted".into() });
                return Action::new("aborted").map_err(|err| err.to_string());
            }
            ctx.turn = ctx.turn.saturating_add(1);
            if let Some(max_turns) = ctx.config.max_turns
                && ctx.turn > max_turns
            {
                ctx.max_turns_exceeded = Some(max_turns);
                ctx.events.push(AgentEvent::AgentError {
                    error: format!("max turns ({}) exceeded", max_turns),
                });
                return Action::new("error").map_err(|err| err.to_string());
            }
            ctx.events.push(AgentEvent::TurnStart { turn: ctx.turn });
            default_action()
        })
    }
}
```

- [ ] **Step 3: Add `DrainQueuedInputNode`**

Add to `nodes.rs`:

```rust
pub struct DrainQueuedInputNode;

impl FlowNode<AgentTurnContext> for DrainQueuedInputNode {
    fn name(&self) -> &str {
        "drain_queued_input"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            let steering_mode = ctx.config.steering_mode;
            let steered = crate::queues::drain_queue(&mut ctx.steering_queue, steering_mode);
            ctx.messages.extend(steered);
            default_action()
        })
    }
}
```

- [ ] **Step 4: Replace `PrepareContextNode` with full provider request preparation**

Rename the node to `PrepareProviderRequestNode` and keep the current `prepare_context` logic as the base. Extend it to apply `transform_context` and `convert_to_llm` before constructing `ProviderRequestSnapshot`, matching the old loop behavior:

```rust
pub struct PrepareProviderRequestNode;

impl FlowNode<AgentTurnContext> for PrepareProviderRequestNode {
    fn name(&self) -> &str {
        "prepare_provider_request"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            prepare_provider_request(ctx).await?;
            default_action()
        })
    }
}
```

The implementation must preserve these old-loop inputs:

```rust
ctx.config.hooks.transform_context.clone()
ctx.config.hooks.convert_to_llm.clone()
ctx.config.resources.clone()
ctx.config.thinking_level
ctx.config.stream_options.clone()
ctx.cancel_token.clone()
```

- [ ] **Step 5: Add before-provider hook node**

Add:

```rust
pub struct ApplyBeforeProviderRequestHookNode;

impl FlowNode<AgentTurnContext> for ApplyBeforeProviderRequestHookNode {
    fn name(&self) -> &str {
        "apply_before_provider_request_hook"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            apply_before_provider_request_hook(ctx).await?;
            if let Some(request) = ctx.provider_request.clone() {
                ctx.events.push(AgentEvent::BeforeProviderRequest { request });
            }
            default_action()
        })
    }
}
```

- [ ] **Step 6: Split stop/tool decision from next-turn preparation**

Rename `DecideStopOrToolsNode` to `DecideAfterAssistantNode`. It should:

- append the assistant message to `ctx.messages`;
- return `tools` for tool use;
- return `error` for provider error stop reasons;
- return `aborted` for aborted stop reasons;
- return `continue` for stop/length so `MaybePrepareNextTurnNode` can run hooks and queue checks.

Add `MaybePrepareNextTurnNode`:

```rust
pub struct MaybePrepareNextTurnNode;

impl FlowNode<AgentTurnContext> for MaybePrepareNextTurnNode {
    fn name(&self) -> &str {
        "maybe_prepare_next_turn"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            maybe_prepare_next_turn(ctx).await?;
            if ctx.has_more_queued_input {
                Action::new("continue").map_err(|err| err.to_string())
            } else {
                Action::new("done").map_err(|err| err.to_string())
            }
        })
    }
}
```

- [ ] **Step 7: Add behavior tests for the missing nodes**

Extend the imports at the top of `crates/pi-agent-core/tests/agent_turn_flow.rs`:

```rust
use pi_agent_core::BeforeProviderRequestResult;
use pi_agent_core::flow::FlowNode;
use pi_agent_core::agent_turn_flow::{
    ApplyBeforeProviderRequestHookNode, MaybePrepareNextTurnNode, PrepareProviderRequestNode,
    StartTurnNode,
};
```

Add these tests:

```rust
#[tokio::test]
async fn start_turn_node_emits_turn_start_and_enforces_max_turns() {
    let mut config = AgentConfig::new(common::faux_model("agent-turn-start"));
    config.max_turns = Some(1);
    let agent = Agent::new(config);
    let mut context = AgentTurnContext::from_agent(&agent);

    let first = StartTurnNode.run(&mut context).await.unwrap();
    assert_eq!(first.as_str(), "default");
    assert_eq!(context.turn, 1);
    assert!(matches!(
        context.events.last(),
        Some(AgentEvent::TurnStart { turn: 1 })
    ));

    let second = StartTurnNode.run(&mut context).await.unwrap();
    assert_eq!(second.as_str(), "error");
    assert!(context.events.iter().any(|event| matches!(
        event,
        AgentEvent::AgentError { error } if error == "max turns (1) exceeded"
    )));
}

#[tokio::test]
async fn provider_request_node_emits_before_provider_request_after_hook_update() {
    let mut config = AgentConfig::new(common::faux_model("agent-turn-provider-hook"));
    config.hooks.before_provider_request = Some(Arc::new(|mut request| {
        Box::pin(async move {
            request.stream_options.temperature = Some(0.7);
            Ok(Some(BeforeProviderRequestResult {
                context: Some(request.context),
                stream_options: Some(request.stream_options),
            }))
        })
    }));
    let agent = Agent::new(config);
    agent.add_message(user_msg("user_0", "hello"));
    let mut context = AgentTurnContext::from_agent(&agent);

    PrepareProviderRequestNode.run(&mut context).await.unwrap();
    let action = ApplyBeforeProviderRequestHookNode
        .run(&mut context)
        .await
        .unwrap();

    assert_eq!(action.as_str(), "default");
    assert!(context.events.iter().any(|event| matches!(
        event,
        AgentEvent::BeforeProviderRequest { request }
            if request.stream_options.temperature == Some(0.7)
    )));
}

#[tokio::test]
async fn maybe_prepare_next_turn_drains_follow_up_before_done() {
    let config = AgentConfig::new(common::faux_model("agent-turn-follow-up"));
    let agent = Agent::new(config);
    let mut context = AgentTurnContext::from_agent(&agent);
    context.assistant_message = Some(AssistantMessage::empty("assistant_0", "test-model"));
    context.follow_up_queue.push_back(user_msg("follow_0", "follow up"));

    let action = MaybePrepareNextTurnNode.run(&mut context).await.unwrap();

    assert_eq!(action.as_str(), "continue");
    assert!(context.messages.iter().any(|message| matches!(
        message,
        AgentMessage::UserText { text, .. } if text == "follow up"
    )));
}
```

- [ ] **Step 8: Run focused tests**

Run:

```text
cargo test -p pi-agent-core --test agent_turn_flow start_turn_node_emits_turn_start_and_enforces_max_turns
cargo test -p pi-agent-core --test agent_turn_flow provider_request_node_emits_before_provider_request_after_hook_update
cargo test -p pi-agent-core --test agent_turn_flow maybe_prepare_next_turn_drains_follow_up_before_done
cargo test -p pi-agent-core --test agent_turn_flow
```

Expected:

```text
test result: ok
```

- [ ] **Step 9: Commit**

```text
git add crates/pi-agent-core/src/agent_turn_flow/context.rs crates/pi-agent-core/src/agent_turn_flow/nodes.rs crates/pi-agent-core/tests/agent_turn_flow.rs
git commit -m "feat(agent-core): complete agent turn flow nodes"
```

## Task 4: Switch `Agent::run()` to the Graph Runtime

**Files:**
- Modify: `crates/pi-agent-core/src/agent_turn_flow/runtime.rs`
- Modify: `crates/pi-agent-core/src/agent_turn_flow/context.rs`
- Modify: `crates/pi-agent-core/tests/agent_turn_flow.rs`
- Modify: `crates/pi-agent-core/tests/agent_loop.rs`
- Modify: `crates/pi-agent-core/tests/parallel_tools.rs`
- Modify: `crates/pi-agent-core/tests/hooks.rs`
- Modify: `crates/pi-agent-core/tests/compaction.rs`

- [ ] **Step 1: Add state export from context**

Add to `context.rs`:

```rust
impl AgentTurnContext {
    pub(crate) fn apply_to_state(&self, state: &mut AgentState) {
        state.config = self.config.clone();
        state.messages = self.messages.clone();
        state.tools = self.tools.clone();
        state.steering_queue = self.steering_queue.clone();
        state.follow_up_queue = self.follow_up_queue.clone();
    }
}
```

- [ ] **Step 2: Replace `run_state` bridge**

Replace:

```rust
pub(crate) fn run_state(state: Arc<RwLock<AgentState>>) -> AgentStream {
    run_loop(state)
}
```

with:

```rust
pub(crate) fn run_state(state: Arc<RwLock<AgentState>>) -> AgentStream {
    Box::pin(stream! {
        let flow = match AgentTurnFlow::new() {
            Ok(flow) => flow,
            Err(error) => {
                yield AgentEvent::AgentError { error: error.to_string() };
                return;
            }
        };

        loop {
            let mut context = {
                let state = state.read().unwrap();
                AgentTurnContext::from_state(&state)
            };

            let outcome = match flow.run_with_options(
                &mut context,
                FlowRunOptions {
                    strict_missing_transition: false,
                    ..Default::default()
                },
            ).await {
                Ok(outcome) => outcome,
                Err(error) => {
                    yield AgentEvent::AgentError { error: error.to_string() };
                    return;
                }
            };

            {
                let mut state = state.write().unwrap();
                context.apply_to_state(&mut state);
            }

            for event in context.events {
                yield event;
            }

            match outcome.last_action.as_str() {
                ACTION_CONTINUE => continue,
                ACTION_DONE => return,
                ACTION_ERROR | ACTION_ABORTED => return,
                _ => return,
            }
        }
    })
}
```

- [ ] **Step 3: Keep old `run_loop` available only as migration reference**

Move the old `run_loop` implementation below the graph runtime and mark it:

```rust
#[deprecated(note = "migration reference only; AgentTurnFlow::run_state drives the graph runtime")]
pub fn run_loop(state: Arc<RwLock<AgentState>>) -> AgentStream {
    graph_runtime_reference_loop(state)
}
```

If keeping the full reference function causes duplicated active behavior, delete it in the same task after behavior tests pass.

- [ ] **Step 4: Run core behavior regressions**

Run:

```text
cargo test -p pi-agent-core --test agent_turn_flow
cargo test -p pi-agent-core --test agent_loop
cargo test -p pi-agent-core --test parallel_tools
cargo test -p pi-agent-core --test hooks
cargo test -p pi-agent-core --test compaction
cargo test -p pi-agent-core --test agent_runtime_boundary
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit**

```text
git add crates/pi-agent-core/src/agent_turn_flow/runtime.rs crates/pi-agent-core/src/agent_turn_flow/context.rs crates/pi-agent-core/tests/agent_turn_flow.rs crates/pi-agent-core/tests/agent_loop.rs crates/pi-agent-core/tests/parallel_tools.rs crates/pi-agent-core/tests/hooks.rs crates/pi-agent-core/tests/compaction.rs crates/pi-agent-core/tests/agent_runtime_boundary.rs
git commit -m "feat(agent-core): run agent turns through flow graph"
```

## Task 5: Add Product Flow Construction and Outcome Helpers

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/flow_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/export_flow.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/manual_compaction_flow.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/branch_summary_flow.rs`
- Modify: `crates/pi-coding-agent/tests/event_boundary_guards.rs`

- [ ] **Step 1: Add a local linear-flow helper**

Add this helper inside `flow_service.rs` or a new private `flow_helpers.rs` module:

```rust
pub(crate) fn add_linear_edges<C>(
    flow: &mut pi_agent_core::flow::Flow<C>,
    node_ids: &[&str],
) -> Result<(), CodingSessionError> {
    for pair in node_ids.windows(2) {
        flow.edge(pair[0], pair[1]).map_err(|error| {
            CodingSessionError::Flow {
                message: format!("flow graph configuration failed: {error}"),
            }
        })?;
    }
    Ok(())
}
```

- [ ] **Step 2: Replace repeated `windows(2)` wiring in three flows**

In `export_flow.rs`, `manual_compaction_flow.rs`, and `branch_summary_flow.rs`, replace:

```rust
for pair in EXPORT_NODE_IDS.windows(2) {
    flow.edge(pair[0], pair[1]).map_err(flow_error)?;
}
```

with:

```rust
super::flow_service::add_linear_edges(&mut flow, EXPORT_NODE_IDS)?;
```

Use each flow's own node ID constant.

- [ ] **Step 3: Keep graph shape visible**

Add or keep tests that assert:

```rust
assert_eq!(ExportFlow::node_ids(), EXPORT_NODE_IDS);
assert_eq!(ManualCompactionFlow::node_ids(), MANUAL_COMPACTION_NODE_IDS);
assert_eq!(BranchSummaryFlow::node_ids(), BRANCH_SUMMARY_NODE_IDS);
```

- [ ] **Step 4: Run product flow tests**

Run:

```text
cargo test -p pi-coding-agent export_flow
cargo test -p pi-coding-agent manual_compaction
cargo test -p pi-coding-agent branch_summary
cargo test -p pi-coding-agent event_boundary_guards
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit**

```text
git add crates/pi-coding-agent/src/coding_session/flow_service.rs crates/pi-coding-agent/src/coding_session/export_flow.rs crates/pi-coding-agent/src/coding_session/manual_compaction_flow.rs crates/pi-coding-agent/src/coding_session/branch_summary_flow.rs crates/pi-coding-agent/tests/event_boundary_guards.rs
git commit -m "refactor(coding-agent): share linear product flow wiring"
```

## Task 6: Introduce an Explicit Subflow Convention

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/flow_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/agent_invocation_flow.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/agent_team_flow.rs`
- Modify: `crates/pi-coding-agent/tests/agent_invocation.rs`
- Modify: `crates/pi-coding-agent/tests/agent_team_flow.rs`

- [ ] **Step 1: Add subflow runner methods with explicit names**

Add methods to `FlowService`:

```rust
pub(crate) async fn run_prompt_subflow_for_agent_invocation(
    &self,
    ctx: &mut PromptTurnContext,
) -> Result<PromptTurnOutcome, CodingSessionError> {
    self.run_prompt_turn(ctx).await
}

pub(crate) async fn run_agent_invocation_subflow(
    &self,
    ctx: &mut AgentInvocationContext,
) -> Result<AgentInvocationOutcome, CodingSessionError> {
    self.run_agent_invocation(ctx).await
}

pub(crate) async fn run_agent_team_subflow(
    &self,
    ctx: &mut AgentTeamContext,
) -> Result<AgentTeamOutcome, CodingSessionError> {
    self.run_agent_team(ctx).await
}
```

- [ ] **Step 2: Replace direct nested `PromptTurnFlow::new()?.run(...)` calls**

In `agent_invocation_flow.rs` and `agent_team_flow.rs`, route nested prompt execution through the explicit subflow methods. The call site should read as subflow execution, not ad hoc graph construction.

Use this shape:

```rust
let prompt_outcome = context
    .flow_service()
    .run_prompt_subflow_for_agent_invocation(child_context)
    .await?;
```

If `AgentInvocationContext` does not currently hold a `FlowService` reference, pass a narrow `SubflowRunner` trait object into the context instead of passing `CodingAgentSession`.

- [ ] **Step 3: Test nested flow behavior stays isolated**

Add assertions to existing tests:

```rust
assert!(events.iter().any(|event| matches!(
    event,
    CodingAgentEvent::AgentInvocationStarted { .. }
)));
assert!(events.iter().any(|event| matches!(
    event,
    CodingAgentEvent::AgentInvocationCompleted { .. }
)));
assert!(!events_as_json.contains("flowNode"));
```

- [ ] **Step 4: Run nested flow tests**

Run:

```text
cargo test -p pi-coding-agent --test agent_invocation
cargo test -p pi-coding-agent --test agent_team_flow
cargo test -p pi-coding-agent --test protocol_events product_event_protocol_adapter_does_not_emit_flow_node_fields
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit**

```text
git add crates/pi-coding-agent/src/coding_session/flow_service.rs crates/pi-coding-agent/src/coding_session/agent_invocation_flow.rs crates/pi-coding-agent/src/coding_session/agent_team_flow.rs crates/pi-coding-agent/tests/agent_invocation.rs crates/pi-coding-agent/tests/agent_team_flow.rs
git commit -m "refactor(coding-agent): make nested workflow subflows explicit"
```

## Task 7: Add Deterministic Batch and Parallel Helper Scope

**Files:**
- Modify: `crates/pi-agent-core/src/agent_turn_flow/nodes.rs`
- Modify: `crates/pi-agent-core/tests/agent_turn_flow.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/agent_team_flow.rs`
- Modify: `crates/pi-coding-agent/tests/agent_team_flow.rs`

- [ ] **Step 1: Extract ordered parallel tool aggregation**

In `nodes.rs`, extract the parallel tool execution ordering into:

```rust
async fn collect_parallel_tool_executions(
    prepared: Vec<(PendingToolCall, Option<AgentTool>, Option<AgentToolResult>)>,
    after_hook: Option<AfterToolCallHook>,
    assistant_message: Option<AssistantMessage>,
    messages: Vec<AgentMessage>,
) -> Vec<ToolCallExecution> {
    let mut futures: FuturesUnordered<_> = prepared
        .into_iter()
        .map(|(call, tool, blocked)| {
            let after_hook = after_hook.clone();
            let assistant_message = assistant_message.clone();
            let messages = messages.clone();
            async move {
                let result = match blocked {
                    Some(result) => result,
                    None => {
                        let result = execute_tool(tool, call.name.clone(), call.arguments.clone()).await;
                        apply_after_tool_hook(after_hook, assistant_message, messages, &call, result).await
                    }
                };
                ToolCallExecution {
                    index: call.index,
                    tool_call_id: call.id,
                    tool_name: call.name,
                    result,
                }
            }
        })
        .collect();

    let mut executions = Vec::new();
    while let Some(execution) = futures.next().await {
        executions.push(execution);
    }
    executions.sort_by_key(|execution| execution.index);
    executions
}
```

- [ ] **Step 2: Assert deterministic result ordering**

Extend the delayed parallel tool test with:

```rust
let tool_messages: Vec<_> = context
    .messages
    .iter()
    .filter_map(|message| match message {
        AgentMessage::ToolResult { tool_call_id, .. } => Some(tool_call_id.as_str()),
        _ => None,
    })
    .collect();
assert_eq!(tool_messages, vec!["call_slow", "call_fast"]);
```

- [ ] **Step 3: Keep product team parallelism out of low-level Flow runtime**

If `AgentTeamFlow` gains parallel member execution, implement it as a product-level helper that returns member outcomes sorted by member index or profile ID. Do not add a generic parallel executor to `Flow<C>` in this task.

- [ ] **Step 4: Run focused parallel checks**

Run:

```text
cargo test -p pi-agent-core --test agent_turn_flow execute_parallel
cargo test -p pi-agent-core --test parallel_tools
cargo test -p pi-coding-agent --test agent_team_flow
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit**

```text
git add crates/pi-agent-core/src/agent_turn_flow/nodes.rs crates/pi-agent-core/tests/agent_turn_flow.rs crates/pi-coding-agent/src/coding_session/agent_team_flow.rs crates/pi-coding-agent/tests/agent_team_flow.rs
git commit -m "refactor(flow): preserve deterministic parallel aggregation"
```

## Task 8: Final Verification and Documentation Closeout

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/specs/2026-07-06-flow-core-abstraction-hardening-design.md`
- Modify: `docs/superpowers/plans/2026-07-06-flow-core-abstraction-hardening-plan.md`

- [ ] **Step 1: Run workspace checks**

Run:

```text
cargo fmt --check
cargo test -p pi-agent-core
cargo test -p pi-coding-agent
cargo check --workspace
cargo test --workspace
git diff --check
```

Expected:

```text
test result: ok
Finished dev profile
```

- [ ] **Step 2: Update TODO status**

Mark the post-Phase 6 Flow core abstraction hardening item complete only when:

- `AgentTurnFlow::run_state` drives `Flow<AgentTurnContext>`;
- core behavior checks pass;
- product flow helpers are in place for at least three repeated linear flows;
- subflow convention is explicit for invocation/team nesting;
- adapter event guards still pass.

- [ ] **Step 3: Commit closeout**

```text
git add docs/TODO.md docs/superpowers/specs/2026-07-06-flow-core-abstraction-hardening-design.md docs/superpowers/plans/2026-07-06-flow-core-abstraction-hardening-plan.md
git commit -m "docs: close flow core abstraction hardening plan"
```

## Self-Review

Spec coverage:

- Expanded crate audit is covered by the design document.
- PocketFlow comparison is covered by the design document.
- Feasible implementation path is covered by Tasks 1-8.
- Value is covered by the design document.
- Verification commands are listed in Task 8.

Placeholder scan:

- The plan avoids placeholder markers and avoids unspecified error handling.
- Every code-changing task includes concrete paths, snippets, commands, and expected outcomes.

Type consistency:

- `AgentTurnFlow::node_ids()` is introduced before tests depend on it.
- `AgentTurnFlow::new()` is introduced before `run_state` constructs it.
- `AgentTurnContext::apply_to_state()` is introduced before the runtime bridge calls it.
- Product subflow helpers use existing outcome/context names.
