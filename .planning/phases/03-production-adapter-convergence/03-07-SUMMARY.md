---
phase: 03-production-adapter-convergence
plan: 07
subsystem: interactive-adapter
tags: [rust, interactive, tui, owner-continuity, fork, delegation, product-events, source-guards, gap-closure]

requires:
  - phase: 03-production-adapter-convergence
    plan: 06
    provides: Canonical direct fork and summary-before-fork navigation on the interactive PromptTask owner lifecycle
  - phase: 03-production-adapter-convergence
    plan: 05
    provides: Canonical interactive default-profile mutation and delegation rejection
  - phase: 02-canonical-facade-correctness
    plan: 03
    provides: Canonical error and PartialCommit behavior plus test-only fault-control boundaries
provides:
  - Every owner-bearing interactive PromptTask returns the live CodingAgentSession on operation failure before the unchanged CliError is projected
  - Successful direct and navigation forks synchronize prompt_context.session_target with the forked Rust-native owner used by subsequent requests
  - Delegation rejection fallback is controlled by visible UiEvent projection rather than raw ProductEvent receipt, without forwarding events twice
  - Named per-runner source guards replace the subscription magic count and enforce both product-event subscription and owner completion
  - INTER-02 requirement evidence is reconciled after the full Phase 3 behavioral gate passed
affects: [phase-04, phase-05, stage-10]

tech-stack:
  added: []
  patterns:
    - "Owner-preserving async completion: PromptTaskCompletion distinguishes Completed, Failed(owner + CliError), and SetupFailed before an owner exists."
    - "Fork target continuity: successful fork-bearing results carry ResolvedSessionTarget::OpenOrCreateId for the mutated owner into the next PromptRunOptions."
    - "Visibility-aware fallback: a private CodingEventBridge classifies UiEvent output while the original ProductEvent is forwarded exactly once."
    - "Semantic source guard: each named run_coding_* owner runner must contain subscribe_product_events and complete_owned_task, with diagnostics naming the missing runner."

key-files:
  created: []
  modified:
    - crates/pi-coding-agent/src/interactive/prompt_task.rs
    - crates/pi-coding-agent/src/interactive/loop.rs
    - crates/pi-coding-agent/tests/interactive_mode.rs
    - crates/pi-coding-agent/tests/interactive_sessions.rs
    - .planning/REQUIREMENTS.md

key-decisions:
  - "Separate setup failure from operation failure: only a failure after acquiring a live owner carries CodingAgentSession, while initial open/create failure remains SetupFailed because no owner exists to restore."
  - "Apply one complete_owned_task boundary to all thirteen owner-returning interactive runners rather than special-casing profile mutation, rejection, and fork."
  - "Derive the post-fork request target from the successfully mutated owner's public session view and update it only in Completed branches."
  - "Use a separate CodingEventBridge only for visibility classification; continue sending the original ProductEvent through the existing channel exactly once."
  - "Keep compatibility methods, private runtime types, control paths, event contracts, and Stage 10 work unchanged."

patterns-established:
  - "Interactive owned-task failure envelope: operation errors retain the owner and exact adapter error, while setup errors do not invent a replacement owner."
  - "Post-transition request routing: persistent owner mutations that replace session identity must carry the new ResolvedSessionTarget through completion."
  - "Per-runner boundary ledgers: structural tests enumerate semantic owners and validate each function body instead of asserting aggregate counts."

requirements-completed: [INTER-01, INTER-02, INTER-03, INTER-04]

coverage:
  - id: D1
    description: "Interactive operation failures restore the live CodingAgentSession before projecting the unchanged error, and the restored owner remains usable for canonical operations."
    requirement: INTER-01
    verification:
      - kind: unit
        ref: "crates/pi-coding-agent/src/interactive/loop.rs#prompt_task_failures_restore_the_live_owner_before_projecting_errors"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_mode.rs#scripted_interactive_delegation_rejection_preserves_owner_and_visible_fallback_semantics"
        status: pass
    human_judgment: false
  - id: D2
    description: "Direct fork and both tree-navigation paths route subsequent prompts into the forked Rust-native session target."
    requirement: INTER-03
    verification:
      - kind: unit
        ref: "crates/pi-coding-agent/src/interactive/loop.rs#fork_completion_replaces_the_prompt_session_target"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_mode.rs#scripted_interactive_fork_after_rust_native_prompt_creates_session"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_sessions.rs#interactive_tree_navigation_forks_to_selected_rust_native_leaf"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_sessions.rs#interactive_tree_navigation_summarizes_abandoned_leaf_before_forking"
        status: pass
    human_judgment: false
  - id: D3
    description: "Delegation rejection emits fallback only when projected UiEvent output is empty and suppresses fallback when the rejection event is visibly rendered."
    requirement: INTER-02
    verification:
      - kind: unit
        ref: "crates/pi-coding-agent/src/interactive/prompt_task.rs#delegation_fallback_visibility_follows_ui_event_projection"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/interactive_mode.rs#scripted_interactive_delegation_rejection_preserves_owner_and_visible_fallback_semantics"
        status: pass
    human_judgment: false
  - id: D4
    description: "Every named owner-returning interactive runner establishes the product-event subscription and owner-completion boundaries without compatibility subscription use or a magic count."
    requirement: INTER-04
    verification:
      - kind: unit
        ref: "crates/pi-coding-agent/src/interactive/prompt_task.rs#interactive_prompt_tasks_use_product_event_stream_boundary"
        status: pass
      - kind: unit
        ref: "crates/pi-coding-agent/src/interactive/loop.rs#interactive_loop_restores_owner_and_projects_completion_without_compat_subscription"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs#production_interactive_uses_canonical_operations"
        status: pass
    human_judgment: false
  - id: D5
    description: "The complete Phase 3 closure gate passes from the final production tree."
    requirement: INTER-04
    verification:
      - kind: other
        ref: "cargo fmt --check; cargo test -p pi-coding-agent --lib; cargo test -p pi-coding-agent --test interactive_mode --test interactive_sessions --test product_runtime_boundary_guards; cargo check -p pi-coding-agent; cargo test --workspace; cargo check --workspace; git diff --check"
        status: pass
    human_judgment: false

duration: 1h 14m
completed: 2026-07-12
status: complete
---

# Phase 03 Plan 07: Interactive Owner Continuity Gap Closure Summary

**Owner-bearing interactive tasks now return the live CodingAgentSession on failure, fork completions synchronize the next request target, and delegation fallback follows visible UiEvent projection.**

## Performance

- **Duration:** 1h 14m
- **Started:** 2026-07-12T07:57:35Z
- **Completed:** 2026-07-12T09:11:57Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added `PromptTaskCompletion::{Completed, Failed, SetupFailed}` and `PromptTaskFailure { session, error }`, then applied the shared `complete_owned_task` boundary to all thirteen owner-returning interactive task runners. Operation failures now return the exact live owner and unchanged `CliError`; initial session setup failures remain distinct because no owner exists yet.
- Updated `finish_prompt` to restore `coding_session` before applying `UiEvent::AgentError`, retain Idle transitions and existing projection text, and preserve the previous `session_target` on failure.
- Carried successful fork targets through both `ForkSessionTaskResult` and navigation-bearing `CodingPromptTaskResult`, updating the prompt context only after canonical `SessionForked` success. Direct fork, selected-leaf navigation, and summary-before-fork navigation now continue subsequent prompts in the forked session.
- Replaced delegation rejection's raw `had_events` flag with `CodingEventBridge` visibility classification. Invisible ProductEvents retain the existing fallback notice; visible rejection projection suppresses duplicate feedback; original ProductEvents still cross the task channel exactly once.
- Replaced the brittle subscription count with a named thirteen-runner structural ledger that requires both `subscribe_product_events()` and `complete_owned_task()` in every runner and reports the missing function name.
- Renamed the stale synchronous rejection loop test to describe the current async owner/projection boundary and strengthened it to enforce owner restoration, `UiProjection`, `AgentError`, and compatibility-subscription absence.
- Marked INTER-02 complete in both the checklist and traceability table only after focused and full workspace verification passed.

## Task Commits

TDD tasks produced explicit RED and GREEN commits; Task 3 was committed after the full gate:

1. **Task 1 RED: owner restoration coverage** - `3d68c84` (test)
2. **Task 1 GREEN: owner-preserving failure contract** - `2720ab8` (fix)
3. **Task 2 RED: fork target and fallback visibility coverage** - `5ad1f1c` (test)
4. **Task 2 GREEN: target synchronization and visibility semantics** - `cc62d07` (fix)
5. **Task 3: semantic boundary guards and requirement reconciliation** - `8622a11` (test)

## Files Created/Modified

- `crates/pi-coding-agent/src/interactive/prompt_task.rs` - Added the owner-preserving completion envelope, uniform completion helper, successful fork target payloads, visibility-aware rejection fallback, and named per-runner structural guard.
- `crates/pi-coding-agent/src/interactive/loop.rs` - Restores the owner before error projection, updates successful fork targets, and tests owner/error/target/projection behavior.
- `crates/pi-coding-agent/tests/interactive_mode.rs` - Proves rejection owner continuity and single visible feedback, and verifies direct fork continuation is persisted in the forked session.
- `crates/pi-coding-agent/tests/interactive_sessions.rs` - Verifies direct selected-leaf and summary-before-fork navigation continue subsequent prompts in the forked session.
- `.planning/REQUIREMENTS.md` - Marks INTER-02 checked and Complete without changing unrelated requirements.

## Decisions Made

- Used a three-state private completion enum so setup failures never fabricate an owner while every failure after owner acquisition returns that exact owner.
- Kept result payloads and canonical outcome extraction unchanged; the new wrapper changes ownership transport only.
- Derived successful persistent targets as `ResolvedSessionTarget::OpenOrCreateId(session.view().session_id)` from the already-mutated owner, avoiding a replacement open or extra persistence transition.
- Used an isolated `CodingEventBridge` as a projection classifier and discarded its UiEvents after the emptiness decision, leaving the main loop as the only consumer that applies projected events to UI state.
- Retained exact `CliError`/`PartialCommit` conversion, ProductEvent ordering, final drains, control channels, hydration flags, notices, stable `crate::api` imports, and compatibility deletion order.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- One delegation integration assertion initially waited on a provider notification before ensuring the new prompt chunk had been consumed, allowing a stale Idle signal to end the driver early. The test was corrected to wait for input consumption before provider completion; production behavior was already correct.
- The first manual source-audit regex matched legitimate `root.set_default_agent_profile_id` projection setters. The audit was narrowed to `session.<replaced workflow method>` receivers, matching the established boundary guard and yielding zero violations.
- Existing dead-code warnings for broad compatibility methods and `OperationControl::ensure_idle` remain expected Phase 4 deletion work; no warning originated from this plan.

## Known Stubs

None. No TODO, FIXME, placeholder, empty UI data source, or production failure-injection hook was added.

## Threat Flags

None. No network, authentication, filesystem trust-boundary, dependency, or schema surface was introduced. T-03-26 is mitigated by exactly-one-owner completion and restore-before-projection; T-03-27 by unchanged canonical errors and owner return; T-03-28 by success-only target synchronization and failure target retention; T-03-29 by established projection classification with single forwarding; T-03-30 by named per-runner structural diagnostics.

## User Setup Required

None - all verification uses deterministic offline providers, in-memory owners, and tempfile Rust-native sessions.

## Next Phase Readiness

- CR-01 and WR-01 through WR-04 are closed with behavioral and structural evidence; Phase 3 now has all seven plans implemented.
- All Phase 3 production adapters remain on `CodingAgentSession::run(CodingAgentOperation)` with no private runtime exposure, broad-method deletion, presentation redesign, or Stage 10 event convergence pulled forward.
- Phase 4 can migrate remaining test/owner callers and delete broad workflow methods in the required order.

## Self-Check: PASSED

- All five modified files exist on disk.
- Commits `3d68c84`, `2720ab8`, `5ad1f1c`, `cc62d07`, and `8622a11` exist in repository history.
- `cargo fmt --check` passed.
- `cargo test -p pi-coding-agent --lib` passed 647 tests with 1 ignored and 0 failures.
- Focused integration gate passed: 42 `interactive_mode`, 30 `interactive_sessions`, and 13 `product_runtime_boundary_guards` tests.
- `cargo check -p pi-coding-agent`, `cargo test --workspace`, and `cargo check --workspace` passed.
- `git diff --check` passed, the precise replaced-workflow/deprecation source audit returned zero matches, and no tracked files were deleted.

---
*Phase: 03-production-adapter-convergence*
*Completed: 2026-07-12*
