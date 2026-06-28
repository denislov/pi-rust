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
- [ ] Implement Phase 1: `CodingAgentSession` skeleton and Rust-native session log.
- [ ] Implement Phase 2: `PromptTurnFlow` on headless/json path.
- [ ] Implement Phase 3: converge CLI/RPC/interactive adapters.
- [ ] Implement Phase 4: introduce `AgentTurnFlow` in `pi-agent-core`.
- [ ] Implement Phase 5: plugin kernel on session/flow boundaries.
- [ ] Implement Phase 6: advanced Flow workflows.

## Phase 1: CodingAgentSession and Rust-Native Session Log

Guide: [Phase 1](superpowers/guides/2026-06-29-phase-1-coding-session-and-session-log-guide.md)

- [ ] Add `crates/pi-coding-agent/src/coding_session/` module shell.
- [ ] Export `CodingAgentSession` and `CodingAgentEvent` through `pi_coding_agent::api`.
- [ ] Add `CodingSessionError` typed product error boundary.
- [ ] Add base `CodingAgentEvent`.
- [ ] Add `EventService`.
- [ ] Add Rust-native session manifest model.
- [ ] Add typed `SessionEventEnvelope` and `SessionEventData`.
- [ ] Add deterministic ID and clock test boundaries.
- [ ] Add `SessionLogStore`.
- [ ] Add `TurnTransaction`.
- [ ] Add replay/fold transcript view.
- [ ] Add `CodingAgentSession` owner skeleton.
- [ ] Add Phase 1 tests.
- [ ] Run Phase 1 focused checks.

## Phase 2: PromptTurnFlow on Headless/JSON

Guide: [Phase 2](superpowers/guides/2026-06-29-phase-2-prompt-turn-flow-guide.md)

- [ ] Add `PromptTurnOptions`.
- [ ] Add `PromptTurnOutcome`.
- [ ] Add `PromptTurnContext`.
- [ ] Add `RuntimeSnapshot`.
- [ ] Add `PromptTurnFlow` graph.
- [ ] Add prompt flow nodes.
- [ ] Map `AgentEvent` to `CodingAgentEvent`.
- [ ] Add `RunAgentTurn` node using existing `Agent::run()`.
- [ ] Record agent output into session events through `TurnTransaction`.
- [ ] Add `CodingAgentSession::prompt()`.
- [ ] Route print mode through `CodingAgentSession`.
- [ ] Route JSON mode through `CodingAgentEvent`.
- [ ] Keep old `session_runner` as transitional wrapper.
- [ ] Add Phase 2 tests.
- [ ] Run Phase 2 focused checks.

## Phase 3: Adapter Convergence

Guide: [Phase 3](superpowers/guides/2026-06-29-phase-3-adapter-convergence-guide.md)

- [ ] Add concrete `CodingAgentCapabilities`.
- [ ] Route RPC prompt command through `CodingAgentSession`.
- [ ] Add RPC adapter from `CodingAgentEvent` to protocol events.
- [ ] Add RPC capability reporting.
- [ ] Route interactive prompt tasks through `CodingAgentSession`.
- [ ] Add interactive bridge from `CodingAgentEvent` to `UiEvent`.
- [ ] Move migrated session actions to `SessionService`.
- [ ] Stop creating old session JSONL from migrated product prompt paths.
- [ ] Add Phase 3 tests.
- [ ] Run Phase 3 focused checks.

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
