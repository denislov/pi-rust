# Self-Healing Edit Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first internal `SelfHealingEditFlow` slice that wraps existing edit behavior in a stable Flow and proves edits can run through an `ExecutionEnv` boundary.

**Architecture:** Create a focused `self_healing_edit_flow.rs` module under `pi-coding-agent` and register it with `FlowService`. The first slice reuses the existing edit tool algorithm through injected operations, extracts typed outcome data from tool output details, and keeps public RPC/interactive/plugin APIs out of scope.

**Tech Stack:** Rust 2024, `pi_agent_core::flow`, `pi_agent_core::ExecutionEnv`, existing `pi_coding_agent::tools::edit` internals, `tokio` tests, `tempfile` and `InMemoryExecutionEnv` fixtures.

---

## File Structure

- Create `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs`: flow options, context, outcome, node definitions, operation adapter, and flow execution.
- Modify `crates/pi-coding-agent/src/coding_session/mod.rs`: declare the module and keep it crate-private for now.
- Modify `crates/pi-coding-agent/src/coding_session/flow_service.rs`: register and run `SelfHealingEditFlow`; add stable-node and behavior tests.
- Modify `docs/TODO.md`: add the new spec/plan links and mark the self-healing edit item as active with the first internal Flow slice.

## Task 1: Add FlowService Tests First

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/flow_service.rs`
- Test: `crates/pi-coding-agent/src/coding_session/flow_service.rs`

- [x] **Step 1: Add imports for self-healing edit tests**

Add the following imports inside the existing `#[cfg(test)] mod tests` block in `flow_service.rs`:

```rust
use pi_agent_core::{ExecutionEnv, InMemoryExecutionEnv};
use crate::coding_session::self_healing_edit_flow::{
    SelfHealingEditContext, SelfHealingEditOptions, SelfHealingEditReplacement,
};
```

If `pi_agent_core` items are already imported in one grouped line, merge `ExecutionEnv` and `InMemoryExecutionEnv` into that import instead of adding a duplicate.

- [x] **Step 2: Add stable node ID test**

Add this test near the other `*_flow_node_ids_are_stable` tests:

```rust
#[test]
fn self_healing_edit_flow_node_ids_are_stable() {
    let service = FlowService::new();

    service.self_healing_edit_flow().unwrap();

    assert_eq!(
        crate::coding_session::self_healing_edit_flow::SelfHealingEditFlow::node_ids(),
        &[
            "start_edit_workflow",
            "read_target",
            "propose_patch",
            "validate_patch",
            "apply_patch",
            "run_check",
            "repair_patch",
            "record_result",
        ]
    );
}
```

- [x] **Step 3: Add successful edit test**

Add this async test in the same test module:

```rust
#[tokio::test]
async fn self_healing_edit_flow_applies_successful_edit() {
    let service = FlowService::new();
    let env = InMemoryExecutionEnv::new("/workspace");
    env.write_file("src/app.txt", b"one\ntwo\n").await.unwrap();
    let options = SelfHealingEditOptions::new(
        env.cwd(),
        "src/app.txt",
        vec![SelfHealingEditReplacement::new("two", "deux")],
    )
    .with_execution_env(env.clone());
    let mut context = SelfHealingEditContext::new(options);

    let outcome = service.run_self_healing_edit(&mut context).await.unwrap();

    assert_eq!(outcome.path, "src/app.txt");
    assert_eq!(outcome.attempts, 1);
    assert!(outcome.message.contains("Successfully replaced 1 block"));
    assert!(outcome.diff.contains("-2 two"), "{}", outcome.diff);
    assert!(outcome.diff.contains("+2 deux"), "{}", outcome.diff);
    assert!(outcome.patch.contains("--- src/app.txt"), "{}", outcome.patch);
    assert!(outcome.patch.contains("+deux"), "{}", outcome.patch);
    assert_eq!(outcome.first_changed_line, Some(2));
    assert!(outcome.diagnostics.is_empty(), "{:#?}", outcome.diagnostics);
    assert_eq!(
        env.read_text_file("src/app.txt").await.unwrap(),
        "one\ndeux\n"
    );
}
```

- [x] **Step 4: Add validation failure test**

Add this async test:

```rust
#[tokio::test]
async fn self_healing_edit_flow_reports_validation_failure_without_write() {
    let service = FlowService::new();
    let env = InMemoryExecutionEnv::new("/workspace");
    env.write_file("src/app.txt", b"one\ntwo\n").await.unwrap();
    let options = SelfHealingEditOptions::new(
        env.cwd(),
        "src/app.txt",
        vec![SelfHealingEditReplacement::new("", "deux")],
    )
    .with_execution_env(env.clone());
    let mut context = SelfHealingEditContext::new(options);

    let error = service
        .run_self_healing_edit(&mut context)
        .await
        .expect_err("empty old text should fail validation");

    assert!(
        error.to_string().contains("oldText must not be empty"),
        "{error}"
    );
    assert_eq!(
        env.read_text_file("src/app.txt").await.unwrap(),
        "one\ntwo\n"
    );
    assert_eq!(context.diagnostics().len(), 1);
    assert!(context.diagnostics()[0].message.contains("oldText must not be empty"));
}
```

- [x] **Step 5: Add no-direct-filesystem test**

Add this async test:

```rust
#[tokio::test]
async fn self_healing_edit_flow_uses_execution_env_operations() {
    let service = FlowService::new();
    let temp = tempfile::tempdir().unwrap();
    let local_path = temp.path().join("src/app.txt");
    std::fs::create_dir_all(local_path.parent().unwrap()).unwrap();
    std::fs::write(&local_path, "local should not change\n").unwrap();

    let env = InMemoryExecutionEnv::new(temp.path());
    env.write_file("src/app.txt", b"env one\nenv two\n").await.unwrap();
    let options = SelfHealingEditOptions::new(
        env.cwd(),
        "src/app.txt",
        vec![SelfHealingEditReplacement::new("env two", "env deux")],
    )
    .with_execution_env(env.clone());
    let mut context = SelfHealingEditContext::new(options);

    let outcome = service.run_self_healing_edit(&mut context).await.unwrap();

    assert_eq!(outcome.first_changed_line, Some(2));
    assert_eq!(
        env.read_text_file("src/app.txt").await.unwrap(),
        "env one\nenv deux\n"
    );
    assert_eq!(
        std::fs::read_to_string(&local_path).unwrap(),
        "local should not change\n"
    );
}
```

- [x] **Step 6: Run the new tests and verify they fail**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent flow_service::tests::self_healing_edit_flow -- --nocapture
```

Expected: compilation fails because `self_healing_edit_flow`, `SelfHealingEditOptions`, `SelfHealingEditContext`, and `FlowService::self_healing_edit_flow` do not exist yet.

## Task 2: Implement SelfHealingEditFlow

**Files:**
- Create: `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`

- [x] **Step 1: Declare the module**

Add this module declaration in `coding_session/mod.rs` next to the other flow modules:

```rust
mod self_healing_edit_flow;
```

- [x] **Step 2: Create the flow module**

Create `self_healing_edit_flow.rs` with the following structure:

```rust
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use futures::future::{BoxFuture, FutureExt};
use pi_agent_core::flow::{Action, Flow, FlowError, FlowNode, FlowOutcome};
use pi_agent_core::{ExecutionEnv, FileError};
use pi_ai::types::ContentBlock;

use crate::tools::edit::{EditOperations, RealEditOperations, edit_execute_with_operations};

use super::CodingSessionError;

const DEFAULT_ACTION: &str = "default";

pub(crate) const SELF_HEALING_EDIT_NODE_IDS: &[&str] = &[
    "start_edit_workflow",
    "read_target",
    "propose_patch",
    "validate_patch",
    "apply_patch",
    "run_check",
    "repair_patch",
    "record_result",
];
```

Then add `SelfHealingEditReplacement`, `SelfHealingEditDiagnostic`, `SelfHealingEditOptions`, `SelfHealingEditOutcome`, `SelfHealingEditContext`, node specs, `SelfHealingEditFlow`, and `ExecutionEnvEditOperations` as described in the next steps.

- [x] **Step 3: Add request and outcome types**

Add these types after the constants:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelfHealingEditReplacement {
    old_text: String,
    new_text: String,
}

impl SelfHealingEditReplacement {
    pub(crate) fn new(old_text: impl Into<String>, new_text: impl Into<String>) -> Self {
        Self {
            old_text: old_text.into(),
            new_text: new_text.into(),
        }
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "oldText": self.old_text,
            "newText": self.new_text,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelfHealingEditDiagnostic {
    pub(crate) message: String,
}

#[derive(Clone)]
pub(crate) struct SelfHealingEditOptions {
    cwd: PathBuf,
    path: String,
    replacements: Vec<SelfHealingEditReplacement>,
    operations: Arc<dyn EditOperations>,
}

impl SelfHealingEditOptions {
    pub(crate) fn new(
        cwd: impl Into<PathBuf>,
        path: impl Into<String>,
        replacements: Vec<SelfHealingEditReplacement>,
    ) -> Self {
        Self {
            cwd: cwd.into(),
            path: path.into(),
            replacements,
            operations: Arc::new(RealEditOperations),
        }
    }

    pub(crate) fn with_operations(mut self, operations: Arc<dyn EditOperations>) -> Self {
        self.operations = operations;
        self
    }

    pub(crate) fn with_execution_env<E>(self, env: E) -> Self
    where
        E: ExecutionEnv + Clone + 'static,
    {
        self.with_operations(Arc::new(ExecutionEnvEditOperations::new(env)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelfHealingEditOutcome {
    pub(crate) path: String,
    pub(crate) message: String,
    pub(crate) diff: String,
    pub(crate) patch: String,
    pub(crate) first_changed_line: Option<usize>,
    pub(crate) attempts: usize,
    pub(crate) diagnostics: Vec<SelfHealingEditDiagnostic>,
}
```

- [x] **Step 4: Add context behavior**

Add `SelfHealingEditContext` with validation, apply, and outcome extraction:

```rust
pub(crate) struct SelfHealingEditContext {
    options: SelfHealingEditOptions,
    target_was_read: bool,
    proposal_ready: bool,
    apply_output: Option<pi_agent_core::AgentToolOutput>,
    outcome: Option<SelfHealingEditOutcome>,
    diagnostics: Vec<SelfHealingEditDiagnostic>,
    attempts: usize,
    failure_error: Option<CodingSessionError>,
}

impl SelfHealingEditContext {
    pub(crate) fn new(options: SelfHealingEditOptions) -> Self {
        Self {
            options,
            target_was_read: false,
            proposal_ready: false,
            apply_output: None,
            outcome: None,
            diagnostics: Vec::new(),
            attempts: 0,
            failure_error: None,
        }
    }

    pub(crate) fn diagnostics(&self) -> &[SelfHealingEditDiagnostic] {
        &self.diagnostics
    }

    pub(crate) fn take_failure_error(&mut self) -> Option<CodingSessionError> {
        self.failure_error.take()
    }

    pub(crate) fn finish_success(&self) -> Result<SelfHealingEditOutcome, CodingSessionError> {
        self.outcome.clone().ok_or_else(|| CodingSessionError::Session {
            message: "self-healing edit cannot finish without a recorded result".into(),
        })
    }

    fn fail(&mut self, error: CodingSessionError) -> String {
        let message = error.to_string();
        self.diagnostics.push(SelfHealingEditDiagnostic {
            message: message.clone(),
        });
        self.failure_error = Some(error);
        message
    }

    fn start_edit_workflow(&mut self) -> Result<(), CodingSessionError> {
        if self.options.path.trim().is_empty() {
            return Err(session_error("self-healing edit path must not be empty"));
        }
        if self.options.replacements.is_empty() {
            return Err(session_error("self-healing edit requires at least one replacement"));
        }
        self.target_was_read = false;
        self.proposal_ready = false;
        self.apply_output = None;
        self.outcome = None;
        self.attempts = 0;
        Ok(())
    }

    async fn read_target(&mut self) -> Result<(), CodingSessionError> {
        self.options
            .operations
            .read_file(&self.options.cwd.join(&self.options.path))
            .await
            .map_err(session_error)?;
        self.target_was_read = true;
        Ok(())
    }

    fn propose_patch(&mut self) -> Result<(), CodingSessionError> {
        if !self.target_was_read {
            return Err(session_error("self-healing edit cannot propose before reading target"));
        }
        self.proposal_ready = true;
        Ok(())
    }

    fn validate_patch(&mut self) -> Result<(), CodingSessionError> {
        if !self.proposal_ready {
            return Err(session_error("self-healing edit cannot validate before proposal"));
        }
        for replacement in &self.options.replacements {
            if replacement.old_text.is_empty() {
                return Err(session_error(format!(
                    "oldText must not be empty in {}.",
                    self.options.path
                )));
            }
        }
        Ok(())
    }

    async fn apply_patch(&mut self) -> Result<(), CodingSessionError> {
        self.attempts += 1;
        let args = serde_json::json!({
            "path": self.options.path,
            "edits": self
                .options
                .replacements
                .iter()
                .map(SelfHealingEditReplacement::to_json)
                .collect::<Vec<_>>(),
        });
        let output = edit_execute_with_operations(
            &self.options.cwd,
            args,
            self.options.operations.clone(),
        )
        .await
        .map_err(session_error)?;
        self.apply_output = Some(output);
        Ok(())
    }

    fn run_check(&mut self) -> Result<(), CodingSessionError> {
        Ok(())
    }

    fn repair_patch(&mut self) -> Result<(), CodingSessionError> {
        Ok(())
    }

    fn record_result(&mut self) -> Result<(), CodingSessionError> {
        let output = self.apply_output.as_ref().ok_or_else(|| {
            session_error("self-healing edit cannot record result before apply")
        })?;
        let message = output
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        let details = output.details.as_ref().ok_or_else(|| {
            session_error("self-healing edit output did not include edit details")
        })?;
        self.outcome = Some(SelfHealingEditOutcome {
            path: self.options.path.clone(),
            message,
            diff: details
                .get("diff")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_owned(),
            patch: details
                .get("patch")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_owned(),
            first_changed_line: details
                .get("firstChangedLine")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize),
            attempts: self.attempts,
            diagnostics: self.diagnostics.clone(),
        });
        Ok(())
    }
}
```

- [x] **Step 5: Add node implementation and error helpers**

Add the node spec enum and flow wrapper following the `export_flow.rs` pattern:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelfHealingEditNodeSpec {
    id: &'static str,
    name: &'static str,
    kind: SelfHealingEditNodeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelfHealingEditNodeKind {
    StartEditWorkflow,
    ReadTarget,
    ProposePatch,
    ValidatePatch,
    ApplyPatch,
    RunCheck,
    RepairPatch,
    RecordResult,
}

const SELF_HEALING_EDIT_NODE_SPECS: &[SelfHealingEditNodeSpec] = &[
    SelfHealingEditNodeSpec { id: "start_edit_workflow", name: "StartEditWorkflow", kind: SelfHealingEditNodeKind::StartEditWorkflow },
    SelfHealingEditNodeSpec { id: "read_target", name: "ReadTarget", kind: SelfHealingEditNodeKind::ReadTarget },
    SelfHealingEditNodeSpec { id: "propose_patch", name: "ProposePatch", kind: SelfHealingEditNodeKind::ProposePatch },
    SelfHealingEditNodeSpec { id: "validate_patch", name: "ValidatePatch", kind: SelfHealingEditNodeKind::ValidatePatch },
    SelfHealingEditNodeSpec { id: "apply_patch", name: "ApplyPatch", kind: SelfHealingEditNodeKind::ApplyPatch },
    SelfHealingEditNodeSpec { id: "run_check", name: "RunCheck", kind: SelfHealingEditNodeKind::RunCheck },
    SelfHealingEditNodeSpec { id: "repair_patch", name: "RepairPatch", kind: SelfHealingEditNodeKind::RepairPatch },
    SelfHealingEditNodeSpec { id: "record_result", name: "RecordResult", kind: SelfHealingEditNodeKind::RecordResult },
];

pub(crate) struct SelfHealingEditFlow {
    flow: Flow<SelfHealingEditContext>,
}

impl SelfHealingEditFlow {
    pub(crate) fn new() -> Result<Self, CodingSessionError> {
        let mut flow = Flow::new(SELF_HEALING_EDIT_NODE_IDS[0]).map_err(flow_error)?;
        for spec in SELF_HEALING_EDIT_NODE_SPECS {
            flow.add_node(spec.id, SelfHealingEditNode::new(spec.name, spec.kind))
                .map_err(flow_error)?;
        }
        for pair in SELF_HEALING_EDIT_NODE_IDS.windows(2) {
            flow.edge(pair[0], pair[1]).map_err(flow_error)?;
        }
        Ok(Self { flow })
    }

    #[cfg(test)]
    pub(crate) fn node_ids() -> &'static [&'static str] {
        SELF_HEALING_EDIT_NODE_IDS
    }

    pub(crate) async fn run(
        &self,
        ctx: &mut SelfHealingEditContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.flow.run(ctx).await.map_err(flow_error)
    }
}

#[derive(Debug, Clone, Copy)]
struct SelfHealingEditNode {
    name: &'static str,
    kind: SelfHealingEditNodeKind,
}

impl SelfHealingEditNode {
    fn new(name: &'static str, kind: SelfHealingEditNodeKind) -> Self {
        Self { name, kind }
    }
}

impl FlowNode<SelfHealingEditContext> for SelfHealingEditNode {
    fn name(&self) -> &str {
        self.name
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut SelfHealingEditContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            let result = match self.kind {
                SelfHealingEditNodeKind::StartEditWorkflow => ctx.start_edit_workflow(),
                SelfHealingEditNodeKind::ReadTarget => ctx.read_target().await,
                SelfHealingEditNodeKind::ProposePatch => ctx.propose_patch(),
                SelfHealingEditNodeKind::ValidatePatch => ctx.validate_patch(),
                SelfHealingEditNodeKind::ApplyPatch => ctx.apply_patch().await,
                SelfHealingEditNodeKind::RunCheck => ctx.run_check(),
                SelfHealingEditNodeKind::RepairPatch => ctx.repair_patch(),
                SelfHealingEditNodeKind::RecordResult => ctx.record_result(),
            };
            match result {
                Ok(()) => default_action(),
                Err(error) => Err(ctx.fail(error)),
            }
        })
    }
}

fn default_action() -> Result<Action, String> {
    Action::new(DEFAULT_ACTION).map_err(|error| error.to_string())
}

fn session_error(message: impl Into<String>) -> CodingSessionError {
    CodingSessionError::Session {
        message: message.into(),
    }
}

fn flow_error(error: FlowError) -> CodingSessionError {
    CodingSessionError::Flow {
        message: error.to_string(),
    }
}
```

- [x] **Step 6: Add ExecutionEnv operation adapter**

Add this adapter at the end of `self_healing_edit_flow.rs`:

```rust
struct ExecutionEnvEditOperations<E> {
    env: E,
}

impl<E> ExecutionEnvEditOperations<E> {
    fn new(env: E) -> Self {
        Self { env }
    }
}

impl<E> EditOperations for ExecutionEnvEditOperations<E>
where
    E: ExecutionEnv + Clone + 'static,
{
    fn read_file<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<Vec<u8>, String>> {
        async move {
            self.env
                .read_binary_file(path.to_string_lossy().as_ref())
                .await
                .map_err(file_error_message)
        }
        .boxed()
    }

    fn write_file<'a>(
        &'a self,
        path: &'a Path,
        content: &'a [u8],
    ) -> BoxFuture<'a, Result<(), String>> {
        async move {
            self.env
                .write_file(path.to_string_lossy().as_ref(), content)
                .await
                .map_err(file_error_message)
        }
        .boxed()
    }
}

fn file_error_message(error: FileError) -> String {
    error.to_string()
}
```

- [x] **Step 7: Run format for the new module**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt
```

Expected: rustfmt completes with exit 0.

## Task 3: Register FlowService Entry Points

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/flow_service.rs`

- [x] **Step 1: Add imports**

Add this import near the other flow imports:

```rust
use super::self_healing_edit_flow::{
    SelfHealingEditContext, SelfHealingEditFlow, SelfHealingEditOutcome,
};
```

- [x] **Step 2: Add FlowService constructors and runners**

Add these methods in `impl FlowService` near the other workflow methods:

```rust
pub(crate) fn self_healing_edit_flow(
    &self,
) -> Result<SelfHealingEditFlow, CodingSessionError> {
    SelfHealingEditFlow::new()
}

pub(crate) async fn run_self_healing_edit_graph(
    &self,
    ctx: &mut SelfHealingEditContext,
) -> Result<FlowOutcome, CodingSessionError> {
    self.self_healing_edit_flow()?.run(ctx).await
}

pub(crate) async fn run_self_healing_edit(
    &self,
    ctx: &mut SelfHealingEditContext,
) -> Result<SelfHealingEditOutcome, CodingSessionError> {
    match self.run_self_healing_edit_graph(ctx).await {
        Ok(_) => ctx.finish_success(),
        Err(error) => Err(ctx.take_failure_error().unwrap_or(error)),
    }
}
```

- [x] **Step 3: Run the focused tests**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent flow_service::tests::self_healing_edit_flow -- --nocapture
```

Expected: all self-healing edit flow tests pass.

## Task 4: Update TODO and Plan Status

**Files:**
- Modify: `docs/TODO.md`

- [x] **Step 1: Add source document links**

Under `## Source Documents`, add:

```markdown
- [Self-healing edit workflow design](superpowers/specs/2026-07-04-self-healing-edit-workflow-design.md)
- [Self-healing edit workflow plan](superpowers/plans/2026-07-04-self-healing-edit-workflow-plan.md)
```

- [x] **Step 2: Update Phase 6 current-state summary**

Change the `Current North Star` Phase 6 sentence ending from:

```markdown
Self-healing edit workflows remain follow-up.
```

to:

```markdown
Self-healing edit workflow design and first internal Flow slice are in progress.
```

- [x] **Step 3: Update the self-healing edit checklist item**

Change:

```markdown
- [x] Design and prototype self-healing edit workflow after runtime control stabilization passes its readiness gate.
```

to:

```markdown
- [~] Design and prototype self-healing edit workflow after runtime control stabilization passes its readiness gate. Design and implementation plan are in place; the first internal `SelfHealingEditFlow` slice wraps existing edit behavior with stable Flow nodes and an `ExecutionEnv`-backed operation path.
```

- [x] **Step 4: Add progress note**

Append under `## Progress Notes`:

```markdown
- 2026-07-04: Self-healing edit workflow design and first implementation plan added. The selected path is an internal `SelfHealingEditFlow` that wraps existing edit behavior first, proves stable Flow nodes and `ExecutionEnv`-backed file operations, then later layers check/repair, capability integration, durable events, and adapter exposure.
```

## Task 5: Final Verification

**Files:**
- Verify full repo state from `/home/whai/dev_wkspace/pi2rust/pi-rust`

- [x] **Step 1: Run focused tests**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent flow_service::tests::self_healing_edit_flow -- --nocapture
```

Expected: self-healing edit flow tests pass.

- [x] **Step 2: Run pi-coding-agent tests**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet
```

Expected: exit 0 with all non-ignored pi-coding-agent tests passing.

- [x] **Step 3: Run workspace checks**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt --check
/home/whai/.cargo/bin/cargo check --workspace --quiet
/home/whai/.cargo/bin/cargo test --workspace --quiet
git diff --check
```

Expected: all commands exit 0.

- [x] **Step 4: Review git status**

Run:

```bash
git status --short
```

Expected: changes include the new design, plan, self-healing edit flow module, FlowService registration/tests, and TODO update alongside existing active worktree changes. Do not revert unrelated files.

## Commit Boundaries

Use these logical commit boundaries if the user requests commits after verification:

```bash
git add docs/superpowers/specs/2026-07-04-self-healing-edit-workflow-design.md docs/superpowers/plans/2026-07-04-self-healing-edit-workflow-plan.md docs/TODO.md
git commit -m "docs: design self-healing edit workflow"

git add crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs crates/pi-coding-agent/src/coding_session/flow_service.rs crates/pi-coding-agent/src/coding_session/mod.rs
git commit -m "feat(coding-agent): add self-healing edit flow slice"
```

Because this workspace already contains active uncommitted Phase 6 work, do not run these commit commands unless a commit is explicitly requested.
