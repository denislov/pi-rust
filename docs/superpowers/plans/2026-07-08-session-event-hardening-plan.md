# Session Event Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Harden the Rust-native `SessionEvent` log so replay can distinguish committed, failed, aborted, recovered, and in-doubt operation history while old Rust-native logs stay readable.

**Architecture:** Keep `SessionEvent` as the only durable session fact source. Add durable sequencing at the `SessionLogStore` append/read boundary first, then layer recovery classification and explicit recovery markers on top without rewriting historical logs by default.

**Tech Stack:** Rust 2024, `serde`/`serde_json` JSONL event logs, `pi-coding-agent` session log/store/replay services, deterministic tempfile-backed tests.

---

## File Structure

- Modify: `crates/pi-coding-agent/src/coding_session/session_log/event.rs`
  - Adds the optional migration-era `session_sequence` field and helper method on `SessionEventEnvelope`.
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/store.rs`
  - Assigns durable session sequences when appending, synthesizes compatibility sequences while reading old logs, and validates session ownership.
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/replay.rs`
  - Adds operation recovery classification without changing transcript folding behavior for already-terminal operations.
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`
  - Keeps transaction terminal-marker behavior explicit and later records recovered/in-doubt markers through the same transaction boundary.
- Modify: `crates/pi-coding-agent/src/coding_session/session_service.rs`
  - Runs recovery scan on open/replay boundaries and exposes recovered/in-doubt state to session-owned projections.
- Modify: `docs/TODO.md`
  - Tracks Stage 4 entry and each durable/recovery hardening slice.

## Task 1: Durable Session Sequence Compatibility

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/event.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/store.rs`
- Modify: `docs/TODO.md`

- [x] **Step 1: Write failing store tests**

Add tests to `crates/pi-coding-agent/src/coding_session/session_log/store.rs`:

```rust
#[test]
fn append_events_assigns_contiguous_session_sequences() {
    let temp = tempfile::tempdir().unwrap();
    let store = SessionLogStore::new(temp.path());
    let handle = store
        .create_session(create_options("sess_sequence"))
        .unwrap();
    let events = vec![
        event(
            "sess_sequence",
            "evt_1",
            SessionEventData::SessionCreated { cwd: None },
        ),
        event("sess_sequence", "evt_2", SessionEventData::TurnStarted {}),
    ];

    store.append_events(&handle, &events).unwrap();

    let decoded = store.read_events(&handle).unwrap();
    assert_eq!(
        decoded
            .iter()
            .map(|event| event.session_sequence)
            .collect::<Vec<_>>(),
        vec![Some(1), Some(2)]
    );

    let raw = fs::read_to_string(handle.event_log_path().unwrap()).unwrap();
    let raw_sequences = raw
        .lines()
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).unwrap()["session_sequence"]
                .as_u64()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(raw_sequences, vec![1, 2]);
}

#[test]
fn read_events_synthesizes_sequences_for_legacy_logs() {
    let temp = tempfile::tempdir().unwrap();
    let store = SessionLogStore::new(temp.path());
    let handle = store
        .create_session(create_options("sess_legacy_sequence"))
        .unwrap();
    let legacy_events = vec![
        event(
            "sess_legacy_sequence",
            "evt_legacy_1",
            SessionEventData::SessionCreated { cwd: None },
        ),
        event(
            "sess_legacy_sequence",
            "evt_legacy_2",
            SessionEventData::TurnStarted {},
        ),
    ];
    let raw = legacy_events
        .iter()
        .map(|event| serde_json::to_string(event).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(handle.event_log_path().unwrap(), format!("{raw}\n")).unwrap();

    let decoded = store.read_events(&handle).unwrap();

    assert_eq!(
        decoded
            .iter()
            .map(|event| event.session_sequence)
            .collect::<Vec<_>>(),
        vec![Some(1), Some(2)]
    );
}
```

- [x] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent append_events_assigns_contiguous_session_sequences --lib
cargo test -p pi-coding-agent read_events_synthesizes_sequences_for_legacy_logs --lib
```

Expected: fail because `SessionEventEnvelope` has no `session_sequence` field yet.

- [x] **Step 3: Add migration-era sequence field**

In `SessionEventEnvelope`, insert the field after `session_id`:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub session_sequence: Option<u64>,
```

Initialize it in `SessionEventEnvelope::new()`:

```rust
session_sequence: None,
```

Add helper:

```rust
pub(crate) fn with_session_sequence(mut self, sequence: u64) -> Self {
    self.session_sequence = Some(sequence);
    self
}
```

- [x] **Step 4: Assign and synthesize sequences in the store**

Change `append_events()` so it calls a helper to find the next sequence, clones each event, overwrites `session_sequence`, and writes the sequenced copy. Change `read_events()` so events missing `session_sequence` get `Some(line_number)` in memory.

Add helper functions:

```rust
fn next_session_sequence(
    event_log_path: &Path,
    session_id: &str,
) -> Result<u64, CodingSessionError> {
    let content = fs::read_to_string(event_log_path).map_err(|error| {
        session_error(format!(
            "failed to read session event log {}: {error}",
            event_log_path.display()
        ))
    })?;
    let mut compatibility_sequence = 0_u64;
    let mut last_sequence = 0_u64;
    for (index, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        compatibility_sequence += 1;
        let event: SessionEventEnvelope = serde_json::from_str(line).map_err(|error| {
            session_error(format!(
                "failed to parse session event at line {} in {}: {error}",
                index + 1,
                event_log_path.display()
            ))
        })?;
        validate_event_for_session(&event, session_id)?;
        last_sequence = last_sequence.max(event.session_sequence.unwrap_or(compatibility_sequence));
    }
    Ok(last_sequence + 1)
}
```

- [x] **Step 5: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent append_events_assigns_contiguous_session_sequences --lib
cargo test -p pi-coding-agent read_events_synthesizes_sequences_for_legacy_logs --lib
cargo test -p pi-coding-agent session_log::store --lib
```

Expected: all selected tests pass.

- [x] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/session_log/event.rs crates/pi-coding-agent/src/coding_session/session_log/store.rs docs/TODO.md docs/superpowers/plans/2026-07-08-session-event-hardening-plan.md
git commit -m "feat: add durable session event sequences"
```

## Task 2: Operation Recovery Classification

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/replay.rs`
- Modify: `docs/TODO.md`

- [x] **Step 1: Write failing replay classification tests**

Add tests that build event arrays with `OperationStarted` and each terminal marker:

```rust
#[test]
fn replay_classifies_terminal_operation_statuses() {
    let events = vec![
        op_event(
            "evt_1",
            SessionEventData::OperationStarted {
                operation: OperationKind::Prompt,
            },
        ),
        op_event(
            "evt_2",
            SessionEventData::OperationCommitted { new_leaf_id: None },
        ),
        event(
            "evt_3",
            Some("op_failed"),
            Some("turn_failed"),
            SessionEventData::OperationStarted {
                operation: OperationKind::Prompt,
            },
        ),
        event(
            "evt_4",
            Some("op_failed"),
            Some("turn_failed"),
            SessionEventData::OperationFailed {
                error_code: "provider".into(),
                message: "provider failed".into(),
            },
        ),
        event(
            "evt_5",
            Some("op_aborted"),
            Some("turn_aborted"),
            SessionEventData::OperationStarted {
                operation: OperationKind::Prompt,
            },
        ),
        event(
            "evt_6",
            Some("op_aborted"),
            Some("turn_aborted"),
            SessionEventData::OperationAborted {
                reason: "user abort".into(),
            },
        ),
    ];

    let replay = fold_events(&events);

    assert_eq!(replay.operation_status("op_1"), Some(OperationReplayStatus::Committed));
    assert_eq!(replay.operation_status("op_failed"), Some(OperationReplayStatus::Failed));
    assert_eq!(replay.operation_status("op_aborted"), Some(OperationReplayStatus::Aborted));
}

#[test]
fn replay_marks_started_operation_without_terminal_marker_in_doubt() {
    let events = vec![op_event(
        "evt_1",
        SessionEventData::OperationStarted {
            operation: OperationKind::Prompt,
        },
    )];

    let replay = fold_events(&events);

    assert_eq!(replay.operation_status("op_1"), Some(OperationReplayStatus::InDoubt));
}
```

- [x] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent replay_classifies_terminal_operation_statuses --lib
cargo test -p pi-coding-agent replay_marks_started_operation_without_terminal_marker_in_doubt --lib
```

Expected: fail because `OperationReplayStatus` and `SessionReplay::operation_status()` do not exist.

Actual RED: confirmed -- 8 compile errors: `OperationReplayStatus` undeclared type, `operation_status` method not found on `SessionReplay`.

- [x] **Step 3: Add replay status model**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationReplayStatus {
    Committed,
    Failed,
    Aborted,
    Recovered,
    InDoubt,
}
```

Store operation statuses in `SessionReplay` and update `fold_events()` when it sees operation lifecycle events.

- [x] **Step 4: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent replay_classifies_terminal_operation_statuses --lib
cargo test -p pi-coding-agent replay_marks_started_operation_without_terminal_marker_in_doubt --lib
cargo test -p pi-coding-agent session_log::replay --lib
```

Expected: all selected tests pass.

Actual GREEN: confirmed -- both new tests pass, all 16 `session_log::replay` tests pass. Discovery: 9 existing `SessionReplay` construction sites across 5 files (`branch_summary_flow.rs`, `prompt_flow.rs`, `prompt.rs`, `runtime_service.rs`, `flow_service.rs`) needed `operation_statuses: Default::default()` added. Full verification: `cargo fmt --check` clean, `session_log` 44 passed, `session_service` 21 passed, `event_service` 17 passed, `protocol_events` 1 passed, `cargo check -p pi-coding-agent` clean, `git diff --check` clean.

- [x] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/session_log/replay.rs docs/TODO.md
git commit -m "feat: classify session operation recovery state"
```

## Task 3: Recovery Marker Event

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/event.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/replay.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/prompt_flow.rs`
- Modify: `docs/TODO.md`

- [x] **Step 1: Write failing serialization and replay tests**

Add a `SessionEventData::OperationRecovered` serialization test and a replay test where `OperationRecovered` changes an in-doubt operation to `Recovered`.

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent operation_recovered_event_round_trips --lib
cargo test -p pi-coding-agent replay_classifies_recovered_operation --lib
```

Expected: fail because the event variant does not exist.

Actual RED: confirmed -- 3 compile errors: `OperationRecovered` variant not found in `SessionEventData`. Tests reference the variant in `event.rs` (kind-name case + round-trip) and `replay.rs` (classification test).

- [x] **Step 3: Add event variant**

Add to `SessionEventData`:

```rust
#[serde(rename = "operation.recovered")]
OperationRecovered {
    reason: String,
    recovery_id: String,
},
```

Update replay classification so this marker maps to `OperationReplayStatus::Recovered`.

- [x] **Step 4: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent operation_recovered_event_round_trips --lib
cargo test -p pi-coding-agent replay_classifies_recovered_operation --lib
cargo test -p pi-coding-agent session_log --lib
```

Expected: selected session log tests pass.

Actual GREEN: confirmed -- `operation_recovered_event_round_trips` (1), `replay_classifies_recovered_operation` (1), `session_log::replay` (17), and `session_log` (46) all pass. Discovery: two additional exhaustive `SessionEventData` match sites needed `OperationRecovered` arms -- `transaction.rs::event_kinds()` and `prompt_flow.rs::event_kinds()` (test helper). `OperationRecovered` was also added to `finalized_operation_ids()` so recovered operations are treated as terminal (payload retained, not omitted as incomplete). Full verification: `session_service` 21 passed, `event_service` 17 passed, `protocol_events` 1 passed, `cargo check -p pi-coding-agent` clean, `cargo fmt --check` clean.

- [x] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/session_log/event.rs crates/pi-coding-agent/src/coding_session/session_log/replay.rs crates/pi-coding-agent/src/coding_session/session_log/transaction.rs crates/pi-coding-agent/src/coding_session/prompt_flow.rs docs/TODO.md
git commit -m "feat: add session operation recovery markers"
```

## Task 4: Session Open Recovery Scan

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/session_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/replay.rs`
- Modify: `docs/TODO.md`

- [x] **Step 1: Write failing service-boundary tests**

Add a test that opens a session log containing `OperationStarted` without a terminal marker and verifies the service exposes the operation as in-doubt instead of silently treating it as normal committed history.

- [x] **Step 2: Run RED test**

Run:

```bash
cargo test -p pi-coding-agent open_reports_in_doubt_operations --lib
```

Expected: fail because `SessionService` does not expose recovery scan state yet.

Actual RED: confirmed -- `error[E0599]: no method named 'recovery_summary' found for struct 'session_service::SessionService'`. Test was named `open_reports_in_doubt_operations` (slightly shorter than the plan's `open_session_reports_in_doubt_operations`).

- [x] **Step 3: Add recovery scan view**

Add a lightweight session-owned recovery summary:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionRecoverySummary {
    pub(crate) in_doubt_operations: Vec<String>,
}
```

Derive it from `SessionReplay` operation statuses at `SessionService::open()` and keep it read-only.

- [x] **Step 4: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent open_reports_in_doubt_operations --lib
cargo test -p pi-coding-agent session_service --lib
```

Expected: selected service tests pass.

Actual GREEN: confirmed -- `open_reports_in_doubt_operations` (1), `session_service` (22), `session_log` (46) all pass. `SessionRecoverySummary` is derived from `SessionReplay::operation_statuses` via `SessionReplay::recovery_summary()` and exposed read-only through `SessionService::recovery_summary()`. `cargo check -p pi-coding-agent` clean, `cargo fmt --check` clean.

- [x] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/session_service.rs crates/pi-coding-agent/src/coding_session/session_log/replay.rs docs/TODO.md
git commit -m "feat: expose session recovery scan state"
```

## Task 5: Partial Commit Uncertainty Guard

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/session_service.rs`
- Modify: `docs/TODO.md`

- [ ] **Step 1: Write failing transaction test**

Add a focused test around a store append error or manifest update error to verify a transaction does not report `Committed` after durable event append state becomes ambiguous.

- [ ] **Step 2: Run RED test**

Run:

```bash
cargo test -p pi-coding-agent transaction_reports_in_doubt_when_manifest_update_fails_after_append --lib
```

Expected: fail because transaction state is only internal and partial commit uncertainty is not modeled.

- [ ] **Step 3: Add in-doubt outcome state**

Extend transaction finalization internals so append success plus manifest update failure records an explicit in-doubt result for the session service. Do not emit root operation success from this path.

- [ ] **Step 4: Run GREEN and regression tests**

Run:

```bash
cargo test -p pi-coding-agent transaction_reports_in_doubt_when_manifest_update_fails_after_append --lib
cargo test -p pi-coding-agent session_log::transaction --lib
cargo test -p pi-coding-agent session_service --lib
```

Expected: selected transaction and service tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/session_log/transaction.rs crates/pi-coding-agent/src/coding_session/session_service.rs docs/TODO.md
git commit -m "feat: guard partial session commit uncertainty"
```

## Task 6: Runtime And Capability Generation References

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/event.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/session_service.rs`
- Modify: `docs/TODO.md`

- [ ] **Step 1: Write failing durable audit tests**

Add tests for prompt/profile/plugin-affecting operations that verify durable session facts include the runtime/profile/capability generation identifiers needed to explain replay behavior.

- [ ] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent session_events_record_runtime_generation_references --lib
cargo test -p pi-coding-agent session_events_record_capability_generation_references --lib
```

Expected: fail because generation reference fields are not modeled.

- [ ] **Step 3: Add generation reference model**

Add a compact durable reference struct to `event.rs` and attach it only to operation families where replay/audit needs it:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PersistedRuntimeGenerationRef {
    pub profile_id: Option<ProfileId>,
    pub capability_generation: Option<u64>,
}
```

- [ ] **Step 4: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent session_events_record_runtime_generation_references --lib
cargo test -p pi-coding-agent session_events_record_capability_generation_references --lib
cargo test -p pi-coding-agent session_log --lib
```

Expected: selected session log tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/session_log/event.rs crates/pi-coding-agent/src/coding_session/session_log/transaction.rs crates/pi-coding-agent/src/coding_session/session_service.rs docs/TODO.md
git commit -m "feat: persist runtime generation references"
```

## Verification Checklist

- [ ] `cargo fmt --check`
- [ ] `cargo test -p pi-coding-agent session_log --lib`
- [ ] `cargo test -p pi-coding-agent session_service --lib`
- [ ] `cargo test -p pi-coding-agent event_service --lib`
- [ ] `cargo test -p pi-coding-agent protocol_events`
- [ ] `cargo check -p pi-coding-agent`
- [ ] `git diff --check`

## Spec Coverage

- Durable session sequence semantics: Task 1.
- Runtime/capability generation references: Task 6.
- Recovery markers for incomplete operation families: Tasks 2-4.
- Replay/projection tolerance for existing Rust-native logs: Tasks 1-4.
- Transaction/idempotency behavior around partial commit uncertainty: Task 5.
