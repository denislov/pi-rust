# Self-Healing Edit Repair Observability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist self-healing edit repair attempts as typed Rust-native session events and expose them on the product outcome.

**Architecture:** Keep `SelfHealingEditFlow` free of direct session-log writes. The flow records structured repair-attempt metadata in `SelfHealingEditOutcome`; `CodingAgentSession` owner records each attempt through `TurnTransaction` before the final completed or failed marker. The event stores product-level fields only: path, attempt number, proposed replacements, diagnostics, and post-attempt check output; it does not persist model prompts, runtime snapshots, provider internals, or API keys.

**Tech Stack:** Rust 2024, `SelfHealingEditFlow`, `SelfHealingEditOutcome`, `SessionEventData`, `TurnTransaction`, deterministic public API and flow tests.

---

### Task 1: Add RED Flow And Public API Coverage

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/flow_service.rs`
- Modify: `crates/pi-coding-agent/tests/public_api.rs`

- [x] **Step 1: Assert flow outcome repair attempt metadata**

In `self_healing_edit_flow_repairs_after_failed_check`, after `assert_eq!(outcome.attempts, 2);`, add:

```rust
assert_eq!(outcome.repair_attempts.len(), 1);
let repair = &outcome.repair_attempts[0];
assert_eq!(repair.attempt, 1);
assert_eq!(repair.replacements.len(), 1);
assert_eq!(repair.replacements[0].old_text, "deux");
assert_eq!(repair.replacements[0].new_text, "dos");
assert_eq!(repair.check_output.as_ref().unwrap().exit_code, 0);
assert!(
    repair
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("compile error")),
    "{:#?}",
    repair.diagnostics
);
```

- [x] **Step 2: Assert public model repair event log**

In `coding_session_self_healing_edit_uses_model_repair_strategy`, after final check assertions, read the event log and assert the typed repair event shape:

```rust
let event_log = std::fs::read_to_string(
    sessions
        .join("sess_self_healing_model_repair")
        .join("events.jsonl"),
)
.unwrap();
assert!(
    event_log.contains(r#""kind":"self_healing_edit.repair_attempted""#),
    "{event_log}"
);
assert!(event_log.contains(r#""attempt":1"#), "{event_log}");
assert!(event_log.contains(r#""old_text":"deux""#), "{event_log}");
assert!(event_log.contains(r#""new_text":"dos""#), "{event_log}");
assert!(event_log.contains(r#""exit_code":0"#), "{event_log}");
```

Also assert the public outcome carries the same repair metadata:

```rust
assert_eq!(outcome.repair_attempts.len(), 1);
assert_eq!(outcome.repair_attempts[0].attempt, 1);
```

- [x] **Step 3: Run RED tests**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent self_healing_edit_flow_repairs_after_failed_check --lib -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test public_api coding_session_self_healing_edit_uses_model_repair_strategy -- --nocapture
```

Expected: compile failure because `SelfHealingEditOutcome::repair_attempts` and the repair-attempt event do not exist yet.

### Task 2: Add Product Repair Attempt Types

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/src/lib.rs`

- [x] **Step 1: Add public repair attempt struct**

Add near `SelfHealingEditCheckOutput`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfHealingEditRepairAttempt {
    pub attempt: usize,
    pub replacements: Vec<SelfHealingEditReplacement>,
    pub diagnostics: Vec<SelfHealingEditDiagnostic>,
    pub check_output: Option<SelfHealingEditCheckOutput>,
}
```

- [x] **Step 2: Extend outcome**

Add to `SelfHealingEditOutcome`:

```rust
pub repair_attempts: Vec<SelfHealingEditRepairAttempt>,
```

Update all `SelfHealingEditOutcome` literals to include `repair_attempts: Vec::new()` until real attempts are recorded.

- [x] **Step 3: Export the public symbol**

Re-export `SelfHealingEditRepairAttempt` from `coding_session` and the `api` facade.

### Task 3: Capture Repair Attempts In Flow

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs`

- [x] **Step 1: Add context storage**

Add `repair_attempt_records: Vec<SelfHealingEditRepairAttempt>` to `SelfHealingEditContext`, initialize it to `Vec::new()`, reset it in `start_edit_workflow`, and expose:

```rust
pub(crate) fn repair_attempts(&self) -> &[SelfHealingEditRepairAttempt] {
    &self.repair_attempt_records
}
```

- [x] **Step 2: Record each applied repair attempt**

In `repair_patch`, clone the strategy replacements before applying them, then push a `SelfHealingEditRepairAttempt` after the post-repair `run_check()`:

```rust
let applied_replacements = replacements.clone();
self.options.replacements = replacements;
self.proposal_ready = true;
self.validate_patch()?;
self.apply_patch().await?;
self.run_check().await?;
self.repair_attempt_records.push(SelfHealingEditRepairAttempt {
    attempt: self.repair_attempts,
    replacements: applied_replacements,
    diagnostics: self.diagnostics.clone(),
    check_output: self.check_output.clone(),
});
```

- [x] **Step 3: Include attempts in outcome**

In `record_result`, set:

```rust
repair_attempts: self.repair_attempt_records.clone(),
```

### Task 4: Persist Repair Attempt Events

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/event.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/session_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`

- [x] **Step 1: Add persisted replacement type and event variant**

Add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PersistedSelfHealingEditReplacement {
    pub(crate) old_text: String,
    pub(crate) new_text: String,
}
```

Add `SessionEventData` variant:

```rust
#[serde(rename = "self_healing_edit.repair_attempted")]
SelfHealingEditRepairAttempted {
    path: String,
    attempt: usize,
    replacements: Vec<PersistedSelfHealingEditReplacement>,
    diagnostics: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    check_output: Option<PersistedSelfHealingEditCheckOutput>,
},
```

- [x] **Step 2: Update stable-kind tests and transaction event kind helper**

Add `self_healing_edit.repair_attempted` to `session_event_data_variants_keep_stable_kind_names` and `event_kinds` in transaction tests.

- [x] **Step 3: Add transaction recorder**

Add `TurnTransaction::record_self_healing_edit_repair_attempted(path, repair)` that maps replacements, diagnostics, and check output into the persisted event.

- [x] **Step 4: Add SessionService wrapper**

Add `SessionService::record_self_healing_edit_repair_attempted(transaction, path, repair)` and delegate to the transaction recorder.

- [x] **Step 5: Record attempts before finalization**

In `CodingAgentSession::self_healing_edit_inner`, after the flow returns success and before `record_self_healing_edit_completed`, iterate `outcome.repair_attempts`. In the error path, iterate `context.repair_attempts()` before failing the transaction:

```rust
for repair in outcome.repair_attempts.iter() {
    SessionService::record_self_healing_edit_repair_attempted(
        &mut transaction,
        &outcome.path,
        repair,
    )?;
}
```

For errors, use the original request path retained before moving into options or read it from context if exposed.

### Task 5: Verify And Update Docs

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-04-self-healing-edit-repair-observability-plan.md`

- [x] **Step 1: Run focused GREEN tests**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent self_healing_edit_flow_repairs_after_failed_check --lib -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent session_event_data_variants_keep_stable_kind_names self_healing_edit_transaction_records_lifecycle_events --lib -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test public_api coding_session_self_healing_edit_uses_model_repair_strategy -- --nocapture
```

Expected: all pass.

- [x] **Step 2: Update TODO**

Add this plan to Source Documents and update the workflow session event integration note to say self-healing edit repair attempts persist as typed event-log entries.

- [x] **Step 3: Run verification**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt --check
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test public_api --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet
/home/whai/.cargo/bin/cargo check --workspace --quiet
/home/whai/.cargo/bin/cargo test --workspace --quiet
git diff --check
```

Expected: all commands exit 0.
