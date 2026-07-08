# Intent Router Admission Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Start Stage 3 of the operation runtime reference architecture by introducing one internal intent-admission boundary for operation dispatch mode validation and active-operation guard acquisition.

**Architecture:** Keep `CodingAgentSession` as the public facade and preserve all current public methods. Add a crate-internal `IntentRouter` that turns `Operation` metadata into `OperationAdmission`, validates the requested dispatcher mode, and begins the admitted operation through `OperationControl`. Dynamic operations such as delegation approval remain resolved by the session owner before being handed to the router.

**Tech Stack:** Rust 2024, `pi-coding-agent`, existing operation/session tests, deterministic offline cargo checks.

---

## File Structure

- `crates/pi-coding-agent/src/coding_session/intent_router.rs`
  - New crate-internal admission helper.
  - Owns static `Operation` admission, dispatch-mode validation, and `OperationControl::begin()` delegation.
- `crates/pi-coding-agent/src/coding_session/mod.rs`
  - Wires the router into sync, sync-mutable, and async dispatchers.
  - Keeps dynamic delegation approval kind resolution in the session owner.
- `crates/pi-coding-agent/src/coding_session/operation.rs`
  - Adds small helpers for dispatch-mode error labels if needed by the router.
  - Hosts the first failing boundary test before `intent_router.rs` exists.
- `docs/TODO.md`
  - Records Stage 3 progress once the first router slice is in place.

## Task 1: Introduce IntentRouter Admission Boundary

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Create: `crates/pi-coding-agent/src/coding_session/intent_router.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-08-intent-router-admission-plan.md`

- [x] **Step 1: Write the failing router tests**

Add tests requiring a new internal `IntentRouter` boundary:

```rust
#[test]
fn intent_router_rejects_dynamic_operation_without_owner_resolution() {
    let operation = Operation::ApproveDelegationConfirmation {
        operation_id: "op_parent".into(),
        tool_call_id: "tool_delegate".into(),
    };

    let error = IntentRouter::static_admission(&operation).unwrap_err();

    assert_eq!(
        error,
        CodingSessionError::UnsupportedCapability {
            capability: "dynamic operation requires async dispatcher".into(),
        }
    );
}

#[test]
fn intent_router_validates_dispatch_mode_before_beginning_operation() {
    let operation = Operation::PluginCommand {
        command_id: "plugin.echo".into(),
        args: serde_json::json!({}),
    };
    let admission = IntentRouter::static_admission(&operation).unwrap();
    let control = OperationControl::new();

    let error = IntentRouter::begin(
        &control,
        &admission,
        OperationDispatchMode::Async,
    )
    .unwrap_err();

    assert_eq!(
        error,
        CodingSessionError::UnsupportedCapability {
            capability: "plugin_command operation requires read-only sync dispatcher".into(),
        }
    );
    assert_eq!(control.active(), None);
}

#[test]
fn intent_router_begins_admitted_operation_and_uses_busy_guard() {
    let operation = Operation::PluginCommand {
        command_id: "plugin.echo".into(),
        args: serde_json::json!({}),
    };
    let admission = IntentRouter::static_admission(&operation).unwrap();
    let control = OperationControl::new();

    let guard = IntentRouter::begin(
        &control,
        &admission,
        OperationDispatchMode::SyncReadOnly,
    )
    .unwrap();

    assert_eq!(control.active(), Some(OperationKind::PluginCommand));
    assert_eq!(
        IntentRouter::begin(
            &control,
            &admission,
            OperationDispatchMode::SyncReadOnly,
        )
        .unwrap_err(),
        CodingSessionError::Busy {
            operation: "plugin_command".into(),
        }
    );

    drop(guard);
    assert_eq!(control.active(), None);
}
```

- [x] **Step 2: Run the focused RED command**

Run:

```bash
cargo test -p pi-coding-agent intent_router_ --lib
```

Expected RED: compile failure because `IntentRouter` does not exist yet.

RED result: after fixing a missing test import, the focused command failed only with `could not find intent_router in super`, proving the new tests were exercising the missing boundary.

- [x] **Step 3: Add the minimal router implementation**

Create `intent_router.rs`:

```rust
use super::CodingSessionError;
use super::operation::{Operation, OperationAdmission, OperationDispatchMode};
use super::operation_control::{OperationControl, OperationGuard};

pub(crate) struct IntentRouter;

impl IntentRouter {
    pub(crate) fn static_admission(
        operation: &Operation,
    ) -> Result<OperationAdmission, CodingSessionError> {
        let metadata = operation.metadata();
        if operation.static_kind().is_none() {
            return Err(CodingSessionError::UnsupportedCapability {
                capability: "dynamic operation requires async dispatcher".into(),
            });
        }
        Ok(OperationAdmission::new(operation.kind(), metadata, None))
    }

    pub(crate) fn begin(
        control: &OperationControl,
        admission: &OperationAdmission,
        expected: OperationDispatchMode,
    ) -> Result<OperationGuard, CodingSessionError> {
        if admission.metadata.dispatch_mode != expected {
            return Err(CodingSessionError::UnsupportedCapability {
                capability: format!(
                    "{} operation requires {} dispatcher",
                    admission.kind.as_str(),
                    admission.metadata.dispatch_mode.dispatcher_label(),
                ),
            });
        }
        control.begin(admission.kind)
    }

    pub(crate) fn unsupported_dispatch(admission: &OperationAdmission) -> CodingSessionError {
        CodingSessionError::UnsupportedCapability {
            capability: format!(
                "{} operation requires {} dispatcher",
                admission.kind.as_str(),
                admission.metadata.dispatch_mode.dispatcher_label(),
            ),
        }
    }
}
```

Add `OperationDispatchMode::dispatcher_label()`:

```rust
impl OperationDispatchMode {
    pub(crate) fn dispatcher_label(self) -> &'static str {
        match self {
            Self::Async => "async",
            Self::SyncReadOnly => "read-only sync",
            Self::SyncMutable => "sync mutable",
        }
    }
}
```

- [x] **Step 4: Wire dispatchers through the router**

Update `CodingAgentSession` so `run_sync_operation`, `run_sync_mut_operation`, and `run_operation` validate dispatcher mode through `IntentRouter::begin()` before running operation-specific match arms. Keep `resolve_operation_admission()` for dynamic delegation approval, but use `IntentRouter::static_admission()` for all static operations.

- [x] **Step 5: Run GREEN checks**

Run:

```bash
cargo test -p pi-coding-agent intent_router_ --lib
cargo test -p pi-coding-agent operation --lib
cargo check -p pi-coding-agent
```

Expected GREEN: all router and operation tests pass, and `pi-coding-agent` compiles.

GREEN result: `cargo test -p pi-coding-agent intent_router_ --lib` passed 3 tests, `cargo test -p pi-coding-agent operation --lib` passed 51 tests, and `cargo check -p pi-coding-agent` finished without warnings.

- [x] **Step 6: Update docs and commit**

Update `docs/TODO.md` to state that Stage 3 has started with an internal `IntentRouter` admission boundary. Mark this task complete in this plan with the actual RED/GREEN notes, then commit:

```bash
git add crates/pi-coding-agent/src/coding_session/operation.rs crates/pi-coding-agent/src/coding_session/intent_router.rs crates/pi-coding-agent/src/coding_session/mod.rs docs/TODO.md docs/superpowers/plans/2026-07-08-intent-router-admission-plan.md
git commit -m "feat: add intent router admission boundary"
```

Docs result: `docs/TODO.md` now records Stage 3 startup and this plan records the first RED/GREEN implementation slice. Commit remains pending until final verification for this turn passes.

## Task 2: Route Prompt Control Commands Through Intent Admission

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/intent_router.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-08-intent-router-admission-plan.md`

- [x] **Step 1: Write failing prompt-control admission tests**

Add a test that requests a prompt control handle through the router and verifies it is classified as `OperationClass::Control`, rejects creation while another operation is active, and still sends abort/steer/follow-up commands through the existing `PromptControlHandle`.

RED result: `cargo test -p pi-coding-agent prompt_control --lib` failed because `ControlIntent` and `IntentRouter::prompt_control_handle()` did not exist.

- [x] **Step 2: Add control intent metadata**

Add a small internal control-intent type for prompt abort, steer, and follow-up. It should not replace the existing `PromptControlCommand`; it only gives admission a typed control request before acquiring the existing handle.

Implemented `ControlIntent::PromptControl` with metadata targeting `OperationKind::Prompt` and `OperationClass::Control`. The existing `PromptControlCommand` channel remains unchanged.

- [x] **Step 3: Wire existing prompt-control handle creation through the router**

Keep `CodingAgentSession::prompt_control_handle()` as the adapter-facing method, but make it delegate admission to the router before creating the channel through `OperationControl`.

`CodingAgentSession::prompt_control_handle()` now calls `IntentRouter::prompt_control_handle(&mut self.operation_control, ControlIntent::PromptControl)`. `OperationControl` itself did not need changes; it still owns receiver lifecycle and busy/pending checks.

- [x] **Step 4: Run focused and adapter checks**

Run:

```bash
cargo test -p pi-coding-agent prompt_control --lib
cargo test -p pi-coding-agent --test rpc_mode
cargo test -p pi-coding-agent --test interactive_mode
```

Expected GREEN: existing RPC and interactive control behavior stays unchanged while handle creation now has an explicit admission path.

GREEN result: `cargo test -p pi-coding-agent prompt_control --lib` passed 8 tests, `cargo test -p pi-coding-agent --test rpc_mode` passed 36 tests, `cargo test -p pi-coding-agent --test interactive_mode` passed 41 tests, and `cargo check -p pi-coding-agent` finished without warnings.

## Task 3: Admit Query Intents Through IntentRouter

**Scope:** Add the first non-operation client-intent admission path for pure query surfaces. This task intentionally does not change `OperationClass::ReadOnly` operation scheduling for `Export`; read-only operation concurrency should be handled by a later scheduler task because it affects committed-state read semantics while a session writer is active.

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/intent_router.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-08-intent-router-admission-plan.md`

- [x] **Step 1: Write failing query-intent router tests**

Add tests requiring a new query admission path:

```rust
#[test]
fn query_intents_are_classified_as_query() {
    for intent in [
        QueryIntent::Capabilities,
        QueryIntent::SessionView,
        QueryIntent::AgentProfiles,
        QueryIntent::TeamProfiles,
        QueryIntent::ProfileDiagnostics,
        QueryIntent::PendingDelegationConfirmations,
    ] {
        let metadata = intent.metadata();

        assert_eq!(metadata.intent, intent);
        assert_eq!(metadata.class, OperationClass::Query);
    }
}

#[test]
fn intent_router_admits_queries_while_root_operation_is_busy() {
    let control = OperationControl::new();
    let guard = control.begin(OperationKind::Prompt).unwrap();

    let admission =
        IntentRouter::admit_query(&control, QueryIntent::PendingDelegationConfirmations);

    assert_eq!(admission.intent, QueryIntent::PendingDelegationConfirmations);
    assert_eq!(admission.class, OperationClass::Query);
    assert_eq!(control.active(), Some(OperationKind::Prompt));
    drop(guard);
    assert_eq!(control.active(), None);
}
```

- [x] **Step 2: Run focused RED command**

Run:

```bash
cargo test -p pi-coding-agent query_intent --lib
```

Expected RED: compile failure because `QueryIntent` and `IntentRouter::admit_query()` do not exist.

RED result: after fixing an initial test-placement mistake, `cargo test -p pi-coding-agent query_intent --lib` failed only because `QueryIntent` and `IntentRouter::admit_query()` did not exist. A second wiring guard, `cargo test -p pi-coding-agent session_query_facade --lib`, failed until the session facade methods were routed through query admission.

- [x] **Step 3: Add minimal query intent admission**

Add a query-intent type and metadata to `intent_router.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QueryIntent {
    Capabilities,
    SessionView,
    AgentProfiles,
    TeamProfiles,
    ProfileDiagnostics,
    PendingDelegationConfirmations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueryIntentMetadata {
    pub(crate) intent: QueryIntent,
    pub(crate) class: OperationClass,
}

impl QueryIntent {
    pub(crate) fn metadata(self) -> QueryIntentMetadata {
        QueryIntentMetadata {
            intent: self,
            class: OperationClass::Query,
        }
    }
}
```

Add the router method:

```rust
pub(crate) fn admit_query(
    control: &OperationControl,
    intent: QueryIntent,
) -> QueryIntentMetadata {
    let metadata = intent.metadata();
    debug_assert_eq!(metadata.class, OperationClass::Query);
    let _ = control;
    metadata
}
```

- [x] **Step 4: Route session query surfaces through the router**

Update `CodingAgentSession` query/read facade methods so they call `IntentRouter::admit_query()` before reading state:

```rust
IntentRouter::admit_query(&self.operation_control, QueryIntent::Capabilities);
IntentRouter::admit_query(&self.operation_control, QueryIntent::SessionView);
IntentRouter::admit_query(&self.operation_control, QueryIntent::AgentProfiles);
IntentRouter::admit_query(&self.operation_control, QueryIntent::TeamProfiles);
IntentRouter::admit_query(&self.operation_control, QueryIntent::ProfileDiagnostics);
IntentRouter::admit_query(
    &self.operation_control,
    QueryIntent::PendingDelegationConfirmations,
);
```

Keep existing return types and adapter behavior unchanged.

- [x] **Step 5: Run GREEN and adapter checks**

Run:

```bash
cargo test -p pi-coding-agent query_intent --lib
cargo test -p pi-coding-agent prompt_control --lib
cargo test -p pi-coding-agent --test rpc_mode list_delegation_confirmations
cargo test -p pi-coding-agent --test interactive_mode delegation_confirmation
cargo check -p pi-coding-agent
```

Expected GREEN: query admission tests pass, prompt control behavior stays unchanged, and RPC/interactive delegation-confirmation query paths retain their existing behavior.

GREEN result: `cargo test -p pi-coding-agent intent_router --lib` passed 9 tests, `cargo test -p pi-coding-agent query_intent --lib` passed 1 test, `cargo test -p pi-coding-agent prompt_control --lib` passed 8 tests, `cargo test -p pi-coding-agent --test rpc_mode rpc_list_agent_profiles_reports_registry` passed, `cargo test -p pi-coding-agent --test rpc_mode rpc_list_team_profiles_reports_registry` passed, `cargo test -p pi-coding-agent --test rpc_mode rpc_lists_and_approves_delegation_confirmation` passed, `cargo test -p pi-coding-agent --test rpc_mode rpc_rejects_delegation_confirmation` passed, and `cargo test -p pi-coding-agent --test interactive_mode delegation_confirmation` passed 1 test. The initially listed `list_delegation_confirmations` RPC filter matched no test names, so the named RPC delegation confirmation tests above were used for behavior coverage. Final verification also passed: `cargo fmt --check`, `git diff --check`, `cargo check -p pi-coding-agent`, `cargo check --workspace`, and `cargo test --workspace`.

- [x] **Step 6: Update docs and commit**

Update `docs/TODO.md` to record that Stage 3 now admits pure query client intents through `IntentRouter` without taking the active root-operation guard. Mark this task complete with actual RED/GREEN notes, then commit:

```bash
git add crates/pi-coding-agent/src/coding_session/intent_router.rs crates/pi-coding-agent/src/coding_session/mod.rs docs/TODO.md docs/superpowers/plans/2026-07-08-intent-router-admission-plan.md
git commit -m "feat: admit query intents through intent router"
```

Docs result: `docs/TODO.md` now records that capabilities, session view, profile listings, profile diagnostics, and pending delegation confirmation queries flow through `QueryIntent` admission without taking the active root-operation guard. Commit remains pending until final verification for this turn passes.

## Task 4: Admit ReadOnly Operations Without Taking the Root Guard

**Scope:** Introduce an explicit operation permit from `IntentRouter` so `OperationClass::ReadOnly` operations validate through the same admission path but do not acquire the active root-operation guard. Keep `PluginCommand` and other non-read-only operations guarded. This task only changes scheduler/admission semantics; it does not change `ExportFlow` replay or rendering behavior.

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/intent_router.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-08-intent-router-admission-plan.md`

- [x] **Step 1: Write failing read-only admission tests**

Add router and session tests that require `Export` to admit while another root operation is active, while keeping non-read-only sync operations guarded:

```rust
#[test]
fn read_only_admission_allows_export_while_root_operation_is_busy() {
    let operation = Operation::Export(ExportOptions::view());
    let admission = IntentRouter::static_admission(&operation).unwrap();
    let control = OperationControl::new();
    let guard = control.begin(OperationKind::Prompt).unwrap();

    let permit = IntentRouter::admit_operation(
        &control,
        &admission,
        OperationDispatchMode::SyncReadOnly,
    )
    .unwrap();

    assert_eq!(permit.kind(), OperationKind::Export);
    assert_eq!(permit.class(), OperationClass::ReadOnly);
    assert!(!permit.is_guarded());
    assert_eq!(control.active(), Some(OperationKind::Prompt));
    drop(permit);
    assert_eq!(control.active(), Some(OperationKind::Prompt));
    drop(guard);
    assert_eq!(control.active(), None);
}
```

```rust
#[tokio::test]
async fn run_sync_operation_export_uses_read_only_admission_while_root_busy() {
    let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
        .await
        .unwrap();
    let _guard = session.operation_control.begin(OperationKind::Prompt).unwrap();

    let error = session
        .run_sync_operation(Operation::Export(ExportOptions::view()))
        .unwrap_err();

    assert_eq!(error.code(), "unsupported_capability");
    assert_eq!(
        error.to_string(),
        "unsupported capability: export requires a persistent Rust-native session"
    );
    assert_eq!(session.operation_control.active(), Some(OperationKind::Prompt));
}
```

- [x] **Step 2: Run focused RED commands**

Run:

```bash
cargo test -p pi-coding-agent read_only_admission --lib
cargo test -p pi-coding-agent run_sync_operation_export_uses_read_only_admission_while_root_busy --lib
```

Expected RED: the router test fails to compile because `IntentRouter::admit_operation()` and `OperationPermit` do not exist; the session test fails with a `busy` error until `run_sync_operation()` uses read-only admission.

RED result: `cargo test -p pi-coding-agent read_only_admission --lib` first failed with `no associated function or constant named admit_operation found for struct IntentRouter`. After adding the router permit API but before wiring session dispatch, the same command ran 3 tests with the two router tests passing and `run_sync_operation_export_uses_read_only_admission_while_root_busy` failing because it still returned `busy` instead of the expected export persistence error.

- [x] **Step 3: Add an operation permit to IntentRouter**

Add an internal permit that either holds an `OperationGuard` or represents an unguarded read-only admission:

```rust
#[derive(Debug)]
#[must_use = "dropping OperationPermit releases any guarded operation"]
pub(crate) struct OperationPermit {
    guard: Option<OperationGuard>,
    #[cfg(test)]
    kind: OperationKind,
    #[cfg(test)]
    class: OperationClass,
}
```

Add `IntentRouter::admit_operation()`:

```rust
pub(crate) fn admit_operation(
    control: &OperationControl,
    admission: &OperationAdmission,
    expected: OperationDispatchMode,
) -> Result<OperationPermit, CodingSessionError> {
    Self::validate_dispatch_mode(admission, expected)?;

    if admission.metadata.class == OperationClass::ReadOnly {
        return Ok(OperationPermit::unguarded(
            admission.kind,
            admission.metadata.class,
        ));
    }

    control
        .begin(admission.kind)
        .map(|guard| OperationPermit::guarded(admission.kind, admission.metadata.class, guard))
}
```

Keep `IntentRouter::begin()` as a guarded compatibility helper for existing focused tests, but have both methods share dispatch-mode validation.

Implementation note: `OperationPermit` now holds an optional `OperationGuard`; permit introspection and the legacy `IntentRouter::begin()` helper are test-only so normal `cargo check -p pi-coding-agent` remains warning-free.

- [x] **Step 4: Wire operation dispatchers through permits**

Update `CodingAgentSession::run_sync_operation()`, `run_sync_mut_operation()`, and async `run_operation()` to bind the result of `IntentRouter::admit_operation()`:

```rust
let _operation_permit = IntentRouter::admit_operation(
    &self.operation_control,
    &admission,
    OperationDispatchMode::SyncReadOnly,
)?;
```

Use `SyncMutable` and `Async` respectively in the other dispatchers. This preserves guards for session writes, runtime writes, plugin commands, delegation approval, and delegation rejection, while allowing read-only `Export` admission to leave any active root operation intact.

- [x] **Step 5: Run GREEN checks**

Run:

```bash
cargo test -p pi-coding-agent read_only_admission --lib
cargo test -p pi-coding-agent intent_router --lib
cargo test -p pi-coding-agent run_sync_operation_export_uses_read_only_admission_while_root_busy --lib
cargo test -p pi-coding-agent run_sync_operation_plugin_command --lib
cargo check -p pi-coding-agent
```

Expected GREEN: read-only admission tests pass, existing intent-router tests pass, `PluginCommand` remains guarded, and `pi-coding-agent` compiles.

GREEN result: `cargo test -p pi-coding-agent read_only_admission --lib` passed 3 tests, `cargo test -p pi-coding-agent intent_router --lib` passed 11 tests, `cargo test -p pi-coding-agent run_sync_operation_plugin_command --lib` passed 2 tests, `cargo test -p pi-coding-agent export_current_html_uses_read_only_operation_admission_while_root_busy --lib` passed 1 test, and `cargo check -p pi-coding-agent` finished without warnings. Full `cargo test --workspace` initially exposed an old guarded-export assertion in `export_current_html_uses_export_operation_boundary`; after updating that test to the read-only admission contract, `cargo test --workspace` passed.

- [x] **Step 6: Update docs and commit**

Update `docs/TODO.md` to record that Stage 3 now admits `OperationClass::ReadOnly` operations without taking the active root-operation guard. Mark this task complete with actual RED/GREEN notes, then commit:

```bash
git add crates/pi-coding-agent/src/coding_session/intent_router.rs crates/pi-coding-agent/src/coding_session/mod.rs docs/TODO.md docs/superpowers/plans/2026-07-08-intent-router-admission-plan.md
git commit -m "feat: admit read-only operations without root guard"
```

Docs result: `docs/TODO.md` now records that `OperationClass::ReadOnly` operations admit through `IntentRouter` as unguarded permits, while non-read-only operations keep the active root-operation guard. Commit remains pending until final verification for this turn passes.
