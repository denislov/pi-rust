---
status: diagnosed
phase: 03-production-adapter-convergence
source: [03-VERIFICATION.md]
started: 2026-07-12T10:19:57Z
updated: 2026-07-12T12:32:25Z
---

## Current Test

[testing complete]

## Tests

### 1. Real interactive operation-failure owner continuity
expected: A real post-acquisition profile, rejection, or prompt failure carries the same owner and exact error through the task channel; finish_prompt restores it before error projection; a subsequent canonical operation succeeds; one case covers PartialCommit.
result: issue
reported: "好，采用第二种"
severity: major

### 2. Real ForkSession failure continuity
expected: A deterministic real ForkSession failure preserves the pre-fork owner, subscriber continuity, and old session target; finish_prompt projects the exact error and the restored session remains usable without opening a replacement owner.
result: issue
reported: "同样采用严格验证：缺少真实 ForkSession failure、PromptTask.done、subscriber 和旧 session target 连续性的自动测试，当前无法手工验证。"
severity: major

## Summary

total: 2
passed: 0
issues: 2
pending: 0
skipped: 0
blocked: 0

## Gaps

- truth: "A real post-acquisition profile, rejection, or prompt failure carries the same owner and exact error through the task channel; finish_prompt restores it before error projection; a subsequent canonical operation succeeds; one case covers PartialCommit."
  status: failed
  reason: "User reported: 好，采用第二种（采用严格验证：当前缺少通过真实 PromptTask.done、真实 operation failure 和 PartialCommit fault injection 的自动测试，无法手工验证。）"
  severity: major
  test: 1
  root_cause: "Phase 03-07 implemented the shared owner-return envelope but replaced the required real runner/channel failure tests with a fabricated PromptTaskCompletion::Failed test. The existing store fault controls are private test-only coding_session APIs, so interactive tests cannot arm them. In addition, From<CodingSessionError> for CliError collapses PartialCommit into SessionFailure(message), discarding operation_id and the partial_commit classification before PromptTaskFailure crosses the channel."
  artifacts:
    - path: "crates/pi-coding-agent/src/interactive/prompt_task.rs"
      issue: "Real runners and PromptTask.done are wired, but no deterministic operation-failure test drives them through complete_owned_task."
    - path: "crates/pi-coding-agent/src/interactive/loop.rs"
      issue: "The owner-restoration test fabricates PromptTaskCompletion::Failed and bypasses the real runner, channel, subscription, and fault path."
    - path: "crates/pi-coding-agent/src/coding_session/error.rs"
      issue: "PartialCommit is converted to CliError::SessionFailure(message), losing operation_id and explicit durability classification."
    - path: "crates/pi-coding-agent/src/coding_session/session_service.rs"
      issue: "The deterministic store failure control is cfg(test) and inaccessible from the interactive test boundary."
  missing:
    - "Add a narrow cfg(test), crate-visible CodingAgentSession fault-arm bridge without adding a production failure hook."
    - "Exercise real profile, rejection, and prompt runners through PromptTask.done, finish_prompt, and a subsequent canonical operation."
    - "Preserve PartialCommit classification and operation_id through the interactive error/task boundary and assert the exact durable error contract."
  debug_session: ".planning/debug/phase-03-real-interactive-operation-failure-owner-continuity.md"
- truth: "A deterministic real ForkSession failure preserves the pre-fork owner, subscriber continuity, and old session target; finish_prompt projects the exact error and the restored session remains usable without opening a replacement owner."
  status: failed
  reason: "User reported: 同样采用严格验证：缺少真实 ForkSession failure、PromptTask.done、subscriber 和旧 session target 连续性的自动测试，当前无法手工验证。"
  severity: major
  test: 2
  root_cause: "The production ForkSession flow retains the source owner, EventService, and old target on pre-swap persistence failure, but strict verification cannot trigger that path because StoreFailurePoint and fail_store_after_for_tests are private beneath coding_session. Existing tests separately cover successful real forks and a fabricated failure envelope, leaving spawn_coding_fork_session -> PromptTask.done -> finish_prompt under deterministic failure untested."
  artifacts:
    - path: "crates/pi-coding-agent/src/interactive/prompt_task.rs"
      issue: "spawn_coding_fork_session and run_coding_fork_session_task have no real deterministic failure test."
    - path: "crates/pi-coding-agent/src/interactive/loop.rs"
      issue: "The current fork failure case manually constructs the completion envelope and cannot prove subscriber or task-channel continuity."
    - path: "crates/pi-coding-agent/src/coding_session/mod.rs"
      issue: "CodingAgentSession lacks a narrow interactive-visible cfg(test) bridge for arming the existing store fault injector."
    - path: "crates/pi-coding-agent/src/coding_session/session_log/store.rs"
      issue: "StoreFailurePoint is correctly test-only but currently unreachable from the interactive unit-test module."
  missing:
    - "Expose a narrow cfg(test), crate-visible helper that arms the existing AppendEvents failure on a persistent CodingAgentSession."
    - "Add a real ForkSession PromptTask.done failure test that preserves source owner, old target, source session count, exact error, and the pre-task subscriber."
    - "Run a post-restoration canonical mutation and assert the original receiver still observes its product event."
  debug_session: ".planning/debug/phase-03-real-fork-session-failure-continuity.md"
