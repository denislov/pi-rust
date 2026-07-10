# Canonical Operation Runtime Convergence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `CodingAgentSession::run(CodingAgentOperation)` the only public live-session operation dispatcher, migrate every first-party adapter and test to it, and delete the replaced broad workflow methods.

**Architecture:** Convert the public operation enum into the existing crate-private `Operation` contract, select the existing async/sync-read/sync-mutable dispatcher from `OperationMetadata`, and project every internal outcome through one exhaustive mapping. Expand the public contract for plugin, delegation, profile, fork, and active-leaf mutations; migrate adapters in behavior-preserving slices; then delete compatibility methods and install source guards.

**Tech Stack:** Rust 2024, Tokio, `pi-coding-agent`, typed `CodingAgentOperation`, crate-private `Operation`/`OperationOutcome`, `IntentRouter`, Rust-native session log, deterministic faux-provider tests, source boundary guards.

---

## Design Reference

- `docs/superpowers/specs/2026-07-10-canonical-operation-runtime-convergence-design.md`
- `docs/superpowers/specs/2026-07-07-operation-runtime-reference-architecture.md`
- `docs/superpowers/plans/2026-07-10-operation-runtime-stage-8-public-facade-narrowing-plan.md`

## Current Context

- `CodingAgentSession::run()` currently carries `#[allow(deprecated)]` and calls the deprecated prompt/compact/branch-summary/self-healing/agent/team/export wrappers.
- Internal `Operation` already covers plugin load/command, delegation confirmation, fork, and default-profile mutation.
- Internal `Operation::ForkSession` is admitted but rejected by the sync-mutable dispatcher; `fork_current_session()` performs a separate direct admission path.
- `SessionService::switch_active_leaf()` exists and records `active_leaf.changed`, but it has no owner operation contract.
- JSON, print, RPC, and interactive production adapters still call broad workflow methods and use local `#[allow(deprecated)]` suppressions.
- Integration and owner tests still call the broad workflow methods extensively.
- Compatibility event subscription remains intentionally deferred to Stage 10.

## File Structure

- Modify: `crates/pi-coding-agent/src/coding_session/public_operation.rs`
  - Own the complete public request/outcome contract, branch-summary reuse policy, narrow plugin-load result projection, public-to-internal conversion, and the one internal-to-public outcome mapping.
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
  - Add branch-summary reuse, active-leaf switch, fork/switch outcomes, and complete metadata coverage.
- Modify: `crates/pi-coding-agent/src/coding_session/operation_control.rs`
  - Add `SwitchActiveLeaf` operation kind.
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
  - Make `run()` canonical; implement fork/switch dispatch; fold navigation summary reuse into operation execution; delete broad workflow methods.
- Modify: `crates/pi-coding-agent/src/lib.rs`
  - Export the new stable operation support types.
- Modify: `crates/pi-coding-agent/src/protocol/json_mode.rs`
  - Run prompt through `CodingAgentOperation::Prompt`.
- Modify: `crates/pi-coding-agent/src/print_mode.rs`
  - Run persistent and non-persistent prompt through the operation facade.
- Modify: `crates/pi-coding-agent/src/protocol/rpc/prompt.rs`
  - Run streaming prompt/agent/team/delegation-approval tasks through operations.
- Modify: `crates/pi-coding-agent/src/protocol/rpc/commands.rs`
  - Run self-healing edit, profile mutation, delegation rejection, plugin load, and plugin command through operations.
- Modify: `crates/pi-coding-agent/src/interactive/prompt_task.rs`
  - Run all background product work through operations and preserve event/control multiplexing.
- Modify: `crates/pi-coding-agent/src/interactive/loop.rs`
  - Run profile/rejection mutations through async `run()` and refresh projections after navigation.
- Modify: `crates/pi-coding-agent/tests/public_api.rs`
  - Cover the expanded public operation contract and migrate broad workflow tests.
- Modify: `crates/pi-coding-agent/tests/agent_invocation.rs`
- Modify: `crates/pi-coding-agent/tests/agent_profile_runtime.rs`
- Modify: `crates/pi-coding-agent/tests/agent_profile_session.rs`
- Modify: `crates/pi-coding-agent/tests/agent_team_flow.rs`
- Modify: `crates/pi-coding-agent/tests/delegation_execution.rs`
  - Migrate integration coverage to `run()` without reducing assertions.
- Modify: `crates/pi-coding-agent/tests/api_boundary_guards.rs`
  - Replace Stage 8 deprecation guards with Stage 9 deletion/canonical-dispatch guards.
- Modify: `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs`
  - Reject adapter broad-workflow calls and production deprecation suppressions.
- Modify: `docs/TODO.md`
  - Track Stage 9 start, task progress, and closure.

## Non-Goals

- Do not change public ProductEvent payload shape.
- Do not delete compatibility event subscriptions in this plan.
- Do not add a lifecycle-grade public control handle.
- Do not redesign RPC wire commands or interactive rendering.
- Do not expose raw plugin load options, plugin registries, session services, provider internals, or Flow nodes through `pi_coding_agent::api`.
- Do not retain deleted workflow methods under new names.

### Task 1: Expand The Public Operation Contract

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/public_operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/src/lib.rs`
- Modify: `crates/pi-coding-agent/tests/public_api.rs`

- [ ] **Step 1: Write the failing public contract test**

Add imports in `crates/pi-coding-agent/tests/public_api.rs`:

```rust
use pi_coding_agent::api::{
    BranchSummaryReusePolicy, CodingAgentOperation, CodingAgentOperationOutcome,
    CodingAgentPluginDiagnostic, CodingAgentPluginLoadOutcome,
};
```

Add a compile-level contract test:

```rust
#[test]
fn canonical_operation_runtime_variants_are_public() {
    let _ = CodingAgentOperation::PluginLoad;
    let _ = CodingAgentOperation::PluginCommand {
        command_id: "plugin.command".into(),
        args: serde_json::json!({"value": 1}),
    };
    let _ = CodingAgentOperation::SetDefaultAgentProfile {
        profile_id: ProfileId::from("reviewer"),
    };
    let _ = CodingAgentOperation::ApproveDelegation {
        operation_id: "op_parent".into(),
        tool_call_id: "tool_delegate".into(),
    };
    let _ = CodingAgentOperation::RejectDelegation {
        operation_id: "op_parent".into(),
        tool_call_id: "tool_delegate".into(),
        reason: "not now".into(),
    };
    let _ = CodingAgentOperation::ForkSession {
        target_leaf_id: Some("leaf_target".into()),
    };
    let _ = CodingAgentOperation::SwitchActiveLeaf {
        target_leaf_id: "leaf_target".into(),
    };
    let _ = BranchSummaryReusePolicy::ReuseExisting;
    let _ = CodingAgentPluginLoadOutcome {
        loaded_plugin_ids: vec!["sample".into()],
        diagnostics: vec![CodingAgentPluginDiagnostic {
            plugin_id: Some("sample".into()),
            message: "loaded".into(),
        }],
        capability_changed: true,
    };
    let _ = CodingAgentOperationOutcome::DelegationApproved;
    let _ = CodingAgentOperationOutcome::SessionForked;
    let _ = CodingAgentOperationOutcome::ActiveLeafSwitched;
}
```

- [ ] **Step 2: Run the RED public API test**

Run:

```bash
cargo test -p pi-coding-agent --test public_api canonical_operation_runtime_variants_are_public -- --nocapture
```

Expected: FAIL because the Stage 9 variants and support types are not exported.

- [ ] **Step 3: Define the complete public request and outcome types**

Replace the enum definitions in `public_operation.rs` with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchSummaryReusePolicy {
    AlwaysCreate,
    ReuseExisting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentPluginDiagnostic {
    pub plugin_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentPluginLoadOutcome {
    pub loaded_plugin_ids: Vec<String>,
    pub diagnostics: Vec<CodingAgentPluginDiagnostic>,
    pub capability_changed: bool,
}

#[derive(Debug)]
pub enum CodingAgentOperation {
    Prompt(PromptTurnOptions),
    Compact(PromptTurnOptions),
    BranchSummary {
        options: PromptTurnOptions,
        source_leaf_id: String,
        target_leaf_id: String,
        custom_instructions: Option<String>,
        reuse: BranchSummaryReusePolicy,
    },
    SelfHealingEdit(SelfHealingEditRequest),
    InvokeAgent(AgentInvocationOptions),
    InvokeTeam(AgentTeamOptions),
    PluginLoad,
    PluginCommand { command_id: String, args: serde_json::Value },
    SetDefaultAgentProfile { profile_id: ProfileId },
    ApproveDelegation { operation_id: String, tool_call_id: String },
    RejectDelegation {
        operation_id: String,
        tool_call_id: String,
        reason: String,
    },
    ForkSession { target_leaf_id: Option<String> },
    SwitchActiveLeaf { target_leaf_id: String },
    ExportCurrent,
    ExportCurrentHtml(PathBuf),
}

#[derive(Debug)]
pub enum CodingAgentOperationOutcome {
    Prompt(PromptTurnOutcome),
    Compact(PromptTurnOutcome),
    BranchSummary(PromptTurnOutcome),
    SelfHealingEdit(SelfHealingEditOutcome),
    AgentInvocation(AgentInvocationOutcome),
    AgentTeam(AgentTeamOutcome),
    PluginLoad(CodingAgentPluginLoadOutcome),
    PluginCommand(String),
    DefaultAgentProfileChanged,
    DelegationApproved,
    DelegationRejected,
    SessionForked,
    ActiveLeafSwitched,
    Export(CodingAgentSessionExport),
    ExportHtml(PathBuf),
}
```

Import `ProfileId` and retain the existing option/outcome imports.

- [ ] **Step 4: Re-export the support types**

In `coding_session/mod.rs` re-export all five public operation types. In `lib.rs`, add them to `pi_coding_agent::api`.

- [ ] **Step 5: Run the GREEN public contract test**

Run the command from Step 2. Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/public_operation.rs crates/pi-coding-agent/src/coding_session/mod.rs crates/pi-coding-agent/src/lib.rs crates/pi-coding-agent/tests/public_api.rs
git commit -m "feat: expand canonical coding operations"
```

### Task 2: Make `run()` The Canonical Dispatcher

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/public_operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/tests/api_boundary_guards.rs`

- [ ] **Step 1: Replace the Stage 8 deprecation guard with a RED canonical-dispatch guard**

Add a brace-counting `function_body()` helper to `api_boundary_guards.rs`, then add:

```rust
#[test]
fn coding_session_run_is_the_canonical_operation_dispatcher() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_root.join("src/coding_session/mod.rs"))
        .expect("coding session owner should be readable");
    let run_body = function_body(&source, "pub async fn run(")
        .expect("CodingAgentSession::run should exist");

    assert!(run_body.contains("into_internal("));
    assert!(run_body.contains("OperationDispatchMode::Async"));
    assert!(run_body.contains("OperationDispatchMode::SyncReadOnly"));
    assert!(run_body.contains("OperationDispatchMode::SyncMutable"));
    assert!(run_body.contains("run_operation(operation).await"));
    assert!(run_body.contains("run_sync_operation(operation)"));
    assert!(run_body.contains("run_sync_mut_operation(operation)"));

    for forbidden in [
        ".prompt(", ".compact(", ".summarize_branch(",
        ".self_healing_edit_with_options(", ".invoke_agent(", ".invoke_team(",
        ".export_current(", ".export_current_html(",
    ] {
        assert!(!run_body.contains(forbidden),
            "CodingAgentSession::run must not call compatibility workflow {forbidden}");
    }
}
```

- [ ] **Step 2: Run the RED dispatcher guard**

```bash
cargo test -p pi-coding-agent --test api_boundary_guards coding_session_run_is_the_canonical_operation_dispatcher -- --nocapture
```

Expected: FAIL because `run()` still calls deprecated wrappers.

- [ ] **Step 3: Add public-to-internal conversion**

In `public_operation.rs`, implement `CodingAgentOperation::into_internal(self, plugin_load: PluginLoadOptions) -> Operation` with this complete mapping:

```rust
match self {
    Self::Prompt(options) => Operation::Prompt(options),
    Self::Compact(options) => Operation::ManualCompaction(options),
    Self::BranchSummary {
        options, source_leaf_id, target_leaf_id, custom_instructions, reuse,
    } => Operation::BranchSummary {
        options,
        source_leaf_id,
        target_leaf_id,
        custom_instructions,
        reuse_existing: matches!(reuse, BranchSummaryReusePolicy::ReuseExisting),
    },
    Self::SelfHealingEdit(request) => Operation::SelfHealingEdit(request),
    Self::InvokeAgent(options) => Operation::AgentInvocation(options),
    Self::InvokeTeam(options) => Operation::AgentTeam(options),
    Self::PluginLoad => Operation::PluginLoad(plugin_load),
    Self::PluginCommand { command_id, args } => Operation::PluginCommand { command_id, args },
    Self::SetDefaultAgentProfile { profile_id } => Operation::SetDefaultAgentProfile { profile_id },
    Self::ApproveDelegation { operation_id, tool_call_id } => {
        Operation::ApproveDelegationConfirmation { operation_id, tool_call_id }
    }
    Self::RejectDelegation { operation_id, tool_call_id, reason } => {
        Operation::RejectDelegationConfirmation { operation_id, tool_call_id, reason }
    }
    Self::ForkSession { target_leaf_id } => Operation::ForkSession { target_leaf_id },
    Self::SwitchActiveLeaf { target_leaf_id } => Operation::SwitchActiveLeaf { target_leaf_id },
    Self::ExportCurrent => Operation::Export(ExportOptions::view()),
    Self::ExportCurrentHtml(path) => Operation::Export(ExportOptions::html(path)),
}
```

Update internal `Operation::BranchSummary` with `reuse_existing: bool`, and add `Operation::SwitchActiveLeaf`.

- [ ] **Step 4: Add the one internal-to-public outcome projection**

Implement `CodingAgentOperationOutcome::from_internal(OperationOutcome)`:

```rust
match outcome {
    OperationOutcome::Prompt(outcome) => Self::Prompt(outcome),
    OperationOutcome::ManualCompaction(outcome) => Self::Compact(outcome),
    OperationOutcome::PluginLoad(outcome) => Self::PluginLoad(outcome.into()),
    OperationOutcome::PluginCommand(output) => Self::PluginCommand(output),
    OperationOutcome::DelegationApproval => Self::DelegationApproved,
    OperationOutcome::DelegationRejection => Self::DelegationRejected,
    OperationOutcome::BranchSummary(outcome) => Self::BranchSummary(outcome),
    OperationOutcome::SelfHealingEdit(outcome) => Self::SelfHealingEdit(outcome),
    OperationOutcome::AgentInvocation(outcome) => Self::AgentInvocation(outcome),
    OperationOutcome::AgentTeam(outcome) => Self::AgentTeam(outcome),
    OperationOutcome::SetDefaultAgentProfile => Self::DefaultAgentProfileChanged,
    OperationOutcome::ForkSession => Self::SessionForked,
    OperationOutcome::SwitchActiveLeaf => Self::ActiveLeafSwitched,
    OperationOutcome::Export(outcome) => match outcome.path {
        Some(path) => Self::ExportHtml(path),
        None => Self::Export(outcome.export),
    },
}
```

Implement `From<PluginLoadOutcome> for CodingAgentPluginLoadOutcome` by projecting loaded ids, `plugin_id`/`message` diagnostics, and `capability_changed`. Do not expose internal `capabilities`.

- [ ] **Step 5: Rewrite `CodingAgentSession::run()`**

```rust
pub async fn run(
    &mut self,
    operation: CodingAgentOperation,
) -> Result<CodingAgentOperationOutcome, CodingSessionError> {
    let operation = operation.into_internal(self.default_plugin_load_options.clone());
    let dispatch_mode = operation.metadata().dispatch_mode;
    let outcome = match dispatch_mode {
        OperationDispatchMode::Async => self.run_operation(operation).await?,
        OperationDispatchMode::SyncReadOnly => self.run_sync_operation(operation)?,
        OperationDispatchMode::SyncMutable => self.run_sync_mut_operation(operation)?,
    };
    Ok(CodingAgentOperationOutcome::from_internal(outcome))
}
```

Remove `#[allow(deprecated)]` from `run()`.

- [ ] **Step 6: Run GREEN dispatcher tests**

```bash
cargo test -p pi-coding-agent --test api_boundary_guards coding_session_run_is_the_canonical_operation_dispatcher -- --nocapture
cargo test -p pi-coding-agent --test public_api coding_session_run_public_operation_facade_is_importable -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/public_operation.rs crates/pi-coding-agent/src/coding_session/operation.rs crates/pi-coding-agent/src/coding_session/mod.rs crates/pi-coding-agent/tests/api_boundary_guards.rs
git commit -m "refactor: make session run canonical dispatcher"
```

### Task 3: Complete Sync-Mutable And Navigation Operations

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/operation_control.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/intent_router.rs`
- Modify: `crates/pi-coding-agent/tests/public_api.rs`

- [ ] **Step 1: Write failing behavior tests**

Add public tests for `SetDefaultAgentProfile` success and non-persistent `SwitchActiveLeaf` rejection. Add owner tests using existing session fixtures for successful active-leaf switch, fork replacement, delegation approval/rejection, and branch-summary reuse.

Representative public test:

```rust
#[tokio::test]
async fn canonical_run_rejects_non_persistent_leaf_navigation() {
    let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
        .await
        .unwrap();
    let error = session
        .run(CodingAgentOperation::SwitchActiveLeaf {
            target_leaf_id: "leaf_missing".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), "unsupported_capability");
}
```

- [ ] **Step 2: Run RED tests**

```bash
cargo test -p pi-coding-agent --test public_api canonical_run_rejects_non_persistent_leaf_navigation -- --nocapture
cargo test -p pi-coding-agent coding_session::tests::canonical_run_forks_current_session -- --nocapture
```

Expected: FAIL because switch/fork dispatch is incomplete.

- [ ] **Step 3: Add metadata and operation kind coverage**

Add `SwitchActiveLeaf` to `OperationKind`, map it to `"switch_active_leaf"`, and give `Operation::SwitchActiveLeaf` `ClientRoot`, `SessionWriteRoot`, `SyncMutable` metadata.

- [ ] **Step 4: Implement active-leaf switch and fork**

In `run_sync_mut_operation()` replace the unsupported fork arm and add switch:

```rust
Operation::ForkSession { target_leaf_id } => {
    let SessionPersistence::Persistent(session_service) = &self.persistence else {
        return Err(CodingSessionError::UnsupportedCapability {
            capability: "fork requires a persistent Rust-native session".into(),
        });
    };
    let forked_service = session_service.fork_current(target_leaf_id.as_deref())?;
    let replacement = Self::from_services(
        forked_service,
        self.default_plugin_load_options.clone(),
        self.profile_registry.clone(),
    )?;
    *self = replacement;
    Ok(OperationOutcome::ForkSession)
}
Operation::SwitchActiveLeaf { target_leaf_id } => {
    let SessionPersistence::Persistent(session_service) = &mut self.persistence else {
        return Err(CodingSessionError::UnsupportedCapability {
            capability: "active leaf navigation requires a persistent Rust-native session".into(),
        });
    };
    session_service.switch_active_leaf(&target_leaf_id)?;
    Ok(OperationOutcome::SwitchActiveLeaf)
}
```

Update all exhaustive dispatcher matches.

- [ ] **Step 5: Move branch-summary reuse into operation dispatch**

In the async `Operation::BranchSummary` arm, call `branch_summary_service.reused_outcome(...)` when `reuse_existing` is true, returning `OperationOutcome::BranchSummary(outcome)` before starting a new flow.

- [ ] **Step 6: Update intent-router source tests**

Replace direct-fork admission assertions with metadata/dispatcher assertions for `ForkSession`, `SwitchActiveLeaf`, and `SetDefaultAgentProfile`.

- [ ] **Step 7: Run GREEN tests**

```bash
cargo test -p pi-coding-agent coding_session::operation -- --nocapture
cargo test -p pi-coding-agent coding_session::intent_router -- --nocapture
cargo test -p pi-coding-agent coding_session::tests::canonical_run_forks_current_session -- --nocapture
cargo test -p pi-coding-agent --test public_api canonical_run_ -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/operation.rs crates/pi-coding-agent/src/coding_session/operation_control.rs crates/pi-coding-agent/src/coding_session/mod.rs crates/pi-coding-agent/src/coding_session/intent_router.rs crates/pi-coding-agent/tests/public_api.rs
git commit -m "feat: complete session mutation operations"
```

### Task 4: Migrate JSON And Print Adapters

**Files:**
- Modify: `crates/pi-coding-agent/src/protocol/json_mode.rs`
- Modify: `crates/pi-coding-agent/src/print_mode.rs`
- Modify: `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs`

- [ ] **Step 1: Add a RED adapter source guard**

Assert JSON and print sources contain `CodingAgentOperation::Prompt` and do not contain `.prompt(` or `#[allow(deprecated)]`.

- [ ] **Step 2: Run the RED guard**

```bash
cargo test -p pi-coding-agent --test product_runtime_boundary_guards json_and_print_adapters_use_canonical_operations -- --nocapture
```

Expected: FAIL.

- [ ] **Step 3: Migrate JSON prompt execution**

Replace the spawned call with:

```rust
let result = session
    .run(CodingAgentOperation::Prompt(prompt_options))
    .await
    .and_then(|outcome| match outcome {
        CodingAgentOperationOutcome::Prompt(outcome) => Ok(outcome),
        _ => unreachable!("prompt operation returned a non-prompt public outcome"),
    });
let _ = done_tx.send(result);
```

Remove `#[allow(deprecated)]`.

- [ ] **Step 4: Migrate print execution**

Use `session.run(CodingAgentOperation::Prompt(prompt_options)).await?` in persistent and non-persistent paths, match `CodingAgentOperationOutcome::Prompt`, and remove both deprecation suppressions.

- [ ] **Step 5: Run adapter tests**

```bash
cargo test -p pi-coding-agent --test product_runtime_boundary_guards json_and_print_adapters_use_canonical_operations -- --nocapture
cargo test -p pi-coding-agent json_mode -- --nocapture
cargo test -p pi-coding-agent print_mode -- --nocapture
```

Expected: PASS with unchanged output behavior.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/protocol/json_mode.rs crates/pi-coding-agent/src/print_mode.rs crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs
git commit -m "refactor: run json and print through operations"
```

### Task 5: Migrate RPC Operations

**Files:**
- Modify: `crates/pi-coding-agent/src/protocol/rpc/prompt.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/commands.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/state.rs`
- Test: `crates/pi-coding-agent/tests/rpc_mode.rs`
- Test: `crates/pi-coding-agent/tests/protocol_sessions.rs`

- [ ] **Step 1: Add a RED RPC guard**

Reject `.prompt(`, `.invoke_agent(`, `.invoke_team(`, `.self_healing_edit_with_options(`, delegation confirmation methods, profile mutation, plugin reload/command, and `#[allow(deprecated)]` under `src/protocol/rpc`. Require `CodingAgentOperation::` in `prompt.rs` and `commands.rs`.

- [ ] **Step 2: Run the RED guard**

```bash
cargo test -p pi-coding-agent --test product_runtime_boundary_guards rpc_adapters_use_canonical_operations -- --nocapture
```

Expected: FAIL.

- [ ] **Step 3: Migrate RPC background operations**

Replace each pinned future with `session.run(...)`, for example:

```rust
let mut invocation = Box::pin(session.run(CodingAgentOperation::InvokeAgent(
    invocation_options,
)));
```

Match the expected public outcome into the existing RPC-local `CodingOperationOutcome`. Apply the same pattern to team invocation, delegation approval, and prompt. Preserve `tokio::select!`, control handling, event forwarding, and response types. Remove the three deprecation suppressions.

- [ ] **Step 4: Migrate RPC command operations**

Route operations as follows:

```text
self_healing_edit -> SelfHealingEdit
set_default_agent_profile -> SetDefaultAgentProfile
reject_delegation -> RejectDelegation
reload -> PluginLoad
plugin_command -> PluginCommand
```

Example plugin command:

```rust
let output = match session
    .run(CodingAgentOperation::PluginCommand {
        command_id: command_id.clone(),
        args,
    })
    .await
{
    Ok(CodingAgentOperationOutcome::PluginCommand(output)) => output,
    Ok(_) => unreachable!("plugin command returned a different public outcome"),
    Err(error) => {
        self.coding_session = Some(session);
        write_rpc_response(
            writer,
            RpcResponse::error(id, "plugin_command", error.to_string()),
        )
        .await?;
        return Ok(());
    }
};
self.coding_session = Some(session);
write_rpc_response(
    writer,
    RpcResponse::success(
        id,
        "plugin_command",
        Some(serde_json::json!({
            "commandId": command_id,
            "output": output,
        })),
    ),
)
.await?;
return Ok(());
```

Change `rpc_plugin_reload_data` to accept `&CodingAgentPluginLoadOutcome`; keep wire fields unchanged.

- [ ] **Step 5: Run focused RPC tests**

```bash
cargo test -p pi-coding-agent --test product_runtime_boundary_guards rpc_adapters_use_canonical_operations -- --nocapture
cargo test -p pi-coding-agent --test rpc_mode -- --nocapture
cargo test -p pi-coding-agent --test protocol_sessions -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/protocol/rpc/prompt.rs crates/pi-coding-agent/src/protocol/rpc/commands.rs crates/pi-coding-agent/src/protocol/rpc/state.rs crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs
git commit -m "refactor: run rpc workflows through operations"
```

### Task 6: Migrate Interactive Operations And Navigation

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/prompt_task.rs`
- Modify: `crates/pi-coding-agent/src/interactive/loop.rs`
- Test: `crates/pi-coding-agent/tests/interactive_mode.rs`
- Test: `crates/pi-coding-agent/tests/interactive_abort.rs`

- [ ] **Step 1: Add a RED interactive guard**

Reject broad session workflow calls and production `#[allow(deprecated)]` in `prompt_task.rs` and `loop.rs`; require `CodingAgentOperation::` in both.

- [ ] **Step 2: Run the RED guard**

```bash
cargo test -p pi-coding-agent --test product_runtime_boundary_guards interactive_adapters_use_canonical_operations -- --nocapture
```

Expected: FAIL.

- [ ] **Step 3: Migrate background prompt tasks**

Map current tasks to operations:

```text
prompt -> Prompt
agent invocation -> InvokeAgent
team invocation -> InvokeTeam
delegation approval -> ApproveDelegation
compaction -> Compact
self-healing edit -> SelfHealingEdit
plugin reload -> PluginLoad
plugin command -> PluginCommand
direct branch summary -> BranchSummary { reuse: AlwaysCreate }
navigation branch summary -> BranchSummary { reuse: ReuseExisting }
```

Match the expected public outcome into existing task result types. Change `PluginReloadTaskResult.outcome` to `CodingAgentPluginLoadOutcome`.

- [ ] **Step 4: Migrate navigation fork**

```rust
let outcome = session
    .run(CodingAgentOperation::ForkSession {
        target_leaf_id: Some(target_leaf_id.clone()),
    })
    .await
    .map_err(CliError::from)?;
assert!(matches!(outcome, CodingAgentOperationOutcome::SessionForked));
send_ui_snapshot(&event_tx, &session);
```

Return the mutated `session`; do not reuse the pre-fork receiver.

- [ ] **Step 5: Migrate loop mutations**

Run default-profile mutation through `SetDefaultAgentProfile`. Make `reject_pending_delegation_confirmation` async, run `RejectDelegation`, update call sites to `.await?`, and preserve product-event draining. Do not rename `InteractiveRoot::set_default_agent_profile_id`; it is local UI state.

- [ ] **Step 6: Remove interactive deprecation suppressions**

Remove the six Stage 8 `#[allow(deprecated)]` attributes from `prompt_task.rs`.

- [ ] **Step 7: Run focused interactive tests**

```bash
cargo test -p pi-coding-agent --test product_runtime_boundary_guards interactive_adapters_use_canonical_operations -- --nocapture
cargo test -p pi-coding-agent --test interactive_mode -- --nocapture
cargo test -p pi-coding-agent --test interactive_abort -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/pi-coding-agent/src/interactive/prompt_task.rs crates/pi-coding-agent/src/interactive/loop.rs crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs
git commit -m "refactor: run interactive workflows through operations"
```

### Task 7: Migrate Owner And Integration Tests

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/tests/public_api.rs`
- Modify: `crates/pi-coding-agent/tests/agent_invocation.rs`
- Modify: `crates/pi-coding-agent/tests/agent_profile_runtime.rs`
- Modify: `crates/pi-coding-agent/tests/agent_profile_session.rs`
- Modify: `crates/pi-coding-agent/tests/agent_team_flow.rs`
- Modify: `crates/pi-coding-agent/tests/delegation_execution.rs`

- [ ] **Step 1: Add test-only outcome extraction helpers**

In each large integration file, add only the helpers it needs:

```rust
fn prompt_outcome(outcome: CodingAgentOperationOutcome) -> PromptTurnOutcome {
    match outcome {
        CodingAgentOperationOutcome::Prompt(outcome) => outcome,
        _ => panic!("prompt operation returned a different public outcome"),
    }
}
```

Add equivalent one-arm helpers for export, agent invocation, team invocation, and self-healing edit where repeated. Do not add production compatibility methods.

- [ ] **Step 2: Migrate agent invocation and team tests**

Replace `.invoke_agent(...)`, `.invoke_team(...)`, and `.export_current()` in `agent_invocation.rs` and `agent_team_flow.rs` with `run()` and the corresponding operation variants. Remove file-level Stage 8 deprecation allowances only when no unrelated deprecated provider-registry compatibility call remains.

- [ ] **Step 3: Migrate profile and delegation tests**

Replace prompt, export, profile change, delegation approval, and delegation rejection calls in `agent_profile_runtime.rs`, `agent_profile_session.rs`, and `delegation_execution.rs`. Preserve provider-context, durable event, pending queue, lineage, folded export, and error assertions.

- [ ] **Step 4: Migrate public API tests**

Replace branch-summary and self-healing broad calls in `public_api.rs`. Remove only allowances that existed for broad workflow methods. Preserve focused compatibility subscription tests and their Stage 10 allowances.

- [ ] **Step 5: Migrate owner unit tests**

Inside `coding_session/mod.rs` tests, replace public broad calls with public `run()` calls. Tests injecting custom `PluginLoadOptions` call private `run_operation(Operation::PluginLoad(options))` directly. Replace broad-method routing assertions with canonical public-run behavior and source guards.

- [ ] **Step 6: Run focused integration suites**

```bash
cargo test -p pi-coding-agent --test agent_invocation -- --nocapture
cargo test -p pi-coding-agent --test agent_team_flow -- --nocapture
cargo test -p pi-coding-agent --test agent_profile_runtime -- --nocapture
cargo test -p pi-coding-agent --test agent_profile_session -- --nocapture
cargo test -p pi-coding-agent --test delegation_execution -- --nocapture
cargo test -p pi-coding-agent --test public_api -- --nocapture
cargo test -p pi-coding-agent coding_session::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/mod.rs crates/pi-coding-agent/tests/public_api.rs crates/pi-coding-agent/tests/agent_invocation.rs crates/pi-coding-agent/tests/agent_profile_runtime.rs crates/pi-coding-agent/tests/agent_profile_session.rs crates/pi-coding-agent/tests/agent_team_flow.rs crates/pi-coding-agent/tests/delegation_execution.rs
git commit -m "test: migrate coding workflows to canonical run"
```

### Task 8: Delete Broad Workflow Methods

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/tests/api_boundary_guards.rs`

- [ ] **Step 1: Add a RED deletion guard**

```rust
#[test]
fn broad_live_session_workflow_methods_are_deleted() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_root.join("src/coding_session/mod.rs"))
        .expect("coding session owner should be readable");
    for signature in [
        "pub async fn prompt(",
        "pub async fn compact(",
        "pub async fn self_healing_edit(",
        "pub async fn self_healing_edit_with_options(",
        "pub async fn invoke_agent(",
        "pub async fn invoke_team(",
        "pub async fn summarize_branch(",
        "pub(crate) async fn summarize_branch_for_navigation(",
        "pub(crate) async fn reload_plugins(",
        "pub(crate) async fn load_plugins(",
        "pub(crate) fn run_plugin_command(",
        "pub fn set_default_agent_profile_id(",
        "pub async fn approve_delegation_confirmation(",
        "pub fn reject_delegation_confirmation(",
        "pub(crate) fn fork_current_session(",
        "pub fn export_current(",
        "pub fn export_current_html(",
    ] {
        assert!(!source.contains(signature),
            "broad live-session workflow should be deleted: {signature}");
    }
}
```

- [ ] **Step 2: Run the RED deletion guard**

```bash
cargo test -p pi-coding-agent --test api_boundary_guards broad_live_session_workflow_methods_are_deleted -- --nocapture
```

Expected: FAIL.

- [ ] **Step 3: Delete the broad methods**

Remove the complete method bodies listed by the guard. Keep construction/opening, static repository helpers, snapshot/connect/product-event subscription, query facades, the crate-private prompt control path, plugin UI definition queries, and `run(CodingAgentOperation)`.

- [ ] **Step 4: Run GREEN deletion checks**

```bash
cargo test -p pi-coding-agent --test api_boundary_guards broad_live_session_workflow_methods_are_deleted -- --nocapture
cargo check -p pi-coding-agent
```

Expected: PASS. Migrate any missed caller; do not restore a deleted method.

- [ ] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/mod.rs crates/pi-coding-agent/tests/api_boundary_guards.rs
git commit -m "chore: delete broad session workflow methods"
```

### Task 9: Harden Production Adapter Boundaries

**Files:**
- Modify: `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs`
- Modify: `crates/pi-coding-agent/tests/api_boundary_guards.rs`

- [ ] **Step 1: Consolidate recursive adapter guards**

Scan `src/protocol`, `src/interactive`, and `src/print_mode.rs`. Reject:

```rust
const FORBIDDEN_WORKFLOW_CALLS: &[&str] = &[
    ".prompt(",
    ".compact(",
    ".summarize_branch(",
    ".summarize_branch_for_navigation(",
    ".self_healing_edit_with_options(",
    ".invoke_agent(",
    ".invoke_team(",
    ".approve_delegation_confirmation(",
    ".reject_delegation_confirmation(",
    ".fork_current_session(",
    ".reload_plugins(",
    ".run_plugin_command(",
    ".export_current(",
    ".export_current_html(",
];
```

Separately reject `#[allow(deprecated)]` in JSON, print, RPC prompt/commands, and interactive prompt-task production files. Do not reject `InteractiveRoot::set_default_agent_profile_id`; deletion of the session method makes session misuse a compile error.

- [ ] **Step 2: Add stable facade completeness assertions**

Assert the `api` module exports `CodingAgentOperation`, `CodingAgentOperationOutcome`, `BranchSummaryReusePolicy`, and `CodingAgentPluginLoadOutcome`, while not exporting internal `Operation`, `OperationOutcome`, `OperationDispatchMode`, or `PluginLoadOptions`.

- [ ] **Step 3: Run boundary guards**

```bash
cargo test -p pi-coding-agent --test api_boundary_guards -- --nocapture
cargo test -p pi-coding-agent --test product_runtime_boundary_guards -- --nocapture
cargo test -p pi-coding-agent --test event_boundary_guards -- --nocapture
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs crates/pi-coding-agent/tests/api_boundary_guards.rs
git commit -m "test: guard canonical operation runtime boundary"
```

### Task 10: Closure Audit And Documentation

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md`

- [ ] **Step 1: Run source audits**

```bash
rg -n '#\[allow\(deprecated\)\]' crates/pi-coding-agent/src/protocol crates/pi-coding-agent/src/interactive crates/pi-coding-agent/src/print_mode.rs
rg -n '\.(prompt|compact|summarize_branch|summarize_branch_for_navigation|self_healing_edit_with_options|invoke_agent|invoke_team|approve_delegation_confirmation|reject_delegation_confirmation|fork_current_session|reload_plugins|run_plugin_command|export_current|export_current_html)\(' crates/pi-coding-agent/src crates/pi-coding-agent/tests --glob '*.rs'
rg -n 'pub(\(crate\))? (async )?fn (prompt|compact|summarize_branch|summarize_branch_for_navigation|self_healing_edit|self_healing_edit_with_options|invoke_agent|invoke_team|approve_delegation_confirmation|reject_delegation_confirmation|fork_current_session|reload_plugins|load_plugins|run_plugin_command|export_current|export_current_html)' crates/pi-coding-agent/src/coding_session/mod.rs
```

Expected: no production adapter suppressions, broad workflow calls, or deleted method definitions. Focused Stage 10 compatibility event allowances may remain.

- [ ] **Step 2: Run focused verification**

```bash
cargo fmt --check
cargo test -p pi-coding-agent
cargo check -p pi-coding-agent
git diff --check
```

Expected: PASS.

- [ ] **Step 3: Run workspace verification**

```bash
cargo test --workspace
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 4: Update TODO and plan status**

Mark Stage 9 complete, check all plan boxes, and add:

```text
Stage 9 canonical operation runtime convergence is complete: CodingAgentSession::run
directly dispatches the typed internal operation contract, all first-party adapters
execute product work through run(), plugin/delegation/profile/navigation mutations
have public operation contracts, broad workflow methods are deleted, and source
guards prevent compatibility execution paths from returning. Stage 10 typed
ProductEvent payload convergence is the next runtime simplification stage.
```

- [ ] **Step 5: Commit**

```bash
git add docs/TODO.md docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md
git commit -m "docs: close canonical operation runtime stage"
```

## Plan Self-Review Checklist

- Every live-session workflow currently called by JSON, print, RPC, or interactive adapters maps to a named `CodingAgentOperation` task above.
- Plugin load uses session-owned discovery roots and does not expose raw load options.
- Branch-summary navigation preserves reuse behavior before the adapter-only helper is deleted.
- Fork and active-leaf switch have explicit sync-mutable dispatch behavior.
- Compatibility event subscription deletion is not accidentally pulled into Stage 9.
- Control signals remain separate from ordinary operations.
- Broad method deletion occurs only after production adapters and tests migrate.
- Source guards cover both method calls and local deprecation suppressions.
- Full workspace verification is required before closure.
