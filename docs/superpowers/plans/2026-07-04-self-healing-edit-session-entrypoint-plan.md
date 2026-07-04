# Self-Healing Edit Session Entrypoint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Promote the internal `SelfHealingEditFlow` into a `CodingAgentSession`-owned workflow entrypoint with durable typed lifecycle events.

**Architecture:** Keep RPC and interactive adapters out of this slice. Expose a Rust API on `CodingAgentSession`, reuse the existing `SelfHealingEditFlow`, gate execution behind `OperationKind::SelfHealingEdit`, and persist non-leaf workflow events through the Rust-native session log.

**Tech Stack:** Rust 2024, `pi-coding-agent::api`, `CodingAgentSession`, `SelfHealingEditFlow`, `TurnTransaction`, serde session events, deterministic tempfile-backed tests.

---

## File Structure

- Modify `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs`: make replacement/outcome/diagnostic types product-facing while keeping operation injection crate-private.
- Modify `crates/pi-coding-agent/src/coding_session/mod.rs`: export self-healing edit types and add `CodingAgentSession::self_healing_edit()` plus internal transaction finalization.
- Modify `crates/pi-coding-agent/src/coding_session/session_log/event.rs`: add typed self-healing edit event variants and event-kind stability coverage.
- Modify `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`: add transaction record helpers and tests for event ordering.
- Modify `crates/pi-coding-agent/src/coding_session/session_service.rs`: add self-healing transaction begin/commit/fail helpers.
- Modify `crates/pi-coding-agent/src/lib.rs`: re-export public self-healing edit API types through `api`.
- Modify `crates/pi-coding-agent/tests/public_api.rs`: assert the public Rust API symbols are importable and callable.
- Modify `docs/TODO.md`: link this plan and record session-owned entrypoint progress.

## Task 1: Public API and Owner Entrypoint RED Tests

- [x] **Step 1: Add public API imports**

In `crates/pi-coding-agent/tests/public_api.rs`, extend the `pi_coding_agent::api` import list with:

```rust
SelfHealingEditDiagnostic, SelfHealingEditOutcome, SelfHealingEditReplacement,
```

- [x] **Step 2: Add persistent entrypoint test**

Add this async test to `crates/pi-coding-agent/tests/public_api.rs`:

```rust
#[tokio::test]
async fn coding_session_self_healing_edit_persists_typed_events() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let sessions = temp.path().join("sessions");
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::write(workspace.join("src/app.txt"), "one\ntwo\n").unwrap();
    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_session_id("sess_self_healing_entrypoint")
            .with_cwd(&workspace)
            .with_session_log_root(&sessions),
    )
    .await
    .unwrap();

    let outcome = session
        .self_healing_edit(
            "src/app.txt",
            vec![SelfHealingEditReplacement::new("two", "deux")],
        )
        .await
        .unwrap();

    assert_eq!(outcome.path, "src/app.txt");
    assert_eq!(outcome.attempts, 1);
    assert_eq!(outcome.first_changed_line, Some(2));
    assert!(outcome.message.contains("Successfully replaced 1 block"));
    assert_eq!(
        std::fs::read_to_string(workspace.join("src/app.txt")).unwrap(),
        "one\ndeux\n"
    );
    let event_log = std::fs::read_to_string(
        sessions
            .join("sess_self_healing_entrypoint")
            .join("events.jsonl"),
    )
    .unwrap();
    assert!(event_log.contains(r#""kind":"operation.started""#), "{event_log}");
    assert!(event_log.contains(r#""operation":"self_healing_edit""#), "{event_log}");
    assert!(event_log.contains(r#""kind":"self_healing_edit.started""#), "{event_log}");
    assert!(event_log.contains(r#""kind":"self_healing_edit.completed""#), "{event_log}");
    assert!(event_log.contains(r#""path":"src/app.txt""#), "{event_log}");
    assert!(event_log.contains(r#""attempts":1"#), "{event_log}");
    assert!(event_log.contains(r#""kind":"operation.committed""#), "{event_log}");
    assert_eq!(session.capabilities().self_healing_edit, CapabilityStatus::Available);
}
```

- [x] **Step 3: Add disabled non-persistent test**

Add this async test to `crates/pi-coding-agent/tests/public_api.rs`:

```rust
#[tokio::test]
async fn coding_session_self_healing_edit_requires_persistent_session() {
    let temp = tempfile::tempdir().unwrap();
    let mut session = CodingAgentSession::non_persistent(
        CodingAgentSessionOptions::new().with_cwd(temp.path()),
    )
    .await
    .unwrap();

    let error = session
        .self_healing_edit(
            "src/app.txt",
            vec![SelfHealingEditReplacement::new("two", "deux")],
        )
        .await
        .unwrap_err();

    assert_eq!(error.code(), "unsupported_capability");
    assert!(
        error
            .to_string()
            .contains("self-healing edit requires a persistent Rust-native session"),
        "{error}"
    );
}
```

- [x] **Step 4: Add failed edit persistence test**

Add this async test to `crates/pi-coding-agent/tests/public_api.rs`:

```rust
#[tokio::test]
async fn coding_session_self_healing_edit_failure_records_failed_operation() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let sessions = temp.path().join("sessions");
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::write(workspace.join("src/app.txt"), "one\ntwo\n").unwrap();
    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_session_id("sess_self_healing_failure")
            .with_cwd(&workspace)
            .with_session_log_root(&sessions),
    )
    .await
    .unwrap();

    let error = session
        .self_healing_edit(
            "src/app.txt",
            vec![SelfHealingEditReplacement::new("", "deux")],
        )
        .await
        .unwrap_err();

    assert_eq!(
        std::fs::read_to_string(workspace.join("src/app.txt")).unwrap(),
        "one\ntwo\n"
    );
    assert!(error.to_string().contains("oldText must not be empty"), "{error}");
    let event_log = std::fs::read_to_string(
        sessions.join("sess_self_healing_failure").join("events.jsonl"),
    )
    .unwrap();
    assert!(event_log.contains(r#""operation":"self_healing_edit""#), "{event_log}");
    assert!(event_log.contains(r#""kind":"self_healing_edit.started""#), "{event_log}");
    assert!(event_log.contains(r#""kind":"operation.failed""#), "{event_log}");
    assert!(!event_log.contains(r#""kind":"self_healing_edit.completed""#), "{event_log}");
}
```

- [x] **Step 5: Run RED public API tests**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent coding_session_self_healing_edit --test public_api -- --nocapture
```

Expected: compilation fails because the public self-healing edit types and `CodingAgentSession::self_healing_edit` are not exported yet.

## Task 2: Session Event and Transaction RED Tests

- [x] **Step 1: Add stable kind cases**

In `crates/pi-coding-agent/src/coding_session/session_log/event.rs`, add `SessionEventData::SelfHealingEditStarted` and `SessionEventData::SelfHealingEditCompleted` cases to `session_event_data_variants_keep_stable_kind_names()` expecting:

```rust
"self_healing_edit.started"
"self_healing_edit.completed"
```

- [x] **Step 2: Add transaction event-order test**

In `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`, add a test that begins an `OperationKind::SelfHealingEdit`, records started/completed events, commits with `None`, and expects this order:

```rust
vec![
    "operation.started",
    "turn.started",
    "self_healing_edit.started",
    "self_healing_edit.completed",
    "operation.committed",
]
```

- [x] **Step 3: Run RED compile check**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --no-run
```

Expected: compilation fails because the new session event variants, transaction helpers, and session-log `OperationKind::SelfHealingEdit` do not exist yet.

## Task 3: Implement Product Types and Public Entrypoint

- [x] **Step 1: Export self-healing edit types**

In `crates/pi-coding-agent/src/coding_session/mod.rs`, add:

```rust
pub use self_healing_edit_flow::{
    SelfHealingEditDiagnostic, SelfHealingEditOutcome, SelfHealingEditReplacement,
};
```

In `crates/pi-coding-agent/src/lib.rs`, add those three types to the `api` facade export list.

- [x] **Step 2: Make flow result types public**

In `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs`, change these from `pub(crate)` to `pub` where they are product API surface:

```rust
pub struct SelfHealingEditReplacement {
    pub old_text: String,
    pub new_text: String,
}

impl SelfHealingEditReplacement {
    pub fn new(old_text: impl Into<String>, new_text: impl Into<String>) -> Self {
        Self {
            old_text: old_text.into(),
            new_text: new_text.into(),
        }
    }
}

pub struct SelfHealingEditDiagnostic {
    pub message: String,
}

pub struct SelfHealingEditOutcome {
    pub path: String,
    pub message: String,
    pub diff: String,
    pub patch: String,
    pub first_changed_line: Option<usize>,
    pub attempts: usize,
    pub diagnostics: Vec<SelfHealingEditDiagnostic>,
}
```

Keep `SelfHealingEditOptions`, `SelfHealingEditContext`, operation injection, and `FlowService` entrypoints crate-private.

- [x] **Step 3: Add owner method**

In `impl CodingAgentSession`, add near the other workflow methods:

```rust
pub async fn self_healing_edit(
    &mut self,
    path: impl Into<String>,
    replacements: Vec<SelfHealingEditReplacement>,
) -> Result<SelfHealingEditOutcome, CodingSessionError> {
    let _operation = self.operation_control.begin(OperationKind::SelfHealingEdit)?;
    self.self_healing_edit_inner(path.into(), replacements).await
}
```

Add `self_healing_edit_inner()` near `compact_inner()`:

```rust
async fn self_healing_edit_inner(
    &mut self,
    path: String,
    replacements: Vec<SelfHealingEditReplacement>,
) -> Result<SelfHealingEditOutcome, CodingSessionError> {
    let SessionPersistence::Persistent(session_service) = &mut self.persistence else {
        return Err(CodingSessionError::UnsupportedCapability {
            capability: "self-healing edit requires a persistent Rust-native session".into(),
        });
    };
    let cwd = session_cwd(session_service).unwrap_or_else(default_cwd);
    let mut transaction = session_service.begin_self_healing_edit_transaction();
    let operation_id = transaction.operation_id().to_owned();
    SessionService::record_self_healing_edit_started(
        &mut transaction,
        path.clone(),
        replacements.len(),
    )?;
    let mut context = SelfHealingEditContext::new(
        SelfHealingEditOptions::new(cwd, path, replacements),
    );

    match self.flow_service.run_self_healing_edit(&mut context).await {
        Ok(outcome) => {
            SessionService::record_self_healing_edit_completed(&mut transaction, &outcome)?;
            let finalized = session_service.commit_self_healing_edit_transaction(
                Some(transaction),
                operation_id,
            )?;
            self.emit_session_write_events(&finalized);
            Ok(outcome)
        }
        Err(error) => {
            let finalized = session_service.fail_self_healing_edit_transaction(
                Some(transaction),
                operation_id,
                error.code(),
                error.to_string(),
            )?;
            self.emit_session_write_events(&finalized);
            Err(error)
        }
    }
}
```

## Task 4: Implement Typed Session Events

- [x] **Step 1: Add event variants**

In `SessionEventData`, add:

```rust
#[serde(rename = "self_healing_edit.started")]
SelfHealingEditStarted {
    path: String,
    replacements: usize,
},
#[serde(rename = "self_healing_edit.completed")]
SelfHealingEditCompleted {
    path: String,
    message: String,
    diff: String,
    patch: String,
    first_changed_line: Option<usize>,
    attempts: usize,
    diagnostics: Vec<String>,
},
```

- [x] **Step 2: Add session-log operation kind**

In `crates/pi-coding-agent/src/coding_session/session_log/event.rs`, add `SelfHealingEdit` to the internal persisted `OperationKind` enum. With the existing serde `snake_case` rule it serializes as `self_healing_edit`.

- [x] **Step 3: Add transaction helpers**

In `TurnTransaction`, add:

```rust
pub(crate) fn record_self_healing_edit_started(
    &mut self,
    path: impl Into<String>,
    replacements: usize,
) -> Result<(), CodingSessionError> {
    self.ensure_open()?;
    self.push_event(SessionEventData::SelfHealingEditStarted {
        path: path.into(),
        replacements,
    });
    Ok(())
}

pub(crate) fn record_self_healing_edit_completed(
    &mut self,
    outcome: &SelfHealingEditOutcome,
) -> Result<(), CodingSessionError> {
    self.ensure_open()?;
    self.push_event(SessionEventData::SelfHealingEditCompleted {
        path: outcome.path.clone(),
        message: outcome.message.clone(),
        diff: outcome.diff.clone(),
        patch: outcome.patch.clone(),
        first_changed_line: outcome.first_changed_line,
        attempts: outcome.attempts,
        diagnostics: outcome
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect(),
    });
    Ok(())
}
```

- [x] **Step 4: Add session service helpers**

In `SessionService`, add:

```rust
pub(crate) fn begin_self_healing_edit_transaction(&self) -> PromptTurnTransaction {
    TurnTransaction::begin(
        &self.store,
        self.handle.clone(),
        SystemIdGenerator,
        SystemClock,
        OperationKind::SelfHealingEdit,
    )
}

pub(crate) fn commit_self_healing_edit_transaction(
    &mut self,
    transaction: Option<PromptTurnTransaction>,
    operation_id: impl Into<String>,
) -> Result<FinalizedSessionWrite, CodingSessionError> {
    self.commit_non_leaf_transaction(
        transaction,
        operation_id,
        "no active self-healing edit transaction",
    )
}

pub(crate) fn fail_self_healing_edit_transaction(
    &mut self,
    transaction: Option<PromptTurnTransaction>,
    operation_id: impl Into<String>,
    error_code: impl Into<String>,
    message: impl Into<String>,
) -> Result<FinalizedSessionWrite, CodingSessionError> {
    self.fail_non_leaf_transaction(
        transaction,
        operation_id,
        error_code,
        message,
        "no active self-healing edit transaction",
    )
}

pub(crate) fn record_self_healing_edit_started(
    transaction: &mut PromptTurnTransaction,
    path: String,
    replacements: usize,
) -> Result<(), CodingSessionError> {
    transaction.record_self_healing_edit_started(path, replacements)
}

pub(crate) fn record_self_healing_edit_completed(
    transaction: &mut PromptTurnTransaction,
    outcome: &SelfHealingEditOutcome,
) -> Result<(), CodingSessionError> {
    transaction.record_self_healing_edit_completed(outcome)
}
```

Commit/fail should use the existing non-leaf transaction helpers so self-healing edits update the session manifest timestamp without creating a transcript leaf.

## Task 5: Wire Owner Finalization

- [x] **Step 1: On success**

`self_healing_edit_inner()` should:

1. begin a self-healing transaction;
2. record `self_healing_edit.started` before the flow runs;
3. run `self.flow_service.run_self_healing_edit(&mut context).await`;
4. record `self_healing_edit.completed` with the outcome;
5. commit the non-leaf transaction;
6. emit `SessionWritePending` and `SessionWriteCommitted` through existing helpers;
7. return the `SelfHealingEditOutcome`.

- [x] **Step 2: On failure**

If the flow fails after the transaction starts, call the self-healing fail helper with `error.code()` and `error.to_string()`, emit finalized session write events, then return the original error. The file should remain unchanged for validation failures.

- [x] **Step 3: On non-persistent session**

Return `CodingSessionError::UnsupportedCapability` with the exact message `self-healing edit requires a persistent Rust-native session`.

## Task 6: Verify and Update TODO

- [x] **Step 1: Run focused tests**

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent coding_session_self_healing_edit --test public_api -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent session_event_data_variants_keep_stable_kind_names -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent self_healing_edit_transaction_records_lifecycle_events -- --nocapture
```

Expected: all focused tests pass.

- [x] **Step 2: Update `docs/TODO.md`**

Add this plan to Source Documents and update the Phase 6 self-healing edit notes to say the session-owned Rust API entrypoint and typed event log lifecycle are in place.

- [x] **Step 3: Run final verification**

```bash
/home/whai/.cargo/bin/cargo fmt --check
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet
/home/whai/.cargo/bin/cargo check --workspace --quiet
/home/whai/.cargo/bin/cargo test --workspace --quiet
git diff --check
```

Expected: all commands exit 0.

## Commit Boundaries

Do not commit unless explicitly requested. If commits are requested later, use a single logical commit for this slice because API, event schema, owner method, tests, and TODO update are one behavior boundary:

```bash
git add crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs \
  crates/pi-coding-agent/src/coding_session/mod.rs \
  crates/pi-coding-agent/src/coding_session/session_log/event.rs \
  crates/pi-coding-agent/src/coding_session/session_log/transaction.rs \
  crates/pi-coding-agent/src/coding_session/session_service.rs \
  crates/pi-coding-agent/src/lib.rs \
  crates/pi-coding-agent/tests/public_api.rs \
  docs/TODO.md \
  docs/superpowers/plans/2026-07-04-self-healing-edit-session-entrypoint-plan.md
git commit -m "feat(coding-agent): add self-healing edit session entrypoint"
```
