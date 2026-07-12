---
phase: 03-production-adapter-convergence
plan: 04
subsystem: interactive-adapter
tags: [rust, canonical-operations, interactive, tui, background-tasks, branch-summary, plugin-actions, adapter-convergence]

requires:
  - phase: 03-production-adapter-convergence
    plan: 03
    provides: RPC mutation command convergence with exhaustive outcome extraction and narrow source guard
  - phase: 02-canonical-facade-correctness
    plan: 03
    provides: Canonical run durability, exhaustive public outcome projection, and closed facade ledger
provides:
  - Interactive prompt, agent, team, delegation approval, compaction, self-healing edit, plugin reload/command, and direct branch summary background tasks route through CodingAgentSession::run(CodingAgentOperation) with preserved TUI controls, events, timing, and projections
  - Focused Wave 0 direct branch-summary parity test locking AlwaysCreate semantics, no hydration, and no session replacement
  - PluginReloadTaskResult.outcome uses the public CodingAgentPluginLoadOutcome projection, keeping internal plugin services private per T-03-15
affects: [interactive-mutation-convergence, interactive-navigation-convergence, phase-04, phase-05]

tech-stack:
  added: []
  patterns:
    - "Interactive background canonical-call pattern: Box::pin(session.run(CodingAgentOperation::<Variant>(...))) inside tokio::spawn, polled via tokio::select! alongside ProductEvent forwarding and PromptControlHandle, then outcome extracted with .map_err(CliError::from).and_then(|o| match o { Variant(o) => Ok(o), _ => unreachable!(...) })"
    - "Direct branch-summary AlwaysCreate pattern: session.run(CodingAgentOperation::BranchSummary { ..., reuse: BranchSummaryReusePolicy::AlwaysCreate }) with hydrate_transcript: false and no navigation completion notice"
    - "Public plugin outcome projection at the interactive boundary: PluginReloadTaskResult.outcome changed from internal PluginLoadOutcome to public CodingAgentPluginLoadOutcome, matching the RPC adapter pattern from 03-03"

key-files:
  created: []
  modified:
    - crates/pi-coding-agent/src/interactive/prompt_task.rs
    - crates/pi-coding-agent/src/interactive/loop.rs
    - crates/pi-coding-agent/tests/interactive_sessions.rs

key-decisions:
  - "Import CodingAgentOperation, CodingAgentOperationOutcome, BranchSummaryReusePolicy, and CodingAgentPluginLoadOutcome through crate::api per D-16, matching the JSON/print, RPC, and interactive adapter patterns from 03-01/03-02/03-03."
  - "Treat an unexpected public outcome variant as an internal invariant (unreachable!) rather than a new user-visible error, matching the closed-enum discipline from Phase 2, the T-03-04 accept disposition, and the 03-01/03-02/03-03 adapter pattern."
  - "Preserve the Box::pin(session.run(...)) borrow pattern exactly as Box::pin(session.<broad>(...)) since both run and the deprecated broad methods take &mut self; the pinned future borrows session, releases on completion, and the owner is restored through the existing PromptTaskResult envelope."
  - "Change PluginReloadTaskResult.outcome from internal PluginLoadOutcome to public CodingAgentPluginLoadOutcome, matching the 03-03 RPC pattern where rpc_plugin_reload_data was changed to accept &CodingAgentPluginLoadOutcome. The plugin_reload_notice_lines function in loop.rs only uses loaded_plugin_ids and diagnostics, which exist on both types."
  - "Clone command_id before passing it into CodingAgentOperation::PluginCommand because the owned string is also stored in PluginCommandTaskResult for the visible transcript notice, matching the existing run_plugin_command(&command_id, ...) borrow pattern."
  - "Explicitly construct BranchSummaryReusePolicy::AlwaysCreate for direct /branch-summary, matching the deprecated summarize_branch method's reuse_existing: false, and retaining hydrate_transcript: false with no navigation completion notice. The navigation variant (summarize_branch_for_navigation + fork_current_session) remains unchanged, reserved for Plan 06 per D-14/D-15."
  - "Remove all six #[allow(deprecated)] attributes from run_coding_prompt_task, run_coding_agent_invocation_task, run_coding_agent_team_task, run_coding_compact_task, run_coding_self_healing_edit_task, and run_coding_branch_summary_task now that the deprecated broad calls are gone."

patterns-established:
  - "Interactive background canonical-call pattern: replace Box::pin(session.<broad>(opts)) with Box::pin(session.run(CodingAgentOperation::<Variant>(opts))) inside the spawned task, preserving every tokio::select! branch, guard, PromptControlHandle, receiver timing, final try_recv drain, and PromptTaskResult owner restoration."
  - "Direct branch-summary AlwaysCreate pattern: session.run(CodingAgentOperation::BranchSummary { options, source_leaf_id, target_leaf_id, custom_instructions, reuse: BranchSummaryReusePolicy::AlwaysCreate }) feeds the same PromptTurnOutcome into CodingPromptTaskResult with hydrate_transcript: false and completion_notice: None, distinguishing it from the navigation variant's ReuseExisting + fork + hydrate path."

requirements-completed: [INTER-01, INTER-04]

coverage:
  - id: D1
    description: "Interactive prompt background task executes through CodingAgentSession::run(CodingAgentOperation::Prompt) with preserved abort/steer/follow-up control routing, ProductEvent forwarding, final drain, and session owner restoration."
    requirement: INTER-01
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_mode.rs#scripted_interactive_prompt_renders_assistant_text"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_mode.rs#scripted_interactive_submit_while_running_sends_steer_control"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_mode.rs#scripted_interactive_shift_enter_while_running_sends_follow_up_control"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_abort.rs#ctrl_c_cancels_running_prompt_on_coding_session_path"
        status: pass
    human_judgment: false
  - id: D2
    description: "Interactive agent invocation and agent team background tasks execute through CodingAgentSession::run(CodingAgentOperation::InvokeAgent/InvokeTeam) with preserved control routing, event projection, member replies, and session owner restoration."
    requirement: INTER-01
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_mode.rs#scripted_interactive_agent_invocation_renders_selected_profile_reply"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_mode.rs#scripted_interactive_agent_team_renders_member_replies"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_abort.rs#ctrl_c_cancels_running_agent_invocation_child_prompt"
        status: pass
    human_judgment: false
  - id: D3
    description: "Interactive delegation approval, compaction, and self-healing edit background tasks execute through CodingAgentSession::run(CodingAgentOperation::ApproveDelegation/Compact/SelfHealingEdit) with preserved event drain, diagnostics, transcript projection, and session owner restoration."
    requirement: INTER-01
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_mode.rs#scripted_interactive_approves_pending_delegation_confirmation"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_mode.rs#scripted_interactive_compact_after_rust_native_prompt_records_compaction"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_mode.rs#scripted_interactive_self_healing_edit_uses_model_repair_policy"
        status: pass
    human_judgment: false
  - id: D4
    description: "Interactive plugin reload and plugin command background tasks execute through CodingAgentSession::run(CodingAgentOperation::PluginLoad/PluginCommand) with preserved conditional load-before-first-command, extension/menu/dialog/keybinding refresh, command output notices, and session owner restoration."
    requirement: INTER-04
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_sessions.rs#interactive_reload_reports_project_plugin_manifest_diagnostics"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_sessions.rs#interactive_plugin_command_runs_loaded_lua_plugin_command"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_sessions.rs#interactive_plugin_command_slash_alias_runs_loaded_lua_plugin_command"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_sessions.rs#interactive_plugin_keybinding_opens_loaded_lua_dialog"
        status: pass
    human_judgment: false
  - id: D5
    description: "Direct interactive /branch-summary executes through CodingAgentSession::run(CodingAgentOperation::BranchSummary) with AlwaysCreate semantics, visible projection, durable facts, no navigation hydration, and no session replacement."
    requirement: INTER-01
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_sessions.rs#scripted_interactive_branch_summary_preserves_visible_and_persisted_behavior"
        status: pass
    human_judgment: false

duration: 13 min
completed: 2026-07-12
status: complete
---

# Phase 03 Plan 04: Interactive Background Operation Convergence Summary

**Every ordinary interactive background workflow (prompt, agent, team, approval, compact, self-heal, plugin reload/command, direct branch summary) now routes through CodingAgentSession::run(CodingAgentOperation) with unchanged TUI controls, events, timing, projections, and owner state.**

## Performance

- **Duration:** 13 min
- **Started:** 2026-07-12T00:08:00Z
- **Completed:** 2026-07-12T00:21:44Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- Added the `scripted_interactive_branch_summary_preserves_visible_and_persisted_behavior` Wave 0 parity test proving the direct `/branch-summary` command is visibly projected ("Summarizing branch..."), does not trigger navigation hydration ("Navigated to selected point" is absent), does not create or replace sessions, and preserves prior durable facts.
- Migrated the interactive prompt background future from `session.prompt(prompt_options)` to `session.run(CodingAgentOperation::Prompt(prompt_options))` with exhaustive `CodingAgentOperationOutcome::Prompt` extraction, preserving the pinned future, every `tokio::select!` branch/guard, `PromptControlHandle` abort/steer/follow-up routing, `ProductEvent` forwarding, completion-time `try_recv` drain, `CodingPromptTaskResult` owner return, and `finish_prompt` ordering.
- Migrated the interactive agent invocation background future from `session.invoke_agent(invocation_options)` to `session.run(CodingAgentOperation::InvokeAgent(invocation_options))` with exhaustive `CodingAgentOperationOutcome::AgentInvocation` extraction, preserving prompt control, event forwarding, final drain, and `AgentInvocationTaskResult` owner return.
- Migrated the interactive agent team background future from `session.invoke_team(team_options)` to `session.run(CodingAgentOperation::InvokeTeam(team_options))` with exhaustive `CodingAgentOperationOutcome::AgentTeam` extraction, preserving the abort branch, event forwarding, final drain, outcome storage in `AgentTeamTaskResult`, and owner return.
- Migrated the interactive delegation approval background future from `session.approve_delegation_confirmation(&operation_id, &tool_call_id)` to `session.run(CodingAgentOperation::ApproveDelegation { operation_id, tool_call_id })` with exhaustive `CodingAgentOperationOutcome::DelegationApproved` extraction, preserving the abort branch, event forwarding, final drain, and `DelegationApprovalTaskResult` owner return.
- Migrated the interactive compaction background future from `session.compact(compact_options)` to `session.run(CodingAgentOperation::Compact(compact_options))` with exhaustive `CodingAgentOperationOutcome::Compact` extraction, preserving the unsupported-abort behavior, event forwarding, final drain, and `CodingPromptTaskResult` owner return.
- Migrated the interactive self-healing edit background future from `session.self_healing_edit_with_options(request)` to `session.run(CodingAgentOperation::SelfHealingEdit(request))` with exhaustive `CodingAgentOperationOutcome::SelfHealingEdit` extraction, preserving the unsupported-abort behavior, event forwarding, final drain, diagnostics projection, and `SelfHealingEditTaskResult` owner return.
- Migrated the interactive plugin reload background future from `session.reload_plugins()` to `session.run(CodingAgentOperation::PluginLoad)` with exhaustive `CodingAgentOperationOutcome::PluginLoad` extraction, preserving the abort branch, event forwarding, final drain, extension/menu/dialog/keybinding refresh, and `PluginReloadTaskResult` owner return.
- Migrated the interactive plugin command background task from `session.reload_plugins()` (conditional load) and `session.run_plugin_command(&command_id, args)` to `session.run(CodingAgentOperation::PluginLoad)` and `session.run(CodingAgentOperation::PluginCommand { command_id, args })` with exhaustive outcome extraction, preserving the conditional load-before-first-command behavior, abort branch, event forwarding, final drain, extension refresh, command output notice, and `PluginCommandTaskResult` owner return.
- Migrated the direct interactive branch summary from `session.summarize_branch(branch_options, source_leaf_id, target_leaf_id, custom_instructions)` to `session.run(CodingAgentOperation::BranchSummary { options, source_leaf_id, target_leaf_id, custom_instructions, reuse: BranchSummaryReusePolicy::AlwaysCreate })` with exhaustive `CodingAgentOperationOutcome::BranchSummary` extraction, preserving the abort branch, event forwarding, final drain, `hydrate_transcript: false`, `completion_notice: None`, and `CodingPromptTaskResult` owner return.
- Changed `PluginReloadTaskResult.outcome` from the internal `PluginLoadOutcome` to the public `CodingAgentPluginLoadOutcome` projection, and updated `plugin_reload_notice_lines` in `loop.rs` to accept the public type, matching the 03-03 RPC adapter pattern and keeping internal plugin capabilities private per T-03-15.
- Removed all six `#[allow(deprecated)]` attributes from the migrated task functions now that the deprecated broad calls are gone.
- Left the navigation variant (`summarize_branch_for_navigation` + `fork_current_session`) unchanged, reserved for Plan 06 per D-14/D-15.
- Closed the combined interactive background gate: 41 `interactive_mode` tests, 3 `interactive_abort` tests, 29 `interactive_sessions` tests (including the new Wave 0 branch-summary test), `cargo check -p pi-coding-agent`, `cargo fmt --check`, and `git diff --check` are all green together.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add focused direct branch-summary and background parity gates** - `c4558f1` (test)
2. **Task 2: Migrate prompt, agent, team, approval, compact, and self-healing tasks** - `613cc1b` (feat)
3. **Task 3: Migrate plugin actions and direct branch summary** - `5389192` (feat)

## Files Created/Modified

- `crates/pi-coding-agent/tests/interactive_sessions.rs` - Added `scripted_interactive_branch_summary_preserves_visible_and_persisted_behavior` Wave 0 test verifying direct `/branch-summary` visible projection, no navigation hydration, no session replacement, and durable fact preservation.
- `crates/pi-coding-agent/src/interactive/prompt_task.rs` - All eight ordinary interactive background workflows (prompt, agent, team, delegation approval, compaction, self-healing edit, plugin reload, plugin command) and direct branch summary now call `session.run(CodingAgentOperation::...)` with exhaustive `CodingAgentOperationOutcome` extraction; six `#[allow(deprecated)]` attributes removed; imports `CodingAgentOperation`, `CodingAgentOperationOutcome`, `BranchSummaryReusePolicy`, `CodingAgentPluginLoadOutcome` via `crate::api`; `PluginReloadTaskResult.outcome` changed to `CodingAgentPluginLoadOutcome`; `PluginLoadOutcome` import removed.
- `crates/pi-coding-agent/src/interactive/loop.rs` - `plugin_reload_notice_lines` now accepts `&CodingAgentPluginLoadOutcome`; import changed from `crate::coding_session::PluginLoadOutcome` to `crate::api::CodingAgentPluginLoadOutcome`.

## Decisions Made

- Imported `CodingAgentOperation`, `CodingAgentOperationOutcome`, `BranchSummaryReusePolicy`, and `CodingAgentPluginLoadOutcome` through `crate::api` per D-16, matching the JSON/print (03-01), RPC background (03-02), and RPC mutation (03-03) adapter patterns, while leaving existing concrete type imports from `crate::coding_session` untouched to keep the diff minimal.
- Used `unreachable!` for impossible `CodingAgentOperationOutcome` variants in each of the nine migrated background tasks, matching the closed-enum discipline established in Phase 2, the T-03-04 accept disposition, and the 03-01/03-02/03-03 adapter pattern.
- Preserved the `Box::pin(session.run(...))` borrow pattern exactly as `Box::pin(session.<broad>(...))` since both `run` and the deprecated broad methods take `&mut self`; the pinned future borrows `session`, releases the borrow on completion, and the owner is restored through the existing `PromptTaskResult` envelope.
- Changed `PluginReloadTaskResult.outcome` from internal `PluginLoadOutcome` to public `CodingAgentPluginLoadOutcome`, matching the 03-03 RPC pattern where `rpc_plugin_reload_data` was changed to accept `&CodingAgentPluginLoadOutcome`. The `plugin_reload_notice_lines` function only uses `loaded_plugin_ids` and `diagnostics`, which exist on both types.
- Cloned `command_id` before passing it into `CodingAgentOperation::PluginCommand` because the owned string is also stored in `PluginCommandTaskResult` for the visible transcript notice, matching the existing `run_plugin_command(&command_id, ...)` borrow pattern.
- Explicitly constructed `BranchSummaryReusePolicy::AlwaysCreate` for direct `/branch-summary`, matching the deprecated `summarize_branch` method's `reuse_existing: false`, and retaining `hydrate_transcript: false` with `completion_notice: None`. The navigation variant (`summarize_branch_for_navigation` + `fork_current_session`) remains unchanged, reserved for Plan 06 per D-14/D-15.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## Known Stubs

None. The `unreachable!` branches are exhaustive invariant handling for the closed `CodingAgentOperationOutcome` enum (T-03-04 accept), not placeholder behavior. The `dead_code` warning for `reload_plugins`, `run_plugin_command`, and `load_plugins` internal methods is expected because all production callers have migrated to canonical operations; these methods will be deleted in Phase 4 per D-19.

## Threat Flags

None. The migration introduces no new network endpoints, auth paths, file access patterns, or trust-boundary schema changes. The plan's threat register (T-03-13 through T-03-16) is addressed: the existing take/task/result/finish lifecycle is reused with the same session owner returned on every path, mitigating T-03-13; select branches, pre-run subscription, final drain, sequence projection, and abort tests are preserved, mitigating T-03-14; only public plugin operations are submitted with curated public outcomes and extension projection, mitigating T-03-15; `AlwaysCreate`, no hydration, visible output, and durable facts are locked in a focused test, mitigating T-03-16.

## User Setup Required

None - all verification uses deterministic offline faux providers and tempfile sessions.

## Next Phase Readiness

- INTER-01 and the ordinary background portion of INTER-04 are complete: every ordinary interactive background workflow executes through `CodingAgentSession::run(CodingAgentOperation)` while the TUI observes identical controls, events, notices, extensions, transcript, and owner state.
- The interactive background boundary (D-11/D-12) is closed with the full `interactive_mode`, `interactive_abort`, and `interactive_sessions` suites plus crate check, formatting, and diff checks, unblocking interactive mutation convergence (Plan 03-05) and interactive navigation convergence (Plan 03-06).
- The navigation variant (`summarize_branch_for_navigation` + `fork_current_session`) remains on the internal path, reserved for Plan 06 per D-14/D-15.
- The narrow source guard from 03-01/03-03 remains scoped to JSON/print and RPC; extending it to interactive source is the natural follow-up for Plan 03-06 or Phase 5.

## Self-Check: PASSED

- All three modified files exist on disk: `crates/pi-coding-agent/src/interactive/prompt_task.rs`, `crates/pi-coding-agent/src/interactive/loop.rs`, and `crates/pi-coding-agent/tests/interactive_sessions.rs`.
- Commits `c4558f1` (Task 1), `613cc1b` (Task 2), and `5389192` (Task 3) exist in repository history.
- `cargo test -p pi-coding-agent --test interactive_mode` (41 tests), `cargo test -p pi-coding-agent --test interactive_abort` (3 tests), and `cargo test -p pi-coding-agent --test interactive_sessions` (29 tests, including `scripted_interactive_branch_summary_preserves_visible_and_persisted_behavior`) all pass with 0 failures.
- `cargo check -p pi-coding-agent`, `cargo fmt --check`, and `git diff --check` all pass.
- The interactive prompt_task.rs production source contains 0 deprecated broad workflow calls (`.prompt(`, `.invoke_agent(`, `.invoke_team(`, `.compact(`, `.self_healing_edit_with_options(`, `.summarize_branch(`, `.approve_delegation_confirmation(`, `.reload_plugins(`, `.run_plugin_command(`) and 0 `#[allow(deprecated)]` attributes.
- The navigation variant (`summarize_branch_for_navigation` + `fork_current_session`) remains unchanged and reserved for Plan 06.
- No tracked files were deleted by any task commit, and no new production threat surface or dependency was introduced.

---
*Phase: 03-production-adapter-convergence*
*Completed: 2026-07-12*
