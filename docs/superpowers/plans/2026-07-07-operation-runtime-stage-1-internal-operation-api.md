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
