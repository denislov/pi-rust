# Self-Healing Edit Provider Tool Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route the provider-visible builtin `edit` tool through `SelfHealingEditFlow` while preserving the existing low-level edit algorithm and direct `edit_execute()` compatibility API.

**Architecture:** Keep `tools::edit::edit_execute()` and `edit_execute_with_operations()` as the low-level exact/fuzzy replacement implementation. Change `edit_tool()` and `edit_tool_with_operations()` so agent/provider-visible tool calls parse the same arguments, run `SelfHealingEditFlow`, and convert `SelfHealingEditOutcome` back to `AgentToolOutput` with the original diff/patch fields plus workflow metadata. This keeps the flow boundary visible to agent runtime paths without exposing session internals or requiring RPC/interactive command changes in this slice.

**Tech Stack:** Rust 2024, `AgentTool`, `SelfHealingEditFlow`, `FlowService`, existing `EditOperations`, deterministic tempfile-backed tool tests.

---

### Task 1: Add RED Test for Provider-Visible Edit Tool

**Files:**
- Modify: `crates/pi-coding-agent/tests/tool_edit.rs`

- [x] **Step 1: Add a builtin edit tool test**

Add `builtin_edit_tool_reports_self_healing_workflow_details`. The test should select the `edit` tool from `pi_coding_agent::builtin_tools(cwd)`, execute an edit, verify the file changed, verify existing `details.diff`/`details.patch` fields still exist, and verify `details.selfHealingEdit.attempts == 1`.

- [x] **Step 2: Run focused test and verify RED**

Run: `/home/whai/.cargo/bin/cargo test -p pi-coding-agent builtin_edit_tool_reports_self_healing_workflow_details --test tool_edit -- --nocapture`

Expected: failure because provider-visible `edit_tool()` still returns only the old low-level edit details without `selfHealingEdit` workflow metadata.

### Task 2: Route `edit_tool` Through SelfHealingEditFlow

**Files:**
- Modify: `crates/pi-coding-agent/src/tools/edit.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs` if minor visibility or conversion helpers are required.

- [x] **Step 1: Reuse existing argument parsing**

Keep `parse_edits()` as the single parser for legacy and array edit arguments. Convert parsed `Edit` values into `SelfHealingEditReplacement` values inside the provider-visible tool execution path.

- [x] **Step 2: Add flow execution helper for tool output**

Add a helper that creates `SelfHealingEditOptions::new(cwd, path, replacements).with_operations(ops)`, runs `FlowService::run_self_healing_edit()`, and converts the outcome into `AgentToolOutput`.

- [x] **Step 3: Preserve old output shape**

The flow-backed tool output should keep text message content, top-level `diff`, `patch`, and optional `firstChangedLine` fields. Add `selfHealingEdit` metadata with `attempts`, `diagnostics`, and optional `checkOutput`.

- [x] **Step 4: Keep direct algorithm entrypoints stable**

Do not change `edit_execute()` or `edit_execute_with_operations()` behavior. Existing direct edit tests should continue to pass.

### Task 3: Update Docs and Verify

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-04-self-healing-edit-provider-tool-migration-plan.md`

- [x] **Step 1: Update TODO progress**

Record that provider-visible builtin `edit` tool calls now route through `SelfHealingEditFlow`, while direct low-level edit helpers remain as compatibility/test entrypoints.

- [x] **Step 2: Mark this plan's completed steps**

Replace completed checkboxes with `[x]`.

- [x] **Step 3: Run verification**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt --check
/home/whai/.cargo/bin/cargo test -p pi-coding-agent builtin_edit_tool_reports_self_healing_workflow_details --test tool_edit -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test tool_edit -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet
/home/whai/.cargo/bin/cargo check --workspace --quiet
/home/whai/.cargo/bin/cargo test --workspace --quiet
git diff --check
```

Expected: all commands exit 0.
