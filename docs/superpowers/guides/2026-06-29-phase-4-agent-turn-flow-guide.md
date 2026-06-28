# Phase 4 Guide: AgentTurnFlow in pi-agent-core

## Phase Goal

Move the low-level agent loop internals into a Flow while preserving the public `Agent::run()` and `Agent::prompt()` behavior.

Phase 4 should start only after:

- `CodingAgentSession` is the product owner;
- `PromptTurnFlow` is the product prompt path;
- `RunAgentTurn` is isolated as the bridge to current `Agent::run()`.

## Non-Negotiable Constraints

- Preserve existing `Agent::run()` and `Agent::prompt()` public APIs.
- Preserve `AgentEvent` as low-level output.
- Preserve tool execution behavior.
- Preserve steering/follow-up queues.
- Preserve cancellation semantics.
- Preserve provider hook behavior.
- Do not move coding-agent product concepts into `pi-agent-core`.

## Target Module Layout

Add:

```text
crates/pi-agent-core/src/agent_turn_flow/
  mod.rs
  context.rs
  nodes.rs
  events.rs
  outcome.rs
```

Update:

```text
crates/pi-agent-core/src/lib.rs
crates/pi-agent-core/src/agent.rs
crates/pi-agent-core/src/agent_loop.rs
crates/pi-agent-core/src/loop_runtime/context.rs
crates/pi-agent-core/src/loop_runtime/tools.rs
```

The old `agent_loop.rs` can remain during extraction. Remove or shrink it only after behavior parity is covered.

## AgentTurnContext

`AgentTurnContext` belongs in `pi-agent-core`.

It should contain:

```text
agent shared state handle
AgentConfig snapshot
current messages
tools
resources
queues
abort/cancellation state
turn counter
provider request context
assistant accumulator
pending tool calls
tool results
runtime compaction state
low-level AgentEvent sink
```

It must not contain:

- `CodingAgentSession`;
- `SessionService`;
- Rust-native session event log;
- CLI/RPC/TUI concepts;
- plugin Lua host.

## AgentTurnFlow Graph

Conceptual nodes:

```text
drain_queued_input
prepare_context
maybe_compact_runtime_context
before_provider_request
stream_provider
accumulate_assistant_message
decide_stop_or_tools
execute_tools
append_tool_results
prepare_next_turn
finish_agent
```

Actions:

```text
default
continue
tools
done
error
aborted
max_turns
```

Graph shape:

```text
drain_queued_input
  -> prepare_context
  -> maybe_compact_runtime_context
  -> before_provider_request
  -> stream_provider
  -> accumulate_assistant_message
  -> decide_stop_or_tools
      done -> finish_agent
      tools -> execute_tools -> append_tool_results -> prepare_next_turn -> drain_queued_input
      error -> finish_agent
      aborted -> finish_agent
```

The exact graph can vary, but these phase boundaries should remain recognizable.

## Event Emission Contract

`AgentTurnFlow` emits `AgentEvent`, not `CodingAgentEvent`.

Required preservation:

- `TurnStart`;
- `BeforeProviderRequest`;
- `LlmEvent`;
- `ToolCallStart`;
- `ToolCallUpdate`;
- `ToolCallEnd`;
- `SessionCompacted`;
- `AgentDone`;
- `AgentError`.

If Flow emits debug `FlowEvent`, keep it internal or behind advanced diagnostics. Product adapters should still receive `CodingAgentEvent` through Phase 2/3 mapping.

## Extraction Strategy

Use staged extraction instead of a full rewrite.

### Step 1. Add AgentTurnContext Without Behavior Change

Create context from existing `Agent` state and config. Add tests that context construction preserves current message/tool/resource snapshots.

### Step 2. Extract Prepare Context Node

Move context conversion and request preparation logic out of `agent_loop.rs` into a node/helper.

Existing anchors:

- `loop_runtime/context.rs`;
- `convert.rs`;
- `agent_loop.rs` provider request setup.

Tests:

- before/after provider request context is equivalent.

### Step 3. Extract Runtime Compaction Node

Existing anchors:

- `compaction/estimate.rs`;
- `compaction/prepare.rs`;
- `compaction/summarize.rs`;
- `agent_loop.rs` compaction branch.

Node output:

- updated provider context;
- `AgentEvent::SessionCompacted` for runtime compaction if existing behavior emits it.

Important:

- This is runtime compaction, not Rust-native session compaction.
- It should not write session events.

### Step 4. Extract Provider Stream Node

Existing anchors:

- provider stream call in `agent_loop.rs`;
- `pi-ai` stream events.

Node responsibilities:

- apply hooks/options already supported by current loop;
- call provider stream;
- emit `LlmEvent`;
- accumulate assistant message pieces enough for downstream decision.

Tests:

- faux provider text stream;
- faux provider tool-call stream;
- provider error maps to `AgentEvent::AgentError`.

### Step 5. Extract Decide Node

Input:

- accumulated assistant message;
- stop reason;
- tool calls;
- max turn state.

Output actions:

- `done`;
- `tools`;
- `continue`;
- `error`;
- `max_turns`.

Tests:

- no tool calls -> done;
- tool calls -> tools;
- max turns -> error/done according to existing behavior.

### Step 6. Extract Tool Execution Node

Existing anchors:

- `loop_runtime/tools.rs`;
- sequential/parallel tool execution sections in `agent_loop.rs`.

Preserve:

- `ToolExecutionMode`;
- parallel default;
- before/after tool hooks;
- tool updates;
- terminate behavior;
- tool error result semantics.

Tests:

- existing parallel tool tests;
- sequential mode ordering;
- tool update events;
- terminate result.

### Step 7. Replace Agent::run() Internals

Once nodes match behavior, make `Agent::run()` delegate to `AgentTurnFlow`.

Do not change:

- method signature;
- stream item type;
- abort handle behavior;
- queue APIs.

## Compatibility With PromptTurnFlow

`pi-coding-agent` should not need broad changes when Phase 4 lands.

Expected replacement:

```text
RunAgentTurn node:
  before Phase 4 -> existing Agent::run()
  after Phase 4  -> Agent::run() wrapper over AgentTurnFlow
```

If product code needs direct `AgentTurnFlow`, that is a smell. Keep `Agent::run()` as the low-level public boundary unless there is a clear reason.

## Tests

Must keep existing `pi-agent-core` tests meaningful:

```text
agent_loop.rs tests
parallel_tools.rs
queues_thinking.rs
hooks.rs
m9_harness.rs
compaction.rs
agent_hydration.rs
```

Add:

```text
agent_turn_flow.rs
agent_turn_flow_tools.rs
agent_turn_flow_compaction.rs
agent_turn_flow_abort.rs
```

Test categories:

- text-only provider run;
- tool-call provider run;
- parallel tools;
- sequential tools;
- provider error;
- tool error;
- abort before provider;
- abort during tool;
- runtime compaction;
- steering/follow-up queues.

## Phase 4 Handoff to Phase 5

Phase 4 must leave:

- low-level agent behavior represented by `AgentTurnFlow`;
- `Agent::run()` still stable;
- `AgentEvent` still stable enough for `EventService`;
- hook points clearer for plugin integration;
- no coding-agent product state in `pi-agent-core`.

Phase 5 can then add plugin hooks around stable product and agent boundaries.

## Stop Conditions

Stop and reassess if:

- `AgentTurnContext` includes `CodingAgentSession`;
- `Agent::run()` signature must change;
- product events leak into `pi-agent-core`;
- tool execution behavior changes to make tests easier;
- runtime compaction starts writing session event logs directly.

## Suggested Checks

Focused:

```text
cargo fmt --check
cargo test -p pi-agent-core agent_turn_flow
cargo test -p pi-agent-core agent_loop
cargo test -p pi-agent-core parallel_tools
cargo test -p pi-agent-core hooks
cargo test -p pi-agent-core compaction
```

Full:

```text
cargo test -p pi-agent-core
cargo test -p pi-coding-agent
cargo check --workspace
```
