# Self-Healing Edit Model Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a bounded session-owned model repair policy for self-healing edit after check-command failures.

**Architecture:** Keep the existing `SelfHealingEditRepairStrategy` as the internal Flow extension point. Expose a product-level `SelfHealingEditModelRepairOptions` wrapper around `PromptTurnOptions`, convert it to a crate-private `RuntimeSnapshot` inside `CodingAgentSession`, and inject a `ModelSelfHealingEditRepairStrategy` into `SelfHealingEditFlow`. The model strategy streams a deterministic text response through the configured model and parses a constrained JSON object shaped as `{ "edits": [{ "oldText": "...", "newText": "..." }] }`; RPC and interactive adapters remain follow-up work.

**Tech Stack:** Rust 2024, `pi-ai` provider registry/faux provider, `PromptTurnOptions`, `RuntimeSnapshot`, `SelfHealingEditFlow`, public API tests, deterministic check commands.

---

### Task 1: Add RED Public API Coverage

**Files:**
- Modify: `crates/pi-coding-agent/tests/public_api.rs`

- [x] **Step 1: Import model repair API and faux provider helpers**

Add `SelfHealingEditModelRepairOptions` to the public API import list and import `pi_agent_core::AgentResources`, `pi_ai::providers::faux::FauxProvider`, and `pi_ai::registry`.

- [x] **Step 2: Add model repair test**

Add `coding_session_self_healing_edit_uses_model_repair_strategy`. The test should create `src/app.txt` with `one\ntwo\n`, register a faux provider that returns `{ "edits": [{ "oldText": "deux", "newText": "dos" }] }`, configure a check command `grep -q dos src/app.txt`, configure `SelfHealingEditModelRepairOptions::new(repair_prompt_options)` on the request, and assert the final file is `one\ndos\n`, `attempts == 2`, diagnostics retain the failed check, and final check output exits `0`.

- [x] **Step 3: Run RED test**

Run: `/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test public_api coding_session_self_healing_edit_uses_model_repair_strategy -- --nocapture`

Expected: compile failure because `SelfHealingEditModelRepairOptions` and `SelfHealingEditRequest::with_model_repair` do not exist.

### Task 2: Add Product Request API

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/src/lib.rs`

- [x] **Step 1: Add `SelfHealingEditModelRepairOptions`**

Define a public struct containing `PromptTurnOptions` and a bounded `max_attempts` value. `new(prompt_options)` should default to one repair attempt. `with_max_attempts(attempts)` should clamp `0` to `1` so callers cannot request an inert model policy.

- [x] **Step 2: Extend `SelfHealingEditRequest`**

Add `model_repair: Option<SelfHealingEditModelRepairOptions>`, `with_model_repair(...)`, `model_repair()`, and include the value in `into_parts()`.

- [x] **Step 3: Export the public symbol**

Re-export `SelfHealingEditModelRepairOptions` from `coding_session` and `api`.

### Task 3: Implement Model Repair Strategy

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`

- [x] **Step 1: Add crate-private strategy**

Implement `ModelSelfHealingEditRepairStrategy` with a `RuntimeSnapshot` field. In `repair(...)`, build a user prompt from the path, previous replacement JSON, attempt number, and diagnostic messages. Stream through the `RuntimeService` global-runtime compatibility boundary using the runtime model and stream options derived from the runtime settings; do not call `pi_ai::registry::stream_model` directly from the workflow.

- [x] **Step 2: Parse constrained JSON**

Collect the final assistant text, parse it as JSON, and accept only an object with non-empty `edits` array containing `oldText` and `newText` strings. Return `Vec<SelfHealingEditReplacement>` or a clear strategy error.

- [x] **Step 3: Wire session-owned policy**

In `CodingAgentSession::self_healing_edit_inner`, reject requests that configure both planned repair attempts and model repair. For model repair, apply the default agent profile to the supplied `PromptTurnOptions`, extract its runtime snapshot, install `ModelSelfHealingEditRepairStrategy`, and set `max_repair_attempts` from the public options.

### Task 4: Verify and Update Docs

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-04-self-healing-edit-model-repair-plan.md`

- [x] **Step 1: Mark plan steps complete as they pass**

Update this plan from `[ ]` to `[x]` as each step is completed.

- [x] **Step 2: Update TODO**

Add this plan to Source Documents and update the Phase 6 self-healing edit note to say Rust API supports bounded session-owned model repair policy, while RPC/interactive model repair exposure remains follow-up.

- [x] **Step 3: Run verification**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt --check
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test public_api coding_session_self_healing_edit_uses_model_repair_strategy -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test public_api --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet
/home/whai/.cargo/bin/cargo check --workspace --quiet
git diff --check
```

Expected: all commands exit 0.
