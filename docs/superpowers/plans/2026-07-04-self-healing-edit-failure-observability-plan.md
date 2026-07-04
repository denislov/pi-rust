# Self-Healing Edit Failure Observability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve structured check-command output when a self-healing edit fails its configured check.

**Architecture:** Keep `SelfHealingEditOutcome` for successful edits. Add a dedicated `CodingSessionError` variant for self-healing edit check failures that carries the failure message, diagnostics, and optional `SelfHealingEditCheckOutput`. RPC error responses can include optional `data` using the same shape already returned on successful self-healing edits, without exposing runner/runtime/provider internals.

**Tech Stack:** Rust 2024, `pi-coding-agent` session owner APIs, existing RPC JSON response type, existing self-healing edit Flow.

---

### Task 1: Public API Failure Shape

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/error.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs`
- Test: `crates/pi-coding-agent/tests/public_api.rs`

- [ ] **Step 1: Write the failing test**

Add a public API test that runs `self_healing_edit_with_options(SelfHealingEditRequest::new(...).with_check_command("printf check-failed >&2; exit 7"))` and asserts `CodingSessionError::SelfHealingEditFailed` includes `check_output.command`, `stderr`, `exit_code`, and diagnostics.

- [ ] **Step 2: Run the focused test to verify RED**

Run: `/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test public_api coding_session_self_healing_edit_failed_check_exposes_output -- --nocapture`

Expected: compile or assertion failure because the error variant does not exist or no structured output is exposed.

- [ ] **Step 3: Implement the public error variant**

Add `CodingSessionError::SelfHealingEditFailed { message, diagnostics, check_output }`, return code `self_healing_edit_failed`, and map it to `CliError::SessionFailure(message)`.

- [ ] **Step 4: Return the variant from check failure paths**

When `SelfHealingEditContext::repair_patch()` fails because the check command exited non-zero and no repair resolves it, build the new error variant from the latest check output and diagnostics.

- [ ] **Step 5: Run the focused public API test to verify GREEN**

Run: `/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test public_api coding_session_self_healing_edit_failed_check_exposes_output -- --nocapture`

Expected: PASS.

### Task 2: RPC Error Data

**Files:**
- Modify: `crates/pi-coding-agent/src/protocol/types.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/commands.rs`
- Test: `crates/pi-coding-agent/tests/rpc_mode.rs`

- [ ] **Step 1: Write the failing RPC test**

Add a test that sends `self_healing_edit` with `checkCommand":"printf rpc-check-failed >&2; exit 9"` and asserts the response has `success:false`, an error string, and `data.checkOutput` with command, stderr, and exitCode.

- [ ] **Step 2: Run the focused test to verify RED**

Run: `/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test rpc_mode rpc_self_healing_edit_failed_check_returns_check_output -- --nocapture`

Expected: failure because error responses currently omit structured data.

- [ ] **Step 3: Add an error response constructor with data**

Add `RpcResponse::error_with_data(id, command, error, data)` that sets `success:false`, `error:Some(...)`, and `data:Some(...)`.

- [ ] **Step 4: Serialize self-healing edit failure data**

In the RPC self-healing edit error branch, detect `CodingSessionError::SelfHealingEditFailed` and include diagnostics plus checkOutput in `data`.

- [ ] **Step 5: Run the focused RPC test to verify GREEN**

Run: `/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test rpc_mode rpc_self_healing_edit_failed_check_returns_check_output -- --nocapture`

Expected: PASS.

### Task 3: Docs And Verification

**Files:**
- Modify: `docs/TODO.md`

- [ ] **Step 1: Update TODO**

Record that self-healing edit check failures now preserve structured check output through the public Rust error and RPC error response.

- [ ] **Step 2: Run full verification**

Run:
- `/home/whai/.cargo/bin/cargo fmt --check`
- `/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet`
- `/home/whai/.cargo/bin/cargo check --workspace --quiet`
- `/home/whai/.cargo/bin/cargo test --workspace --quiet`
- `git diff --check`

Expected: all commands exit 0.
