---
phase: 03-production-adapter-convergence
reviewed: 2026-07-12T15:38:52Z
depth: standard
files_reviewed: 17
files_reviewed_list:
  - crates/pi-coding-agent/src/coding_session/error.rs
  - crates/pi-coding-agent/src/coding_session/mod.rs
  - crates/pi-coding-agent/src/error.rs
  - crates/pi-coding-agent/src/interactive/app.rs
  - crates/pi-coding-agent/src/interactive/commands.rs
  - crates/pi-coding-agent/src/interactive/event_bridge.rs
  - crates/pi-coding-agent/src/interactive/loop.rs
  - crates/pi-coding-agent/src/interactive/prompt_task.rs
  - crates/pi-coding-agent/src/interactive/root.rs
  - crates/pi-coding-agent/src/interactive/session_actions.rs
  - crates/pi-coding-agent/src/print_mode.rs
  - crates/pi-coding-agent/src/protocol/json_mode.rs
  - crates/pi-coding-agent/src/protocol/rpc/commands.rs
  - crates/pi-coding-agent/src/protocol/rpc/prompt.rs
  - crates/pi-coding-agent/tests/interactive_mode.rs
  - crates/pi-coding-agent/tests/interactive_sessions.rs
  - crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs
findings:
  critical: 0
  warning: 4
  info: 0
  total: 4
status: issues_found
---

# Phase 03: Code Review Report

**Reviewed:** 2026-07-12T15:38:52Z
**Depth:** standard
**Files Reviewed:** 17
**Status:** issues_found
**Dispatch:** generic-agent workaround for unavailable typed `gsd-code-reviewer`

## Summary

The two UAT gaps now traverse real `PromptTask::spawn_*` runners, the actual oneshot completion channel, canonical persistence failures, and `finish_prompt`. The previous review's fabricated-completion warning is resolved. `CodingSessionError::PartialCommit` is converted losslessly into `CliError::PartialCommit`, its `Display` text remains identical, and the reviewed print, JSON, RPC, and interactive adapters consume errors through existing display/conversion paths without a variant-specific compatibility regression.

The test-only owner bridge is directly `#[cfg(test)]`, `pub(crate)`, action-specific, and absent from `pi_coding_agent::api`; no generic `StoreFailurePoint` selector or production fault hook escaped `coding_session`. The only production behavior change in the gap commits is the planned lossless error conversion.

Four regression-lock weaknesses remain. One is the previous manual runner-list warning; the other three are in the new real-runner tests. They do not demonstrate a current production failure, but each permits an important claimed contract to regress while tests remain green or hang indefinitely.

Focused checks run during review passed:

- `cargo test -p pi-coding-agent --lib 'interactive::r#loop::tests::real_' -- --nocapture` (4 passed)
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards session_store_failure_controls_remain_test_only -- --exact --nocapture` (1 passed)

## Narrative Findings (AI reviewer)

### WR-01: The runner boundary guard still cannot discover newly added owner-returning runners

**Classification:** WARNING

**File:** `crates/pi-coding-agent/src/interactive/prompt_task.rs:1962`; `crates/pi-coding-agent/src/interactive/prompt_task.rs:2020`

**Evidence:** `interactive_prompt_tasks_use_product_event_stream_boundary` still iterates a manually maintained array of thirteen function names. A new `async fn run_coding_* -> PromptTaskCompletion` is invisible unless a developer also edits this array, so the guard can stay green while the new runner omits `subscribe_product_events()` or `complete_owned_task()`. Its `function_body` helper also counts braces in raw source, so braces in a string or comment can truncate or extend the inspected body. The four new behavioral tests close the named profile/rejection/prompt/fork paths, but they do not repair this open-world source guard for the remaining and future runners.

**Fix:** Parse sanitized Rust source, preferably with `syn`, enumerate every `async fn run_coding_*` whose return type is `PromptTaskCompletion`, and compare that discovered set against the verified set. At minimum, apply the repository's source sanitizer before brace matching and fail with the exact newly discovered runner name. Keep behavioral tests authoritative for owner return; use this guard only to enforce complete structural coverage.

### WR-02: Real task-channel tests can hang forever instead of failing the completion contract

**Classification:** WARNING

**File:** `crates/pi-coding-agent/src/interactive/loop.rs:2449`

**Evidence:** All four strict UAT tests call `await_prompt_task`, which performs `task.done.await` with no deadline. A panic drops the sender and fails quickly, but a regression that deadlocks a runner, leaves an operation pending, or never sends completion hangs the Rust test process because the standard test harness has no per-test timeout. Failure to deliver `PromptTask.done` is one of the exact contracts these tests are intended to detect, so an unbounded wait turns the regression into an indefinitely stalled phase/workspace gate rather than a deterministic assertion failure.

**Fix:** Wrap the oneshot receive in `tokio::time::timeout` with a short deterministic test deadline and include the runner/completion context in the panic message. For example, return an error from the helper when the deadline expires before draining events, then have each test identify its runner in the expectation.

### WR-03: Durable-log and failed-fork cleanup assertions silently discard evidence of corruption

**Classification:** WARNING

**File:** `crates/pi-coding-agent/src/interactive/loop.rs:2468`; `crates/pi-coding-agent/src/interactive/loop.rs:2483`; `crates/pi-coding-agent/src/interactive/loop.rs:2807`; `crates/pi-coding-agent/src/interactive/loop.rs:2886`; `crates/pi-coding-agent/src/interactive/loop.rs:2982`

**Evidence:** `appended_operation_ids` uses `filter_map(... .ok())`, so malformed appended JSONL records are silently ignored, and the tests only require that any surviving record carries the operation ID. They do not require a complete committed transaction or fail on corrupt durable output. Separately, `rust_native_session_count` ignores unreadable directory entries and counts only directories that already contain both `session.json` and `events.jsonl`. A failed fork that leaves a partial target directory, or cleanup that removes only one required file, therefore produces the same count and satisfies the claimed "no replacement session survives" assertion.

**Fix:** Parse every appended line with `expect`, assert the expected transaction markers and exact operation ID rather than an `any` match, and fail on any malformed envelope. Snapshot the exact set of root directory entries before the fork and compare it after failure, including partial directories and unexpected files; additionally assert that only the known source session directory remains.

### WR-04: Restore-before-error-projection ordering is still asserted only as two final postconditions

**Classification:** WARNING

**File:** `crates/pi-coding-agent/src/interactive/loop.rs:2315`; `crates/pi-coding-agent/src/interactive/loop.rs:2757`; `crates/pi-coding-agent/src/interactive/loop.rs:2821`; `crates/pi-coding-agent/src/interactive/loop.rs:3100`

**Evidence:** Production currently performs `*coding_session = Some(session)` before `root.apply_events(AgentError)`, which is the required ordering. The profile and rejection tests call `finish_prompt` and then independently assert that an owner exists and an error exists. Reversing those two production statements would leave every new test green. The structural test `interactive_loop_restores_owner_and_projects_completion_without_compat_subscription` only checks that both source fragments occur somewhere in the file; it does not compare their order or scope. Thus the summaries' restore-before-projection claim is correct today but not regression-locked.

**Fix:** Extract the failed-completion arm into a small helper that restores the owner and invokes an injected/projector closure afterward; in the unit test, make the closure assert that `coding_session` already contains the expected owner. If refactoring is undesirable, add a sanitized/AST-based structural assertion scoped to the `PromptTaskCompletion::Failed` match arm that verifies the assignment precedes the `AgentError` projection.

---

_Reviewed: 2026-07-12T15:38:52Z_
_Reviewer: the agent (gsd-code-reviewer, generic-agent workaround)_
_Depth: standard_
