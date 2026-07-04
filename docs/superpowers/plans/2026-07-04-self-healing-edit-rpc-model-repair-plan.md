# Self-Healing Edit RPC Model Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose the bounded session-owned self-healing edit model repair policy through RPC without exposing raw provider, runtime, or session internals.

**Architecture:** Extend the RPC `self_healing_edit` command with `modelRepair: { maxAttempts?: number }`. The RPC adapter maps that product-level policy to `SelfHealingEditModelRepairOptions` using the current RPC model/API key/settings/session state to build `PromptTurnOptions`; callers cannot pass provider internals or arbitrary runtime snapshots. Planned `repairAttempts` remains supported, and the existing session API rejects requests that configure both planned and model repair.

**Tech Stack:** Rust 2024, `pi-coding-agent` RPC protocol types, `CodingAgentSession::self_healing_edit_with_options`, `SelfHealingEditModelRepairOptions`, faux provider RPC tests.

---

### Task 1: Add RED RPC Coverage

**Files:**
- Modify: `crates/pi-coding-agent/tests/rpc_mode.rs`

- [x] **Step 1: Add model repair RPC test**

Add `rpc_self_healing_edit_uses_model_repair_policy`. It should register a faux provider returning `{ "edits": [{ "oldText": "deux", "newText": "dos" }] }`, send a `self_healing_edit` command with `checkCommand: "grep -q dos src/app.txt"` and `modelRepair: { "maxAttempts": 1 }`, and assert the final file is `one\ndos\nthree\n`, response success is true, attempts is `2`, final check output exits `0`, and diagnostics include the first failed check.

- [x] **Step 2: Run RED test**

Run: `/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test rpc_mode rpc_self_healing_edit_uses_model_repair_policy -- --nocapture`

Expected: test fails because RPC ignores `modelRepair`, leaving the file at `one\ndeux\nthree\n` or returning failed check output.

### Task 2: Decode RPC Policy

**Files:**
- Modify: `crates/pi-coding-agent/src/protocol/types.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/commands.rs`

- [x] **Step 1: Add RPC model repair type**

Add `RpcSelfHealingEditModelRepair` with `max_attempts: Option<usize>` renamed from `maxAttempts`.

- [x] **Step 2: Extend `RpcCommand::SelfHealingEdit`**

Add `#[serde(rename = "modelRepair")] model_repair: Option<RpcSelfHealingEditModelRepair>` to the command variant.

- [x] **Step 3: Pass the field into the handler**

Update `handle_command` and `handle_self_healing_edit` so the field reaches request construction.

### Task 3: Map Policy To Session API

**Files:**
- Modify: `crates/pi-coding-agent/src/protocol/rpc/commands.rs`

- [x] **Step 1: Import runtime option types**

Import `SelfHealingEditModelRepairOptions`, `PromptTurnMode`, `PromptTurnOptions`, `PromptRunOptions`, `PromptInvocation`, and `AgentResources` where the RPC handler builds the request.

- [x] **Step 2: Build repair prompt options from RPC state**

Add a helper that creates `PromptTurnOptions::from_prompt_run_options(PromptRunOptions { ... }).with_mode(PromptTurnMode::Rpc)` using `self.model`, `self.api_key`, `self.options.session`, `self.session_name`, `self.thinking_level`, `self.options.tools`, `self.settings`, and a fixed product prompt string such as `repair self-healing edit`.

- [x] **Step 3: Attach `SelfHealingEditModelRepairOptions`**

When `modelRepair` is present, construct `SelfHealingEditModelRepairOptions::new(prompt_options).with_max_attempts(maxAttempts.unwrap_or(1))` and call `request.with_model_repair(...)`.

### Task 4: Update Docs And Verify

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-04-self-healing-edit-rpc-model-repair-plan.md`

- [x] **Step 1: Update TODO**

Add this plan to Source Documents and update the Phase 6 self-healing edit notes to say RPC exposes bounded `modelRepair` policy while interactive model repair exposure remains follow-up.

- [x] **Step 2: Mark plan steps complete**

Mark this plan's checkboxes as each step completes.

- [x] **Step 3: Run verification**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt --check
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test rpc_mode rpc_self_healing_edit_uses_model_repair_policy -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test rpc_mode --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet
/home/whai/.cargo/bin/cargo check --workspace --quiet
git diff --check
```

Expected: all commands exit 0.
