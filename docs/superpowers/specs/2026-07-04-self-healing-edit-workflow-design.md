# Self-Healing Edit Workflow Design

Date: 2026-07-04
Status: Accepted by standing user instruction to proceed with recommended decisions without review gates.

## Context

Phase 6 has completed the runtime-control readiness gate for new advanced workflows: prompt abort, steer, follow-up, owner-issued operation controls, ExportFlow, and runtime-vs-session compaction cleanup are in place. The remaining Phase 6 workflow called out in `docs/TODO.md` is self-healing edit.

The current `edit` tool already implements exact/fuzzy replacement, CRLF/BOM preservation, mutation queueing, diff generation, and patch details. It exposes an injectable `EditOperations` trait for tests, but the production implementation still calls `tokio::fs` directly. `pi-agent-core` already has `ExecutionEnv`, `FileSystem`, `Shell`, and `InMemoryExecutionEnv`, which are the intended capability boundary for future first-party tools and workflows.

## Goal

Add a session-owned internal `SelfHealingEditFlow` that turns edit/read/validate/apply/check/repair into an explicit Flow workflow without exposing raw session, runtime, provider, or filesystem internals.

The first implementation slice should be deliberately small:

- register a stable `SelfHealingEditFlow` in `FlowService`;
- wrap the existing edit validation/apply behavior through a flow context;
- run file reads and writes through an injectable operation boundary that can be backed by `ExecutionEnv`;
- return typed outcome data including changed path, message, diff, patch, first changed line, attempts, and diagnostics;
- cover successful edit, validation failure, stable node IDs, and no-direct-filesystem behavior with deterministic tests.

## Non-Goals

The first slice does not add public RPC, interactive, or plugin APIs. It does not ask models to propose repairs yet. It does not run real shell checks by default. It does not persist durable workflow events until the session-owner product operation entrypoint is added. It does not replace the existing `edit` tool in provider-visible tool lists until the flow wrapper is proven compatible.

## Considered Approaches

### A. Replace the existing edit tool with a full self-healing workflow immediately

This moves quickly toward the final product shape but has a large blast radius: provider-visible tool behavior, tests, filesystem access, diagnostics, and potential shell checks all change at once.

### B. Add an internal Flow wrapper first, then migrate callers

This preserves current edit behavior while creating the Flow boundary, stable node IDs, test fixtures, and operation abstraction needed for self-healing. It lets later slices add repair/check/persistence without rewriting the tool twice.

### C. Add a new public `/edit` or RPC command first

This gives users visible functionality early but risks adapter-specific business logic before the session-owned workflow exists.

## Decision

Use approach B. Build the internal Flow wrapper first. The flow becomes the product-owned workflow boundary; adapters and plugins can be added after the core behavior is testable and capability-gated.

## Architecture

Add `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs` with:

- `SelfHealingEditOptions`: cwd, path, replacements, optional check command, max repair attempts, and an injectable edit operation backend.
- `SelfHealingEditContext`: mutable workflow state, diagnostics, tool output details, attempt count, and failure error.
- `SelfHealingEditOutcome`: changed path, user-facing message, diff, patch, first changed line, attempts, diagnostics, and optional check output.
- `SelfHealingEditFlow`: stable-node Flow wrapper registered by `FlowService`.

Initial stable node IDs:

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

The first slice implements deterministic behavior in these nodes:

- `start_edit_workflow`: validate cwd/path/edit input shape and reset attempt state.
- `read_target`: perform an explicit read through the operation backend to prove the workflow observes the target through the capability boundary.
- `propose_patch`: materialize the existing replacement request as the initial patch proposal.
- `validate_patch`: reject empty edits and malformed replacement data before mutation.
- `apply_patch`: call the existing edit application path through injected operations.
- `run_check`: no-op unless a check command is configured; shell execution is deferred.
- `repair_patch`: no-op unless validation/apply/check failed and repair is configured; model-driven repair is deferred.
- `record_result`: convert edit output details into `SelfHealingEditOutcome`.

## Operation Boundary

The flow should not call `std::fs` or `tokio::fs` directly. The initial production backend can adapt the existing edit operation trait, but the workflow API must allow an `ExecutionEnv`-backed adapter so tests can prove reads and writes happen through `InMemoryExecutionEnv`.

The first target is an adapter from `Arc<dyn ExecutionEnv>` to the edit operation boundary. The provider-visible builtin `edit` tool now routes through `SelfHealingEditFlow` while preserving `edit_execute()` and `edit_execute_with_operations()` as low-level compatibility entrypoints for direct callers and focused algorithm tests.

## Data Flow

1. Caller creates `SelfHealingEditOptions` with cwd, path, and replacements.
2. `FlowService::run_self_healing_edit()` builds and runs the stable graph.
3. The context validates the request and reads the target through the configured operation backend.
4. The context applies the existing edit algorithm through injected operations.
5. The final node extracts `diff`, `patch`, and `firstChangedLine` from `AgentToolOutput.details` into a typed outcome.

## Error Handling

Validation and apply errors return `CodingSessionError::Session` with a concise message and are also captured as workflow diagnostics before the failure is surfaced. The flow should not partially report success if apply fails. Check failures and repair failures are future slices; when added, they should record diagnostics and only return success if the final file state satisfies the configured check policy.

## Capability and Persistence Direction

After the internal flow exists, add `self_healing_edit` to `CodingAgentCapabilities`. Persistent session owners should report it busy during active operations and disabled for non-persistent sessions until durable event recording is implemented.

A later session-owner entrypoint should wrap the flow in an `OperationKind::SelfHealingEdit`, record operation start/final markers, and persist typed workflow result/diagnostic events or artifact references. The first slice intentionally stops before that owner API to keep the change small and testable.

## Tests

Required first-slice coverage:

- `self_healing_edit_flow_node_ids_are_stable` verifies the exact node list.
- `self_healing_edit_flow_applies_successful_edit` verifies changed path, message, diff, patch, first changed line, and attempts.
- `self_healing_edit_flow_reports_validation_failure_without_write` verifies malformed or empty replacements fail before mutation.
- `self_healing_edit_flow_uses_execution_env_operations` verifies an `InMemoryExecutionEnv` receives read/write calls and the local filesystem is not used.

Later coverage:

- check command success/failure;
- model repair retry after validation/apply/check failure;
- durable session event recording and replay visibility;
- RPC/interactive adapter rendering;
- RPC/interactive adapter rendering and command exposure.
