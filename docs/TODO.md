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
- [~] Implement Phase 2: `PromptTurnFlow` on headless/json path. Prompt turn options/outcome/context, runtime snapshot boundary, real graph with stable node IDs, AgentEvent-to-product-event mapping, real ResolveRequest/PrepareInput/ResolveRuntime/LoadResources/OpenSession/BuildAgentRuntime/RecordUserInput/RunAgentTurn/FinalizeTurn/EmitCompletion nodes, pending agent-output event recording through TurnTransaction, `CodingAgentSession::prompt()` for runtime-backed options, completed user/assistant/tool-call replay hydration, Rust-native session open-or-create/list groundwork, enabled print-session target routing, and JSON protocol rendering through `CodingAgentEvent` are in place; non-persistent/no-session product runtime and Rust-native fork/branch semantics remain.
- [~] Implement Phase 3: converge CLI/RPC/interactive adapters. Concrete `CodingAgentCapabilities`/`CapabilityStatus` model, RPC `get_state` capability reporting, RPC `CodingAgentEvent` adapter, and enabled-session RPC prompt routing through `CodingAgentSession` are in place; disabled/no-session RPC prompt still uses the old runner until non-persistent product runtime exists, and interactive convergence remains.
- [ ] Implement Phase 4: introduce `AgentTurnFlow` in `pi-agent-core`.
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
- [x] Add prompt flow nodes. Stable Phase 2 graph boundaries now have real node behavior: ResolveRequest validates runtime-backed prompt options and marks request resolution, PrepareInput normalizes/validates prompt invocation into prepared persisted input, ResolveRuntime owns runtime snapshot attachment from PromptTurnOptions into PromptTurnContext, LoadResources attaches the runtime resource snapshot into turn state before agent construction, OpenSession validates that the owner prepared session id/replay/transaction state before agent construction, BuildAgentRuntime builds the Agent from RuntimeSnapshot and hydrated replay only after resources are loaded, RecordUserInput records prepared prompt input, RunAgentTurn drives an existing Agent stream, FinalizeTurn commits successful transactions, and EmitCompletion appends the success completion product event from the graph.
- [x] Map `AgentEvent` to `CodingAgentEvent`.
- [x] Add `RunAgentTurn` node using existing `Agent::run()`.
- [x] Record agent output into session events through `TurnTransaction`.
- [~] Converge prompt transaction finalization under `SessionService` and emit `SessionWrite*` product events. Design is approved and documented; implementation remains.
- [x] Add `CodingAgentSession::prompt()`.
- [~] Design non-persistent product runtime for no-session/disabled prompt convergence. Design is approved and documented; implementation remains.
- [~] Route print mode through `CodingAgentSession`. Enabled default/New/OpenTarget/OpenOrCreateId/ContinueMostRecent print session targets now use `CodingAgentSession` and Rust-native session logs; ForkTarget fails explicitly until Rust-native fork/branch semantics exist; no-session/disabled print execution stays on the old runner until a non-persistent product runtime exists.
- [x] Route JSON mode through `CodingAgentEvent`.
- [~] Keep old `session_runner` as transitional wrapper. It remains the execution source for unmigrated adapters and no-session/disabled print/RPC execution while migrated enabled print/RPC session targets and JSON rendering move onto `CodingAgentSession`/`CodingAgentEvent`.
- [~] Add Phase 2 tests. Prompt turn option/outcome/context, runtime snapshot, graph path/node ID, misconfigured flow, FlowService, RuntimeService agent building and replay hydration, real ResolveRequest validation/idempotency/order coverage, real PrepareInput normalization/error coverage, real ResolveRuntime attachment/error coverage, real LoadResources attachment/error coverage, real OpenSession boundary validation coverage, real BuildAgentRuntime resource-precondition and replay hydration coverage, real EmitCompletion completion/idempotency/error coverage, AgentEvent mapping, RecordUserInput prepared-event recording, RunAgentTurn/faux-provider flow execution, pending assistant/tool session-event recording with tool arguments, `CodingAgentSession::prompt()` success/config-error/failure-event-deduplication with user+assistant replay and reopened-session provider-context hydration for prior user/assistant/tool result history, Rust-native session open-or-create/list coverage, public API smoke coverage, default/New/OpenTarget id/path/OpenOrCreateId/ContinueMostRecent print-mode Rust-native session-log coverage, explicit unsupported ForkTarget coverage, direct `CodingAgentEvent` protocol adapter coverage, and JSON mode success/tool/failure coverage added.
- [~] Run Phase 2 focused checks. `cargo fmt --check`, `cargo test -p pi-coding-agent coding_session`, `cargo test -p pi-coding-agent --test print_mode`, `cargo test -p pi-coding-agent --test session_print_mode`, `cargo test -p pi-coding-agent --test session_cli`, `cargo check --workspace`, and `cargo test --workspace` pass for the current Phase 2 type/context/graph/real-ResolveRequest/real-PrepareInput/real-ResolveRuntime/real-LoadResources/real-OpenSession/real-BuildAgentRuntime/event-mapping/RecordUserInput/RunAgentTurn/real-EmitCompletion/replay-hydration/tool-call-hydration/session-recording/session-prompt/session-open-or-create-list/print-session-target-convergence/json-event-adapter slice.

## Phase 3: Adapter Convergence

Guide: [Phase 3](superpowers/guides/2026-06-29-phase-3-adapter-convergence-guide.md)

- [x] Add concrete `CodingAgentCapabilities`. `CapabilityStatus` now reports available/unsupported/busy/disabled states across prompt, abort, steer, follow-up, compact, fork, clone/switch/export session, tools, shell, and plugins.
- [~] Route RPC prompt command through `CodingAgentSession`. Enabled-session RPC prompts now run through `CodingAgentSession`, stream `CodingAgentEvent` through `RpcCodingEventAdapter`, and persist Rust-native session logs; disabled/no-session RPC prompts stay on the old runner until non-persistent product runtime support lands.
- [x] Add RPC adapter from `CodingAgentEvent` to protocol events. `RpcCodingEventAdapter` wraps the product-event protocol adapter at the RPC boundary and has prompt stream/failure mapping coverage; it is ready to wire into RPC prompt migration.
- [x] Add RPC capability reporting. RPC `get_state` now includes protocol-stable capability status objects derived from the concrete capability model, including idle prompt availability and running prompt busy state.
- [ ] Route interactive prompt tasks through `CodingAgentSession`.
- [ ] Add interactive bridge from `CodingAgentEvent` to `UiEvent`.
- [ ] Move migrated session actions to `SessionService`.
- [ ] Stop creating old session JSONL from migrated product prompt paths.
- [~] Add Phase 3 tests. CapabilityService idle/busy coverage, public API smoke coverage, RPC `get_state` idle/running capability reporting coverage, RPC product-event adapter prompt stream/failure coverage, and enabled RPC Rust-native session persistence/state coverage added.
- [~] Run Phase 3 focused checks. `cargo fmt --check`, `cargo test -p pi-coding-agent capabilities`, `cargo test -p pi-coding-agent public_api`, `cargo test -p pi-coding-agent coding_session`, `cargo test -p pi-coding-agent --test rpc_mode`, `cargo test -p pi-coding-agent --test protocol_sessions`, `cargo test -p pi-coding-agent rpc_adapter`, `cargo check --workspace`, and `cargo test --workspace` pass for completed Phase 3 slices.

## Phase 4: AgentTurnFlow

Guide: [Phase 4](superpowers/guides/2026-06-29-phase-4-agent-turn-flow-guide.md)

- [ ] Add `pi-agent-core/src/agent_turn_flow/` module.
- [ ] Add `AgentTurnContext`.
- [ ] Extract prepare-context node.
- [ ] Extract runtime compaction node.
- [ ] Extract provider stream node.
- [ ] Extract decide-stop-or-tools node.
- [ ] Extract tool execution node.
- [ ] Preserve `AgentEvent` behavior.
- [ ] Make `Agent::run()` delegate to `AgentTurnFlow`.
- [ ] Add Phase 4 tests.
- [ ] Run Phase 4 focused checks.

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
- 2026-06-29: Phase 2 print mode now routes explicit enabled `New` session targets through `CodingAgentSession`, writes Rust-native `session.json`/`events.jsonl`, and keeps legacy CLI session paths on `session_runner` until replay/target migration is ready.
- 2026-06-29: Phase 2 JSON mode now renders protocol output from `CodingAgentEvent` via `CodingProtocolEventAdapter`, while the old runner remains the transitional execution source until full session ownership moves over.
- 2026-06-29: Phase 2 RecordUserInput node now persists prompt input through TurnTransaction before RunAgentTurn; `CodingAgentSession::prompt()` replay now includes both user input and assistant output.
- 2026-06-29: Phase 2 replay hydration now feeds completed Rust-native user/assistant transcript items into the Agent runtime before the next prompt; reopened-session tests verify prior context reaches the provider.
- 2026-06-29: Phase 2 tool-call replay hydration now persists tool arguments in Rust-native `tool.call.started` events, folds them into transcript tool-call items, and restores completed/failed assistant tool-call plus tool-result groups before reopened-session prompts.
- 2026-06-29: Phase 2 BuildAgentRuntime is now a real PromptTurnFlow node: `CodingAgentSession::prompt()` passes replay into context, the graph builds/hydrates the Agent from RuntimeSnapshot, and graph/service tests exercise the node-owned runtime boundary.
- 2026-06-29: Phase 2 Rust-native session target groundwork added: `SessionLogStore` can try-open by explicit id and list native session manifests, `SessionService`/`CodingAgentSession` expose open-or-create plus list summaries, and public API smoke coverage verifies the new owner-level entrypoints.
- 2026-06-29: Phase 2 ResolveRuntime is now a real PromptTurnFlow node: `PromptTurnContext` no longer eagerly carries runtime from options, the node attaches RuntimeSnapshot at the graph boundary, and focused tests cover successful attachment plus missing-runtime errors.
- 2026-06-29: Phase 2 PrepareInput is now a real PromptTurnFlow node: prompt invocations are normalized into prepared persisted input before recording, RecordUserInput requires that prepared input, and focused tests cover normal text input, empty-input rejection, and misordered record-node execution.
- 2026-06-29: Phase 2 LoadResources is now a real PromptTurnFlow node: the graph attaches the runtime `AgentResources` snapshot into turn state, BuildAgentRuntime now requires the resources stage, and focused tests cover successful resource loading, missing-runtime errors, and skipped-resource build failures.
- 2026-06-29: Phase 2 EmitCompletion is now a real PromptTurnFlow node: successful graph runs append `PromptCompleted` through `PromptTurnContext`, owner-level outcome emission is a missing-event fallback, and focused tests cover completion emission, idempotency, and missing-final-message errors.
- 2026-06-29: Phase 2 print session target convergence design added. The design prioritizes Rust-native `CodingAgentSession` ownership for migrated print session targets and treats old runner use as a temporary unmigrated-path gap, not a compatibility requirement.
- 2026-06-29: Phase 2 print session target convergence implemented: enabled default/New/OpenTarget id/path/OpenOrCreateId/ContinueMostRecent print session targets route through `CodingAgentSession`, ForkTarget now fails explicitly until Rust-native fork semantics exist, OpenSession validates owner-prepared session state, and focused print/session/coding-session checks pass.
- 2026-06-30: Phase 2 ResolveRequest node design added. The design keeps adapter parsing outside the graph while making the flow validate runtime-backed `PromptTurnOptions` before input preparation and runtime attachment.
- 2026-06-30: Phase 2 ResolveRequest is now a real PromptTurnFlow node: it validates runtime-backed prompt options, rejects empty text/content and manual compaction, marks request resolution idempotently, and makes PrepareInput/ResolveRuntime fail clearly if run before request resolution.
- 2026-06-30: Phase 3 started with a concrete `CodingAgentCapabilities`/`CapabilityStatus` model. `CodingAgentSession::capabilities()` now reports prompt availability/busy state and explicit unsupported reasons for unmigrated adapter/session/plugin capabilities.
- 2026-06-30: Phase 3 RPC capability reporting added: `get_state` includes protocol-stable capability status objects so clients can distinguish available, disabled, unsupported, and busy operations before RPC prompt execution migrates to `CodingAgentSession`.
- 2026-06-30: Phase 3 RPC product-event adapter added. `RpcCodingEventAdapter` provides the RPC boundary for converting `CodingAgentEvent` into existing protocol events.
- 2026-06-30: Phase 3 enabled-session RPC prompt migration started: RPC prompts with `SessionMode::Enabled` now run through `CodingAgentSession`, stream product events through `RpcCodingEventAdapter`, and persist Rust-native `session.json`/`events.jsonl`; disabled/no-session RPC prompts remain on the old runner until non-persistent product runtime support exists.
- 2026-06-30: Session finalization convergence design added. The design moves prompt transaction commit/fail/abort ownership back under `SessionService` and makes `SessionWrite*` product events part of the persistent prompt event stream before final prompt outcome events.
- 2026-06-30: Non-persistent product runtime design added. The design keeps `CodingAgentSession` as product owner when durable persistence is disabled, uses the same `PromptTurnFlow`, emits `SessionWriteSkipped`, and gives no-session print/RPC plus later JSON execution convergence a path off the old runner.
