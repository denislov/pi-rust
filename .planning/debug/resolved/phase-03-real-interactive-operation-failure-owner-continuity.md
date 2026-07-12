---
status: resolved
trigger: "Phase 3 UAT strict verification gap: real interactive operation failure owner continuity"
created: 2026-07-12T20:20:00+08:00
updated: 2026-07-12T16:16:09Z
---

## Current Focus

hypothesis: The strict UAT is missing because Plan 03-07 implemented a generic owner-return envelope but substituted a fabricated finish_prompt unit test for the required real runner/channel tests; additionally, the CliError conversion erases PartialCommit identity before PromptTaskFailure, so exact canonical PartialCommit preservation is a production-contract defect rather than test debt alone.
test: Compare the Phase 03-07 required behavior with the concrete runner, task channel, finish_prompt, fault-injection visibility, and CodingSessionError-to-CliError conversion paths.
expecting: Real runners will preserve the owner mechanically, but no test will spawn/await them under deterministic failure; StoreFailurePoint will be cfg(test)/crate-private; and PartialCommit.operation_id will be discarded during conversion.
next_action: Return the diagnosed root causes and private test seams to the verify-work orchestrator; do not modify production code in diagnose-only mode.

## Symptoms

expected: A real post-acquisition profile, rejection, or prompt failure carries the same owner and exact error through the task channel; finish_prompt restores it before error projection; a subsequent canonical operation succeeds; one case covers PartialCommit.
actual: User selected strict verification because the repository lacks an automated test through real PromptTask.done, a real operation failure, and PartialCommit fault injection; this cannot be tested manually.
errors: None reported; this is a verification-coverage gap, not an observed runtime failure.
reproduction: Test 1 in .planning/phases/03-production-adapter-convergence/03-UAT.md
started: Discovered during Phase 3 UAT.

## Eliminated

- hypothesis: The strict verification gap is caused by a known runtime failure that loses CodingAgentSession after the operation has acquired it.
  evidence: Every relevant runner owns a mutable CodingAgentSession until complete_owned_task, and complete_owned_task puts that same owner in PromptTaskFailure on every Err result; finish_prompt restores failure.session before applying AgentError.
  timestamp: 2026-07-12T20:28:00+08:00

- hypothesis: Existing integration tests already exercise the real runner/channel failure path and the verifier merely overlooked them.
  evidence: CodeGraph reports no covering tests for complete_owned_task or the named runners; source search finds only the loop test that manually constructs PromptTaskCompletion::Failed and success-path interactive integration tests.
  timestamp: 2026-07-12T20:29:00+08:00

## Evidence

- timestamp: 2026-07-12T20:20:00+08:00
  checked: Repository indexing availability
  found: A .codegraph directory exists at the repository root.
  implication: CodeGraph must be used before grep or direct source reads for code-location and call-path investigation.

- timestamp: 2026-07-12T20:24:00+08:00
  checked: Real interactive runner ownership path
  found: run_coding_prompt_task, run_coding_set_default_agent_profile_task, and run_coding_delegation_rejection_task retain the mutable session locally, map operation errors, and call complete_owned_task(session, result, ...); complete_owned_task constructs PromptTaskCompletion::Failed(PromptTaskFailure { session, error }).
  implication: The shared production owner-return mechanism exists and is not disproven by an observed runtime failure.

- timestamp: 2026-07-12T20:25:00+08:00
  checked: Spawned task channel and main-loop restoration
  found: PromptTask spawn methods send the runner result through a oneshot done channel; the main loop awaits task.done; finish_prompt assigns coding_session = Some(session) before applying UiEvent::AgentError.
  implication: The desired owner/channel/restore order is wired in production, but requires a real runner-level failure test to verify behavior end to end.

- timestamp: 2026-07-12T20:27:00+08:00
  checked: Existing owner-continuity test
  found: prompt_task_failures_restore_the_live_owner_before_projecting_errors creates a non-persistent session and directly calls finish_prompt with a fabricated PromptTaskCompletion::Failed for four task-name strings; it never invokes a runner, PromptTask spawn, task.done, session.run failure, event receiver, or store fault control.
  implication: The test proves only the final restoration/projection arm and cannot satisfy the Phase 03-07 behavioral acceptance criteria.

- timestamp: 2026-07-12T20:28:00+08:00
  checked: Deterministic persistence failure seam
  found: StoreFailurePoint and SessionService::fail_store_after_for_tests are both cfg(test) and crate-private. Existing uses are co-located coding_session unit tests; integration tests compile the library without those cfg(test) APIs.
  implication: The Plan 03-07 artifact expectation naming interactive_mode.rs/interactive_sessions.rs is incompatible with using the existing fault seam directly. Strict tests must be co-located crate unit tests, or a new test-only bridge must be introduced without exposing a production hook.

- timestamp: 2026-07-12T20:30:00+08:00
  checked: PartialCommit preservation through the interactive error boundary
  found: All runners call map_err(CliError::from). From<CodingSessionError> for CliError matches PartialCommit { message, .. } and produces CliError::SessionFailure(message), discarding operation_id and the explicit partial-commit classification. PromptTaskFailure can store only CliError.
  implication: The Phase 03-07 requirement to preserve canonical PartialCommit including operation IDs cannot currently be verified because production code rewrites it before the task channel. This is a production contract defect, not merely missing coverage.

- timestamp: 2026-07-12T20:31:00+08:00
  checked: Phase 03-07 plan versus delivered tests
  found: The plan explicitly required deterministic real failures for profile mutation, delegation rejection, fork, and a pre-existing async task, plus unchanged PartialCommit operation IDs. The delivered prompt_task test module contains projection and structural source tests only, while loop.rs contains the fabricated completion test.
  implication: The root coverage cause is incomplete execution of the plan's TDD/behavioral test task, masked by a test whose four string labels look like four paths without executing any of them.

## Resolution

root_cause: The strict verification gap has two linked causes. First, Phase 03-07 implemented the generic owner-return production envelope but did not implement its required real runner/channel failure tests; the only failure test bypasses PromptTask spawning, task.done, canonical operations, subscriptions, and fault injection by fabricating PromptTaskCompletion::Failed. The existing deterministic store controls are cfg(test)/crate-private and therefore usable from co-located crate unit tests, not the integration-test files named by the plan, which explains the missing executable seam. Second, exact PartialCommit preservation is actually impossible in the current production path because every interactive runner maps CodingSessionError into CliError, and the conversion collapses PartialCommit { operation_id, message } into SessionFailure(message), losing the operation ID and classification before PromptTaskFailure crosses the channel.
fix: Add co-located async unit tests at the private PromptTask/loop boundary that spawn real tasks, await done, inject deterministic operation/store failures, pass completions through finish_prompt, and execute a subsequent canonical operation on the restored owner. Reconcile the error contract so PartialCommit identity and operation_id survive the interactive task boundary and are projected without suppression; avoid any production failure hook.
verification: Diagnosis only; no production code or tests changed and no fix verification performed.
files_changed: []
