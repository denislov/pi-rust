# Phase 6 Guide: Advanced Flow Workflows

## Phase Goal

Use the new architecture for workflows that were hard to reason about in the old structure:

- manual/session compaction;
- branch summary;
- export;
- plugin load/reload;
- delegation-first helper orchestration;
- self-healing edit workflows.

Phase 6 is where `pi-rust` should start gaining capabilities from the Flow-centered architecture rather than only matching previous behavior.

## Preconditions

Phase 1:

- Rust-native event log and transaction model exist.

Phase 2:

- `PromptTurnFlow` exists.

Phase 3:

- adapters use `CodingAgentSession` and `CodingAgentEvent`.

Phase 4:

- `AgentTurnFlow` exists for agent-loop-level extension.

Phase 5:

- plugin registry and capability-scoped host exist.

Not every advanced workflow requires all previous phases, but each workflow should use the same owner/context/transaction/event rules.

## Non-Negotiable Constraints

- New workflows use operation-scoped contexts.
- New workflows commit through `SessionService`.
- New workflows emit `CodingAgentEvent`.
- New workflows do not write `events.jsonl` directly.
- New workflows do not expose raw internal services to plugins.
- New workflows do not depend on TypeScript session formats.

## Common Workflow Pattern

Each advanced workflow should define:

```text
Operation options
Operation context
Flow graph
Typed outcome
Session event writes
CodingAgentEvent mapping
Capability requirement
Tests
```

Example:

```text
ManualCompactionOptions
ManualCompactionContext
ManualCompactionFlow
ManualCompactionOutcome
session.compaction.* events
RuntimeCompactionStarted/Completed or SessionCompactionStarted/Completed
CapabilityService::compact
manual_compaction_flow tests
```

## ManualCompactionFlow

Purpose:

- compact long-term session history as an explicit session operation.

This is different from runtime compaction inside `AgentTurnFlow`.

Nodes:

```text
start_compaction
load_session_replay
select_compaction_range
prepare_summary_context
run_summary_model
record_compaction_events
finalize_compaction
emit_completion
```

Session events:

```text
operation.started { operation: "session.compaction" }
session.compaction.started
message.started role="assistant" or summary-specific event
message.completed
session.compaction.completed
operation.committed
active_leaf.changed
```

Rules:

- failure must not modify old transcript view;
- compaction result creates a new leaf;
- original history remains in event log;
- replay can choose compacted view or full audit view.

Tests:

- compact range produces new leaf;
- failure leaves active leaf unchanged;
- replay can include summary event;
- prompt after compaction uses compacted context if feature is enabled.

Current P5 status:

- `ManualCompactionFlow` now exists as a Rust-native session workflow with `ManualCompactionOptions`, `ManualCompactionContext`, `ManualCompactionOutcome`, stable node IDs, `FlowService` execution helpers, and session-owned `CodingAgentSession::compact()` integration.
- The flow records `session.compaction.started` and `session.compaction.completed` through the manual-compaction transaction, then `SessionService` owns commit/fail finalization.
- Failure coverage proves summary/model failure writes an operation failure without `session.compaction.completed`, without replay folding, and without changing the active leaf.
- The P5 slice intentionally preserves existing active-leaf behavior. Producing a dedicated compaction leaf and splitting adapter-facing runtime/session compaction event names remain follow-up boundary cleanup.

## RuntimeCompactionFlow Boundary

Runtime compaction remains agent-turn-local:

- affects provider context;
- can emit `runtime.compaction.*` session events only if product policy asks for audit;
- does not rewrite session history;
- does not update active leaf by itself.

Keep this distinction explicit in code names:

- `RuntimeCompactionNode`;
- `SessionCompactionFlow`.

Avoid generic `CompactionFlow` names unless the type is only an umbrella.

## BranchSummaryFlow

Purpose:

- summarize abandoned branch work into a durable branch summary event.

Nodes:

```text
start_branch_summary
load_branch_events
select_abandoned_range
prepare_summary_prompt
run_summary_model
record_branch_summary
finalize_branch_summary
```

Session events:

```text
operation.started { operation: "branch.summary" }
branch.summary.created
operation.committed
```

Integration:

- uses `SessionService` replay/tree view;
- uses `RuntimeService` for model/provider;
- emits `CodingAgentEvent::Diagnostic` for skipped/no-op cases.

Tests:

- summary captures abandoned branch events;
- no abandoned branch returns a no-op outcome;
- provider failure records operation failed.

## ExportFlow

Purpose:

- export Rust-native session views to user-facing formats.

Supported exports can include:

- JSON event log copy;
- transcript Markdown;
- HTML viewer for Rust-native event log.

Not supported:

- TypeScript-compatible session export.

Nodes:

```text
start_export
load_session_replay
select_export_view
render_export
write_export
emit_completion
```

Rules:

- export is not a session mutation unless the product wants an audit event;
- export should not need direct storage internals outside `SessionService`;
- exported HTML/Markdown is a view, not canonical state.

Tests:

- export transcript from event log;
- export includes tool calls;
- export of incomplete/cancelled messages is clear;
- no TS-compatible session export is produced.

## PluginLoadFlow

Purpose:

- discover, validate, register, and report plugin capabilities.

Nodes:

```text
start_plugin_load
discover_plugins
validate_manifests
load_first_party_plugins
load_lua_plugins_later
register_capabilities
emit_diagnostics
finalize_plugin_load
```

Rules:

- plugin failures do not panic the runtime;
- plugin diagnostics go through `EventService`;
- plugin capability changes emit `CapabilityChanged`;
- plugin host remains capability-scoped.

Tests:

- valid plugin registers tool/command;
- invalid plugin emits diagnostic;
- failed plugin does not block unrelated plugins if policy is fail-open.

## Delegation-First Helper Orchestration

Purpose:

- let the current session `AgentProfile` request bounded help from other
  `AgentProfile` or `TeamProfile` entries through session-owned delegation.

Initial conservative model:

- top-level sessions remain owned by a single default `AgentProfile`;
- standalone helper product concepts are not introduced;
- built-in default profiles may expose read-only helper agents through
  auto-approved delegation;
- custom profiles must explicitly declare their delegation roster;
- delegated helpers receive minimal task packets rather than the parent
  transcript;
- delegation outputs become structured delegation results and folded transcript
  blocks, not direct parent session commits.

Nodes:

```text
capture_delegation_request
authorize_delegation
build_delegation_task_packet
run_delegated_agent_or_team
collect_delegation_result
record_delegation_events
render_folded_delegation_block
```

Rules:

- delegated flow cannot direct-commit parent session;
- parent transaction decides committed facts;
- IDs must correlate parent operation and delegated operation;
- delegated capability scope comes from the target profile, not the parent profile;
- delegated context defaults to the explicit task plus selected evidence only.

Tests:

- default read-only helpers are auto-approved;
- custom profiles expose no helpers unless explicitly configured;
- delegated helpers run deterministically;
- failed delegated run records diagnostic and parent policy applies;
- parent session log has coherent operation lineage;
- parent transcript records folded delegation results instead of delegated token streams.

## Self-Healing Edit Workflow

Purpose:

- turn edit/read/validate/apply/retry into a visible workflow.

Candidate nodes:

```text
start_edit_workflow
read_target
propose_patch
validate_patch
apply_patch
run_check
repair_patch
record_result
```

Integration:

- can wrap existing `edit` tool internals;
- should use `ExecutionEnv`;
- should record durable diagnostics/artifacts through transaction if invoked as product operation;
- tool invocation path can start as internal first-party flow, not public plugin flow.

Tests:

- successful edit;
- failed validation;
- repair retry;
- no direct filesystem outside `ExecutionEnv`.

## Workflow Registration

Advanced flows should register with `FlowService`:

```rust
impl FlowService {
    pub(crate) fn manual_compaction_flow(&self) -> Flow<ManualCompactionContext>;
    pub(crate) fn branch_summary_flow(&self) -> Flow<BranchSummaryContext>;
    pub(crate) fn export_flow(&self) -> Flow<ExportContext>;
    pub(crate) fn plugin_load_flow(&self) -> Flow<PluginLoadContext>;
}
```

Do not expose concrete flow structs through `api` unless a later public workflow API is deliberately designed.

## Capability Integration

Every advanced operation must declare capability requirements:

```text
compact
branch_summary
export
plugin_reload
delegation
self_healing_edit
```

Capabilities should consider:

- provider/model support;
- auth availability;
- filesystem/shell permissions;
- plugin trust;
- active operation busy state.

## Session Event Integration

Do not add ad hoc event shapes per workflow. Extend `SessionEventData` deliberately.

Guidelines:

- operation start/final marker for every mutating workflow;
- typed event family for durable facts;
- diagnostics for nonfatal issues;
- blob references for large artifacts;
- active leaf update only through `SessionService`.

## Tests

Recommended files:

```text
manual_compaction_flow.rs
branch_summary_flow.rs
export_flow.rs
plugin_load_flow.rs
delegation_execution.rs
agent_invocation_flow.rs
agent_team_flow.rs
self_healing_edit_flow.rs
```

Use faux providers and temp dirs.

Avoid real shell/network unless guarded behind explicit opt-in smoke tests.

## Phase 6 Handoff

Phase 6 is open-ended. Each advanced workflow should leave:

- an operation context;
- a flow graph;
- typed events;
- replay behavior if it writes session events;
- capability status;
- tests proving commit/fail/abort behavior.

## Stop Conditions

Stop and redesign if:

- advanced workflow bypasses `CodingAgentSession`;
- advanced workflow writes storage directly;
- delegated flow can mutate parent session without parent transaction;
- export becomes canonical state;
- plugin load gives Lua raw Flow graph mutation.

## Suggested Checks

Focused per workflow:

```text
cargo fmt --check
cargo test -p pi-coding-agent manual_compaction_flow
cargo test -p pi-coding-agent branch_summary_flow
cargo test -p pi-coding-agent export_flow
cargo test -p pi-coding-agent plugin_load_flow
```

Full after several workflows:

```text
cargo test --workspace
cargo check --workspace
```
