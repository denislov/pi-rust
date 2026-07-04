# Self-Healing Edit Check Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Extend `SelfHealingEditFlow` with optional check-command execution and bounded repair retries while preserving the existing session-owned workflow surface.

**Architecture:** Keep the public `CodingAgentSession::self_healing_edit(path, replacements)` entrypoint unchanged for this slice. Add crate-private check runner and repair strategy injection points to `SelfHealingEditOptions`, use `ExecutionEnv` as the default check runner when available, and persist optional check output in self-healing edit completion events. The existing stable node list remains unchanged: `run_check` observes the applied patch, and `repair_patch` performs bounded validate/apply/check retries when a configured check fails.

**Tech Stack:** Rust 2024, `pi-agent-core::ExecutionEnv`, `FlowService`, `SelfHealingEditFlow`, typed session events, deterministic in-memory execution env tests.

---

### Task 1: Add RED Tests for Check Execution

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/flow_service.rs`

- [x] **Step 1: Add a check-success test**

Add a `self_healing_edit_flow_runs_successful_check_command` test in the existing `flow_service::tests` module. It should create an `InMemoryExecutionEnv`, register a command output with exit code `0`, run self-healing edit with `.with_execution_env(env.clone()).with_check_command("cargo test --quiet")`, and assert that `outcome.check_output` contains the command, stdout, stderr, and exit code.

- [x] **Step 2: Run the focused test and verify RED**

Run: `/home/whai/.cargo/bin/cargo test -p pi-coding-agent self_healing_edit_flow_runs_successful_check_command -- --nocapture`

Expected: compilation fails because `with_check_command` and `SelfHealingEditOutcome::check_output` do not exist.

### Task 2: Add RED Tests for Check Failure and Repair

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/flow_service.rs`

- [x] **Step 1: Add a failed-check test**

Add `self_healing_edit_flow_fails_when_check_command_fails_without_repair`. It should register a command output with exit code `1`, run the flow with a check command and no repair strategy, assert the returned error mentions `self-healing edit check failed`, assert diagnostics include the failing check, and assert the file reflects the first applied patch.

- [x] **Step 2: Add a repair-success test**

Add `self_healing_edit_flow_repairs_after_failed_check`. It should use a deterministic crate-private test check runner that returns a failed output first and a successful output second, plus a deterministic repair strategy returning a replacement for the already-edited file. Assert the final file content, `attempts == 2`, final `check_output.exit_code == 0`, and diagnostics retain the first check failure.

- [x] **Step 3: Run the focused tests and verify RED**

Run: `/home/whai/.cargo/bin/cargo test -p pi-coding-agent self_healing_edit_flow -- --nocapture`

Expected: compilation fails because the check runner, repair strategy, and check output API do not exist.

### Task 3: Implement Check Runner, Repair Strategy, and Outcome Fields

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs`

- [x] **Step 1: Add `SelfHealingEditCheckOutput`**

Add a public product-facing struct with `command`, `stdout`, `stderr`, and `exit_code` fields.

- [x] **Step 2: Extend options and context state**

Add optional `check_command`, optional crate-private `check_runner`, optional crate-private `repair_strategy`, `max_repair_attempts`, final `check_output`, and check-failure state.

- [x] **Step 3: Add injected runner traits**

Add crate-private `SelfHealingEditCheckRunner` and `SelfHealingEditRepairStrategy` traits. Implement `ExecutionEnvCheckRunner<E: ExecutionEnv + Clone + 'static>` using `env.exec(command, Some(ExecOptions { cwd }))`.

- [x] **Step 4: Implement `run_check`**

If no command is configured, no-op. If a command is configured without a runner, return a session error. If the runner returns a nonzero exit code, record a diagnostic and mark the check as failed so `repair_patch` can decide whether to retry or fail.

- [x] **Step 5: Implement bounded `repair_patch`**

If the latest check passed, no-op. If the latest check failed without repair configuration or with zero remaining attempts, return a session error. Otherwise call the repair strategy, validate the returned replacements, apply them, rerun the check, and stop when the check passes or the repair attempt bound is exhausted.

### Task 4: Persist Optional Check Output

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/event.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`

- [x] **Step 1: Add a persisted check-output struct**

Add `PersistedSelfHealingEditCheckOutput` with `command`, `stdout`, `stderr`, and `exit_code`.

- [x] **Step 2: Extend `SelfHealingEditCompleted`**

Add `check_output: Option<PersistedSelfHealingEditCheckOutput>` with serde default and skip-if-none behavior.

- [x] **Step 3: Update transaction recording**

Map `outcome.check_output` into the persisted optional check-output value.

- [x] **Step 4: Update session-log tests**

Update existing self-healing lifecycle tests to construct an outcome with check output and assert the persisted event contains it.

### Task 5: Update Docs and Verification

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-04-self-healing-edit-check-repair-plan.md`

- [x] **Step 1: Update TODO progress**

Mark the self-healing edit Phase 6 note as including check-command execution, bounded repair retries, and optional persisted check output.

- [x] **Step 2: Mark this plan's completed steps**

Replace relevant checkboxes with `[x]` as tasks are completed.

- [x] **Step 3: Run verification**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt --check
/home/whai/.cargo/bin/cargo test -p pi-coding-agent self_healing_edit_flow -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent self_healing_edit_transaction_records_lifecycle_events -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet
/home/whai/.cargo/bin/cargo check --workspace --quiet
git diff --check
```

Expected: all commands exit 0.
