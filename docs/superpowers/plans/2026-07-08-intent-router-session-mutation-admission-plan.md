# Intent Router Session Mutation Admission Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining Stage 3 admission gap by routing `set_default_agent_profile_id` and `fork_current_session` through `IntentRouter` so no UI/RPC path starts session-affecting work by directly calling `SessionService`.

**Architecture:** Add two typed `Operation` variants -- `SetDefaultAgentProfile` (RuntimeWrite, SyncMutable) and `ForkSession` (SessionWriteRoot, SyncMutable) -- with their `OperationKind`, metadata, and outcome entries. Route `set_default_agent_profile_id` through the existing `run_sync_mut_operation` dispatcher. Route `fork_current_session` through direct `IntentRouter::admit_operation()` admission (like `prompt_control_handle` and `admit_query`) because it returns a new `CodingAgentSession` that cannot flow through the `OperationOutcome` return type. Keep all current public method signatures and adapter behavior unchanged.

**Tech Stack:** Rust 2024, `pi-coding-agent`, existing operation/intent-router tests, deterministic offline cargo checks.

---

## Context For Implementers

### Current State

The `IntentRouter` in `crates/pi-coding-agent/src/coding_session/intent_router.rs` already owns:
- `static_admission(&Operation) -> Result<OperationAdmission, CodingSessionError>` -- validates static operations and returns admission metadata.
- `admit_operation(&OperationControl, &OperationAdmission, OperationDispatchMode) -> Result<OperationPermit, CodingSessionError>` -- validates dispatch mode, returns an unguarded permit for `OperationClass::ReadOnly`, or a guarded permit (taking the root operation guard) for all other classes.
- `admit_query(&OperationControl, QueryIntent) -> QueryIntentMetadata` -- admits pure query intents without the root guard.
- `prompt_control_handle(&mut OperationControl, ControlIntent) -> Result<PromptControlHandle, CodingSessionError>` -- admits prompt control handle creation.

Three dispatchers on `CodingAgentSession` already route through `IntentRouter::admit_operation`:
- `run_sync_operation(&self, operation) -> Result<OperationOutcome, CodingSessionError>` -- validates `SyncReadOnly`.
- `run_sync_mut_operation(&mut self, operation) -> Result<OperationOutcome, CodingSessionError>` -- validates `SyncMutable`.
- `run_operation(&mut self, operation) -> Result<OperationOutcome, CodingSessionError>` -- validates `Async`.

### The Gap

Two session-affecting public methods bypass admission by calling `SessionService` directly:

1. `CodingAgentSession::set_default_agent_profile_id(&mut self, profile_id)` at `mod.rs:432` -- called from `protocol/rpc/commands.rs:668` (RPC `set_default_agent_profile`) and `interactive/loop.rs:804`. It calls `session_service.set_default_agent_profile_id(profile_id)?` then `event_service.emit_default_agent_profile_changed(profile_id)` without any operation guard. This is a `RuntimeWrite` (installs a new default profile generation).

2. `CodingAgentSession::fork_current_session(&self, target_leaf_id)` at `mod.rs:355` -- called from `interactive/prompt_task.rs:1251`. It calls `session_service.fork_current(target_leaf_id)?` then `Self::from_services(...)` without any operation guard. This is a `SessionWriteRoot` (session lifecycle mutation).

### Why Fork Uses Direct Admission (Not a Dispatcher)

`fork_current_session` returns `Result<Self, CodingAgentSession>` -- a new `CodingAgentSession` -- which cannot be carried by `OperationOutcome` (which derives `Debug`; `CodingAgentSession` does not implement `Debug`). The established pattern for methods that don't fit the `OperationOutcome` return type is direct `IntentRouter` admission: `prompt_control_handle` and `admit_query` both call `IntentRouter` directly rather than through a dispatcher. `fork_current_session` follows the same pattern.

### Key Types

```rust
// crates/pi-coding-agent/src/coding_session/operation_control.rs
pub(crate) enum OperationKind {
    Prompt, Compact, PluginCommand, PluginLoad, DelegationConfirmation,
    BranchSummary, AgentInvocation, AgentTeam, Export, SelfHealingEdit,
}
// Each variant has an as_str() arm.

// crates/pi-coding-agent/src/coding_session/operation.rs
pub(crate) enum Operation { /* 11 variants, each with a metadata() arm */ }
pub(crate) enum OperationOutcome { /* 11 variants */ }
pub(crate) enum OperationClass { Query, ReadOnly, SessionWriteRoot, NonSessionRoot, RuntimeWrite, Child, Control }
pub(crate) enum OperationDispatchMode { Async, SyncReadOnly, SyncMutable }
pub(crate) enum OperationOrigin { ClientRoot, ParentChild, RuntimeInternal }
```

`ProfileId` lives in `crates/pi-coding-agent/src/coding_session/profiles.rs` and derives `Debug, Clone, PartialEq, Eq`. Other modules import it as `use super::profiles::ProfileId;`.

### Exhaustive Match Sites That Need New Arms

Adding `OperationKind::SetDefaultAgentProfile` and `OperationKind::ForkSession` requires updating:
- `OperationKind::as_str()` in `operation_control.rs` (exhaustive match).

Adding `Operation::SetDefaultAgentProfile` and `Operation::ForkSession` requires updating:
- `Operation::metadata()` in `operation.rs` (exhaustive match).
- `run_sync_operation` match in `mod.rs` (exhaustive match -- add to fallthrough).
- `run_sync_mut_operation` match in `mod.rs` (exhaustive match -- add handler + fallthrough).
- `run_operation` match in `mod.rs` (exhaustive match -- add to fallthrough).

All other `OperationKind` references in `context.rs`, `capability_service.rs`, `session_log/`, and `protocol/rpc/prompt.rs` use specific variants in non-exhaustive positions (test assertions, specific construction, or `Some(operation) => operation.as_str()` with `_` fallthrough) and do NOT need changes.

---

## File Structure

- `crates/pi-coding-agent/src/coding_session/operation_control.rs`
  - Add `SetDefaultAgentProfile` and `ForkSession` to `OperationKind` and `as_str()`.
- `crates/pi-coding-agent/src/coding_session/operation.rs`
  - Add `SetDefaultAgentProfile` and `ForkSession` to `Operation`.
  - Add `SetDefaultAgentProfile` to `OperationOutcome`.
  - Add metadata arms for both new variants.
  - Add import for `ProfileId`.
  - Add metadata tests.
- `crates/pi-coding-agent/src/coding_session/mod.rs`
  - Route `set_default_agent_profile_id` through `run_sync_mut_operation`.
  - Route `fork_current_session` through direct `IntentRouter::admit_operation`.
  - Update all three dispatcher match arms.
  - Add busy-guard and behavior tests.
- `docs/TODO.md`
  - Record Stage 3 progress.
- `docs/superpowers/plans/2026-07-08-intent-router-session-mutation-admission-plan.md`
  - This plan; mark tasks complete with RED/GREEN notes.

---

## Task 1: Add `Operation::SetDefaultAgentProfile` (RuntimeWrite) And Route Through Admission

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation_control.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-08-intent-router-session-mutation-admission-plan.md`

- [x] **Step 1: Write the failing operation metadata test**

Add this test to the `#[cfg(test)] mod tests` block in `crates/pi-coding-agent/src/coding_session/operation.rs`, after the existing `export_operation_declares_root_read_only_metadata` test:

```rust
#[test]
fn set_default_agent_profile_operation_declares_runtime_write_metadata() {
    let profile_id = ProfileId::new("agent-main").expect("valid profile id");
    let operation = Operation::SetDefaultAgentProfile { profile_id };

    assert_eq!(operation.kind(), OperationKind::SetDefaultAgentProfile);
    assert_eq!(operation.origin(), OperationOrigin::ClientRoot);
    assert_eq!(operation.class(), OperationClass::RuntimeWrite);
    assert_eq!(
        operation.metadata().dispatch_mode,
        OperationDispatchMode::SyncMutable
    );
}
```

Also add this busy-guard session test to the `#[cfg(test)] mod tests` block in `crates/pi-coding-agent/src/coding_session/mod.rs`, after the `run_sync_operation_export_uses_read_only_admission_while_root_busy` test:

```rust
#[tokio::test]
async fn set_default_agent_profile_rejects_while_operation_is_busy() {
    let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
        .await
        .unwrap();
    let _guard = session
        .operation_control
        .begin(OperationKind::Prompt)
        .unwrap();

    let error = session
        .set_default_agent_profile_id("agent-main")
        .unwrap_err();

    assert_eq!(error.code(), "busy");
    assert_eq!(
        session.operation_control.active(),
        Some(OperationKind::Prompt)
    );
}
```

- [x] **Step 2: Run the focused RED command**

Run:

```bash
cargo test -p pi-coding-agent set_default_agent_profile_operation_declares_runtime_write_metadata --lib
cargo test -p pi-coding-agent set_default_agent_profile_rejects_while_operation_is_busy --lib
```

Expected RED: compile failure because `Operation::SetDefaultAgentProfile`, `OperationKind::SetDefaultAgentProfile`, and the `ProfileId` import do not exist yet.

Actual RED: confirmed -- compile failure with E0433 (`ProfileId` not found in scope), E0599 (`Operation::SetDefaultAgentProfile` variant not found), E0599 (`OperationKind::SetDefaultAgentProfile` variant not found).

- [x] **Step 3: Add `OperationKind::SetDefaultAgentProfile`**

In `crates/pi-coding-agent/src/coding_session/operation_control.rs`, add the variants to the `OperationKind` enum before `SelfHealingEdit`. `ForkSession` gets `#[allow(dead_code)]` because it is not constructed until Task 2; Task 2 Step 3 removes the attribute:

```rust
pub(crate) enum OperationKind {
    Prompt,
    Compact,
    PluginCommand,
    PluginLoad,
    DelegationConfirmation,
    BranchSummary,
    AgentInvocation,
    AgentTeam,
    Export,
    #[allow(dead_code)]
    ForkSession,
    SetDefaultAgentProfile,
    #[allow(dead_code)]
    SelfHealingEdit,
}
```

Add the `as_str` arm in the same `impl OperationKind` block, before the `SelfHealingEdit` arm:

```rust
            Self::ForkSession => "fork_session",
            Self::SetDefaultAgentProfile => "set_default_agent_profile",
```

- [x] **Step 4: Add `Operation::SetDefaultAgentProfile` and its metadata**

In `crates/pi-coding-agent/src/coding_session/operation.rs`:

Add the import at the top of the file, after the existing `use super::operation_control::OperationKind;` line:

```rust
use super::profiles::ProfileId;
```

Add the variant to the `Operation` enum, before `Export`:

```rust
    SetDefaultAgentProfile {
        profile_id: ProfileId,
    },
```

Add the metadata arm in `Operation::metadata()`, before the `Self::Export` arm:

```rust
            Self::SetDefaultAgentProfile { .. } => OperationMetadata::new(
                Some(OperationKind::SetDefaultAgentProfile),
                OperationOrigin::ClientRoot,
                OperationClass::RuntimeWrite,
                OperationDispatchMode::SyncMutable,
            ),
```

Add the outcome variant to `OperationOutcome`, before `Export`:

```rust
    SetDefaultAgentProfile,
```

- [x] **Step 5: Route `set_default_agent_profile_id` through `run_sync_mut_operation`**

In `crates/pi-coding-agent/src/coding_session/mod.rs`, replace the body of `set_default_agent_profile_id` (currently at approximately line 432) with:

```rust
    pub fn set_default_agent_profile_id(
        &mut self,
        profile_id: impl Into<ProfileId>,
    ) -> Result<(), CodingSessionError> {
        let profile_id = profile_id.into();
        match self.run_sync_mut_operation(Operation::SetDefaultAgentProfile {
            profile_id,
        })? {
            OperationOutcome::SetDefaultAgentProfile => Ok(()),
            _ => unreachable!("set default agent profile operation returned wrong outcome"),
        }
    }
```

- [x] **Step 6: Add the `SetDefaultAgentProfile` handler to `run_sync_mut_operation`**

In `crates/pi-coding-agent/src/coding_session/mod.rs`, update the `run_sync_mut_operation` match. Add this arm before the `Operation::Export(_) | Operation::PluginCommand { .. }` fallthrough:

```rust
            Operation::SetDefaultAgentProfile { profile_id } => {
                match &mut self.persistence {
                    SessionPersistence::Persistent(session_service) => {
                        session_service.set_default_agent_profile_id(profile_id.clone())?;
                    }
                    SessionPersistence::NonPersistent(state) => {
                        state.default_agent_profile_id = profile_id.clone();
                    }
                }
                self.event_service
                    .emit_default_agent_profile_changed(profile_id);
                Ok(OperationOutcome::SetDefaultAgentProfile)
            }
```

- [x] **Step 7: Add `SetDefaultAgentProfile` to the other dispatchers' fallthrough lists**

In `run_sync_operation` (SyncReadOnly dispatcher), add `Operation::SetDefaultAgentProfile { .. }` to the final fallthrough `|`-list that currently ends with `Operation::AgentTeam(_) => Err(IntentRouter::unsupported_dispatch(&admission))`.

In `run_operation` (Async dispatcher), add `Operation::SetDefaultAgentProfile { .. }` to the fallthrough `|`-list that currently contains `Operation::Export(_) | Operation::PluginCommand { .. } | Operation::RejectDelegationConfirmation { .. }`.

- [x] **Step 8: Run the focused GREEN command**

Run:

```bash
cargo test -p pi-coding-agent set_default_agent_profile_operation_declares_runtime_write_metadata --lib
cargo test -p pi-coding-agent set_default_agent_profile_rejects_while_operation_is_busy --lib
cargo test -p pi-coding-agent operation --lib
cargo check -p pi-coding-agent
```

Expected GREEN: the new metadata and busy-guard tests pass, all existing operation tests pass, and `pi-coding-agent` compiles without warnings.

Actual GREEN: confirmed -- both new tests pass, all 57 operation tests pass, `cargo check -p pi-coding-agent` clean. Discovery: 13 exhaustive `OperationOutcome` match sites in `mod.rs` public API wrappers also needed `SetDefaultAgentProfile` arms (not enumerated in the plan's exhaustive-match section); all 13 were updated with `unreachable!` arms. `cargo fmt` fixed long-line and import-order formatting.

- [x] **Step 9: Run adapter behavior checks**

Run:

```bash
cargo test -p pi-coding-agent --test rpc_mode set_default_agent_profile
cargo test -p pi-coding-agent --test interactive_mode
cargo test -p pi-coding-agent query_intent --lib
```

Expected GREEN: RPC default-profile switching, interactive mode, and query-intent admission tests pass with unchanged behavior.

Actual GREEN: confirmed -- RPC `set_default_agent_profile` (3 tests), `interactive_mode` (41 tests), and `query_intent` (1 test) all pass. Full `pi-coding-agent` suite (525 lib + all integration tests) passes with 0 failures.

- [x] **Step 10: Update docs and commit**

Update `docs/TODO.md` North Star item to record that `set_default_agent_profile_id` now routes through `Operation::SetDefaultAgentProfile` (RuntimeWrite) admission via `run_sync_mut_operation`. Mark this task complete with actual RED/GREEN notes, then commit:

```bash
git add crates/pi-coding-agent/src/coding_session/operation_control.rs crates/pi-coding-agent/src/coding_session/operation.rs crates/pi-coding-agent/src/coding_session/mod.rs docs/TODO.md docs/superpowers/plans/2026-07-08-intent-router-session-mutation-admission-plan.md
git commit -m "feat: route default profile change through operation admission"
```

---

## Task 2: Add `Operation::ForkSession` (SessionWriteRoot) And Route Fork Through Direct Admission

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-08-intent-router-session-mutation-admission-plan.md`

- [x] **Step 1: Write the failing operation metadata and busy-guard tests**

Add this test to the `#[cfg(test)] mod tests` block in `crates/pi-coding-agent/src/coding_session/operation.rs`, after the `set_default_agent_profile_operation_declares_runtime_write_metadata` test:

```rust
#[test]
fn fork_session_operation_declares_root_session_write_metadata() {
    let operation = Operation::ForkSession {
        target_leaf_id: Some("leaf_1".into()),
    };

    assert_eq!(operation.kind(), OperationKind::ForkSession);
    assert_eq!(operation.origin(), OperationOrigin::ClientRoot);
    assert_eq!(operation.class(), OperationClass::SessionWriteRoot);
    assert_eq!(
        operation.metadata().dispatch_mode,
        OperationDispatchMode::SyncMutable
    );
}
```

Add this busy-guard test to the `#[cfg(test)] mod tests` block in `crates/pi-coding-agent/src/coding_session/mod.rs`, after the `set_default_agent_profile_rejects_while_operation_is_busy` test:

```rust
#[tokio::test]
async fn fork_current_session_rejects_while_operation_is_busy() {
    let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
        .await
        .unwrap();
    let _guard = session
        .operation_control
        .begin(OperationKind::Prompt)
        .unwrap();

    let error = session.fork_current_session(None).unwrap_err();

    assert_eq!(error.code(), "busy");
    assert_eq!(
        session.operation_control.active(),
        Some(OperationKind::Prompt)
    );
}
```

- [x] **Step 2: Run the focused RED command**

Run:

```bash
cargo test -p pi-coding-agent fork_session_operation_declares_root_session_write_metadata --lib
cargo test -p pi-coding-agent fork_current_session_rejects_while_operation_is_busy --lib
```

Expected RED: compile failure because `Operation::ForkSession` does not exist yet.

Actual RED: confirmed -- both tests fail to compile with `error[E0599]: no variant named \`ForkSession\` found for enum \`coding_session::operation::Operation\``.

- [x] **Step 3: Add `Operation::ForkSession` and its metadata**

In `crates/pi-coding-agent/src/coding_session/operation.rs`, add the variant to the `Operation` enum, before `SetDefaultAgentProfile`:

```rust
    ForkSession {
        target_leaf_id: Option<String>,
    },
```

Add the metadata arm in `Operation::metadata()`, before the `Self::SetDefaultAgentProfile` arm:

```rust
            Self::ForkSession { .. } => OperationMetadata::new(
                Some(OperationKind::ForkSession),
                OperationOrigin::ClientRoot,
                OperationClass::SessionWriteRoot,
                OperationDispatchMode::SyncMutable,
            ),
```

Do NOT add a `ForkSession` variant to `OperationOutcome` -- fork returns `Self` through direct admission, not through a dispatcher.

Remove the `#[allow(dead_code)]` attribute from `OperationKind::ForkSession` in `operation_control.rs` (added in Task 1 Step 3), since `Operation::ForkSession` metadata now constructs `OperationKind::ForkSession`.

- [x] **Step 4: Route `fork_current_session` through direct `IntentRouter` admission**

In `crates/pi-coding-agent/src/coding_session/mod.rs`, replace the body of `fork_current_session` (currently at approximately line 355) with:

```rust
    pub(crate) fn fork_current_session(
        &self,
        target_leaf_id: Option<&str>,
    ) -> Result<Self, CodingSessionError> {
        let operation = Operation::ForkSession {
            target_leaf_id: target_leaf_id.map(str::to_owned),
        };
        let admission = IntentRouter::static_admission(&operation)?;
        let _operation_permit = IntentRouter::admit_operation(
            &self.operation_control,
            &admission,
            OperationDispatchMode::SyncMutable,
        )?;

        match &self.persistence {
            SessionPersistence::Persistent(session_service) => Self::from_services(
                session_service.fork_current(target_leaf_id)?,
                self.default_plugin_load_options.clone(),
                self.profile_registry.clone(),
            ),
            SessionPersistence::NonPersistent(_) => Err(CodingSessionError::UnsupportedCapability {
                capability: "fork requires a persistent Rust-native session".into(),
            }),
        }
    }
```

Deviation from prescribed code: the `fork_current` call reads `target_leaf_id` from the destructured `Operation::ForkSession` field (via `let Operation::ForkSession { target_leaf_id } = operation` and `target_leaf_id.as_deref()`) rather than from the original `&str` parameter. This keeps the operation as the single source of truth and eliminates a `dead_code` warning on the `target_leaf_id` field that the prescribed code would otherwise produce.

- [x] **Step 5: Add `ForkSession` to all three dispatcher match arms**

`ForkSession` is admitted directly through `fork_current_session`, not through a dispatcher. All three dispatchers must return a clear error if `Operation::ForkSession` is ever passed to them.

In `run_sync_mut_operation`, add this arm before the `Operation::SetDefaultAgentProfile` handler:

```rust
            Operation::ForkSession { .. } => Err(CodingSessionError::UnsupportedCapability {
                capability: "fork session is admitted through fork_current_session".into(),
            }),
```

In `run_sync_operation`, add `Operation::ForkSession { .. }` to the final fallthrough `|`-list.

In `run_operation`, add `Operation::ForkSession { .. }` to the fallthrough `|`-list that contains `Operation::Export(_) | ...`.

- [x] **Step 6: Run the focused GREEN command**

Run:

```bash
cargo test -p pi-coding-agent fork_session_operation_declares_root_session_write_metadata --lib
cargo test -p pi-coding-agent fork_current_session_rejects_while_operation_is_busy --lib
cargo test -p pi-coding-agent operation --lib
cargo test -p pi-coding-agent intent_router --lib
cargo check -p pi-coding-agent
```

Expected GREEN: the new metadata and busy-guard tests pass, all existing operation and intent-router tests pass, and `pi-coding-agent` compiles without warnings.

Actual GREEN: confirmed -- `fork_session_operation_declares_root_session_write_metadata` (1), `fork_current_session_rejects_while_operation_is_busy` (1), `operation` (59), and `intent_router` (11) all pass. `cargo check -p pi-coding-agent` compiles with 0 warnings.

- [x] **Step 7: Run adapter behavior checks**

Run:

```bash
cargo test -p pi-coding-agent --test interactive_mode
cargo test -p pi-coding-agent --test rpc_mode
```

Expected GREEN: interactive mode (including tree-navigation fork) and RPC mode tests pass with unchanged behavior.

Actual GREEN: confirmed -- `interactive_mode` (41 tests) and `rpc_mode` (36 tests) all pass with unchanged behavior.

- [x] **Step 8: Update docs and commit**

Update `docs/TODO.md` North Star item to record that `fork_current_session` now routes through `Operation::ForkSession` (SessionWriteRoot) direct admission via `IntentRouter::admit_operation`. Mark this task complete with actual RED/GREEN notes, then commit:

```bash
git add crates/pi-coding-agent/src/coding_session/operation.rs crates/pi-coding-agent/src/coding_session/mod.rs docs/TODO.md docs/superpowers/plans/2026-07-08-intent-router-session-mutation-admission-plan.md
git commit -m "feat: route fork session through operation admission"
```

---

## Task 3: Add Source Guard And Close Stage 3 Admission Gap

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-08-intent-router-session-mutation-admission-plan.md`

- [ ] **Step 1: Write the failing source guard test**

Add this test to the `#[cfg(test)] mod tests` block in `crates/pi-coding-agent/src/coding_session/mod.rs`, after the `fork_current_session_rejects_while_operation_is_busy` test:

```rust
    #[test]
    fn session_mutation_facade_routes_through_intent_admission() {
        let source = include_str!("mod.rs");

        assert!(
            source.contains("run_sync_mut_operation(Operation::SetDefaultAgentProfile"),
            "set_default_agent_profile_id should route through run_sync_mut_operation"
        );
        assert!(
            source.contains("Operation::ForkSession"),
            "fork_current_session should construct a ForkSession operation"
        );
        // 3 dispatcher admit_operation calls + 1 fork_current_session direct call = 4
        assert!(
            source.matches("IntentRouter::admit_operation(").count() >= 4,
            "session mutation should admit through IntentRouter (dispatchers + fork)"
        );
    }
```

- [ ] **Step 2: Run the focused command**

Run:

```bash
cargo test -p pi-coding-agent session_mutation_facade_routes_through_intent_admission --lib
```

Expected: GREEN if Tasks 1 and 2 are correctly implemented. If RED, the implementation did not route the methods through admission correctly -- fix before proceeding.

- [ ] **Step 3: Run full workspace verification**

Run:

```bash
cargo fmt --check
cargo test -p pi-coding-agent operation --lib
cargo test -p pi-coding-agent intent_router --lib
cargo test -p pi-coding-agent query_intent --lib
cargo test -p pi-coding-agent read_only_admission --lib
cargo test -p pi-coding-agent session_mutation_facade_routes_through_intent_admission --lib
cargo test -p pi-coding-agent --test rpc_mode
cargo test -p pi-coding-agent --test interactive_mode
cargo test -p pi-coding-agent --test json_mode
cargo check --workspace
cargo test --workspace
```

Expected GREEN: all format, operation, intent-router, adapter, and full workspace checks pass.

- [ ] **Step 4: Update TODO and plan docs**

Update `docs/TODO.md`:

1. In the North Star `[~]` item, append: `Stage 3 admission is now complete: set_default_agent_profile_id routes through Operation::SetDefaultAgentProfile (RuntimeWrite) via run_sync_mut_operation, and fork_current_session routes through Operation::ForkSession (SessionWriteRoot) via direct IntentRouter::admit_operation. No UI/RPC path starts session-affecting work by directly calling SessionService for default-profile changes or session forking.`

2. Add a progress log entry dated today: `Stage 3 intent-router session-mutation admission completed. set_default_agent_profile_id and fork_current_session now route through IntentRouter admission; a source guard prevents regression.`

Mark this task complete with actual GREEN notes, then commit:

```bash
git add crates/pi-coding-agent/src/coding_session/mod.rs docs/TODO.md docs/superpowers/plans/2026-07-08-intent-router-session-mutation-admission-plan.md
git commit -m "test: guard session mutation admission boundary"
```

---

## Verification Summary

After all three tasks, the Stage 3 exit criteria from the reference architecture are satisfied for the session-mutation admission gap:

- **"no UI/RPC path starts session-affecting work by directly calling deep services"** -- `set_default_agent_profile_id` (RPC + interactive) and `fork_current_session` (interactive) now acquire an `IntentRouter` admission permit before touching `SessionService`. A source guard prevents regression.
- **"root vs child agent/team invocation is unambiguous"** -- already satisfied by Stage 1 operation routing.
- **"control commands retain priority under busy/backpressure conditions"** -- already satisfied by the prompt-control admission slice.

Remaining Stage 3 work that is explicitly deferred (not in this plan):
- `OperationClass::{NonSessionRoot, RuntimeWrite}` generation-aware scheduling (FutureOnly vs Interrupting runtime writes). The current exclusive guard is a safe conservative default; the spec allows blocking incompatible runtime writes while operations are active.
- `OperationClass::Child` parent-scope enforcement (delegated child operations). Delegation already runs through session-owned `AgentInvocationFlow`/`AgentTeamFlow` with parent operation correlation; formal Child-class scheduler enforcement is future work.
- `SwitchActiveLeaf` as a standalone `Operation` (currently only a `SessionService` internal, not exposed on `CodingAgentSession`). Not a current adapter path.
