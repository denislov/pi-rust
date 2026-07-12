---
phase: 03-production-adapter-convergence
plan: 09
subsystem: interactive-adapter-testing
tags: [rust, interactive, prompt-task, owner-continuity, partial-commit, fork, product-events, uat-gap-closure]

requires:
  - phase: 03-production-adapter-convergence
    plan: 08
    provides: Structured CliError PartialCommit identity and specialized cfg(test) persistent owner fixtures
  - phase: 03-production-adapter-convergence
    plan: 07
    provides: Owner-preserving PromptTask failure envelope and restore-before-projection finish_prompt behavior
provides:
  - Real profile, delegation rejection, and prompt finalization failures exercised through spawned PromptTask.done channels
  - Durable PartialCommit identity verified across distinct task-level and prompt outcome-level completion contracts
  - Real ForkSession failure coverage for source owner, target, cleanup, subscriber, and post-restoration event continuity
affects: [phase-04, phase-05, interactive-uat, stage-10]

tech-stack:
  added: []
  patterns:
    - "Real-runner adapter verification: spawn the production PromptTask, await its actual oneshot, drain its ProductEvents, and pass the untouched completion through finish_prompt."
    - "Durable identity assertions: parse events.jsonl as JSON and compare appended operation IDs with typed PartialCommit outcomes."
    - "Subscriber continuity: retain a pre-task ProductEventReceiver and prove it observes a later canonical mutation after failed owner restoration."

key-files:
  created:
    - .planning/phases/03-production-adapter-convergence/03-09-SUMMARY.md
  modified:
    - crates/pi-coding-agent/src/interactive/loop.rs

key-decisions:
  - "Keep profile and rejection persistence failures as task-level PromptTaskCompletion::Failed while prompt finalization PartialCommit remains PromptTaskCompletion::Completed(Coding(PromptTurnOutcome::Failed))."
  - "Use only the Plan 03-08 specialized cfg(test) owner methods; add no production fault hook, public API, or additional coding_session seam."
  - "Prove fork subscriber continuity with the exact receiver created before owner transfer instead of resubscribing after finish_prompt."

patterns-established:
  - "Interactive failure tests assert owner and exact error before consuming the same completion in finish_prompt."
  - "Prompt failure ProductEvents are projected before finish_prompt, and completion handling is asserted not to duplicate the visible error."
  - "Failed persistent navigation is checked at both runtime and filesystem boundaries before post-restoration mutation."

requirements-completed: [INTER-01, INTER-02, INTER-03, INTER-04]

coverage:
  - id: D1
    description: "Real profile, delegation rejection, and prompt finalization failures cross actual PromptTask.done channels, preserve their distinct completion/error contracts, restore the source owner, keep the old target, and permit another canonical operation."
    requirement: INTER-01
    verification:
      - kind: unit
        ref: "crates/pi-coding-agent/src/interactive/loop.rs#real_profile_failure_restores_owner_through_prompt_task_done"
        status: pass
      - kind: unit
        ref: "crates/pi-coding-agent/src/interactive/loop.rs#real_rejection_partial_commit_restores_owner_through_prompt_task_done"
        status: pass
      - kind: unit
        ref: "crates/pi-coding-agent/src/interactive/loop.rs#real_prompt_partial_commit_returns_completed_failed_outcome_through_prompt_task_done"
        status: pass
    human_judgment: false
  - id: D2
    description: "Real ForkSession AppendEvents failure preserves the source owner, old target, session-directory count, exact error, pre-task subscriber, and later canonical profile event without publishing SessionOpened for a replacement owner."
    requirement: INTER-03
    verification:
      - kind: unit
        ref: "crates/pi-coding-agent/src/interactive/loop.rs#real_fork_failure_preserves_source_owner_subscriber_and_target_through_prompt_task_done"
        status: pass
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test interactive_mode --test interactive_sessions --test product_runtime_boundary_guards -- --nocapture"
        status: pass
    human_judgment: false
  - id: D3
    description: "The complete Phase 3 gap closure gate passes with canonical interactive source guards and no Stage 10 or compatibility-deletion changes."
    requirement: INTER-04
    verification:
      - kind: other
        ref: "cargo fmt --check; cargo test -p pi-coding-agent --lib; cargo check -p pi-coding-agent; cargo test --workspace; cargo check --workspace; git diff --check"
        status: pass
    human_judgment: false

duration: 47min
completed: 2026-07-12
status: complete
execution_adapter: generic-agent-workaround
---

# Phase 03 Plan 09: Real Interactive Failure Continuity Summary

**Four deterministic real-runner tests now close both strict UAT gaps through actual PromptTask channels, durable persistence failures, finish_prompt restoration, and post-failure owner use.**

## Performance

- **Duration:** 47 min
- **Started:** 2026-07-12T14:18:03Z
- **Completed:** 2026-07-12T15:05:23Z
- **Tasks:** 2
- **Files modified:** 1 test-bearing production module plus this summary
- **Execution:** Generic-agent workaround for unavailable typed `gsd-executor` dispatch

## Accomplishments

- Replaced the fabricated four-label failure test with three async tests that spawn real profile, delegation rejection, and prompt tasks, await the actual `done` oneshot, inspect the returned owner/error contract, and pass the untouched completion through `finish_prompt`.
- Verified delegation rejection preserves structured `CliError::PartialCommit` identity and prompt finalization preserves `CodingSessionError::PartialCommit` inside `Completed(Coding(PromptTurnOutcome::Failed))`; both IDs match replay-authoritative appended JSONL transactions.
- Verified prompt failure ProductEvent projection occurs before `finish_prompt` and is not duplicated by completed-outcome handling.
- Added a real failed fork test that proves attempted target cleanup, unchanged source ID/old target/session count, exact error, absence of replacement `SessionOpened`, and original subscriber continuity through a later canonical profile mutation.
- Kept production orchestration, event ordering, compatibility methods, stable API, and Stage 10 event contracts unchanged.

## Task Commits

Both tasks followed explicit RED then GREEN commits:

1. **Task 1 RED: named real interactive task gaps** - `89adba2` (test)
2. **Task 1 GREEN: real profile/rejection/prompt continuity** - `237a390` (test)
3. **Task 2 RED: named real fork continuity gap** - `258bd78` (test)
4. **Task 2 GREEN: real fork owner/subscriber continuity** - `2c24fd6` (test)

## Files Created/Modified

- `crates/pi-coding-agent/src/interactive/loop.rs` - Adds persistent/faux/durable-log test helpers and four real PromptTask runner tests at the private interactive boundary.
- `.planning/phases/03-production-adapter-convergence/03-09-SUMMARY.md` - Records execution, coverage, decisions, and verification evidence.

## Decisions Made

- Preserved the factual error-path distinction instead of normalizing all failures into one envelope: profile/rejection canonical `Err` values return `Failed(owner + CliError)`, while prompt finalization uncertainty remains a completed coding result with a failed prompt outcome.
- Used structured `serde_json::Value` parsing for appended JSONL operation IDs rather than substring matching or importing private session-log types across module ownership boundaries.
- Seeded the fork source with a deterministic faux prompt so the failure reaches target-session AppendEvents, then reused the pre-spawn ProductEvent receiver after restoration.

## Deviations from Plan

None - plan executed within the specified co-located `loop.rs` test boundary. No production API, test seam, dependency, compatibility deletion, event-contract change, or unrelated refactor was added.

## Issues Encountered

- The first rejection assertion expected the raw injected message. The real durable service intentionally stores `error.to_string()` in PartialCommit, so the exact structured message includes the existing `session error:` prefix. The expectation was corrected to the production contract; operation identity and classification were already correct.
- Existing dead-code warnings for broad compatibility methods and `OperationControl::ensure_idle` remain expected Phase 4 cleanup. This plan introduced no new warning.

## TDD Gate Compliance

- Task 1 RED failed all three named tests with explicit missing-real-runner markers before `237a390` implemented the real fixtures and assertions.
- Task 2 RED failed the exact fork test before `2c24fd6` implemented source-owner, cleanup, target, and subscriber continuity assertions.
- Git history contains each `test(03-09)` RED commit before its corresponding GREEN test commit.

## Known Stubs

None introduced. The modified module's pre-existing startup-banner extension placeholder is unrelated to this plan and was not changed.

## Threat Flags

None. The changes add deterministic crate-local tests only: no network endpoint, authentication path, production filesystem capability, schema, dependency, or public privilege surface changed. T-03-34 through T-03-37 are mitigated by real channel, durable identity, failed-fork cleanup, and original-subscriber assertions.

## Verification

- Four exact named real-runner tests passed individually.
- `cargo test -p pi-coding-agent --lib` passed 653 tests with 1 ignored and 0 failures.
- Focused integration gate passed 42 `interactive_mode`, 30 `interactive_sessions`, and 13 `product_runtime_boundary_guards` tests.
- `session_store_failure_controls_remain_test_only`, `canonical_operation_facade_has_no_new_workflow_wrappers`, and `production_interactive_uses_canonical_operations` passed individually.
- `cargo check -p pi-coding-agent` passed.
- `cargo test --workspace` passed.
- `cargo check --workspace` passed.
- `cargo fmt --check` and `git diff --check` passed.
- Source audit confirmed all four named tests call the intended `PromptTask::spawn_*` methods and await the real `task.done` receiver.

## User Setup Required

None - all verification uses tempfile Rust-native sessions, deterministic faux providers, and test-only owner-local fault controls.

## Next Phase Readiness

- Both Phase 3 strict UAT issues now have deterministic automated coverage and no remaining diagnosed gap.
- Phase 3 production adapter convergence is ready for phase verification/completion routing.
- Phase 4 can migrate remaining tests and delete broad compatibility methods in the required order; Stage 10 ProductEvent convergence remains deferred.

## Self-Check: PASSED

- `crates/pi-coding-agent/src/interactive/loop.rs` and this SUMMARY exist.
- Commits `89adba2`, `237a390`, `258bd78`, and `2c24fd6` exist in repository history in RED/GREEN order.
- All plan-required focused, crate, integration, workspace, source, format, and diff gates passed.
- No tracked file was deleted and the code worktree was clean before summary creation.

---
*Phase: 03-production-adapter-convergence*
*Completed: 2026-07-12*
