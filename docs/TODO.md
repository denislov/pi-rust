# pi-rust TODO

This file is the working checklist for the Flow-centered `pi-rust` architecture.

`pi-rust` is no longer treated as a direct TypeScript `pi` port. The TypeScript `pi` repo remains a behavioral and product reference. PocketFlow remains an architectural reference for explicit graph-shaped orchestration. The Rust project should use both references to build a clearer, more principled agent runtime.

## Update Rule

Update this TODO whenever a task completes meaningful progress against an item below.

Required updates:

- Mark completed items with `[x]`.
- Mark active partial work with `[~]` and add a short note.
- Add new discovered work under the relevant phase instead of leaving it implicit.
- Add a dated progress note when a phase boundary, guide step, or major design decision changes.
- Include TODO updates in the same commit as the implementation or documentation change that changes status.

Do not let this file become historical fiction. If implementation changes the plan, update this file and the relevant guide/spec together.

## Source Documents

- [Flow-centered architecture design](superpowers/specs/2026-06-29-flow-centered-runtime-architecture-design.md)
- [Phase 2 print session target convergence design](superpowers/specs/2026-06-29-phase-2-print-session-target-convergence-design.md)
- [Phase 2 ResolveRequest node design](superpowers/specs/2026-06-30-phase-2-resolve-request-node-design.md)
- [Session finalization convergence design](superpowers/specs/2026-06-30-session-finalization-convergence-design.md)
- [Non-persistent product runtime design](superpowers/specs/2026-06-30-non-persistent-product-runtime-design.md)
- [Interactive CodingEventBridge design](superpowers/specs/2026-06-30-interactive-coding-event-bridge-design.md)
- [Rust-native active leaf commit design](superpowers/specs/2026-06-30-rust-native-active-leaf-commit-design.md)
- [Rust-native session fork and clone design](superpowers/specs/2026-06-30-rust-native-session-fork-clone-design.md)
- [Rust-native manual compaction design](superpowers/specs/2026-06-30-rust-native-manual-compaction-design.md)
- [Rust-native tree navigation design](superpowers/specs/2026-06-30-rust-native-tree-navigation-design.md)
- [Flow-centered implementation plan](superpowers/plans/2026-06-29-flow-centered-runtime-architecture-plan.md)
- [Phase 1 guide](superpowers/guides/2026-06-29-phase-1-coding-session-and-session-log-guide.md)
- [Phase 2 guide](superpowers/guides/2026-06-29-phase-2-prompt-turn-flow-guide.md)
- [Phase 3 guide](superpowers/guides/2026-06-29-phase-3-adapter-convergence-guide.md)
- [Phase 4 guide](superpowers/guides/2026-06-29-phase-4-agent-turn-flow-guide.md)
- [Phase 5 guide](superpowers/guides/2026-06-29-phase-5-plugin-kernel-guide.md)
- [Phase 6 guide](superpowers/guides/2026-06-29-phase-6-advanced-flow-workflows-guide.md)
- [Cross-guide interface review](superpowers/guides/2026-06-29-flow-guides-interface-review.md)

## Current North Star

- [x] Establish minimal `pi_agent_core::flow`.
- [x] Design the Flow-centered runtime architecture.
- [x] Write the implementation plan.
- [x] Write detailed phase implementation guides.
- [x] Implement Phase 1: `CodingAgentSession` skeleton and Rust-native session log. Product runtime shell/API boundary, typed session log schema, filesystem store, turn transactions, owner create/open persistence, and replay/fold transcript support are in place.
- [~] Implement Phase 2: `PromptTurnFlow` on headless/json path. Prompt turn options/outcome/context, runtime snapshot boundary, real graph with stable node IDs, AgentEvent-to-product-event mapping, real ResolveRequest/PrepareInput/ResolveRuntime/LoadResources/OpenSession/BuildAgentRuntime/RecordUserInput/RunAgentTurn/FinalizeTurn/EmitCompletion nodes, pending agent-output event recording through TurnTransaction, live product event broadcast during PromptTurnFlow execution, assistant thinking deltas, completed-message-only assistant session persistence, SessionService-owned prompt transaction finalization with `SessionWrite*` product events, persistent and non-persistent `CodingAgentSession::prompt()` for runtime-backed options, successful persistent prompt active-leaf commits, completed user/assistant/tool-call replay hydration, Rust-native session open-or-create/list groundwork, Rust-native clone/fork/manual-compaction service APIs, enabled and no-session/disabled print routing including ForkTarget, and JSON protocol rendering/execution through `CodingAgentEvent`/`CodingAgentSession` are in place.
- [~] Implement Phase 3: converge CLI/RPC/interactive adapters. Concrete `CodingAgentCapabilities`/`CapabilityStatus` model, RPC `get_state` capability reporting, RPC `CodingAgentEvent` adapter, enabled plus disabled-session RPC prompt routing, primary interactive prompt routing, JSON execution routing, Rust-native resume hydration, Rust-native active-session `/session` info and leaf-backed `/tree` navigation, Rust-native active-session `/clone`, `/fork`, and `/compact`, Rust-native active leaf propagation to RPC/interactive state, adapter-provided cwd recording/filtering, and live assistant text/thinking propagation to RPC and interactive adapters are in place.
- [x] Implement Phase 4: introduce `AgentTurnFlow` in `pi-agent-core`. `agent_turn_flow` now owns the low-level runtime entrypoint, `AgentTurnContext` snapshot boundary, `PrepareContextNode`, runtime compaction node, provider stream node, decide-stop-or-tools node, and tool execution node with sequential/parallel execution, hooks, update callbacks, and terminate behavior. `Agent::run()` now delegates to `AgentTurnFlow`, and the old `agent_loop` module remains only as a compatibility wrapper.
- [ ] Implement Phase 5: plugin kernel on session/flow boundaries.
- [ ] Implement Phase 6: advanced Flow workflows.

## Phase 1: CodingAgentSession and Rust-Native Session Log

Guide: [Phase 1](superpowers/guides/2026-06-29-phase-1-coding-session-and-session-log-guide.md)

- [x] Add `crates/pi-coding-agent/src/coding_session/` module shell.
- [x] Export `CodingAgentSession` and `CodingAgentEvent` through `pi_coding_agent::api`.
- [x] Add `CodingSessionError` typed product error boundary.
- [x] Add base `CodingAgentEvent`.
- [x] Add `EventService`.
- [x] Add Rust-native session manifest model.
- [x] Add typed `SessionEventEnvelope` and `SessionEventData`.
- [x] Add deterministic ID and clock test boundaries.
- [x] Add `SessionLogStore`.
- [x] Add `TurnTransaction`.
- [x] Add replay/fold transcript view.
- [x] Add `CodingAgentSession` owner skeleton.
- [x] Add Phase 1 tests. Public API, coding session shell/error, owner create/open persistence, manifest round-trip, event JSON shape, stable event kind names, deterministic ID, fixed clock, tempdir-backed store, canonical event-log replay, and transaction commit/abort/fail/finalization coverage added.
- [x] Run Phase 1 focused checks. `cargo fmt --check`, `cargo test -p pi-coding-agent coding_session`, `cargo test -p pi-coding-agent public_api`, `cargo check --workspace`, and `cargo test --workspace` pass for the completed Phase 1 slice.

## Phase 2: PromptTurnFlow on Headless/JSON

Guide: [Phase 2](superpowers/guides/2026-06-29-phase-2-prompt-turn-flow-guide.md)

- [x] Add `PromptTurnOptions`.
- [x] Add `PromptTurnOutcome`.
- [x] Add `PromptTurnContext`.
- [x] Add `RuntimeSnapshot`.
- [x] Add `PromptTurnFlow` graph.
- [x] Add prompt flow nodes. Stable Phase 2 graph boundaries now have real node behavior: ResolveRequest validates runtime-backed prompt options and marks request resolution, PrepareInput normalizes/validates prompt invocation into prepared persisted input, ResolveRuntime owns runtime snapshot attachment from PromptTurnOptions into PromptTurnContext, LoadResources attaches the runtime resource snapshot into turn state before agent construction, OpenSession validates that the owner prepared session id/replay/transaction state before agent construction, BuildAgentRuntime builds the Agent from RuntimeSnapshot and hydrated replay only after resources are loaded, RecordUserInput records prepared prompt input, RunAgentTurn drives an existing Agent stream, FinalizeTurn validates final-turn readiness without flushing session events, and EmitCompletion appends the success completion product event from the graph.
- [x] Map `AgentEvent` to `CodingAgentEvent`.
- [x] Add `RunAgentTurn` node using existing `Agent::run()`.
- [x] Record agent output into session events through `TurnTransaction`.
- [x] Converge prompt transaction finalization under `SessionService` and emit `SessionWrite*` product events. `SessionService` now owns prompt commit/fail/abort/skip finalization, `FinalizeTurn` validates readiness without flushing session events, and `CodingAgentSession` emits session write events before final prompt outcome events.
- [x] Add `CodingAgentSession::prompt()`.
- [x] Implement non-persistent product runtime for no-session/disabled prompt convergence. `CodingAgentSession::non_persistent()` now owns prompt execution without durable session storage, `PromptTurnFlow` accepts non-persistent replay boundaries, non-persistent prompts emit `SessionWriteSkipped`, and owner-lifetime transcript hydration works for follow-up prompts on the same owner.
- [x] Route print mode through `CodingAgentSession`. Enabled default/New/OpenTarget/OpenOrCreateId/ContinueMostRecent print session targets use persistent `CodingAgentSession` and Rust-native session logs; enabled ForkTarget forks the Rust-native source session through `SessionService` before running the prompt; no-session/disabled print uses non-persistent `CodingAgentSession`.
- [x] Route JSON mode through `CodingAgentEvent`.
- [x] Remove the old `session_runner` transitional wrapper. Print, JSON, RPC, interactive prompt execution, manual compaction, clone/fork/tree, and no-session runs route through `CodingAgentSession`/`CodingAgentEvent`; old JSONL session targets are rejected instead of executed.
- [~] Add Phase 2 tests. Prompt turn option/outcome/context, runtime snapshot, graph path/node ID, misconfigured flow, FlowService, RuntimeService agent building and replay hydration, real ResolveRequest validation/idempotency/order coverage, real PrepareInput normalization/error coverage, real ResolveRuntime attachment/error coverage, real LoadResources attachment/error coverage, real OpenSession persistent/non-persistent boundary validation coverage, real BuildAgentRuntime resource-precondition and replay hydration coverage, real FinalizeTurn validation-without-flush coverage, real EmitCompletion completion/idempotency/error coverage, AgentEvent mapping including assistant thinking deltas, RecordUserInput prepared-event recording, RunAgentTurn/faux-provider flow execution, completed-message-only assistant session-event persistence, thinking replay hydration coverage, SessionService commit/fail/skip product-event finalization coverage, owner-level session write ordering, active-leaf commit consistency, and failed-operation finalization coverage, persistent and non-persistent `CodingAgentSession::prompt()` success/config-error/failure-event-deduplication with user+assistant replay and reopened/owner-lifetime provider-context hydration, Rust-native session open-or-create/list coverage, public API smoke coverage, default/New/OpenTarget id/path/OpenOrCreateId/ContinueMostRecent print-mode Rust-native session-log coverage, disabled/no-session print no-file coverage, ForkTarget Rust-native fork routing coverage, direct `CodingAgentEvent` protocol adapter coverage, and JSON mode success/tool/failure/enabled-session Rust-native persistence coverage added.
- [~] Run Phase 2 focused checks. `cargo fmt --check`, `cargo test -p pi-coding-agent coding_session`, `cargo test -p pi-coding-agent --test print_mode`, `cargo test -p pi-coding-agent --test session_print_mode`, `cargo test -p pi-coding-agent --test session_cli`, `cargo check --workspace`, and `cargo test --workspace` pass for the current Phase 2 type/context/graph/real-ResolveRequest/real-PrepareInput/real-ResolveRuntime/real-LoadResources/real-OpenSession/real-BuildAgentRuntime/event-mapping/RecordUserInput/RunAgentTurn/real-FinalizeTurn/real-EmitCompletion/replay-hydration/tool-call-hydration/session-recording/session-prompt/session-finalization-convergence/non-persistent-runtime/session-open-or-create-list/print-session-target-convergence/json-event-adapter/json-execution-convergence slice. Latest verified subset: `cargo fmt --check`, `cargo test -p pi-coding-agent coding_session`, `cargo check --workspace`, and `cargo test -p pi-coding-agent`.

## Phase 3: Adapter Convergence

Guide: [Phase 3](superpowers/guides/2026-06-29-phase-3-adapter-convergence-guide.md)

- [x] Add concrete `CodingAgentCapabilities`. `CapabilityStatus` now reports available/unsupported/busy/disabled states across prompt, abort, steer, follow-up, compact, fork, clone/switch/export session, tools, shell, and plugins.
- [x] Route RPC prompt command through `CodingAgentSession`. Enabled-session RPC prompts run through persistent `CodingAgentSession`, stream `CodingAgentEvent` through `RpcCodingEventAdapter`, and persist Rust-native session logs; disabled-session RPC prompts run through non-persistent `CodingAgentSession`, emit product-event-derived protocol events, and leave `get_state.sessionId` as `in-memory` with no session file.
- [x] Add RPC adapter from `CodingAgentEvent` to protocol events. `RpcCodingEventAdapter` wraps the product-event protocol adapter at the RPC boundary and has prompt stream/failure mapping coverage; it is ready to wire into RPC prompt migration.
- [x] Add RPC capability reporting. RPC `get_state` now includes protocol-stable capability status objects derived from the concrete capability model, including idle prompt availability and running prompt busy state.
- [x] Route interactive prompt tasks through `CodingAgentSession`. Interactive text/content prompts run through persistent or non-persistent `CodingAgentSession`, active persistent Rust-native manual compaction runs through `CodingAgentSession`, both stream `CodingAgentEvent` through `CodingEventBridge`, preserve owner-lifetime session state across prompts, and reject old JSONL session targets instead of falling back to the removed runner.
- [x] Add interactive bridge from `CodingAgentEvent` to `UiEvent`. `CodingEventBridge` now maps product assistant/tool/failure/abort/compaction events into existing `UiEvent` values while leaving old `AgentEvent` interactive paths in place.
- [x] Move migrated session actions to `SessionService`. Interactive startup `--resume` and `/resume` session selection use `SessionService`-owned Rust-native replay hydration for transcript/session label/active leaf; newly created Rust-native prompt sessions are tracked as active session choices; `/session` reports Rust-native session details; `/tree` uses a `SessionService`-owned leaf-backed tree view and forks selected historical leaves into independent Rust-native sessions; Rust-native `/clone` and `/fork` create independent Rust-native sessions from committed leaves through `SessionService`; Rust-native `/compact` persists typed compaction events and folds compacted replay through `SessionService`; old JSONL sessions are ignored rather than hydrated.
- [x] Stop creating and executing old session JSONL from product paths. Print, JSON, RPC, interactive prompt/manual-compaction paths now use Rust-native session logs or non-persistent product runtime; explicit old JSONL import/export/session targets are rejected.
- [x] Pass adapter cwd into Rust-native session creation/list filtering. `CodingAgentSessionOptions` now carries adapter-provided cwd, `SessionService` records that cwd in `session.created`, Rust-native session list filtering uses it for workspace-scoped choices, and print/RPC/interactive prompt paths pass their configured cwd into `CodingAgentSession`.
- [~] Add Phase 3 tests. CapabilityService idle/busy coverage, public API smoke coverage, RPC `get_state` idle/running capability reporting coverage, RPC product-event adapter prompt stream/failure/thinking coverage, enabled RPC Rust-native session persistence/state and active-leaf coverage, disabled RPC non-persistent no-file coverage, interactive `CodingEventBridge` assistant text/thinking/tool/failure/abort/compaction/ignored-event coverage, interactive transcript tool/assistant ordering regression coverage, interactive primary prompt Rust-native persistence/no-old-JSONL boundary coverage, interactive owner-lifetime multi-prompt persistence coverage, interactive Rust-native startup `/resume` and selector hydration coverage, Rust-native active `/session` info with committed leaf and leaf-backed `/tree` navigation coverage, Rust-native fork/clone session creation coverage, Rust-native manual compaction persistence/replay coverage, Rust-native adapter cwd recording/list filtering coverage, legacy JSONL resume rejection coverage, and coding-running abort/steer/follow-up disabled behavior coverage added.
- [~] Run Phase 3 focused checks. `cargo fmt --check`, `cargo test -p pi-coding-agent capabilities`, `cargo test -p pi-coding-agent public_api`, `cargo test -p pi-coding-agent coding_session`, `cargo test -p pi-coding-agent --test rpc_mode`, `cargo test -p pi-coding-agent --test protocol_sessions`, `cargo test -p pi-coding-agent --test interactive_event_bridge`, `cargo test -p pi-coding-agent --test interactive_mode`, `cargo test -p pi-coding-agent --test interactive_sessions`, `cargo test -p pi-coding-agent --test interactive_abort`, `cargo test -p pi-coding-agent rpc_adapter`, `cargo check --workspace`, and `cargo test -p pi-coding-agent` pass for completed Phase 3 slices. Latest verified subset: `cargo fmt --check`, `cargo test -p pi-coding-agent --test interactive_event_bridge`, `cargo test -p pi-coding-agent --test protocol_events`, `cargo test -p pi-coding-agent --test rpc_mode rpc_streams_agent_events_before_prompt_finishes`, `cargo check --workspace`, and `cargo test -p pi-coding-agent`.

## Phase 4: AgentTurnFlow

Guide: [Phase 4](superpowers/guides/2026-06-29-phase-4-agent-turn-flow-guide.md)

- [x] Add `pi-agent-core/src/agent_turn_flow/` module.
- [x] Add `AgentTurnContext`. Initial context construction snapshots current agent config, messages, tools, resources, queues, cancellation token, and empty turn-local accumulators without draining queues or changing loop behavior.
- [x] Extract prepare-context node. `PrepareContextNode` now builds a `ProviderRequestSnapshot` from `AgentTurnContext` using the same context conversion and stream-option logic as the existing loop.
- [x] Extract runtime compaction node. `MaybeCompactRuntimeContextNode` now applies context-window-gated runtime compaction to `AgentTurnContext`, calls the existing summarizer, updates compacted context messages, and records low-level `SessionCompacted` events without writing product session state.
- [x] Extract provider stream node. `ProviderStreamNode` now consumes prepared provider requests from `AgentTurnContext`, streams through the existing provider boundary, buffers low-level `LlmEvent` events, stores the final assistant message, and maps streams without `Done` into `AgentError` events.
- [x] Extract decide-stop-or-tools node. `DecideStopOrToolsNode` now appends the assistant message to turn context, emits `AgentDone`/`AgentError` for terminal stop reasons, extracts pending tool calls in assistant order, and returns `done`/`tools`/`continue`/`error`/`aborted` actions without executing tools.
- [x] Extract tool execution node. `ExecuteToolsNode` now emits `ToolCallStart`/`ToolCallEnd`, executes matching tools, records unknown tools as error results, honors sequential vs parallel execution selection including per-tool sequential overrides, applies before/after tool hooks, emits sequential tool update callbacks as `ToolCallUpdate`, appends `ToolResult` messages in assistant order, returns `continue`, and returns `done` with `AgentDone` when all tool results terminate.
- [x] Preserve `AgentEvent` behavior. Existing `agent_loop`, `parallel_tools`, `hooks`, and `compaction` regression tests continue to exercise the public low-level event stream after `Agent::run()` switched to `AgentTurnFlow`.
- [x] Make `Agent::run()` delegate to `AgentTurnFlow`. `Agent::run()` now calls `AgentTurnFlow::run_state`, and `agent_loop::run_loop` is a compatibility wrapper over the same runtime entrypoint.
- [x] Add Phase 4 tests. `agent_turn_flow` integration coverage verifies `AgentTurnContext` construction from existing `Agent` state without queue drainage, `PrepareContextNode` provider-request construction against the current `Agent` snapshot behavior, runtime compaction node context/event updates, provider stream success/error event buffering, decide-stop-or-tools text/tool actions, tool execution success/unknown-tool behavior, parallel tool end-event/message ordering, before/after tool hooks, update callback events, terminate completion, and the runtime entrypoint boundary.
- [x] Run Phase 4 focused checks. Latest verified subset: `cargo fmt --check`, `cargo test -p pi-agent-core agent_turn_flow_runtime_entrypoint_exists`, `cargo test -p pi-agent-core --test agent_loop`, `cargo test -p pi-agent-core --test agent_turn_flow`, `cargo test -p pi-agent-core --test compaction`, `cargo test -p pi-agent-core --test parallel_tools`, `cargo test -p pi-agent-core --test hooks`, `cargo test -p pi-agent-core`, `cargo test -p pi-coding-agent`, and `cargo check --workspace`.

## Phase 5: Plugin Kernel

Guide: [Phase 5](superpowers/guides/2026-06-29-phase-5-plugin-kernel-guide.md)

- [ ] Add plugin registry module.
- [ ] Add capability-scoped plugin hosts.
- [ ] Add `ToolProvider`.
- [ ] Add `CommandProvider`.
- [ ] Add `HookProvider`.
- [ ] Add minimal `UiProvider` and `KeybindProvider` boundaries.
- [ ] Reserve first-party `FlowExtension`.
- [ ] Integrate plugin tools through `RuntimeService`.
- [ ] Integrate prompt hooks through `PromptTurnFlow`.
- [ ] Add plugin failure isolation.
- [ ] Add Phase 5 tests.
- [ ] Run Phase 5 focused checks.

## Phase 6: Advanced Flow Workflows

Guide: [Phase 6](superpowers/guides/2026-06-29-phase-6-advanced-flow-workflows-guide.md)

- [ ] Add `ManualCompactionFlow`.
- [ ] Add explicit runtime vs session compaction boundaries.
- [ ] Add `BranchSummaryFlow`.
- [ ] Add Rust-native `ExportFlow`.
- [ ] Add `PluginLoadFlow`.
- [ ] Design and prototype subagent/supervisor flows.
- [ ] Design and prototype self-healing edit workflow.
- [ ] Add workflow capability integration.
- [ ] Add workflow session event integration.
- [ ] Add Phase 6 tests.

## Cross-Cutting TODO

- [ ] Update `docs/roadmap/cross-cutting.md` to remove TS session compatibility as a current invariant.
- [ ] Add a dedicated Rust-native session format doc once Phase 1 schema stabilizes.
- [~] Prefer retiring migrated legacy paths over preserving compatibility. Workspace `AGENTS.md` now states that old TypeScript and old Rust runner paths are behavioral references, not compatibility targets; implementation/docs should remove migrated old paths or document explicit temporary stop conditions.
- [ ] Keep `pi-agent-core` free of coding-agent product ownership.
- [ ] Keep `CodingAgentSession` as owner/coordinator, not a monolithic implementation class.
- [ ] Keep plugin/Lua APIs from depending on internal operation contexts.
- [ ] Keep product event adapters independent from concrete Flow node IDs.
- [ ] Keep all tests deterministic and offline unless explicitly marked as smoke/opt-in.

## Progress Log

- 2026-06-29: Minimal `pi_agent_core::flow` exists.
- 2026-06-29: Flow-centered runtime architecture design committed.
- 2026-06-29: Flow-centered implementation plan committed.
- 2026-06-29: Six phase implementation guides and interface review committed.
- 2026-06-29: Phase 1 `CodingAgentSession` shell, product event/error boundary, EventService, API exports, and focused smoke tests added.
- 2026-06-29: Phase 1 Rust-native session manifest, typed session event envelope/data, and deterministic ID/clock test boundaries added.
- 2026-06-29: Phase 1 `SessionLogStore` added for Rust-native session directories, manifest I/O, JSONL event append/read, and manifest updates.
- 2026-06-29: Phase 1 `TurnTransaction` added for typed operation event buffering, commit/abort/fail finalization, lifecycle cancellation, and post-commit active leaf updates.
- 2026-06-29: Phase 1 replay and owner persistence boundary completed: `CodingAgentSession` create/open now use Rust-native session directories, create appends `session.created`, and `SessionLogStore` can replay canonical `events.jsonl` into a transcript.
- 2026-06-29: Phase 1 focused checks passed: `cargo fmt --check`, `cargo test -p pi-coding-agent coding_session`, `cargo test -p pi-coding-agent public_api`, `cargo check --workspace`, and `cargo test --workspace`.
- 2026-06-29: Phase 2 started with `PromptTurnOptions`, `PromptTurnOutcome`, `PromptTurnContext`, `RuntimeSnapshot`, coding-session unit tests, public API smoke coverage, and passing fmt/check/workspace tests for the slice.
- 2026-06-29: Phase 2 `PromptTurnFlow` graph skeleton added with stable node IDs, no-op boundary nodes, FlowService construction/run entrypoints, graph-focused coding-session tests, and passing fmt/check/workspace tests for the slice.
- 2026-06-29: Phase 2 AgentEvent-to-CodingAgentEvent mapping added for turn/provider/assistant/tool/error/compaction events, with EventService emission coverage and focused coding-session checks passing for the slice.
- 2026-06-29: Phase 2 RunAgentTurn node now drives a staged Agent runtime with the existing Agent stream APIs, records final assistant messages and mapped product events in PromptTurnContext, and has faux-provider graph coverage.
- 2026-06-29: Phase 2 RunAgentTurn now records assistant deltas/completion and tool lifecycle observations into pending Rust-native session events through TurnTransaction without flushing before FinalizeTurn.
- 2026-06-29: Phase 2 `CodingAgentSession::prompt()` added for runtime-backed prompt options: RuntimeService builds Agent from RuntimeSnapshot, PromptTurnFlow runs through RunAgentTurn and FinalizeTurn, product events are emitted, and committed Rust-native session events replay successfully in focused tests.
- 2026-06-29: Phase 2 print mode routed explicit enabled `New` session targets through `CodingAgentSession` and wrote Rust-native `session.json`/`events.jsonl`; later target convergence removed old CLI runner fallback.
- 2026-06-29: Phase 2 JSON mode began rendering protocol output from `CodingAgentEvent` via `CodingProtocolEventAdapter`; later JSON execution convergence moved execution under `CodingAgentSession`.
- 2026-06-29: Phase 2 RecordUserInput node now persists prompt input through TurnTransaction before RunAgentTurn; `CodingAgentSession::prompt()` replay now includes both user input and assistant output.
- 2026-06-29: Phase 2 replay hydration now feeds completed Rust-native user/assistant transcript items into the Agent runtime before the next prompt; reopened-session tests verify prior context reaches the provider.
- 2026-06-29: Phase 2 tool-call replay hydration now persists tool arguments in Rust-native `tool.call.started` events, folds them into transcript tool-call items, and restores completed/failed assistant tool-call plus tool-result groups before reopened-session prompts.
- 2026-06-29: Phase 2 BuildAgentRuntime is now a real PromptTurnFlow node: `CodingAgentSession::prompt()` passes replay into context, the graph builds/hydrates the Agent from RuntimeSnapshot, and graph/service tests exercise the node-owned runtime boundary.
- 2026-06-29: Phase 2 Rust-native session target groundwork added: `SessionLogStore` can try-open by explicit id and list native session manifests, `SessionService`/`CodingAgentSession` expose open-or-create plus list summaries, and public API smoke coverage verifies the new owner-level entrypoints.
- 2026-06-29: Phase 2 ResolveRuntime is now a real PromptTurnFlow node: `PromptTurnContext` no longer eagerly carries runtime from options, the node attaches RuntimeSnapshot at the graph boundary, and focused tests cover successful attachment plus missing-runtime errors.
- 2026-06-29: Phase 2 PrepareInput is now a real PromptTurnFlow node: prompt invocations are normalized into prepared persisted input before recording, RecordUserInput requires that prepared input, and focused tests cover normal text input, empty-input rejection, and misordered record-node execution.
- 2026-06-29: Phase 2 LoadResources is now a real PromptTurnFlow node: the graph attaches the runtime `AgentResources` snapshot into turn state, BuildAgentRuntime now requires the resources stage, and focused tests cover successful resource loading, missing-runtime errors, and skipped-resource build failures.
- 2026-06-29: Phase 2 EmitCompletion is now a real PromptTurnFlow node: successful graph runs append `PromptCompleted` through `PromptTurnContext`, owner-level outcome emission is a missing-event fallback, and focused tests cover completion emission, idempotency, and missing-final-message errors.
- 2026-06-29: Phase 2 print session target convergence design added. The design prioritizes Rust-native `CodingAgentSession` ownership for migrated print session targets and treats temporary unmigrated paths as cleanup targets, not a compatibility requirement.
- 2026-06-29: Phase 2 print session target convergence implemented: enabled default/New/OpenTarget id/path/OpenOrCreateId/ContinueMostRecent print session targets route through `CodingAgentSession`, ForkTarget now fails explicitly until Rust-native fork semantics exist, OpenSession validates owner-prepared session state, and focused print/session/coding-session checks pass.
- 2026-06-30: Phase 2 ResolveRequest node design added. The design keeps adapter parsing outside the graph while making the flow validate runtime-backed `PromptTurnOptions` before input preparation and runtime attachment.
- 2026-06-30: Phase 2 ResolveRequest is now a real PromptTurnFlow node: it validates runtime-backed prompt options, rejects empty text/content and manual compaction, marks request resolution idempotently, and makes PrepareInput/ResolveRuntime fail clearly if run before request resolution.
- 2026-06-30: Phase 3 started with a concrete `CodingAgentCapabilities`/`CapabilityStatus` model. `CodingAgentSession::capabilities()` now reports prompt availability/busy state and explicit unsupported reasons for unmigrated adapter/session/plugin capabilities.
- 2026-06-30: Phase 3 RPC capability reporting added: `get_state` includes protocol-stable capability status objects so clients can distinguish available, disabled, unsupported, and busy operations before RPC prompt execution migrates to `CodingAgentSession`.
- 2026-06-30: Phase 3 RPC product-event adapter added. `RpcCodingEventAdapter` provides the RPC boundary for converting `CodingAgentEvent` into existing protocol events.
- 2026-06-30: Phase 3 enabled-session RPC prompt migration started: RPC prompts with `SessionMode::Enabled` run through `CodingAgentSession`, stream product events through `RpcCodingEventAdapter`, and persist Rust-native `session.json`/`events.jsonl`; disabled/no-session RPC prompts moved to non-persistent product runtime later.
- 2026-06-30: Session finalization convergence design added. The design moves prompt transaction commit/fail/abort ownership back under `SessionService` and makes `SessionWrite*` product events part of the persistent prompt event stream before final prompt outcome events.
- 2026-06-30: Non-persistent product runtime design added. The design keeps `CodingAgentSession` as product owner when durable persistence is disabled, uses the same `PromptTurnFlow`, emits `SessionWriteSkipped`, and gives no-session print/RPC plus later JSON execution convergence a path into product runtime ownership.
- 2026-06-30: Session finalization convergence implemented. `SessionService` now owns prompt transaction commit/fail/abort/skip finalization, `FinalizeTurn` only validates readiness, owner-level prompt events emit `SessionWritePending`/`SessionWriteCommitted` before `PromptCompleted` or `PromptFailed`, and focused coding-session/print/RPC/protocol checks pass.
- 2026-06-30: Non-persistent product runtime implemented for core owner, print, and RPC prompt paths. `CodingAgentSession::non_persistent()` runs `PromptTurnFlow` without durable session storage, emits `SessionWriteSkipped`, supports owner-lifetime transcript hydration, routes no-session/disabled print through product runtime, and routes disabled-session RPC through `RpcCodingEventAdapter` with no session file.
- 2026-06-30: Interactive CodingEventBridge design added. The bridge is scoped to translating `CodingAgentEvent` into existing `UiEvent` values and was later wired into interactive prompt task ownership.
- 2026-06-30: Interactive CodingEventBridge implemented. Product assistant/tool/failure/abort/compaction events map into existing `UiEvent` values with deterministic bridge coverage and now back the interactive prompt path.
- 2026-06-30: Interactive primary prompt migration implemented. Ordinary interactive text/content prompts run through `CodingAgentSession`, use `CodingEventBridge` for UI events, persist enabled-session prompts as Rust-native `session.json`/`events.jsonl`, reuse the owner across multiple prompts, keep no-session prompts non-persistent, and now reject old JSONL session targets.
- 2026-06-30: Interactive Rust-native resume hydration added. `SessionService` exposes an internal replay-backed hydration view, interactive startup `--resume` and `/resume` use Rust-native `session.json`/`events.jsonl` transcript hydration, and old JSONL resume files are ignored.
- 2026-06-30: Adapter-provided cwd now flows into Rust-native session creation and list filtering. `CodingAgentSessionOptions` carries cwd, `SessionService` records it in `session.created`, print/RPC/interactive prompt paths pass their configured cwd, and interactive Rust-native choices are workspace-cwd scoped instead of only session-root scoped.
- 2026-06-30: JSON execution now runs through the product runtime path via `CodingAgentSession::prompt()`. JSON mode still emits the existing protocol wire through `CodingProtocolEventAdapter`, enabled sessions write Rust-native logs, and focused JSON/protocol event checks pass.
- 2026-06-30: Interactive Rust-native session actions advanced: newly created and resumed Rust-native sessions are tracked as active `SessionChoice`s, `/session` reports Rust-native session details, `/tree` opens a read-only tree projection from `SessionService` hydration, and Rust-native fork/clone/compact/tree navigation now fail with explicit unsupported boundaries instead of falling through to legacy JSONL assumptions.
- 2026-06-30: Rust-native active leaf commit design added. The design makes successful persistent prompt commits allocate and persist a real `leaf_id` through `SessionService`/`TurnTransaction` before implementing fork, clone, compact, or tree navigation actions.
- 2026-06-30: Rust-native active leaf commits implemented. Successful persistent `CodingAgentSession::prompt()` calls now allocate a `leaf_id` in `SessionService`, commit it through `TurnTransaction`, refresh session hydration/list summaries, return it in `PromptTurnOutcome`, and propagate it to RPC/interactive active-session state.
- 2026-07-01: Prompt execution now broadcasts `CodingAgentEvent` values live while `PromptTurnFlow` runs, including assistant thinking deltas for TUI/RPC adapters. Rust-native assistant persistence no longer writes per-delta `message.delta` events; `message.completed` now stores final content blocks, including thinking blocks, under event schema version 2.
- 2026-06-30: Rust-native session fork/clone design added. The design adds `SessionService`-owned clone/fork APIs that create independent Rust-native sessions from committed leaves by rewriting durable source events and recording typed provenance events.
- 2026-06-30: Rust-native session fork/clone implemented for interactive active sessions. `SessionService` now creates independent cloned/forked Rust-native sessions from committed leaves, records `session.cloned`/`session.forked` provenance, rewrites copied event history to the new session id, updates active leaf manifests, and interactive `/clone` and `/fork` select the new session instead of reporting unsupported.
- 2026-06-30: Rust-native manual compaction design added. The design keeps manual compaction separate from `PromptTurnFlow`, persists typed Rust-native compaction events through `SessionService`, folds compaction during replay, and wires interactive `/compact` for active Rust-native sessions without any old JSONL compaction fallback.
- 2026-06-30: Rust-native manual compaction implemented for active interactive sessions. `CodingAgentSession::compact()` now summarizes replayed Rust-native context through the existing compaction summarizer, writes `session.compaction.started`/`session.compaction.completed` events, folds replay into a compaction summary plus kept tail, emits product compaction events through `CodingEventBridge`, and `/compact` uses only the Rust-native product runtime for Rust-native sessions.
- 2026-06-30: Print-mode `ForkTarget` now routes through Rust-native session ownership. Enabled print mode opens the source Rust-native session, forks it through `SessionService`, opens the new forked session, hydrates source transcript into the provider context, records `session.forked`, and keeps the source session unchanged.
- 2026-06-30: Rust-native tree navigation implemented as leaf-backed fork-on-select. `SessionService::tree_view` builds `/tree` nodes from real committed `leaf_id` values, interactive `/tree` opens that view for Rust-native sessions, selecting the current leaf reports no-op, and selecting a historical leaf forks a new Rust-native session at that leaf instead of using temporary projection ids.
- 2026-07-01: Old JSONL execution cleanup removed the `session_runner` module and public exports, routed interactive prompt/compact/session actions exclusively through `CodingAgentSession`, stopped advertising `/import`, rejects JSONL import/export/session targets, and keeps regression coverage that legacy JSONL resume files are ignored.
- 2026-07-01: Interactive transcript rendering now treats tool starts and new agent turns as assistant stream boundaries, preventing post-tool assistant deltas from rewriting the previous assistant block and preserving assistant/tool/assistant visual order in TUI output.
- 2026-07-01: Phase 4 started with the `pi-agent-core::agent_turn_flow` module and `AgentTurnContext` snapshot boundary. `Agent::run()` still delegates to the existing loop while future extraction nodes get a stable low-level state container.
- 2026-07-01: Phase 4 `PrepareContextNode` added. It builds provider request snapshots from `AgentTurnContext` with existing conversion/stream-option helpers, giving the next runtime-compaction extraction a node-shaped boundary without changing `Agent::run()`.
- 2026-07-01: Phase 4 runtime compaction node added. `MaybeCompactRuntimeContextNode` runs auto-compaction against `AgentTurnContext`, preserves existing summarization/threshold behavior, emits low-level `SessionCompacted` into the context event buffer, and still leaves `Agent::run()` on the existing loop.
- 2026-07-01: Phase 4 provider stream node added. `ProviderStreamNode` streams prepared provider requests from `AgentTurnContext`, buffers `LlmEvent` and provider error events, stores the final assistant message for downstream decision nodes, and still leaves `Agent::run()` on the existing loop.
- 2026-07-01: Phase 4 decide-stop-or-tools node added. `DecideStopOrToolsNode` consumes the streamed assistant message, appends it to context messages, emits terminal low-level events for done/error/aborted cases, extracts pending tool calls for the future tool execution node, and still leaves `Agent::run()` on the existing loop.
- 2026-07-01: Phase 4 initial tool execution node added. `ExecuteToolsNode` consumes pending tool calls from `AgentTurnContext`, emits start/end tool events, records successful and unknown-tool results, appends tool-result messages, and still leaves parallel execution, tool hooks, update callbacks, terminate behavior, and `Agent::run()` delegation for follow-up slices.
- 2026-07-01: Phase 4 `ExecuteToolsNode` now honors sequential vs parallel tool execution selection, emits parallel tool end events in completion order, and still appends tool-result messages in assistant order; hook, update callback, terminate, and `Agent::run()` delegation parity remain follow-up work.
- 2026-07-01: Phase 4 tool execution node parity advanced: `ExecuteToolsNode` applies before/after tool hooks, emits sequential tool update callbacks, and finishes with `AgentDone` when all tool results terminate. Remaining Phase 4 focus is preserving full `AgentEvent` behavior and delegating `Agent::run()` to `AgentTurnFlow`.
- 2026-07-01: Phase 4 runtime delegation completed. `AgentTurnFlow` now exposes the low-level runtime entrypoint, `Agent::run()` delegates to it, and the old `agent_loop` module is reduced to a compatibility wrapper over the AgentTurnFlow runtime.
