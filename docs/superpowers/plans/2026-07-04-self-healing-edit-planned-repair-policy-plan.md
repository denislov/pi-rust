# Self-Healing Edit Planned Repair Policy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose a bounded, product-level self-healing edit repair policy using planned replacement attempts.

**Architecture:** Keep model-driven repair as a later slice. Extend `SelfHealingEditRequest` with ordered repair attempts, where each attempt is a list of `SelfHealingEditReplacement`s to apply after a failed check. Session code adapts those attempts into the existing crate-private `SelfHealingEditRepairStrategy` so Flow behavior remains centralized and no raw runner, runtime, provider, or filesystem internals leak through public APIs.

**Tech Stack:** Rust 2024, `pi-coding-agent` public API, existing `SelfHealingEditFlow`, RPC JSON command decoding, deterministic local command tests.

---

### Task 1: Rust API Planned Repair Attempts

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Test: `crates/pi-coding-agent/tests/public_api.rs`

- [ ] **Step 1: Write the failing public API test**

Add `coding_session_self_healing_edit_uses_planned_repair_attempts`. It should create a file containing `one\ntwo\n`, run an initial edit `two -> deux`, configure check command `grep -q dos src/app.txt`, configure one repair attempt `deux -> dos`, and assert the final file is `one\ndos\n`, `attempts == 2`, diagnostics preserve the first failed check, and final check output exits `0`.

- [ ] **Step 2: Run the focused test to verify RED**

Run: `/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test public_api coding_session_self_healing_edit_uses_planned_repair_attempts -- --nocapture`

Expected: compile failure because `SelfHealingEditRequest::with_repair_attempts` does not exist.

- [ ] **Step 3: Add request storage and builder**

Add `repair_attempts: Vec<Vec<SelfHealingEditReplacement>>` to `SelfHealingEditRequest`, expose `with_repair_attempts(...)`, `repair_attempts()`, and include attempts in `into_parts()`.

- [ ] **Step 4: Add a planned repair strategy adapter**

Add a crate-private `PlannedSelfHealingEditRepairStrategy` that implements `SelfHealingEditRepairStrategy` by returning the Nth configured repair attempt for the current repair attempt number.

- [ ] **Step 5: Wire session options**

In `CodingAgentSession::self_healing_edit_inner`, when repair attempts are present, install the planned strategy and set `max_repair_attempts` to the configured attempt count.

- [ ] **Step 6: Run the focused public API test to verify GREEN**

Run: `/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test public_api coding_session_self_healing_edit_uses_planned_repair_attempts -- --nocapture`

Expected: PASS.

### Task 2: RPC Planned Repair Attempts

**Files:**
- Modify: `crates/pi-coding-agent/src/protocol/types.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/commands.rs`
- Test: `crates/pi-coding-agent/tests/rpc_mode.rs`

- [ ] **Step 1: Write the failing RPC test**

Add `rpc_self_healing_edit_uses_planned_repair_attempts`. It should send `repairAttempts` as an array of edit arrays, with a failed first check and successful repair, and assert `success:true`, `attempts == 2`, final `checkOutput.exitCode == 0`, diagnostics include the first failed check, and the file content is repaired.

- [ ] **Step 2: Run the focused test to verify RED**

Run: `/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test rpc_mode rpc_self_healing_edit_uses_planned_repair_attempts -- --nocapture`

Expected: failure because RPC ignores `repairAttempts`.

- [ ] **Step 3: Decode `repairAttempts`**

Add optional `repair_attempts: Option<Vec<Vec<RpcSelfHealingEditReplacement>>>` with serde rename `repairAttempts` to the RPC command variant.

- [ ] **Step 4: Convert RPC attempts into request attempts**

In the RPC self-healing edit handler, map each repair attempt into `Vec<SelfHealingEditReplacement>` and call `request.with_repair_attempts(...)`.

- [ ] **Step 5: Run the focused RPC test to verify GREEN**

Run: `/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test rpc_mode rpc_self_healing_edit_uses_planned_repair_attempts -- --nocapture`

Expected: PASS.

### Task 3: Docs And Verification

**Files:**
- Modify: `docs/TODO.md`

- [ ] **Step 1: Update TODO**

Record that self-healing edit now has a bounded planned repair policy exposed through Rust API and RPC, while model-driven repair remains follow-up.

- [ ] **Step 2: Run full verification**

Run:
- `/home/whai/.cargo/bin/cargo fmt --check`
- `/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test public_api coding_session_self_healing_edit_uses_planned_repair_attempts -- --nocapture`
- `/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test rpc_mode rpc_self_healing_edit_uses_planned_repair_attempts -- --nocapture`
- `/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet`
- `/home/whai/.cargo/bin/cargo check --workspace --quiet`
- `/home/whai/.cargo/bin/cargo test --workspace --quiet`
- `git diff --check`

Expected: all commands exit 0.
