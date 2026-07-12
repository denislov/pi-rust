---
status: root_cause_found
trigger: "Phase 3 UAT Test 2: strict real ForkSession failure continuity is not automated"
created: 2026-07-12
updated: 2026-07-12
---

# Debug Session: Real ForkSession Failure Continuity

## Context

- Mode: root-cause diagnosis only
- Dispatch: generic-agent workaround for unavailable typed `gsd-debugger`
- Symptom: no automated test drives a deterministic real `ForkSession` failure through `PromptTask.done` and `finish_prompt` while checking owner, subscriber, target, exact error, and subsequent usability.
- Classification under investigation: verification debt versus production defect.

## Current Focus

Root cause isolated: the production path is ownership-correct, but the strict runner-level test cannot currently arm the existing persistence fault injector from the interactive test module because the control is private to `coding_session` internals.

## Hypotheses

1. **Refuted:** The production path loses the original owner when canonical fork fails. `run_sync_mut_operation` assigns `self.persistence = ...forked_service` only after fork creation and replay-derived owner state both succeed; every earlier error returns while the original persistence remains installed.
2. **Refuted:** `finish_prompt` mutates the old target on failure. Its `Failed` arm only restores `coding_session` and projects `AgentError`; `session_target` is changed only in successful completion arms.
3. **Confirmed:** Existing tests bypass the real task/channel failure transition, and existing store fault controls are inaccessible at that boundary. `StoreFailurePoint`, `SessionService::fail_store_after_for_tests`, and `CodingAgentSession::persistent_session_service` are test-only but private beneath `coding_session`; `interactive::prompt_task` and `interactive::loop` cannot arm them on the owner they pass into `spawn_coding_fork_session`.

## Evidence Log

- Phase verification explicitly records that the existing failure test fabricates `PromptTaskCompletion::Failed`; success-only direct and navigation fork tests cover continuation but not failure.
- CodeGraph identifies `spawn_coding_fork_session -> run_coding_fork_session_task -> CodingAgentSession::run` and reports no covering test for the spawn function.
- `run_coding_fork_session_task` acquires/moves the owner, subscribes before `run(ForkSession)`, converts the canonical error once, and always calls `complete_owned_task(session, result, ...)`; `complete_owned_task` places that same owner in `PromptTaskFailure` on error.
- Canonical fork creates a separate `forked_service`; `self.persistence`, replay-derived state, and the session-open event are changed only after the copy and replay succeed. An injected `AppendEvents` failure therefore leaves the source owner and its `EventService` untouched.
- `finish_prompt` restores `PromptTaskFailure.session` before applying the visible error and does not touch `session_target` in the failure arm.
- The current unit test `prompt_task_failures_restore_the_live_owner_before_projecting_errors` manually constructs `PromptTaskCompletion::Failed`, so it proves only the final projection/restoration arm.
- The direct interactive fork test and canonical owner/event-stream test exercise success only.
- `SessionLogStore` already has deterministic, shared (`Arc<Mutex<_>>`) test-only failure state. `AppendEvents(0)` reaches target-session creation during fork and cleanup removes the attempted target. The missing capability is not a lower-level injector; it is a narrow test-only owner-facing bridge usable from interactive tests.
- Focused checks pass: `coding_session::tests::canonical_fork_preserves_owner_runtime_and_event_stream` proves success-path owner/subscriber continuity, and `interactive::r#loop::tests::prompt_task_failures_restore_the_live_owner_before_projecting_errors` proves the fabricated-envelope restoration arm. Their separation is the uncovered transition diagnosed here.

## Reasoning Checkpoint

```yaml
reasoning_checkpoint:
  hypothesis: "Strict verification is missing because the interactive test boundary cannot arm the existing coding-session store fault control; consequently the only loop failure test fabricates the completion envelope and never executes spawn_coding_fork_session/run_coding_fork_session_task."
  confirming_evidence:
    - "The only failure test directly constructs PromptTaskCompletion::Failed."
    - "StoreFailurePoint and fail_store_after_for_tests are cfg(test) but private inside coding_session; no CodingAgentSession test bridge is visible to interactive modules."
    - "Production fork mutation occurs only after all fallible copy/replay work, so static control-flow evidence contradicts an owner-replacement production bug."
  falsification_test: "Finding an existing interactive-visible cfg(test) API that arms a SessionLogStore failure on CodingAgentSession, plus a test awaiting PromptTask.done from spawn_fork_session under that fault, would disprove the diagnosis. Repository search found neither."
  fix_rationale: "Expose the already-existing injector through a narrow cfg(test), crate-visible CodingAgentSession helper, then add one real loop-level task test; no production behavior change is required."
  blind_spots: "No new test was implemented in diagnosis-only mode, so runtime confirmation of the inferred source-owner and receiver continuity remains the purpose of the planned fix."
```

## Resolution

This is verification debt caused by a missing cross-module test seam and missing end-to-end test, not evidence of a production runtime defect.

Suggested test direction:

1. Add a narrow `#[cfg(test)] pub(crate)` fault-arm helper on `CodingAgentSession` (and a test-only point abstraction or specialized append-failure helper) that delegates to the persistent `SessionService`.
2. In `interactive/loop.rs` tests, create a persistent source session with at least one committed leaf, retain its session ID/target and a product-event receiver, arm `AppendEvents` failure at zero, move the owner into `PromptTask::spawn_fork_session`, and await the real `task.done` channel.
3. Assert `PromptTaskCompletion::Failed` contains the source session ID and the expected `CliError` text, pass it through `finish_prompt`, and assert the old target and source session-directory count remain unchanged.
4. Run a post-restoration canonical mutation that emits a product event and assert the pre-task receiver still observes it, proving subscriber continuity and owner usability.
