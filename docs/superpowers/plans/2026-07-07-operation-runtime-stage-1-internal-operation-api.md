# Operation Runtime Stage 1 Internal Operation API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the first internal `Operation` boundary in `pi-coding-agent` and route the existing prompt entrypoint through it without changing public adapter behavior.

**Architecture:** Add a small `coding_session::operation` module that owns operation request/outcome typing, origin, and scheduler class metadata. Keep `CodingAgentSession` as the owner, and add a private `run_operation()` dispatcher that initially supports only `Operation::Prompt` by reusing the current prompt path. This is a vertical slice for the operation contract; it does not introduce the full IntentRouter, ProductEvent family split, or scheduler.

**Tech Stack:** Rust 2024, existing `pi-coding-agent` module tests, `cargo test -p pi-coding-agent operation`, `cargo test -p pi-coding-agent coding_session_public_api_symbols_are_importable`, `cargo fmt --check`.

---

### Task 1: Add Internal Operation Contract

**Files:**
- Create: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Test: `crates/pi-coding-agent/src/coding_session/operation.rs`

- [x] **Step 1: Write the failing operation metadata tests**

Create `crates/pi-coding-agent/src/coding_session/operation.rs` with only the test module and imports that describe the desired API:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::PromptInvocation;

    #[test]
    fn prompt_operation_declares_root_session_write_metadata() {
        let operation = Operation::Prompt(PromptTurnOptions::new(PromptInvocation::Text(
            "hello".into(),
        )));

        assert_eq!(operation.kind(), OperationKind::Prompt);
        assert_eq!(operation.origin(), OperationOrigin::ClientRoot);
        assert_eq!(operation.class(), OperationClass::SessionWriteRoot);
    }

    #[test]
    fn prompt_operation_outcome_exposes_prompt_payload() {
        let outcome = OperationOutcome::Prompt(PromptTurnOutcome::Aborted {
            operation_id: "op_test".into(),
            turn_id: Some("turn_test".into()),
            reason: "user cancelled".into(),
            session_id: None,
        });

        assert!(matches!(
            outcome,
            OperationOutcome::Prompt(PromptTurnOutcome::Aborted { reason, .. })
                if reason == "user cancelled"
        ));
    }
}
```

Add `mod operation;` to the module list in `crates/pi-coding-agent/src/coding_session/mod.rs` so the test compiles far enough to fail on missing types.

- [x] **Step 2: Run the operation test and verify RED**

Run:

```bash
cargo test -p pi-coding-agent operation --lib
```

Expected: FAIL to compile because `Operation`, `OperationOrigin`, `OperationClass`, and `OperationOutcome` are not defined yet.

- [x] **Step 3: Implement the minimal operation contract**

Replace `crates/pi-coding-agent/src/coding_session/operation.rs` with:

```rust
use super::operation_control::OperationKind;
use super::prompt::{PromptTurnOptions, PromptTurnOutcome};

#[derive(Debug)]
pub(crate) enum Operation {
    Prompt(PromptTurnOptions),
}

impl Operation {
    pub(crate) fn kind(&self) -> OperationKind {
        match self {
            Self::Prompt(_) => OperationKind::Prompt,
        }
    }

    pub(crate) fn origin(&self) -> OperationOrigin {
        match self {
            Self::Prompt(_) => OperationOrigin::ClientRoot,
        }
    }

    pub(crate) fn class(&self) -> OperationClass {
        match self {
            Self::Prompt(_) => OperationClass::SessionWriteRoot,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationOrigin {
    ClientRoot,
    ParentChild,
    RuntimeInternal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationClass {
    Query,
    ReadOnly,
    SessionWriteRoot,
    NonSessionRoot,
    RuntimeWrite,
    Child,
    Control,
}

#[derive(Debug)]
pub(crate) enum OperationOutcome {
    Prompt(PromptTurnOutcome),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::PromptInvocation;

    #[test]
    fn prompt_operation_declares_root_session_write_metadata() {
        let operation = Operation::Prompt(PromptTurnOptions::new(PromptInvocation::Text(
            "hello".into(),
        )));

        assert_eq!(operation.kind(), OperationKind::Prompt);
        assert_eq!(operation.origin(), OperationOrigin::ClientRoot);
        assert_eq!(operation.class(), OperationClass::SessionWriteRoot);
    }

    #[test]
    fn prompt_operation_outcome_exposes_prompt_payload() {
        let outcome = OperationOutcome::Prompt(PromptTurnOutcome::Aborted {
            operation_id: "op_test".into(),
            turn_id: Some("turn_test".into()),
            reason: "user cancelled".into(),
            session_id: None,
        });

        assert!(matches!(
            outcome,
            OperationOutcome::Prompt(PromptTurnOutcome::Aborted { reason, .. })
                if reason == "user cancelled"
        ));
    }
}
```

- [x] **Step 4: Run the operation test and verify GREEN**

Run:

```bash
cargo test -p pi-coding-agent operation --lib
```

Expected: PASS for the new operation tests.

### Task 2: Route Prompt Through Internal Operation Dispatcher

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Test: `crates/pi-coding-agent/src/coding_session/mod.rs`

- [x] **Step 1: Write the failing routing test**

In the existing `#[cfg(test)] mod tests` in `crates/pi-coding-agent/src/coding_session/mod.rs`, add this test near the other prompt/session tests:

```rust
#[tokio::test]
async fn run_operation_prompt_uses_prompt_guard_and_preserves_prompt_error() {
    let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
        .await
        .unwrap();
    let operation = Operation::Prompt(PromptTurnOptions::new(PromptInvocation::Text(
        "hello".into(),
    )));

    let error = session.run_operation(operation).await.unwrap_err();

    assert_eq!(error.code(), "config");
    assert!(error.to_string().contains("runtime snapshot"), "{error}");
    assert_eq!(session.operation_control.active(), None);
}
```

Import the new operation types in `mod.rs` for production and tests:

```rust
use operation::{Operation, OperationOutcome};
```

- [x] **Step 2: Run the routing test and verify RED**

Run:

```bash
cargo test -p pi-coding-agent run_operation_prompt_uses_prompt_guard_and_preserves_prompt_error --lib
```

Expected: FAIL to compile because `CodingAgentSession::run_operation` does not exist yet.

- [x] **Step 3: Implement minimal dispatcher and route `prompt()` through it**

In `impl CodingAgentSession`, replace the current `prompt()` body with:

```rust
pub async fn prompt(
    &mut self,
    options: PromptTurnOptions,
) -> Result<PromptTurnOutcome, CodingSessionError> {
    match self.run_operation(Operation::Prompt(options)).await? {
        OperationOutcome::Prompt(outcome) => Ok(outcome),
    }
}
```

Add this private dispatcher near `prompt_inner()`:

```rust
async fn run_operation(
    &mut self,
    operation: Operation,
) -> Result<OperationOutcome, CodingSessionError> {
    let kind = operation.kind();
    let _operation_guard = self.operation_control.begin(kind)?;

    match operation {
        Operation::Prompt(options) => {
            let result = self.prompt_inner(options).await;
            self.operation_control.clear_prompt_control_receiver();
            result.map(OperationOutcome::Prompt)
        }
    }
}
```

The prompt-control receiver cleanup stays in the prompt operation arm so existing prompt abort/steer/follow-up behavior is preserved.

- [x] **Step 4: Run the routing test and verify GREEN**

Run:

```bash
cargo test -p pi-coding-agent run_operation_prompt_uses_prompt_guard_and_preserves_prompt_error --lib
```

Expected: PASS.

- [x] **Step 5: Run focused prompt/public API checks**

Run:

```bash
cargo test -p pi-coding-agent operation --lib
cargo test -p pi-coding-agent coding_session_public_api_symbols_are_importable
```

Expected: both commands pass.

### Task 3: Update Architecture TODO For Stage 1 Start

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-07-operation-runtime-stage-1-internal-operation-api.md`

- [x] **Step 1: Mark Stage 1 as the active cut in TODO**

Update the existing operation runtime reference architecture TODO line to mention that Stage 1 has started with the internal operation API and prompt routing slice.

Use wording like:

```markdown
- [~] Adopt the operation runtime reference architecture as the next simplification target. The 2026-07-07 reference architecture now records a current-state-aware contract for narrowing `CodingAgentSession`, normalizing operation admission, grouping product events, defining snapshot semantics, and hardening SessionEvent/ProductEvent boundaries. Stage 1 has started with an internal `Operation`/`OperationOutcome` contract and the prompt entrypoint routing through that operation dispatcher while preserving current public behavior.
```

- [x] **Step 2: Mark completed plan steps**

After executing each task, update this plan file's checkboxes for the steps that were actually completed.

### Task 4: Verification

**Files:**
- Verify: Rust code and markdown docs

- [x] **Step 1: Format check**

Run:

```bash
cargo fmt --check
```

Expected: PASS. If it fails only due to formatting, run `cargo fmt`, then rerun `cargo fmt --check`.

- [x] **Step 2: Focused crate checks**

Run:

```bash
cargo test -p pi-coding-agent operation --lib
cargo test -p pi-coding-agent run_operation_prompt_uses_prompt_guard_and_preserves_prompt_error --lib
cargo test -p pi-coding-agent coding_session_public_api_symbols_are_importable
cargo check -p pi-coding-agent
```

Expected: all commands pass.

- [x] **Step 3: Diff hygiene**

Run:

```bash
git diff --check
git status --short
```

Expected: no diff-check errors; status shows only the intended docs and `pi-coding-agent` files.

### Task 5: Route Manual Compaction Through Internal Operation Dispatcher

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`

- [x] **Step 1: Write the failing manual compaction operation metadata test**

Added `manual_compaction_operation_declares_root_session_write_metadata` to prove `Operation::ManualCompaction` maps to `OperationKind::Compact`, `OperationOrigin::ClientRoot`, and `OperationClass::SessionWriteRoot`.

Verification:

```bash
cargo test -p pi-coding-agent manual_compaction_operation_declares_root_session_write_metadata --lib
```

RED result: compile failed because `Operation::ManualCompaction` did not exist.

- [x] **Step 2: Add the minimal manual compaction operation contract**

Added `Operation::ManualCompaction(PromptTurnOptions)` and `OperationOutcome::ManualCompaction(PromptTurnOutcome)`, then updated `kind()`, `origin()`, and `class()`.

Verification:

```bash
cargo test -p pi-coding-agent manual_compaction_operation_declares_root_session_write_metadata --lib
```

GREEN result: the metadata test passed after adding temporary exhaustive handling in `CodingAgentSession`.

- [x] **Step 3: Write the failing manual compaction dispatcher test**

Added `run_operation_manual_compaction_uses_compact_guard_and_preserves_config_error` to require `Operation::ManualCompaction` to run through the dispatcher, use the compact guard, preserve the existing missing-runtime config error, and clear active operation state on error.

Verification:

```bash
cargo test -p pi-coding-agent run_operation_manual_compaction_uses_compact_guard_and_preserves_config_error --lib
```

RED result: the test failed with `unsupported_capability` from the temporary dispatcher placeholder.

- [x] **Step 4: Move manual compaction routing into `run_operation()`**

Changed `CodingAgentSession::compact()` to call `run_operation(Operation::ManualCompaction(options))`, and moved the existing `ManualCompactionOptions` conversion plus persistent-session execution path into the dispatcher arm.

Verification:

```bash
cargo test -p pi-coding-agent run_operation_manual_compaction_uses_compact_guard_and_preserves_config_error --lib
```

GREEN result: the dispatcher test passed.

### Task 6: Verify Manual Compaction Operation Slice

- [x] **Step 1: Run operation-focused tests**

```bash
cargo test -p pi-coding-agent operation --lib
cargo test -p pi-coding-agent run_operation_prompt_uses_prompt_guard_and_preserves_prompt_error --lib
cargo test -p pi-coding-agent run_operation_manual_compaction_uses_compact_guard_and_preserves_config_error --lib
```

- [x] **Step 2: Run formatting, crate check, and diff hygiene**

```bash
cargo fmt --check
cargo check -p pi-coding-agent
git diff --check
git status --short
```

### Task 7: Route Plugin Load Through Internal Operation Dispatcher

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`

- [x] **Step 1: Write the failing plugin load operation metadata test**

Added `plugin_load_operation_declares_runtime_write_metadata` to prove `Operation::PluginLoad` maps to `OperationKind::PluginLoad`, `OperationOrigin::ClientRoot`, and `OperationClass::RuntimeWrite`.

Verification:

```bash
cargo test -p pi-coding-agent plugin_load_operation_declares_runtime_write_metadata --lib
```

RED result: compile failed because `Operation::PluginLoad` did not exist.

- [x] **Step 2: Add the minimal plugin load operation contract**

Added `Operation::PluginLoad(PluginLoadOptions)`, mapped its metadata, and used a temporary dispatcher placeholder so the request contract could compile independently.

Verification:

```bash
cargo test -p pi-coding-agent plugin_load_operation_declares_runtime_write_metadata --lib
```

GREEN result: the metadata test passed, with the expected temporary dead-code warning before dispatcher routing consumed the payload.

- [x] **Step 3: Write the failing plugin load dispatcher test**

Added `run_operation_plugin_load_uses_plugin_load_guard_and_returns_outcome` to require `Operation::PluginLoad` to run through the dispatcher, return `OperationOutcome::PluginLoad`, and clear active operation state after a successful empty plugin load.

Verification:

```bash
cargo test -p pi-coding-agent run_operation_plugin_load_uses_plugin_load_guard_and_returns_outcome --lib
```

RED result: compile failed because `OperationOutcome::PluginLoad` did not exist.

- [x] **Step 4: Move plugin load routing into `run_operation()`**

Changed `CodingAgentSession::load_plugins()` to call `run_operation(Operation::PluginLoad(options))`, and moved the existing `load_plugins_inner(options)` execution behind the dispatcher arm.

Verification:

```bash
cargo test -p pi-coding-agent run_operation_plugin_load_uses_plugin_load_guard_and_returns_outcome --lib
```

GREEN result: the dispatcher test passed.

### Task 8: Verify Plugin Load Operation Slice

- [x] **Step 1: Run operation-focused tests**

```bash
cargo test -p pi-coding-agent operation --lib
cargo test -p pi-coding-agent run_operation_plugin_load_uses_plugin_load_guard_and_returns_outcome --lib
cargo test -p pi-coding-agent load_plugins --lib
```

- [x] **Step 2: Run formatting, crate check, and diff hygiene**

```bash
cargo fmt --check
cargo check -p pi-coding-agent
git diff --check
git status --short
```

### Task 9: Route Branch Summary Through Internal Operation Dispatcher

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`

- [x] **Step 1: Write the failing branch summary operation metadata test**

Added `branch_summary_operation_declares_root_session_write_metadata` to prove `Operation::BranchSummary` maps to `OperationKind::BranchSummary`, `OperationOrigin::ClientRoot`, and `OperationClass::SessionWriteRoot`.

Verification:

```bash
cargo test -p pi-coding-agent branch_summary_operation_declares_root_session_write_metadata --lib
```

RED result: compile failed because `Operation::BranchSummary` did not exist.

- [x] **Step 2: Add the minimal branch summary operation contract**

Added `Operation::BranchSummary { options, source_leaf_id, target_leaf_id, custom_instructions }`, mapped its metadata, and used a temporary dispatcher placeholder so the request contract could compile independently.

Verification:

```bash
cargo test -p pi-coding-agent branch_summary_operation_declares_root_session_write_metadata --lib
```

GREEN result: the metadata test passed, with the expected temporary dead-code warning before dispatcher routing consumed the payload.

- [x] **Step 3: Write the failing branch summary dispatcher test**

Added `run_operation_branch_summary_uses_branch_summary_guard_and_preserves_persistence_error` to require `Operation::BranchSummary` to run through the dispatcher, preserve the existing non-persistent-session error, and clear active operation state on error.

Verification:

```bash
cargo test -p pi-coding-agent run_operation_branch_summary_uses_branch_summary_guard_and_preserves_persistence_error --lib
```

RED result: the test failed with the temporary dispatcher placeholder error.

- [x] **Step 4: Move branch summary routing into `run_operation()`**

Changed `CodingAgentSession::summarize_branch()` to call `run_operation(Operation::BranchSummary { ... })`. `summarize_branch_for_navigation()` keeps its existing idle/reuse preflight and routes only the non-reused execution path through the dispatcher.

Verification:

```bash
cargo test -p pi-coding-agent run_operation_branch_summary_uses_branch_summary_guard_and_preserves_persistence_error --lib
```

GREEN result: the dispatcher test passed.

### Task 10: Verify Branch Summary Operation Slice

- [x] **Step 1: Run operation-focused and branch summary tests**

```bash
cargo test -p pi-coding-agent operation --lib
cargo test -p pi-coding-agent run_operation_branch_summary_uses_branch_summary_guard_and_preserves_persistence_error --lib
cargo test -p pi-coding-agent branch_summary --lib
cargo test -p pi-coding-agent coding_session_public_api_symbols_are_importable
```

- [x] **Step 2: Run formatting, crate check, and diff hygiene**

```bash
cargo fmt --check
cargo check -p pi-coding-agent
git diff --check
git status --short
```

### Task 11: Route Self-Healing Edit Through Internal Operation Dispatcher

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`

- [x] **Step 1: Write the failing self-healing edit operation metadata test**

Added `self_healing_edit_operation_declares_root_session_write_metadata` to prove `Operation::SelfHealingEdit` maps to `OperationKind::SelfHealingEdit`, `OperationOrigin::ClientRoot`, and `OperationClass::SessionWriteRoot`.

Verification:

```bash
cargo test -p pi-coding-agent self_healing_edit_operation_declares_root_session_write_metadata --lib
```

RED result: compile failed because `Operation::SelfHealingEdit` did not exist.

- [x] **Step 2: Add the minimal self-healing edit operation contract**

Added `Operation::SelfHealingEdit(SelfHealingEditRequest)`, mapped its metadata, and used a temporary dispatcher placeholder so the request contract could compile independently.

Verification:

```bash
cargo test -p pi-coding-agent self_healing_edit_operation_declares_root_session_write_metadata --lib
```

GREEN result: the metadata test passed, with the expected temporary dead-code warning before dispatcher routing consumed the payload.

- [x] **Step 3: Write the failing self-healing edit dispatcher test**

Added `run_operation_self_healing_edit_uses_guard_and_preserves_persistence_error` to require `Operation::SelfHealingEdit` to run through the dispatcher, preserve the existing non-persistent-session error, and clear active operation state on error.

Verification:

```bash
cargo test -p pi-coding-agent run_operation_self_healing_edit_uses_guard_and_preserves_persistence_error --lib
```

RED result: the test failed with the temporary dispatcher placeholder error.

- [x] **Step 4: Move self-healing edit routing into `run_operation()`**

Changed `CodingAgentSession::self_healing_edit_with_options()` to call `run_operation(Operation::SelfHealingEdit(request))`, and moved the existing request decomposition, repair-policy validation, model-repair policy construction, persistent-session gate, service execution, and finalized session-write event emission into the dispatcher arm.

Verification:

```bash
cargo test -p pi-coding-agent run_operation_self_healing_edit_uses_guard_and_preserves_persistence_error --lib
```

GREEN result: the dispatcher test passed.

### Task 12: Verify Self-Healing Edit Operation Slice

- [x] **Step 1: Run operation-focused and self-healing edit tests**

```bash
cargo test -p pi-coding-agent operation --lib
cargo test -p pi-coding-agent run_operation_self_healing_edit_uses_guard_and_preserves_persistence_error --lib
cargo test -p pi-coding-agent self_healing_edit --lib
cargo test -p pi-coding-agent coding_session_public_api_symbols_are_importable
```

- [x] **Step 2: Run formatting, crate check, and diff hygiene**

```bash
cargo fmt --check
cargo check -p pi-coding-agent
git diff --check
git status --short
```

### Task 13: Route Agent Invocation Through Internal Operation Dispatcher

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`

- [x] **Step 1: Write the failing agent invocation operation metadata test**

Added `agent_invocation_operation_declares_root_non_session_metadata` to prove `Operation::AgentInvocation` maps to `OperationKind::AgentInvocation`, `OperationOrigin::ClientRoot`, and `OperationClass::NonSessionRoot`.

Verification:

```bash
cargo test -p pi-coding-agent agent_invocation_operation_declares_root_non_session_metadata --lib
```

RED result: compile failed because `Operation::AgentInvocation` did not exist.

- [x] **Step 2: Add the minimal agent invocation operation contract**

Added `Operation::AgentInvocation(AgentInvocationOptions)`, mapped its metadata, and used a temporary dispatcher placeholder so the request contract could compile independently.

Verification:

```bash
cargo test -p pi-coding-agent agent_invocation_operation_declares_root_non_session_metadata --lib
```

GREEN result: the metadata test passed, with the expected temporary dead-code warning before dispatcher routing consumed the payload.

- [x] **Step 3: Write the failing agent invocation dispatcher test**

Added `run_operation_agent_invocation_uses_guard_and_preserves_input_error` to require `Operation::AgentInvocation` to run through the dispatcher, preserve the existing empty-task input error, clear active operation state, and leave prompt-control receiver lifecycle reusable after the operation.

Verification:

```bash
cargo test -p pi-coding-agent run_operation_agent_invocation_uses_guard_and_preserves_input_error --lib
```

RED result: the test failed with the temporary dispatcher placeholder error.

- [x] **Step 4: Move agent invocation routing into `run_operation()`**

Changed `CodingAgentSession::invoke_agent()` to call `run_operation(Operation::AgentInvocation(options))`, and moved the existing `invoke_agent_inner(options)` execution plus prompt-control receiver cleanup into the dispatcher arm.

Verification:

```bash
cargo test -p pi-coding-agent run_operation_agent_invocation_uses_guard_and_preserves_input_error --lib
```

GREEN result: the dispatcher test passed.

### Task 14: Verify Agent Invocation Operation Slice

- [x] **Step 1: Run operation-focused and agent invocation tests**

```bash
cargo test -p pi-coding-agent operation --lib
cargo test -p pi-coding-agent run_operation_agent_invocation_uses_guard_and_preserves_input_error --lib
cargo test -p pi-coding-agent agent_invocation
cargo test -p pi-coding-agent coding_session_public_api_symbols_are_importable
```

- [x] **Step 2: Run formatting, crate check, and diff hygiene**

```bash
cargo fmt --check
cargo check -p pi-coding-agent
git diff --check
git status --short
```

### Task 15: Route Team Invocation Through Internal Operation Dispatcher

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`

- [x] **Step 1: Write the failing team invocation operation metadata test**

Added `agent_team_operation_declares_root_non_session_metadata` to prove `Operation::AgentTeam` maps to `OperationKind::AgentTeam`, `OperationOrigin::ClientRoot`, and `OperationClass::NonSessionRoot`.

Verification:

```bash
cargo test -p pi-coding-agent agent_team_operation_declares_root_non_session_metadata --lib
```

RED result: compile failed because `Operation::AgentTeam` did not exist.

- [x] **Step 2: Add the minimal team invocation operation contract**

Added `Operation::AgentTeam(AgentTeamOptions)`, mapped its metadata, and used a temporary dispatcher placeholder so the request contract could compile independently.

Verification:

```bash
cargo test -p pi-coding-agent agent_team_operation_declares_root_non_session_metadata --lib
```

GREEN result: the metadata test passed, with the expected temporary dead-code warning before dispatcher routing consumed the payload.

- [x] **Step 3: Write the failing team invocation dispatcher test**

Added `run_operation_agent_team_uses_guard_and_preserves_input_error` to require `Operation::AgentTeam` to run through the dispatcher, preserve the existing empty-task input error, and clear active operation state on error.

Verification:

```bash
cargo test -p pi-coding-agent run_operation_agent_team_uses_guard_and_preserves_input_error --lib
```

RED result: the test failed with the temporary dispatcher placeholder error.

- [x] **Step 4: Move team invocation routing into `run_operation()`**

Changed `CodingAgentSession::invoke_team()` to call `run_operation(Operation::AgentTeam(options))`, and moved the existing `invoke_team_inner(options)` execution into the dispatcher arm.

Verification:

```bash
cargo test -p pi-coding-agent run_operation_agent_team_uses_guard_and_preserves_input_error --lib
```

GREEN result: the dispatcher test passed.

### Task 16: Verify Team Invocation Operation Slice

- [x] **Step 1: Run operation-focused and team invocation tests**

```bash
cargo test -p pi-coding-agent operation --lib
cargo test -p pi-coding-agent run_operation_agent_team_uses_guard_and_preserves_input_error --lib
cargo test -p pi-coding-agent agent_team
cargo test -p pi-coding-agent coding_session_public_api_symbols_are_importable
```

- [x] **Step 2: Run formatting, crate check, and diff hygiene**

```bash
cargo fmt --check
cargo check -p pi-coding-agent
git diff --check
git status --short
```

### Task 17: Route Export Through Sync Operation Dispatcher

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`

- [x] **Step 1: Write the failing export operation metadata test**

Added `export_operation_declares_root_read_only_metadata` to prove `Operation::Export` maps to `OperationKind::Export`, `OperationOrigin::ClientRoot`, and `OperationClass::ReadOnly`.

Verification:

```bash
cargo test -p pi-coding-agent export_operation_declares_root_read_only_metadata --lib
```

RED result: compile failed because `Operation::Export` did not exist.

- [x] **Step 2: Add the minimal export operation contract**

Added `Operation::Export(ExportOptions)` and mapped it as a root read-only operation. A temporary async-dispatcher placeholder kept the contract compiling until the sync dispatcher consumed the payload.

Verification:

```bash
cargo test -p pi-coding-agent export_operation_declares_root_read_only_metadata --lib
```

GREEN result: the metadata test passed, with the expected temporary dead-code warning before sync dispatcher routing consumed the payload.

- [x] **Step 3: Write the failing sync dispatcher test**

Added `run_sync_operation_export_uses_guard_and_preserves_persistence_error` to require `Operation::Export` to run through a sync dispatcher, preserve the existing non-persistent-session export error, and clear active operation state on error.

Verification:

```bash
cargo test -p pi-coding-agent run_sync_operation_export_uses_guard_and_preserves_persistence_error --lib
```

RED result: compile failed because `CodingAgentSession::run_sync_operation()` did not exist.

- [x] **Step 4: Move current-session export routing into `run_sync_operation()`**

Changed `CodingAgentSession::export_current()` and `CodingAgentSession::export_current_html()` to call `run_sync_operation(Operation::Export(...))`, and moved the existing persistent-session gate plus export Flow execution into `export_current_inner()`.

Verification:

```bash
cargo test -p pi-coding-agent run_sync_operation_export_uses_guard_and_preserves_persistence_error --lib
```

GREEN result: the dispatcher test passed.

### Task 18: Verify Export Operation Slice

- [x] **Step 1: Run operation-focused and export tests**

```bash
cargo test -p pi-coding-agent operation --lib
cargo test -p pi-coding-agent run_sync_operation_export_uses_guard_and_preserves_persistence_error --lib
cargo test -p pi-coding-agent export_current_html_uses_export_operation_boundary --lib
cargo test -p pi-coding-agent export_current_html
cargo test -p pi-coding-agent coding_session_public_api_symbols_are_importable
```

- [x] **Step 2: Run formatting, crate check, and diff hygiene**

```bash
cargo fmt --check
cargo check -p pi-coding-agent
git diff --check
git status --short
```

### Task 19: Route Plugin Command Through Sync Operation Dispatcher

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`

- [x] **Step 1: Write the failing plugin command operation metadata test**

Added `plugin_command_operation_declares_root_non_session_metadata` to prove `Operation::PluginCommand` maps to `OperationKind::PluginCommand`, `OperationOrigin::ClientRoot`, and `OperationClass::NonSessionRoot`.

Verification:

```bash
cargo test -p pi-coding-agent plugin_command_operation_declares_root_non_session_metadata --lib
```

RED result: compile failed because `Operation::PluginCommand` did not exist.

- [x] **Step 2: Add the minimal plugin command operation contract**

Added `Operation::PluginCommand { command_id, args }` and mapped it as a root non-session operation. Temporary dispatcher placeholders kept the contract compiling until the sync dispatcher consumed the payload.

Verification:

```bash
cargo test -p pi-coding-agent plugin_command_operation_declares_root_non_session_metadata --lib
```

GREEN result: the metadata test passed, with the expected temporary dead-code warning before sync dispatcher routing consumed the payload.

- [x] **Step 3: Write the failing sync dispatcher test**

Added `run_sync_operation_plugin_command_uses_guard_and_preserves_plugin_error` to require `Operation::PluginCommand` to run through the sync dispatcher, preserve the existing missing-command plugin error, and clear active operation state on error.

Verification:

```bash
cargo test -p pi-coding-agent run_sync_operation_plugin_command_uses_guard_and_preserves_plugin_error --lib
```

RED result: the test failed with the temporary unsupported-capability placeholder instead of the plugin-service error.

- [x] **Step 4: Move plugin command routing into `run_sync_operation()`**

Changed `CodingAgentSession::run_plugin_command()` to call `run_sync_operation(Operation::PluginCommand { ... })`, and moved the existing plugin service command execution behind the dispatcher arm.

Verification:

```bash
cargo test -p pi-coding-agent run_sync_operation_plugin_command_uses_guard_and_preserves_plugin_error --lib
```

GREEN result: the dispatcher test passed.

### Task 20: Verify Plugin Command Operation Slice

- [x] **Step 1: Run operation-focused and plugin command tests**

```bash
cargo test -p pi-coding-agent operation --lib
cargo test -p pi-coding-agent run_sync_operation_plugin_command_uses_guard_and_preserves_plugin_error --lib
cargo test -p pi-coding-agent run_plugin_command
cargo test -p pi-coding-agent plugin_command
cargo test -p pi-coding-agent coding_session_public_api_symbols_are_importable
```

- [x] **Step 2: Run formatting, crate check, and diff hygiene**

```bash
cargo fmt --check
cargo check -p pi-coding-agent
git diff --check
git status --short
```

### Task 21: Route Delegation Approval Through Dynamic Operation Admission

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`

- [x] **Step 1: Write the failing dynamic delegation approval operation metadata test**

Added `delegation_approval_operation_declares_dynamic_root_non_session_metadata` to prove `Operation::ApproveDelegationConfirmation` has no static operation kind, while still declaring `OperationOrigin::ClientRoot` and `OperationClass::NonSessionRoot`.

Verification:

```bash
cargo test -p pi-coding-agent delegation_approval_operation_declares_dynamic_root_non_session_metadata --lib
```

RED result: compile failed because `Operation::ApproveDelegationConfirmation` did not exist.

- [x] **Step 2: Add the minimal dynamic delegation approval operation contract**

Added `Operation::ApproveDelegationConfirmation { operation_id, tool_call_id }` and `Operation::static_kind()`. Static operations continue to expose `kind()`, while delegation approval returns no static kind because its admission kind depends on the pending confirmation target.

Verification:

```bash
cargo test -p pi-coding-agent delegation_approval_operation_declares_dynamic_root_non_session_metadata --lib
```

GREEN result: the metadata test passed, with the expected temporary dead-code warning before dispatcher routing consumed the payload.

- [x] **Step 3: Write the failing dynamic admission dispatcher tests**

Added `delegation_approval_operation_kind_uses_pending_team_target` to require pending team confirmations to resolve to `OperationKind::AgentTeam`, and `run_operation_delegation_approval_preserves_missing_pending_before_busy` to preserve the existing pending-not-found error before acquiring any busy guard.

Verification:

```bash
cargo test -p pi-coding-agent delegation_approval_operation_kind_uses_pending_team_target --lib
cargo test -p pi-coding-agent run_operation_delegation_approval_preserves_missing_pending_before_busy --lib
```

RED result: compile failed because the dynamic admission resolver did not exist.

- [x] **Step 4: Move delegation approval routing into `run_operation()`**

Changed `CodingAgentSession::approve_delegation_confirmation()` to call `run_operation(Operation::ApproveDelegationConfirmation { ... })`. Added dynamic admission resolution that reads the active pending confirmation first, maps agent targets to `OperationKind::AgentInvocation` and team targets to `OperationKind::AgentTeam`, then acquires the operation guard and executes the existing approval/delegated execution flow through an inner method.

Verification:

```bash
cargo test -p pi-coding-agent delegation_approval_operation_kind_uses_pending_team_target --lib
cargo test -p pi-coding-agent run_operation_delegation_approval_preserves_missing_pending_before_busy --lib
```

GREEN result: both dynamic admission tests passed.

### Task 22: Verify Delegation Approval Operation Slice

- [x] **Step 1: Run operation-focused and delegation approval tests**

```bash
cargo test -p pi-coding-agent operation --lib
cargo test -p pi-coding-agent delegation_approval --lib
cargo test -p pi-coding-agent delegation_execution
cargo test -p pi-coding-agent coding_session_public_api_symbols_are_importable
```

- [x] **Step 2: Run formatting, crate check, and diff hygiene**

```bash
cargo fmt --check
cargo check -p pi-coding-agent
git diff --check
rg -n "operation_control\.begin" crates/pi-coding-agent/src/coding_session crates/pi-coding-agent/src/interactive crates/pi-coding-agent/src/protocol
git status --short
```

### Task 23: Route Delegation Rejection Through Sync-Mutable Operation Admission

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/operation_control.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`

- [x] **Step 1: Write the failing delegation rejection operation metadata test**

Added `delegation_rejection_operation_declares_root_control_metadata` to prove `Operation::RejectDelegationConfirmation` maps to `OperationKind::DelegationConfirmation`, `OperationOrigin::ClientRoot`, and `OperationClass::Control`.

Verification:

```bash
cargo test -p pi-coding-agent delegation_rejection_operation_declares_root_control_metadata --lib
```

RED result: compile failed because `Operation::RejectDelegationConfirmation` and `OperationKind::DelegationConfirmation` did not exist.

- [x] **Step 2: Add the minimal delegation rejection operation contract**

Added `Operation::RejectDelegationConfirmation { operation_id, tool_call_id, reason }`, `OperationKind::DelegationConfirmation`, and the root-control operation metadata. Temporary dispatcher placeholders kept the contract compiling until routing consumed the payload.

Verification:

```bash
cargo test -p pi-coding-agent delegation_rejection_operation_declares_root_control_metadata --lib
```

GREEN result: the metadata test passed, with the expected temporary dead-code warning before dispatcher routing consumed the payload.

- [x] **Step 3: Write the failing guarded rejection test**

Added `reject_delegation_confirmation_reports_busy_before_mutating_pending_confirmation` to require the public rejection entrypoint to observe the operation guard before mutating the pending confirmation queue.

Verification:

```bash
cargo test -p pi-coding-agent reject_delegation_confirmation_reports_busy_before_mutating_pending_confirmation --lib
```

RED result: the test failed because the existing public rejection method bypassed operation admission and directly removed the pending confirmation.

- [x] **Step 4: Move delegation rejection routing into a sync-mutable dispatcher**

Changed `CodingAgentSession::reject_delegation_confirmation()` to call `run_sync_mut_operation(Operation::RejectDelegationConfirmation { ... })`. Added the sync-mutable dispatcher for synchronous operations that need `&mut self`, acquired the operation guard before queue/session mutation, and returned `OperationOutcome::DelegationRejection`.

Verification:

```bash
cargo test -p pi-coding-agent reject_delegation_confirmation_reports_busy_before_mutating_pending_confirmation --lib
```

GREEN result: the guarded rejection test passed and the operation payload is consumed by the dispatcher.

### Task 24: Verify Delegation Rejection Operation Slice

- [x] **Step 1: Run operation-focused and delegation rejection tests**

```bash
cargo test -p pi-coding-agent operation --lib
cargo test -p pi-coding-agent reject_delegation_confirmation_reports_busy_before_mutating_pending_confirmation --lib
cargo test -p pi-coding-agent delegation_execution
cargo test -p pi-coding-agent coding_session_public_api_symbols_are_importable
```

- [x] **Step 2: Run formatting, crate check, and diff hygiene**

```bash
cargo fmt --check
cargo check -p pi-coding-agent
git diff --check
rg -n "operation_control\.begin" crates/pi-coding-agent/src/coding_session crates/pi-coding-agent/src/interactive crates/pi-coding-agent/src/protocol
git status --short
```

### Task 25: Structure Operation Metadata And Admission

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`

- [x] **Step 1: Write failing structured metadata tests**

Added `operation_metadata_exposes_static_contract_and_dispatch_mode` and `dynamic_operation_metadata_exposes_dispatch_without_static_kind` to require one `Operation::metadata()` API to expose static kind, origin, class, and dispatch mode for both static and dynamic operations.

Verification:

```bash
cargo test -p pi-coding-agent operation_metadata_exposes_static_contract_and_dispatch_mode --lib
```

RED result: compile failed because `Operation::metadata()` and `OperationDispatchMode` did not exist.

- [x] **Step 2: Add `OperationMetadata` and `OperationDispatchMode`**

Added structured operation metadata and made the existing `kind()`, `static_kind()`, `origin()`, and `class()` helpers derive from `metadata()` instead of maintaining separate mappings.

Verification:

```bash
cargo test -p pi-coding-agent operation_metadata_exposes_static_contract_and_dispatch_mode --lib
cargo test -p pi-coding-agent dynamic_operation_metadata_exposes_dispatch_without_static_kind --lib
```

GREEN result: both metadata tests passed.

- [x] **Step 3: Write failing structured admission tests**

Added `resolve_operation_admission_returns_structured_dynamic_contract` and `resolve_operation_admission_returns_structured_static_contract` to require admission resolution to return kind, metadata, and dynamic admitted-at state instead of a tuple.

Verification:

```bash
cargo test -p pi-coding-agent resolve_operation_admission_returns_structured_dynamic_contract --lib
```

RED result: compile failed because `resolve_operation_admission()` still returned a tuple.

- [x] **Step 4: Add `OperationAdmission` and use it in dispatchers**

Added `OperationAdmission { kind, metadata, admitted_at }`, changed dynamic approval and static operation admission to return this structure, and updated async dispatch to use the structured kind/admitted-at values while preserving current public behavior.

Verification:

```bash
cargo test -p pi-coding-agent resolve_operation_admission_returns_structured_dynamic_contract --lib
cargo test -p pi-coding-agent resolve_operation_admission_returns_structured_static_contract --lib
```

GREEN result: both structured admission tests passed.

### Task 26: Verify Structured Operation Metadata Slice

- [x] **Step 1: Run operation-focused and admission tests**

```bash
cargo test -p pi-coding-agent operation --lib
cargo test -p pi-coding-agent resolve_operation_admission_returns_structured_dynamic_contract --lib
cargo test -p pi-coding-agent resolve_operation_admission_returns_structured_static_contract --lib
cargo test -p pi-coding-agent coding_session_public_api_symbols_are_importable
```

- [x] **Step 2: Run formatting, crate check, and diff hygiene**

```bash
cargo fmt --check
cargo check -p pi-coding-agent
git diff --check
rg -n "operation_control\.begin" crates/pi-coding-agent/src/coding_session crates/pi-coding-agent/src/interactive crates/pi-coding-agent/src/protocol
git status --short
```
