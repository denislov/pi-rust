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
