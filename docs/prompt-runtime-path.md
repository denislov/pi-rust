# Prompt Runtime Path

## Scope

This document follows an ordinary text prompt through the current `0.3.0`
implementation. It is a code-reading guide, not a second architecture contract.
Ownership and boundary rules remain normative in
[`architecture.md`](architecture.md).

A product prompt is not equivalent to one provider request. One admitted
prompt operation may perform runtime compaction, multiple provider requests,
tool calls, steering or follow-up turns, durable session writes, product-event
publication, and adapter projection.

## End-to-End Path

```text
pi-coding-agent binary
  -> app/cli request resolution
  -> print | JSON | RPC | interactive adapter
  -> CodingAgentOperation::Prompt
  -> runtime scheduler + intent admission
  -> operation capability snapshot
  -> PromptTurnContext + PromptTurnFlow
  -> pi-agent-core Agent / AgentTurnFlow
  -> pi-ai scoped provider stream
  -> AgentEvent
  -> typed ProductEvent + session transaction
  -> adapter projection / UI snapshot
  -> CodingAgentOperationOutcome
```

The product operation ID is the correlation root for admission, cancellation,
events, persistence, reconnect, and the public outcome. Nested agent turns and
tool calls have their own identifiers without replacing that root identity.

## 1. Process and Adapter Entry

The binary entry is `crates/pi-coding-agent/src/main.rs`. CLI parsing and
request construction live under:

```text
crates/pi-coding-agent/src/app/cli/
crates/pi-coding-agent/src/app/bootstrap.rs
crates/pi-coding-agent/src/app/session.rs
```

The selected mode then enters one of the adapter owners:

| Mode | Owner |
| --- | --- |
| Print | `src/adapters/print.rs` |
| JSON | `src/adapters/json/mod.rs` |
| RPC stdio | `src/adapters/rpc/` |
| Interactive TUI | `src/adapters/interactive/` |

Adapters resolve input and presentation concerns, but they do not own prompt
execution. Each constructs or submits a `CodingAgentOperation::Prompt` through
the `CodingAgentSession` facade.

Machine-readable adapters must keep stdout protocol-clean. Diagnostics belong
on stderr or in typed protocol/product events.

## 2. Admission and Runtime Ownership

The stable product facade is implemented under
`crates/pi-coding-agent/src/runtime/facade.rs`. Prompt work passes through the
runtime owners instead of calling a prompt Flow directly:

```text
runtime/operation.rs    public operation request vocabulary
runtime/scheduler.rs    operation class and active-operation scheduling
runtime/intent.rs       centralized intent admission
runtime/admission.rs    operation-local admission and capability freezing
runtime/dispatch.rs     canonical operation dispatch
runtime/control.rs      abort, steer, follow-up, and child lineage
runtime/submission.rs   runtime-owned asynchronous task handles
```

Admission freezes an `OperationCapabilitySnapshot`. Provider/model access,
tools, filesystem, shell, plugins, session reads/writes, delegation, and
generation/revocation are decided from that snapshot. The Prompt Flow receives
the scoped handles it needs; it does not receive an unrestricted service
container.

## 3. Product Prompt Flow

The product Flow lives in:

```text
crates/pi-coding-agent/src/operations/prompt/context.rs
crates/pi-coding-agent/src/operations/prompt/flow.rs
```

`PromptTurnFlow` currently executes these nodes in order:

| Node | Responsibility |
| --- | --- |
| `start_prompt_turn` | establish the Flow start point; prompt-start publication is owned outside the node |
| `resolve_request` | normalize and validate the requested invocation |
| `prepare_input` | run prompt preparation hooks and build operation input |
| `resolve_runtime` | resolve the immutable runtime snapshot |
| `load_resources` | load system prompt, skills, templates, and other scoped resources |
| `open_session` | require a prepared persistent transaction or non-persistent replay context |
| `build_agent_runtime` | build and hydrate `pi-agent-core::Agent` through `RuntimeService` and the capability snapshot |
| `record_user_input` | stage the user input in the owning session/runtime context |
| `run_agent_turn` | drive the core agent stream alongside abort/steer/follow-up control |
| `finalize_turn` | validate the final assistant message and pre-commit conditions |
| `emit_completion` | record prompt completion and run the final prompt hook |

Persistent session opening, replay loading, transaction ownership, commit, and
abort/failure finalization are coordinated by the product operation/session
owners around this Flow. The Flow cannot invent a second durable write path.

## 4. Core Agent Loop

`RuntimeService` constructs a provider-neutral `pi-agent-core::Agent`. The core
runtime is owned by:

```text
crates/pi-agent-core/src/agent/runtime.rs
crates/pi-agent-core/src/agent/turn/context.rs
crates/pi-agent-core/src/agent/turn/runtime.rs
crates/pi-agent-core/src/agent/turn/nodes.rs
```

`Agent::prompt` adds the text input and starts `AgentTurnFlow`. The current core
node inventory is:

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

After `provider_stream`, the assistant result either terminates, proceeds to
tool execution, or continues to another provider turn. Tool results, steering,
and follow-up input are folded into core agent state before the next turn.
Cancellation is propagated through the Flow run options and the agent token.

`pi-agent-core` owns this generic loop but does not know about product sessions,
RPC, ProductEvents, UI snapshots, profiles, or coding-agent policy.

## 5. Provider Boundary

The product runtime supplies core with a scoped provider streaming behavior.
Core depends only on the approved `pi-ai` facade categories:

```text
pi_ai::api::model
pi_ai::api::conversation
pi_ai::api::stream
```

Core can construct provider-neutral context, messages, tools, stream options,
and consume `AssistantMessageEvent`. It cannot access provider registries,
credentials, concrete HTTP clients, provider-specific wire modules, or global
mutable provider state.

`pi-ai` owns provider request mapping, authentication inputs, HTTP/SSE
transport, retry, response mapping, and streaming. Provider output returns to
core as the common stream vocabulary and becomes `AgentEvent`.

## 6. Event and Durable Paths

The product prompt context converts core events into product semantics through
the event service and owner-local event families under:

```text
crates/pi-coding-agent/src/events/
crates/pi-coding-agent/src/services/event.rs
```

The resulting `ProductEvent` stream carries stream/sequence identity,
operation and turn association, typed event family data, and terminal outcome
semantics. Adapters consume this stream; raw `AgentEvent` and `FlowEvent` do not
cross the product boundary.

Persistent session facts are staged and committed through:

```text
crates/pi-coding-agent/src/session/repository.rs
crates/pi-coding-agent/src/session/transaction.rs
crates/pi-coding-agent/src/session/replay.rs
crates/pi-coding-agent/src/services/session.rs
```

`SessionEvent` is the durable source of truth. ProductEvents are live semantic
delivery, and UI snapshots are reconnectable projections. These three models
must not be collapsed into one enum or written by adapters.

## 7. Adapter Projection

Every adapter receives the same operation semantics but projects them
differently:

| Adapter | Projection |
| --- | --- |
| Print | final user-facing output and diagnostics |
| JSON | JSONL protocol events without terminal/UI state |
| RPC | versioned commands, responses, ProductEvent replay, snapshot cursors, and control |
| Interactive | ProductEvents plus `UiSnapshot` into generic `pi-tui` components |

RPC event mapping lives under `src/adapters/rpc/events.rs` and the shared
protocol adapter under `src/adapters/events.rs`. Interactive background prompt
tasks live under `src/adapters/interactive/prompt_task.rs`; they receive
ProductEvents and snapshots rather than borrowing operation services.

## 8. Terminal and Failure Semantics

For every started prompt operation, callers must observe exactly one terminal
result: completed, aborted, failed, or skipped as defined by the operation
contract. The final durable marker, terminal ProductEvent, and public operation
outcome must agree.

Important failure boundaries are:

- admission failure: no operation starts and no false durable start is written;
- provider/tool failure: operation identity is retained and failure is emitted
  through typed product semantics;
- abort: cancellation reaches the core stream and the session transaction is
  finalized according to the durable contract;
- adapter disconnect or lag: canonical runtime work and replay state remain
  owned by the runtime, not by the disconnected presentation task;
- unsupported protocol/snapshot major: fail closed and require a compatible
  reconnect or fresh snapshot.

## Verification Map

| Contract | Current test owner |
| --- | --- |
| Facade and cross-crate imports | `tests/api_contract/` in the active crates |
| Scheduler, admission, control, and product ownership | `pi-coding-agent/tests/boundaries/` and `tests/operation/` |
| Core provider/tool loop | `pi-agent-core/tests/agent/` and `tests/tool_hooks/` |
| ProductEvent and snapshot contracts | `pi-coding-agent/tests/events_snapshot/` |
| Session durability and recovery | `pi-coding-agent/tests/recovery/` and `tests/session/` |
| Print and JSON adapters | `pi-coding-agent/tests/print_json/` |
| RPC protocol and reconnect | `pi-coding-agent/tests/rpc/` and `tests/recovery/protocol_sessions.rs` |
| Coding tools | `pi-coding-agent/tests/tools/` |
| Generic terminal behavior | `pi-tui/tests/` |
| End-to-end terminal lifecycle | `scripts/tui-smoke.sh` |

When this path changes, update the owning implementation and focused contract
first, then update this guide. Do not add adapter-specific execution paths or
cross-layer compatibility exports merely to keep this document unchanged.
