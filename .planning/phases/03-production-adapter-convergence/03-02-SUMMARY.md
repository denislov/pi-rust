---
phase: 03-production-adapter-convergence
plan: 02
subsystem: protocol-adapter
tags: [rust, canonical-operations, rpc, select-driven, background-operations, adapter-convergence]

requires:
  - phase: 03-production-adapter-convergence
    plan: 01
    provides: Canonical run operation pattern for adapter prompt migration with exhaustive outcome extraction
  - phase: 02-canonical-facade-correctness
    plan: 03
    provides: Canonical run durability, exhaustive public outcome projection, and closed facade ledger
provides:
  - RPC prompt, agent, team, and delegation approval background operations route through CodingAgentSession::run(CodingAgentOperation) with preserved select-driven concurrency topology
  - All four RPC background operation kinds use exhaustive CodingAgentOperationOutcome extraction with unreachable! on impossible variants
  - RPC production source is free of deprecated broad workflow calls and #[allow(deprecated)] suppression
affects: [rpc-mutation-convergence, interactive-migration, phase-04, phase-05]

tech-stack:
  added: []
  patterns:
    - "Select-driven canonical call pattern: Box::pin(session.run(CodingAgentOperation::<Variant>(...))) inside tokio::spawn, polled via tokio::select! alongside bounded event forwarding, then outcome extracted with .map_err(CliError::from).and_then(|o| match o { Variant(o) => Ok(o), _ => unreachable!(...) })"
    - "Exhaustive public outcome extraction at the select-driven boundary: each break extracts the expected CodingAgentOperationOutcome variant and treats impossible variants as internal invariants"

key-files:
  created: []
  modified:
    - crates/pi-coding-agent/src/protocol/rpc/prompt.rs

key-decisions:
  - "Import CodingAgentOperation and CodingAgentOperationOutcome through crate::api per D-16, matching the JSON/print adapter pattern from 03-01, while leaving existing concrete type imports from crate::coding_session unchanged."
  - "Treat an unexpected public outcome variant as an internal invariant (unreachable!) rather than a new user-visible error, matching T-03-04 accept disposition, the closed-enum discipline from Phase 2, and the 03-01 JSON/print pattern."
  - "Preserve the Box::pin(session.run(...)) borrow pattern exactly as Box::pin(session.<broad>(...)) since both run and the deprecated broad methods take &mut self; the pinned future borrows session, releases on completion, and the owner is restored through CodingOperationTaskResult."
  - "Remove all three #[allow(deprecated)] attributes (handle_invoke_agent, handle_invoke_team, start_coding_session_prompt) now that the deprecated broad calls are gone; handle_approve_delegation needed no removal because approve_delegation_confirmation was never deprecated."
  - "Close the independent D-10 select-driven boundary in Task 2 with the complete RPC background behavior suite (40 rpc_mode + 3 protocol_sessions tests) rather than extending the production source guard, which remains scoped to JSON/print per the 03-01 plan."

patterns-established:
  - "Select-driven canonical-call pattern: replace Box::pin(session.<broad>(opts)) with Box::pin(session.run(CodingAgentOperation::<Variant>(opts))) inside the spawned task, preserving every tokio::select! branch, guard, bounded queue, overflow recovery, replay cursor, final drain, idempotency lifecycle, and owner restoration."
  - "Outcome extraction at the select boundary: break outcome.map_err(CliError::from).and_then(|operation_outcome| match operation_outcome { CodingAgentOperationOutcome::<Expected>(outcome) => Ok(outcome), _ => unreachable!(...) }) produces the same Result<T, CliError> the RPC-local CodingOperationOutcome variant expects."

requirements-completed: [RPC-01, RPC-03]

coverage:
  - id: D1
    description: "RPC prompt executes through CodingAgentSession::run(CodingAgentOperation::Prompt) with preserved response-before-events ordering, live event delivery before provider release, abort/steer/follow-up control routing, final product event drain, and persistent/disabled session behavior."
    requirement: RPC-01
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_prompt_returns_response_then_agent_events"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_streams_agent_events_before_prompt_finishes"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_abort_cancels_running_prompt"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_steer_while_coding_prompt_running_sends_control"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_follow_up_prompt_while_coding_prompt_running_sends_control"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/protocol_sessions.rs#rpc_prompt_persists_session_messages"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/protocol_sessions.rs#rpc_state_reports_persisted_session_path_after_prompt"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/protocol_sessions.rs#rpc_disabled_session_prompt_uses_non_persistent_runtime_without_session_files"
        status: pass
    human_judgment: false
  - id: D2
    description: "RPC agent invocation executes through CodingAgentSession::run(CodingAgentOperation::InvokeAgent) with preserved response fields (profileId, task), event families (agent_invocation_start/end), busy state, and idempotency behavior."
    requirement: RPC-01
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_invoke_agent_returns_response_then_agent_events"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_invoke_agent_rejects_unknown_profile"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_state_reports_agent_invocation_busy_while_running"
        status: pass
    human_judgment: false
  - id: D3
    description: "RPC team invocation executes through CodingAgentSession::run(CodingAgentOperation::InvokeTeam) with preserved response fields (teamId, task), event families (agent_team_start/member_start/member_end/end), busy state, and idempotency behavior."
    requirement: RPC-01
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_invoke_team_returns_response_then_agent_events"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_invoke_team_rejects_unknown_team"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_state_reports_agent_team_busy_while_running"
        status: pass
    human_judgment: false
  - id: D4
    description: "RPC delegation approval executes through CodingAgentSession::run(CodingAgentOperation::ApproveDelegation) with preserved dynamic operation kind resolution (agent vs team from pending target_kind), pending delegation lookup, response shape, durable decision semantics, and delegation_completed event ordering."
    requirement: RPC-03
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_lists_and_approves_delegation_confirmation"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_rejects_delegation_confirmation"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_approve_delegation_rejects_unknown_pending_request"
        status: pass
    human_judgment: false
  - id: D5
    description: "RPC production source contains no deprecated broad workflow calls or #[allow(deprecated)] suppression in the select-driven background operation paths; the bounded event queue, overflow recovery, and event-service projection guards remain green."
    requirement: RPC-03
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs#rpc_running_product_events_do_not_use_unbounded_channels"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs#adapters_do_not_access_event_service_directly_for_projection"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs#canonical_operation_facade_has_no_new_workflow_wrappers"
        status: pass
    human_judgment: false

duration: 6 min
completed: 2026-07-12
status: complete
---

# Phase 03 Plan 02: RPC Background Operation Convergence Summary

**All four select-driven RPC background operations (prompt, agent, team, delegation approval) now route through CodingAgentSession::run(CodingAgentOperation) with exhaustive outcome extraction and unchanged concurrency, control, queue, replay, idempotency, and session semantics.**

## Performance

- **Duration:** 6 min
- **Started:** 2026-07-11T20:01:48Z
- **Completed:** 2026-07-11T20:08:33Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments

- Migrated the RPC prompt background future in `start_coding_session_prompt` from `session.prompt(prompt_options)` to `session.run(CodingAgentOperation::Prompt(prompt_options))` with exhaustive `CodingAgentOperationOutcome::Prompt` extraction, preserving the pinned future, every `tokio::select!` branch/guard, `PromptControlHandle` outside the operation, response-before-`AgentStart` ordering, bounded `RpcProductEventQueue` with overflow recovery, replay/applied sequence cursors, completion drain, idempotency lifecycle, and session owner restoration.
- Migrated the RPC agent invocation background future in `handle_invoke_agent` from `session.invoke_agent(invocation_options)` to `session.run(CodingAgentOperation::InvokeAgent(invocation_options))` with exhaustive `CodingAgentOperationOutcome::AgentInvocation` extraction, preserving pre-validation, profile lookup, response data (`profileId`, `task`), event families, busy/idempotency behavior, event queue/replay state, final drain, and owner restoration.
- Migrated the RPC team invocation background future in `handle_invoke_team` from `session.invoke_team(team_options)` to `session.run(CodingAgentOperation::InvokeTeam(team_options))` with exhaustive `CodingAgentOperationOutcome::AgentTeam` extraction, preserving pre-validation, team lookup, response data (`teamId`, `task`), event families, busy/idempotency behavior, event queue/replay state, final drain, and owner restoration.
- Migrated the RPC delegation approval background future in `handle_approve_delegation` from `session.approve_delegation_confirmation(operation_id, tool_call_id)` to `session.run(CodingAgentOperation::ApproveDelegation { operation_id, tool_call_id })` with exhaustive `CodingAgentOperationOutcome::DelegationApproved` extraction, preserving dynamic operation kind resolution (agent vs team from `pending.target_kind`), pending delegation lookup, response shape, durable decision semantics, event queue/replay state, final drain, and owner restoration.
- Removed all three `#[allow(deprecated)]` attributes from `handle_invoke_agent`, `handle_invoke_team`, and `start_coding_session_prompt` now that the deprecated broad workflow calls are gone; imported `CodingAgentOperation` and `CodingAgentOperationOutcome` through `crate::api` per D-16.
- Closed the independent D-10 select-driven boundary: the complete `rpc_mode` suite (40 tests), `protocol_sessions` suite (3 tests), `product_runtime_boundary_guards` suite (10 tests), `cargo check -p pi-coding-agent`, `cargo fmt --check`, and `git diff --check` are all green together.

## Task Commits

Each task was committed atomically:

1. **Task 1: Migrate RPC prompt without changing select/control semantics** - `9aa17bc` (feat)
2. **Task 2: Migrate agent, team, and delegation approval background operations** - `ff92cb5` (feat)

## Files Created/Modified

- `crates/pi-coding-agent/src/protocol/rpc/prompt.rs` - All four select-driven RPC background operations (prompt, agent, team, delegation approval) now call `session.run(CodingAgentOperation::...)` with exhaustive `CodingAgentOperationOutcome` extraction; imports operation types via `crate::api`; three `#[allow(deprecated)]` attributes removed from production source.

## Decisions Made

- Imported `CodingAgentOperation` and `CodingAgentOperationOutcome` through `crate::api` per D-16, matching the JSON/print adapter pattern from 03-01, while leaving existing concrete type imports (`AgentInvocationOptions`, `AgentTeamOptions`, `PromptTurnOptions`, etc.) from `crate::coding_session` untouched to keep the diff minimal.
- Used `unreachable!` for the impossible `CodingAgentOperationOutcome` variant in each of the four background operations, matching the closed-enum discipline established in Phase 2, the T-03-04 accept disposition, and the 03-01 JSON/print pattern rather than introducing a new user-visible error string.
- Preserved the `Box::pin(session.run(...))` borrow pattern exactly as `Box::pin(session.<broad>(...))` since both `run` and the deprecated broad methods take `&mut self`; the pinned future borrows `session`, releases the borrow on completion, and the owner is restored through `CodingOperationTaskResult`.
- Removed `#[allow(deprecated)]` from `handle_invoke_agent`, `handle_invoke_team`, and `start_coding_session_prompt` since the deprecated broad calls are gone; `handle_approve_delegation` needed no removal because `approve_delegation_confirmation` was never deprecated (per the Phase 1 audit decision).
- Did not extend the `production_json_and_print_use_canonical_operations` source guard to RPC files in this plan; the D-10 boundary is closed by the complete RPC background behavior suite plus crate check, and the guard extension to RPC source is deferred to a later convergence plan.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## Known Stubs

None. The `unreachable!` branches are exhaustive invariant handling for the closed `CodingAgentOperationOutcome` enum (T-03-04 accept), not placeholder behavior.

## Threat Flags

None. The migration introduces no new network endpoints, auth paths, file access patterns, or trust-boundary schema changes. The plan's threat register (T-03-05 through T-03-08) is addressed: canonical admission after existing validation with dynamic delegation target resolution preserved mitigates T-03-05; bounded queue, response/event ordering, sequence cursors, overflow recovery, and final drains unchanged and behavior-tested mitigate T-03-06; `PromptControlHandle` and existing `tokio::select!` branches retained outside the operation mitigate T-03-07; exact wire fields and existing typed error conversion preserved with no new diagnostics mitigate T-03-08.

## User Setup Required

None - all verification uses deterministic offline faux providers and tempfile sessions.

## Next Phase Readiness

- RPC-01 and RPC-03 are behaviorally covered for prompt, agent, team, and delegation approval: every select-driven RPC background operation executes through `CodingAgentSession::run` while clients observe the same responses, events, controls, idempotency, and session behavior.
- The select-driven background operation boundary (D-07/D-08/D-10) is independently closed with the full `rpc_mode` and `protocol_sessions` suites plus crate check, formatting, and diff checks, unblocking Plan 03 (RPC mutation command convergence).
- The narrow source guard from 03-01 remains scoped to JSON/print; extending it to RPC source is a natural follow-up for a later convergence plan but is not required for the D-10 gate.

## Self-Check: PASSED

- The modified file `crates/pi-coding-agent/src/protocol/rpc/prompt.rs` exists on disk.
- Commits `9aa17bc` (Task 1) and `ff92cb5` (Task 2) exist in repository history.
- `cargo test -p pi-coding-agent --test rpc_mode` (40 tests), `cargo test -p pi-coding-agent --test protocol_sessions` (3 tests), and `cargo test -p pi-coding-agent --test product_runtime_boundary_guards` (10 tests) all pass with 0 failures.
- `cargo check -p pi-coding-agent`, `cargo fmt --check`, and `git diff --check` all pass.
- The RPC prompt.rs production source contains 0 deprecated broad workflow calls (`.prompt(`, `.invoke_agent(`, `.invoke_team(`, `.approve_delegation_confirmation(`) and 0 `#[allow(deprecated)]` attributes.
- No tracked files were deleted by any task commit, and no new production threat surface or dependency was introduced.

---
*Phase: 03-production-adapter-convergence*
*Completed: 2026-07-12*
