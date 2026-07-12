---
phase: 03-production-adapter-convergence
reviewed: 2026-07-12T09:46:31Z
depth: standard
files_reviewed: 14
files_reviewed_list:
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
  warning: 2
  info: 0
  total: 2
status: issues_found
---

# Phase 03: Code Review Report

**Reviewed:** 2026-07-12T09:46:31Z
**Depth:** standard
**Files Reviewed:** 14
**Status:** issues_found

## Summary

The Phase 03 gap-closure implementation now returns an acquired `CodingAgentSession` through `PromptTaskCompletion::Failed`, restores it before projecting the error, updates the prompt session target only after successful forks, and forwards each delegation `ProductEvent` once. The reviewed production paths do not reveal a remaining owner-loss, owner-duplication, `PartialCommit` rewrite, event double-forward, stale successful-fork target, private-runtime import, or JSON/print/RPC contract regression.

The phase should not yet be treated as fully regression-locked, however. The main failure-continuity test fabricates the post-task envelope instead of causing any task runner or canonical operation to fail, and the replacement source guard still relies on a manually maintained runner list. Both tests pass even when important parts of the claimed Phase 03 closure are absent.

Focused checks run during review passed:

- `cargo test -p pi-coding-agent --lib prompt_task_failures_restore_the_live_owner_before_projecting_errors -- --nocapture`
- `cargo test -p pi-coding-agent --lib interactive_prompt_tasks_use_product_event_stream_boundary -- --nocapture`
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards production_interactive_uses_canonical_operations -- --nocapture`
- `git diff --check`

## Warnings

### WR-01: The claimed operation-failure coverage never executes a failing task or operation

**File:** `crates/pi-coding-agent/src/interactive/loop.rs:2593`; `crates/pi-coding-agent/tests/interactive_mode.rs:473`; `crates/pi-coding-agent/tests/interactive_mode.rs:610`; `crates/pi-coding-agent/tests/interactive_sessions.rs:110`

**Issue:** The test named `prompt_task_failures_restore_the_live_owner_before_projecting_errors` loops over four task labels, but each iteration directly constructs `PromptTaskCompletion::Failed(PromptTaskFailure { session, error })` and calls `finish_prompt`. It proves only that the final match arm restores an owner that has already been packaged correctly. It does not call `run_coding_set_default_agent_profile_task`, `run_coding_delegation_rejection_task`, `run_coding_fork_session_task`, or `run_coding_prompt_task`; therefore it cannot detect a runner that drops the owner, returns `SetupFailed`, changes the error (including `PartialCommit`), or fails to send completion.

The new integration tests do not close that gap. Delegation rejection at line 473 succeeds and then submits another prompt; direct fork at line 610 succeeds; the navigation tests at lines 110 and 150 also exercise only successful forks. Consequently, the report and summary claim four deterministic error-path continuity checks that do not exist. A future regression at the task/operation boundary can leave all of these tests green.

**Fix:** Add behavioral tests at the `PromptTask` boundary that induce a real canonical failure after the owner is acquired. For profile mutation, delegation rejection, fork, and one pre-existing async prompt path, await `task.done`, assert `PromptTaskCompletion::Failed` carries the same session identity and exact `CliError`, pass that completion through `finish_prompt`, then run another canonical operation on the restored owner. Use existing test-only session-store failure controls or deterministic invalid/admission fixtures; include at least one `PartialCommit` case so the durable ambiguity contract is verified rather than inferred. Rename the current unit test to describe its narrower `finish_prompt` responsibility.

### WR-02: The named-runner source guard does not detect newly added owner-returning runners

**File:** `crates/pi-coding-agent/src/interactive/prompt_task.rs:2020`

**Issue:** `interactive_prompt_tasks_use_product_event_stream_boundary` replaces the old magic count with a hard-coded array of thirteen function names. Removing or renaming one listed function fails, but adding a fourteenth `run_coding_*` owner-returning runner outside the array passes without any subscription or owner-completion assertion. This contradicts the plan acceptance criterion that adding an interactive owner-returning task must fail with the missing task name.

The guard also treats textual presence of `subscribe_product_events()` and `complete_owned_task(` anywhere in a naively brace-counted function body as proof of behavior. It does not establish that all post-acquisition exits flow through `complete_owned_task`, and braces inside future comments or string literals can corrupt `function_body` extraction. Thus the guard can both miss a real boundary omission and fail for unrelated source text.

**Fix:** Discover the runner set from sanitized Rust source by enumerating every `async fn run_coding_*` whose return type is `PromptTaskCompletion`, then compare the discovered names with the checked names and report any unchecked runner. Prefer a parser such as `syn` if already acceptable for test tooling; otherwise reuse the repository's established Rust-source sanitizer before brace matching. Behavioral completion tests from WR-01 should remain authoritative for exactly-one-owner semantics, while this structural guard should enforce complete runner coverage and prohibited compatibility subscriptions.

---

_Reviewed: 2026-07-12T09:46:31Z_
_Reviewer: the agent (gsd-code-reviewer, generic-agent workaround)_
_Depth: standard_
