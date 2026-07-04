# Runtime Control Stabilization Plan

## Goal

Close the gap between the completed low-level AgentTurnFlow runtime and the product-level CodingAgentSession control surface before adding the next advanced workflows. This stabilization phase makes running prompt operations cancellable and steerable through the session owner, keeps adapters on CodingAgentEvent and CodingAgentCapabilities, and prevents new Phase 6 work from concentrating more behavior in CodingAgentSession.

## Current Evidence

- AgentTurnFlow is complete in pi-agent-core and Agent::run delegates to it.
- Agent exposes abort, steer, and follow_up controls, and AgentTurnFlow drains steering and follow-up queues during execution.
- CodingAgentSession now owns active operation state through OperationControl, issues prompt control handles, and transfers prompt-control receivers into PromptTurnFlow without exposing Agent internals.
- PromptTurnFlow processes owner-issued abort, steer, and follow-up commands while the Agent stream is running.
- RPC running abort/steer/follow_up commands and RPC prompt streamingBehavior steer/followUp all use the owner-issued prompt control handle.
- Interactive running controls use the same owner path: Ctrl-C aborts, Enter steers, and Shift+Enter submits follow-up input while preserving typed editor text.
- Capabilities are derived from semantic runtime state and report prompt controls from the current OperationKind without stale AgentTurnFlow blocker reasons.
- Phase 6 has delivered manual compaction, branch summary, plugin load/reload, Lua tool/command/hook/UI/keybind/dialog slices, plugin reload capability reporting, ExportFlow wrapping, and runtime-vs-session compaction boundary cleanup.

## Non-Goals

- Do not add delegation-first helper orchestration or self-healing edit workflows in this stabilization phase.
- Do not expose raw Agent, SessionService, RuntimeService, provider internals, filesystem handles, or Flow graph mutation to plugins.
- Do not revive TypeScript session JSONL compatibility.
- Do not make pi-agent-core own product session or adapter semantics.

## Architecture Rules

- CodingAgentSession remains the product runtime owner and public facade.
- PromptTurnFlow remains the product prompt orchestration graph.
- AgentTurnFlow remains the low-level agent runtime implementation.
- Running operation control must enter through CodingAgentSession or an owner-issued operation handle.
- Session writes continue through SessionService and TurnTransaction.
- Adapters consume CodingAgentEvent and CodingAgentCapabilities, not concrete Flow node ids.
- New workflow code should live in focused modules instead of expanding coding_session/mod.rs.

## Stage 0: Documentation Baseline

Tasks:

- Add this plan to docs/TODO.md source documents.
- Mark Phase 6 as active rather than not started.
- Split runtime control stabilization from future delegation-first helper and self-healing work.
- Replace stale await-AgentTurnFlow wording in planning docs with the real remaining gap: CodingAgentSession-owned running operation control.
- Keep TODO entries precise enough that each later code slice can update one item.

Acceptance:

- docs/TODO.md names runtime control stabilization as the immediate Phase 6 gate.
- The next implementation slice can start without reinterpreting the roadmap.

## Stage 1: Session-Owned Operation Control Model

Tasks:

- Add an internal operation control module under crates/pi-coding-agent/src/coding_session/.
- Introduce OperationKind values for prompt, compact, plugin_command, plugin_load, branch_summary, export, and future workflow kinds.
- Introduce an operation guard that sets and clears active operation state even when an async operation returns an error.
- Introduce prompt control commands for abort, steer, and follow-up.
- Introduce an owner-issued prompt control sender or handle that adapters can hold while the prompt task runs.
- Keep the existing CodingAgentSession::prompt API as a convenience wrapper.

Acceptance:

- Busy behavior is driven by one internal state model instead of repeated active_operation string blocks.
- Capabilities can inspect operation kind and available controls.
- No public service internals are exposed.

Focused checks:

- cargo test -p pi-coding-agent coding_session::operation_control
- cargo test -p pi-coding-agent public_api

## Stage 2: Prompt Control Plumbing

Tasks:

- Extend PromptTurnContext with a prompt control receiver or equivalent session-owned control boundary.
- Pass the control boundary into the RunAgentTurn node.
- While RunAgentTurn streams AgentEvent values, also process prompt control commands.
- Abort command calls Agent::abort and finalizes as PromptTurnOutcome::Aborted when the agent stream reports cancellation.
- Steer command calls Agent::steer.
- Follow-up command calls Agent::follow_up.
- Map accepted control commands to product events or diagnostics where useful.
- Keep session finalization in SessionService.

Acceptance:

- A running prompt can be aborted through the product owner.
- A running prompt can receive steer and follow-up input without adapters touching Agent directly.
- Persistent sessions record aborted prompt operations through operation.aborted.
- Non-persistent sessions emit skipped persistence events consistently.

Focused checks:

- cargo test -p pi-coding-agent prompt_flow
- cargo test -p pi-coding-agent coding_session
- cargo test -p pi-agent-core agent_turn_flow

## Stage 3: Adapter Convergence for Running Controls

Tasks:

- RPC RunningPrompt stores the owner-issued control handle.
- RPC abort sends an abort command and returns cancelled true when accepted.
- RPC steer and follow_up send control commands while a coding prompt is running.
- RPC get_state reports abort, steer, and follow_up according to the current operation kind.
- Interactive PromptTask abort sends the same control command instead of reporting unsupported.
- Interactive running Enter submits steer input and Shift+Enter submits follow-up input through the owner-issued control handle.
- Interactive transcript updates remain CodingAgentEvent driven.

Acceptance:

- RPC no longer reports running prompt controls as blocked on AgentTurnFlow.
- Interactive Ctrl-C or the abort action can cancel a running CodingAgentSession prompt.
- Adapter tests prove the shared owner path for control behavior.

Focused checks:

- cargo test -p pi-coding-agent --test rpc_mode
- cargo test -p pi-coding-agent --test interactive_abort
- cargo test -p pi-coding-agent --test interactive_mode
- cargo test -p pi-coding-agent --test interactive_sessions

## Stage 4: Capability Model Cleanup

Tasks:

- Rename phase-specific constructors such as phase_5 to semantic constructors such as from_runtime_state.
- Model persistent-session workflow gates separately from running prompt control gates.
- Include operation kind in Busy status where useful.
- Keep delegation, self_healing_edit, and explicit export-flow capabilities out until their workflows exist.
- Update protocol capability serialization tests.

Acceptance:

- Capabilities describe real product behavior and do not contain stale implementation milestones as reasons.
- RPC and interactive use the same capability source.

Focused checks:

- cargo test -p pi-coding-agent capability
- cargo test -p pi-coding-agent --test rpc_mode
- cargo test -p pi-coding-agent public_api

## Stage 5: Owner Slimming

Tasks:

- Move operation guard/control code out of coding_session/mod.rs.
- Move workflow-specific owner glue into focused modules where practical.
- Keep CodingAgentSession as the stable facade and coordinator.
- Keep tests close to the module whose behavior they verify.

Acceptance:

- coding_session/mod.rs no longer grows for every workflow detail.
- CodingAgentSession still owns product runtime coordination without exposing raw services.

Focused checks:

- cargo test -p pi-coding-agent coding_session
- cargo check --workspace

## Stage 6: ExportFlow Wrapper

Tasks:

- Add ExportOptions, ExportContext, ExportOutcome, and ExportFlow.
- Register the flow through FlowService.
- Use stable node ids: start_export, load_session_replay, select_export_view, render_export, write_export, emit_completion.
- Keep export non-mutating unless a later policy explicitly adds an audit event.
- Route interactive export through the flow wrapper.
- Continue rejecting TypeScript-compatible JSONL export.

Acceptance:

- Export is represented by the same operation/context/flow/outcome pattern as other Phase 6 workflows.
- Existing HTML export behavior remains intact.
- FlowService has stable node-id coverage for ExportFlow.

Focused checks:

- cargo test -p pi-coding-agent export_flow
- cargo test -p pi-coding-agent --test interactive_sessions

## Stage 7: Runtime vs Session Compaction Boundary Cleanup

Tasks:

- Distinguish runtime compaction events from session/manual compaction events in product naming.
- Decide whether manual session compaction creates a dedicated committed leaf.
- If a dedicated leaf is introduced, update replay, tree view, export, and adapter tests together.
- Keep runtime compaction agent-turn-local and do not rewrite session history from pi-agent-core.

Acceptance:

- Adapter-visible naming makes the runtime/session compaction distinction clear.
- Active leaf changes remain owned by SessionService.

Focused checks:

- cargo test -p pi-coding-agent manual_compaction_flow
- cargo test -p pi-agent-core compaction

## Stage 8: Readiness Gate for New Advanced Workflows

Only start delegation-first helper orchestration and self-healing edit after these gates pass:

- Runtime prompt abort works through CodingAgentSession.
- Runtime steer and follow-up work through CodingAgentSession.
- RPC and interactive use shared owner-issued operation controls.
- Capabilities report real control status without stale AgentTurnFlow blockers.
- ExportFlow wrapper exists or has been deliberately scoped out with a TODO update.
- Owner slimming has prevented coding_session/mod.rs from becoming the implementation sink for the next workflows.
- cargo fmt --check passes.
- cargo test -p pi-coding-agent passes.
- cargo check --workspace passes.
- cargo test --workspace passes.

## Suggested Commit Slices

1. docs: plan runtime control stabilization
2. feat(coding-agent): add session operation control model
3. feat(coding-agent): route prompt abort through session control
4. feat(coding-agent): support prompt steer and follow-up control
5. feat(coding-agent): converge rpc running prompt controls
6. feat(coding-agent): converge interactive running prompt controls
7. refactor(coding-agent): split session operation ownership
8. feat(coding-agent): add export flow wrapper
9. docs: close runtime control stabilization tasks
