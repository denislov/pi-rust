---
phase: 03-production-adapter-convergence
plan: 03
subsystem: protocol-adapter
tags: [rust, canonical-operations, rpc, mutation-commands, plugin-operations, boundary-guards, adapter-convergence]

requires:
  - phase: 03-production-adapter-convergence
    plan: 02
    provides: Select-driven RPC background operations (prompt, agent, team, delegation approval) routed through CodingAgentSession::run with preserved concurrency and control topology
  - phase: 02-canonical-facade-correctness
    plan: 03
    provides: Canonical run durability, exhaustive public outcome projection, and closed facade ledger
provides:
  - RPC self-healing edit, default-profile mutation, delegation rejection, plugin load, and plugin command execute through CodingAgentSession::run(CodingAgentOperation) with exhaustive outcome extraction
  - RPC production source is free of replaced broad workflow calls and #[allow(deprecated)] suppression, enforced by a narrow executable source guard
affects: [interactive-migration, phase-04, phase-05]

tech-stack:
  added: []
  patterns:
    - "Short-lived mutation canonical-call pattern: take() session ownership, subscribe, run(CodingAgentOperation::<Variant>(...)).await, extract expected CodingAgentOperationOutcome variant with unreachable! on impossible variants, drain events, restore session owner on every success and error path, write response, write drained events, mark idempotency complete"
    - "Narrow RPC source guard pattern: sanitize_rust_source + line_is_cfg_test_gated scanning src/protocol/rpc/ for replaced workflow method calls (both deprecated and non-deprecated) and production #[allow(deprecated)] attributes"

key-files:
  created: []
  modified:
    - crates/pi-coding-agent/src/protocol/rpc/commands.rs
    - crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs

key-decisions:
  - "Import CodingAgentOperation, CodingAgentOperationOutcome, and CodingAgentPluginLoadOutcome through crate::api per D-16, matching the JSON/print and RPC background operation patterns from 03-01/03-02."
  - "Switch handle_set_default_agent_profile and handle_reject_delegation from the ensure_mutable_coding_session borrow pattern to the take()/restore ownership pattern because run() is async and requires &mut session across an .await boundary; remove the now-unused ensure_mutable_coding_session helper."
  - "Drain product events and restore the session owner on every error path in all five mutation handlers, matching the self-healing edit pattern and the plan's must_have requiring owner restoration on all operation and projection errors."
  - "Update rpc_plugin_reload_data to accept &CodingAgentPluginLoadOutcome (the public projection) instead of the internal &PluginLoadOutcome, and remove the now-unused PluginLoadOutcome import."
  - "Scope the RPC source guard to 14 replaced workflow methods: 9 deprecated broad methods plus 5 non-deprecated methods (approve_delegation_confirmation, reject_delegation_confirmation, set_default_agent_profile_id, reload_plugins, run_plugin_command) that are replaced by canonical operations at the adapter boundary."

patterns-established:
  - "Short-lived mutation canonical-call pattern: replace session.<broad>(...) with session.run(CodingAgentOperation::<Variant>(...)).await, extract the expected outcome variant, and preserve the existing validation, idempotency, response, event-drain, and owner-restoration shell."
  - "RPC source guard pattern: rust_files_under + sanitize_rust_source + line_is_cfg_test_gated scanning src/protocol/rpc/ for both deprecated and non-deprecated replaced workflow method calls plus production #[allow(deprecated)] suppression."

requirements-completed: [RPC-02, RPC-03, RPC-04]

coverage:
  - id: D1
    description: "RPC self-healing edit executes through CodingAgentSession::run(CodingAgentOperation::SelfHealingEdit) with preserved diagnostic/check/repair fields, error data, event drain, idempotency, and session owner restoration on every path."
    requirement: RPC-02
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_self_healing_edit_applies_edit_through_persistent_session"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_self_healing_edit_runs_check_command"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_self_healing_edit_failed_check_returns_check_output"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_self_healing_edit_uses_planned_repair_attempts"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_self_healing_edit_repair_exhaustion_returns_repair_attempts"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_self_healing_edit_uses_model_repair_policy"
        status: pass
    human_judgment: false
  - id: D2
    description: "RPC default-profile mutation executes through CodingAgentSession::run(CodingAgentOperation::SetDefaultAgentProfile) with preserved profile validation, persistence, event drain, idempotency, and session owner restoration."
    requirement: RPC-02
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_set_default_agent_profile_updates_session_listing"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_set_default_agent_profile_rejects_unknown_profile"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_set_default_agent_profile_rejects_while_prompt_running"
        status: pass
    human_judgment: false
  - id: D3
    description: "RPC delegation rejection executes through CodingAgentSession::run(CodingAgentOperation::RejectDelegation) with preserved pending lookup, default reason, response shape, event drain, idempotency, and session owner restoration."
    requirement: RPC-02
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_rejects_delegation_confirmation"
        status: pass
    human_judgment: false
  - id: D4
    description: "RPC plugin load (reload) executes through CodingAgentSession::run(CodingAgentOperation::PluginLoad) with preserved reload diagnostics, loaded plugin IDs, capability-changed flag, and error protocol."
    requirement: RPC-02
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_reload_reports_project_plugin_manifest_diagnostics"
        status: pass
    human_judgment: false
  - id: D5
    description: "RPC plugin command executes through CodingAgentSession::run(CodingAgentOperation::PluginCommand) with preserved conditional load-before-first-command behavior, commandId/output fields, and error protocol."
    requirement: RPC-02
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/rpc_mode.rs#rpc_plugin_command_runs_loaded_lua_plugin_command"
        status: pass
    human_judgment: false
  - id: D6
    description: "RPC production source contains no replaced broad workflow calls or #[allow(deprecated)] suppression across all src/protocol/rpc/ files, enforced by the production_rpc_uses_canonical_operations boundary guard; the bounded RpcProductEventQueue assertion remains green."
    requirement: RPC-04
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs#production_rpc_uses_canonical_operations"
        status: pass
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

duration: 9 min
completed: 2026-07-12
status: complete
---

# Phase 03 Plan 03: RPC Mutation Command Convergence Summary

**All five short-lived RPC mutation commands (self-healing edit, default-profile mutation, delegation rejection, plugin load, plugin command) now route through CodingAgentSession::run(CodingAgentOperation) with exhaustive outcome extraction, and a narrow source guard locks canonical operations across src/protocol/rpc/.**

## Performance

- **Duration:** 9 min
- **Started:** 2026-07-11T20:12:05Z
- **Completed:** 2026-07-11T20:21:41Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Migrated `handle_self_healing_edit` from `session.self_healing_edit_with_options(request)` to `session.run(CodingAgentOperation::SelfHealingEdit(request))` with exhaustive `CodingAgentOperationOutcome::SelfHealingEdit` extraction, preserving the `#[allow(deprecated)]` removal, diagnostic/check/repair response fields, structured error data, event drain, idempotency lifecycle, and session owner restoration on success and error paths.
- Migrated `handle_set_default_agent_profile` from the `ensure_mutable_coding_session()` borrow pattern calling `session.set_default_agent_profile_id(...)` to the `take()`/restore ownership pattern calling `session.run(CodingAgentOperation::SetDefaultAgentProfile { profile_id })` with exhaustive `CodingAgentOperationOutcome::DefaultAgentProfileChanged` extraction, preserving profile validation, response JSON, event drain, idempotency, and owner restoration on every path; removed the now-unused `ensure_mutable_coding_session` helper.
- Migrated `handle_reject_delegation` from the `self.coding_session.as_mut()` borrow pattern calling `session.reject_delegation_confirmation(...)` to the `take()`/restore ownership pattern calling `session.run(CodingAgentOperation::RejectDelegation { operation_id, tool_call_id, reason })` with exhaustive `CodingAgentOperationOutcome::DelegationRejected` extraction, preserving pending delegation lookup, default reason, response shape, event drain, idempotency, and owner restoration on every path.
- Migrated `handle_reload` from `session.reload_plugins()` to `session.run(CodingAgentOperation::PluginLoad)` with exhaustive `CodingAgentOperationOutcome::PluginLoad` extraction, preserving reload diagnostics, loaded plugin IDs, capability-changed flag, and error protocol.
- Migrated `handle_plugin_command` from `session.reload_plugins()` (conditional load) and `session.run_plugin_command(...)` to `session.run(CodingAgentOperation::PluginLoad)` and `session.run(CodingAgentOperation::PluginCommand { command_id, args })` with exhaustive outcome extraction, preserving the conditional load-before-first-command behavior, commandId/output response fields, and error protocol.
- Updated `rpc_plugin_reload_data` to accept `&CodingAgentPluginLoadOutcome` (the public projection) instead of the internal `&PluginLoadOutcome`; removed the now-unused `PluginLoadOutcome` import from commands.rs.
- Added the `production_rpc_uses_canonical_operations` boundary guard covering all `.rs` files under `src/protocol/rpc/`, sanitizing Rust source (stripping comments and string contents), skipping `#[cfg(test)]` modules, and reporting file:line for any replaced broad workflow method call (14 methods: 9 deprecated + 5 non-deprecated replacements) or production `#[allow(deprecated)]` attribute.
- Closed the combined RPC gate: 40 `rpc_mode` tests, 3 `protocol_sessions` tests, 11 `product_runtime_boundary_guards` tests (including the new RPC source guard and the retained bounded `RpcProductEventQueue` assertion), `cargo check -p pi-coding-agent`, `cargo fmt --check`, and `git diff --check` are all green together.

## Task Commits

Each task was committed atomically:

1. **Task 1: Migrate self-healing, profile, and rejection commands** - `9d114c8` (feat)
2. **Task 2: Migrate plugin load/command and close the RPC source gate** - `21b9bf5` (feat)

## Files Created/Modified

- `crates/pi-coding-agent/src/protocol/rpc/commands.rs` - All five short-lived RPC mutation handlers (self-healing edit, set-default-profile, reject-delegation, plugin command, reload) now call `session.run(CodingAgentOperation::...)` with exhaustive `CodingAgentOperationOutcome` extraction; imports operation types via `crate::api`; `#[allow(deprecated)]` removed from `handle_self_healing_edit`; `ensure_mutable_coding_session` helper removed; `rpc_plugin_reload_data` accepts `&CodingAgentPluginLoadOutcome`; `PluginLoadOutcome` import removed.
- `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` - New `production_rpc_uses_canonical_operations` test enforcing canonical operations and no production deprecation suppression across all RPC adapter source files.

## Decisions Made

- Imported `CodingAgentOperation`, `CodingAgentOperationOutcome`, and `CodingAgentPluginLoadOutcome` through `crate::api` per D-16, matching the JSON/print adapter pattern from 03-01 and the RPC background operation pattern from 03-02, while leaving existing concrete type imports from `crate::coding_session` untouched to keep the diff minimal.
- Switched `handle_set_default_agent_profile` and `handle_reject_delegation` from the `ensure_mutable_coding_session()` / `self.coding_session.as_mut()` borrow pattern to the `take()`/restore ownership pattern because `run()` is async and requires `&mut session` across an `.await` boundary. The `ensure_mutable_coding_session` helper was removed as it became unused after the migration.
- Added event draining and session owner restoration to the error paths of `handle_set_default_agent_profile` and `handle_reject_delegation`, matching the self-healing edit pattern and the plan's must_have requiring "Every success and error path restores the same session owner after preserving validation, idempotency, response fields, and event-drain order." The previous code did not drain events on error because the synchronous methods were unlikely to emit events before failing; the canonical `run()` path goes through full admission, so draining on error is the correct invariant.
- Updated `rpc_plugin_reload_data` to accept `&CodingAgentPluginLoadOutcome` (the public projection type) instead of the internal `&PluginLoadOutcome`, keeping the wire rendering in the existing RPC helper while the capability internals stay private per T-03-10.
- Scoped the RPC source guard to 14 replaced workflow methods: the 9 deprecated broad methods already covered by the JSON/print guard plus 5 non-deprecated methods (`approve_delegation_confirmation`, `reject_delegation_confirmation`, `set_default_agent_profile_id`, `reload_plugins`, `run_plugin_command`) that are replaced by canonical operations at the RPC adapter boundary. This matches the plan's "rejects session-receiver compatibility calls" requirement.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## Known Stubs

None. The `unreachable!` branches are exhaustive invariant handling for the closed `CodingAgentOperationOutcome` enum (T-03-04 accept), not placeholder behavior.

## Threat Flags

None. The migration introduces no new network endpoints, auth paths, file access patterns, or trust-boundary schema changes. The plan's threat register (T-03-09 through T-03-12) is addressed: canonical admission after existing validation with typed operations mitigates T-03-09; public plugin operations and curated outcomes keep services/registries private per T-03-10; owner restoration and event drain on every path with retained structured tests mitigate T-03-11; preserved typed input errors and exact outcome invariants without retry loops mitigate T-03-12.

## User Setup Required

None - all verification uses deterministic offline faux providers and tempfile sessions.

## Next Phase Readiness

- RPC-02, RPC-03, and RPC-04 are complete: every RPC product operation (prompt, agent, team, delegation approval, self-healing edit, default-profile mutation, delegation rejection, plugin load, plugin command) executes through `CodingAgentSession::run(CodingAgentOperation)` while the JSONL client observes the same command responses, errors, events, controls, and session state.
- The RPC adapter boundary is fully converged (D-07 through D-10 and D-16 through D-20) with both the select-driven background operations (03-02) and the short-lived mutation commands (03-03) green together, unblocking interactive migration (Plans 03-04 through 03-06).
- The narrow source guard now covers JSON/print (03-01) and RPC (03-03); extending it to interactive source is the natural follow-up for Plan 03-06 or Phase 5.

## Self-Check: PASSED

- Both modified files exist on disk: `crates/pi-coding-agent/src/protocol/rpc/commands.rs` and `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs`.
- Commits `9d114c8` (Task 1) and `21b9bf5` (Task 2) exist in repository history.
- `cargo test -p pi-coding-agent --test rpc_mode` (40 tests) and `cargo test -p pi-coding-agent --test protocol_sessions` (3 tests) all pass with 0 failures.
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards` (11 tests, including `production_rpc_uses_canonical_operations`) all pass with 0 failures.
- `cargo check -p pi-coding-agent`, `cargo fmt --check`, and `git diff --check` all pass.
- The RPC production source (`src/protocol/rpc/`) contains 0 replaced broad workflow method calls and 0 `#[allow(deprecated)]` attributes.
- No tracked files were deleted by any task commit, and no new production threat surface or dependency was introduced.

---
*Phase: 03-production-adapter-convergence*
*Completed: 2026-07-12*
