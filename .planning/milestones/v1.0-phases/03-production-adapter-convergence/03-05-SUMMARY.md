---
phase: 03-production-adapter-convergence
plan: 05
subsystem: interactive-adapter
tags: [rust, canonical-operations, interactive, tui, profile-mutation, delegation-rejection, async-tasks, adapter-convergence]

requires:
  - phase: 03-production-adapter-convergence
    plan: 04
    provides: Interactive background operation convergence with all nine ordinary workflows routed through CodingAgentSession::run
  - phase: 03-production-adapter-convergence
    plan: 03
    provides: RPC mutation command convergence with take/restore ownership pattern and source guard
provides:
  - Interactive default-profile mutation and delegation rejection execute through CodingAgentSession::run(CodingAgentOperation) with preserved TUI menus, dialogs, queues, errors, events, persistence, projections, and owner state
  - Focused Wave 0 default-profile persistence test locking canonical mutation, visible projection, manifest persistence, and reopen behavior
  - Delegation rejection fallback notice preserved through the async task lifecycle when no ProductEvents arrive
affects: [interactive-navigation-convergence, phase-04, phase-05]

tech-stack:
  added: []
  patterns:
    - "Interactive mutation canonical-call pattern: take() session ownership, spawn PromptTask with Box::pin(session.run(CodingAgentOperation::<Variant>(...))) inside tokio::select! with abort/event-forwarding, extract expected outcome with unreachable! on impossible variants, drain remaining events, return session through PromptTaskResult, restore owner through finish_prompt"
    - "Fallback notice preservation pattern: task tracks whether any ProductEvents were forwarded; if none, includes a fallback SystemNotice string in the task result; finish_prompt pushes it to the transcript only when no events arrived, matching the previous synchronous drain + empty-check semantics"

key-files:
  created: []
  modified:
    - crates/pi-coding-agent/src/interactive/prompt_task.rs
    - crates/pi-coding-agent/src/interactive/loop.rs
    - crates/pi-coding-agent/src/interactive/event_bridge.rs
    - crates/pi-coding-agent/tests/interactive_sessions.rs

key-decisions:
  - "Import CodingAgentOperation and CodingAgentOperationOutcome through crate::api per D-16, matching the 03-01/03-02/03-03/03-04 adapter patterns."
  - "Use the existing owner-returning PromptTask lifecycle for both profile mutation and delegation rejection, matching the delegation approval pattern from 03-04: take() session, spawn task, forward events, return session through PromptTaskResult, restore through finish_prompt."
  - "Track had_events in the delegation rejection task to determine the fallback notice: if no ProductEvents were forwarded during the operation or drain, include a fallback SystemNotice string in DelegationRejectionTaskResult; finish_prompt pushes it to the transcript only when had_events is false, preserving the previous CodingEventBridge drain + ui_events.is_empty() semantics."
  - "Sync prompt_context.default_agent_profile_id from the restored session after every task completion in the main loop, not just after profile mutation. This is safe because the session's default is the source of truth and the sync is a no-op for tasks that don't change the default."
  - "Preserve the root's local projection setter (set_default_agent_profile_id) and its immediate call in handle_profile_menu_input, matching the plan's 'keep the root local projection setter intact' requirement. The root's default_agent_profile_id is local UI selection state; the canonical session mutation is the runtime confirmation."
  - "Mark handle_product_event with #[allow(dead_code)] since the only production caller (the old synchronous rejection code) has migrated to canonical operations; the method is retained for bridge unit tests."
  - "Remove the now-unused CodingEventBridge import from loop.rs since the rejection no longer creates a local bridge instance; event conversion is handled by the existing UiProjection in the main loop."

patterns-established:
  - "Interactive mutation canonical-call pattern: replace synchronous session.<mut_method>(...) with session.run(CodingAgentOperation::<Variant>(...)) inside a spawned PromptTask, preserving the take/spawn/forward/drain/return/restore lifecycle."
  - "Fallback notice preservation: track ProductEvent forwarding in the task, include a fallback string in the result when no events arrive, and push it to the transcript in finish_prompt."

requirements-completed: [INTER-02, INTER-04]

coverage:
  - id: D1
    description: "Interactive default-profile mutation executes through CodingAgentSession::run(CodingAgentOperation::SetDefaultAgentProfile) with preserved menu selection, visible projection, manifest persistence, prompt-context sync, and reopen behavior."
    requirement: INTER-02
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_sessions.rs#scripted_interactive_default_profile_selection_persists_and_refreshes_projection"
        status: pass
    human_judgment: false
  - id: D2
    description: "Interactive delegation rejection executes through CodingAgentSession::run(CodingAgentOperation::RejectDelegation) with preserved pending delegation lookup, reason text, fallback notice, event projection, and session owner restoration."
    requirement: INTER-04
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_mode.rs#scripted_interactive_approves_pending_delegation_confirmation"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_sessions.rs#scripted_interactive_default_profile_selection_persists_and_refreshes_projection"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_abort.rs#ctrl_c_cancels_running_prompt_on_coding_session_path"
        status: pass
    human_judgment: false

duration: 30 min
completed: 2026-07-12
status: complete
---

# Phase 03 Plan 05: Interactive Mutation Convergence Summary

**Interactive default-profile mutation and delegation rejection now execute asynchronously through CodingAgentSession::run(CodingAgentOperation) with preserved menus, dialogs, queues, errors, events, persistence, projections, and owner state.**

## Performance

- **Duration:** 30 min
- **Started:** 2026-07-12T00:31:29Z
- **Completed:** 2026-07-12T01:01:49Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Added the `scripted_interactive_default_profile_selection_persists_and_refreshes_projection` Wave 0 test proving that the interactive `/agent` menu "Use" selection routes through `CodingAgentSession::run(CodingAgentOperation::SetDefaultAgentProfile)`, the manifest persists the new default (`coder`), the visible transcript shows "Default agent profile: coder", and reopening the session preserves the default.
- Migrated the interactive default-profile mutation from synchronous `session.set_default_agent_profile_id(profile_id)` to `session.run(CodingAgentOperation::SetDefaultAgentProfile { profile_id })` inside a spawned `PromptTask` with exhaustive `CodingAgentOperationOutcome::DefaultAgentProfileChanged` extraction, preserving the pinned future, `tokio::select!` abort/event-forwarding loop, final `try_recv` drain, `SetDefaultAgentProfileTaskResult` owner return, and `finish_prompt` restoration.
- Migrated the interactive delegation rejection from synchronous `session.reject_delegation_confirmation(...)` to `session.run(CodingAgentOperation::RejectDelegation { operation_id, tool_call_id, reason })` inside a spawned `PromptTask` with exhaustive `CodingAgentOperationOutcome::DelegationRejected` extraction, preserving the pending delegation lookup, reason text, event forwarding through the existing `UiProjection`/`CodingEventBridge` pipeline, and the fallback system notice when no ProductEvents arrive.
- Preserved the fallback notice semantics: the `DelegationRejectionTaskResult` includes `fallback_notice: Option<String>` set to `Some("Delegation rejected: {operation_id} {tool_call_id}")` only when no ProductEvents were forwarded during the operation or drain; `finish_prompt` pushes it to the transcript only when `fallback_notice` is `Some`, matching the previous `ui_events.is_empty()` check.
- Synced `prompt_context.default_agent_profile_id` from the restored session after every task completion in the main loop, ensuring the prompt context reflects the canonical session state after profile mutation rather than being updated prematurely before the async operation completes.
- Preserved the root's local projection setter (`set_default_agent_profile_id`) and its immediate call in `handle_profile_menu_input` as local UI selection state, with the canonical session mutation serving as the runtime confirmation through `finish_prompt`'s projection sync.
- Removed the now-unused `CodingEventBridge` import from `loop.rs` and marked `handle_product_event` as `#[allow(dead_code)]` since the only production caller (the old synchronous rejection code) has migrated to canonical operations; the method is retained for bridge unit tests.
- Closed the combined interactive mutation gate: 41 `interactive_mode` tests, 3 `interactive_abort` tests, 30 `interactive_sessions` tests (including the new Wave 0 profile test), 11 `interactive_event_bridge` tests, `cargo check -p pi-coding-agent`, `cargo fmt --check`, and `git diff --check` are all green together.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add profile persistence coverage and migrate default-profile mutation** - `09a4fa9` (feat)
2. **Task 2: Migrate delegation rejection and close the mutation boundary** - `7247fa3` (feat)

## Files Created/Modified

- `crates/pi-coding-agent/src/interactive/prompt_task.rs` - Added `SetDefaultAgentProfileTaskResult`, `DelegationRejectionTaskResult`, `spawn_set_default_agent_profile`, `spawn_delegation_rejection`, `spawn_coding_set_default_agent_profile`, `spawn_coding_delegation_rejection`, `run_coding_set_default_agent_profile_task`, and `run_coding_delegation_rejection_task` following the existing owner-returning task pattern; both new task functions use `Box::pin(session.run(CodingAgentOperation::...))` in a `tokio::select!` loop with abort and event forwarding, exhaustive outcome extraction with `unreachable!` on impossible variants, and final `try_recv` drain.
- `crates/pi-coding-agent/src/interactive/loop.rs` - Added `start_set_default_agent_profile_task`; changed `handle_input_event` profile mutation to spawn an owned async task instead of synchronous `session.set_default_agent_profile_id`; changed `reject_pending_delegation_confirmation` to spawn an owned async task instead of synchronous `session.reject_delegation_confirmation`; updated `handle_delegation_confirmation_command` to pass `prompt_context` and `running` to the rejection path; added `SetDefaultAgentProfile` and `DelegationRejection` branches to `finish_prompt` syncing root projection and restoring owner; synced `prompt_context.default_agent_profile_id` from the restored session after every task completion; removed unused `CodingEventBridge` import.
- `crates/pi-coding-agent/src/interactive/event_bridge.rs` - Marked `handle_product_event` with `#[allow(dead_code)]` since the only production caller migrated to canonical operations; the method is retained for bridge unit tests.
- `crates/pi-coding-agent/tests/interactive_sessions.rs` - Added `mod support;` declaration and `scripted_interactive_default_profile_selection_persists_and_refreshes_projection` Wave 0 test verifying menu selection, canonical mutation, visible projection, manifest persistence, and reopen behavior.

## Decisions Made

- Imported `CodingAgentOperation` and `CodingAgentOperationOutcome` through `crate::api` per D-16, matching the JSON/print (03-01), RPC background (03-02), RPC mutation (03-03), and interactive background (03-04) adapter patterns.
- Used the existing owner-returning `PromptTask` lifecycle for both profile mutation and delegation rejection, matching the delegation approval pattern from 03-04: `take()` session, spawn task, forward events through the channel, return session through `PromptTaskResult`, restore through `finish_prompt`.
- Tracked `had_events` in the delegation rejection task to determine the fallback notice. If no ProductEvents were forwarded during the operation or drain, the task includes a fallback `SystemNotice` string in `DelegationRejectionTaskResult`. `finish_prompt` pushes it to the transcript only when `fallback_notice` is `Some`, preserving the previous `CodingEventBridge` drain + `ui_events.is_empty()` semantics.
- Synced `prompt_context.default_agent_profile_id` from the restored session after every task completion in the main loop, not just after profile mutation. This is safe because the session's default is the source of truth and the sync is a no-op for tasks that don't change the default. This avoids the premature `prompt_context` update that the previous synchronous code performed before the session mutation.
- Preserved the root's local projection setter (`set_default_agent_profile_id`) and its immediate call in `handle_profile_menu_input` as local UI selection state. The root's `default_agent_profile_id` reflects the user's selection intent; the canonical session mutation through `run(SetDefaultAgentProfile)` is the runtime confirmation; `finish_prompt` syncs the root from the session's actual default after success.
- Marked `handle_product_event` with `#[allow(dead_code)]` since the only production caller (the old synchronous rejection code) has migrated to canonical operations. The method is retained for bridge unit tests.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## Known Stubs

None. The `unreachable!` branches are exhaustive invariant handling for the closed `CodingAgentOperationOutcome` enum (T-03-04 accept), not placeholder behavior. The `dead_code` warning for `reload_plugins`, `run_plugin_command`, and `load_plugins` internal methods is expected because all production callers have migrated to canonical operations; these methods will be deleted in Phase 4 per D-19.

## Threat Flags

None. The migration introduces no new network endpoints, auth paths, file access patterns, or trust-boundary schema changes. The plan's threat register (T-03-17 through T-03-20) is addressed: only public profile/delegation operations are submitted through canonical admission with typed validation (T-03-17); the existing owner-returning task lifecycle forbids blocking and detached work (T-03-18); subscription before mutation, event draining through the existing bridge/projection pipeline, and fallback notice only when no events arrive mitigate T-03-19; root/prompt profile changes are applied only after the canonical outcome succeeds and verified by persistence/reopen tests (T-03-20).

## User Setup Required

None - all verification uses deterministic offline faux providers and tempfile sessions.

## Next Phase Readiness

- INTER-02 and the mutation-related INTER-04 behaviors are complete: interactive profile mutation and delegation rejection execute through `CodingAgentSession::run(CodingAgentOperation)` while the TUI observes identical menus, dialogs, queues, errors, events, persistence, and projections.
- D-13 and D-16 through D-20 are explicit; D-14/D-15 navigation work remains untouched until Plan 06.
- The interactive mutation boundary is closed with the full `interactive_mode`, `interactive_abort`, `interactive_sessions`, and `interactive_event_bridge` suites plus crate check, formatting, and diff checks, unblocking interactive navigation convergence (Plan 03-06).
- No new public API, blocking bridge, detached owner, event cache, dependency, or compatibility facade was introduced.

## Self-Check: PASSED

- All four modified files exist on disk: `crates/pi-coding-agent/src/interactive/prompt_task.rs`, `crates/pi-coding-agent/src/interactive/loop.rs`, `crates/pi-coding-agent/src/interactive/event_bridge.rs`, and `crates/pi-coding-agent/tests/interactive_sessions.rs`.
- Commits `09a4fa9` (Task 1) and `7247fa3` (Task 2) exist in repository history.
- `cargo test -p pi-coding-agent --test interactive_sessions scripted_interactive_default_profile_selection_persists_and_refreshes_projection -- --exact` passes (Task 1 focused test).
- `cargo test -p pi-coding-agent --test interactive_mode` (41 tests), `cargo test -p pi-coding-agent --test interactive_sessions` (30 tests), `cargo test -p pi-coding-agent --test interactive_abort` (3 tests), and `cargo test -p pi-coding-agent --test interactive_event_bridge` (11 tests) all pass with 0 failures.
- `cargo check -p pi-coding-agent`, `cargo fmt --check`, and `git diff --check` all pass.
- The interactive production source (`src/interactive/`) contains 0 calls to `session.set_default_agent_profile_id()` or `session.reject_delegation_confirmation()` and 0 `#[allow(deprecated)]` attributes.
- The remaining `set_default_agent_profile_id` calls in `loop.rs` and `root.rs` are all on `InteractiveRoot` (the local projection setter), not on `CodingAgentSession`.
- No tracked files were deleted by any task commit, and no new production threat surface or dependency was introduced.

---
*Phase: 03-production-adapter-convergence*
*Completed: 2026-07-12*
