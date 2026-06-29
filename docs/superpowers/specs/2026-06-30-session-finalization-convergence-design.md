# Session Finalization Convergence Design

## Purpose

This design closes a Phase 2/3 ownership gap in the Flow-centered runtime.

The current prompt path records pending session facts through `TurnTransaction`, but successful and failed transaction finalization is still triggered from `PromptTurnContext` and `PromptTurnFlow`/`FlowService`. The architecture direction says `SessionService` owns canonical session writes and success/abort/failure finalization. This slice moves prompt transaction finalization back under `SessionService` and emits the session persistence product events that adapters can consume.

This is a convergence step, not a broader adapter migration.

## Scope

In scope:

- make `SessionService` the owner of prompt transaction commit, fail, abort, and skip decisions;
- keep Flow nodes limited to validation and pending event recording;
- emit `CodingAgentEvent::SessionWritePending`, `SessionWriteCommitted`, and `SessionWriteSkipped` from the product runtime;
- preserve existing Rust-native `session.json` and `events.jsonl` persistence semantics;
- keep existing enabled print/RPC session paths working;
- update focused tests for owner-level event ordering and committed event logs.

Out of scope:

- JSON execution migration from old runner to `CodingAgentSession`;
- interactive prompt migration;
- `AgentTurnFlow`;
- Rust-native fork/branch semantics;
- adding `leaf_id` to `CodingAgentEvent::SessionWriteCommitted`;
- removing old `session_runner`.

## Current Problem

The implementation has the correct data model but the wrong finalization owner:

- `SessionService::begin_prompt_transaction()` creates a transaction.
- `PromptTurnContext` holds the transaction and records pending prompt, assistant, and tool facts.
- `FinalizeTurn` calls `ctx.commit_transaction(None)`.
- `FlowService` calls `ctx.fail_transaction(...)` when the graph fails.
- `TurnTransaction::commit()` and `fail()` flush pending events and append final operation markers.

That keeps Flow nodes from opening files directly, but it still lets Flow/context decide when canonical writes finalize. It also leaves `CodingAgentEvent::SessionWrite*` variants unused, so adapters cannot yet observe session persistence through the canonical product event stream.

## Target Ownership Model

`SessionService` owns all final session write decisions.

```text
CodingAgentSession
  creates PromptTurnContext
  starts TurnTransaction through SessionService
  runs PromptTurnFlow
  asks SessionService to finalize the transaction
  emits returned CodingAgentEvent values through EventService

PromptTurnFlow
  validates operation stages
  records pending session facts through PromptTurnContext
  does not commit, fail, abort, or skip canonical session writes

SessionService
  creates transactions
  commits successful transactions
  records failed transactions
  records aborted transactions
  reports skipped writes when no transaction can be finalized
```

`TurnTransaction` remains the low-level append/flush primitive. The convergence is about who is allowed to call it for finalization.

## API Changes

Add a narrow internal finalization API to `SessionService`.

Recommended shape:

```rust
pub(crate) struct FinalizedSessionWrite {
    pub(crate) events: Vec<CodingAgentEvent>,
    pub(crate) session_id: Option<String>,
    pub(crate) leaf_id: Option<String>,
}

pub(crate) enum SessionWriteSkipReason {
    NoTransaction,
    AlreadyFinalized,
    FinalizationFailed(String),
}

impl SessionService {
    pub(crate) fn commit_prompt_transaction(
        &mut self,
        transaction: Option<PromptTurnTransaction>,
        new_leaf_id: Option<String>,
    ) -> Result<FinalizedSessionWrite, CodingSessionError>;

    pub(crate) fn fail_prompt_transaction(
        &mut self,
        transaction: Option<PromptTurnTransaction>,
        error_code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<FinalizedSessionWrite, CodingSessionError>;

    pub(crate) fn abort_prompt_transaction(
        &mut self,
        transaction: Option<PromptTurnTransaction>,
        reason: impl Into<String>,
    ) -> Result<FinalizedSessionWrite, CodingSessionError>;
}
```

The exact helper names can change during implementation, but the boundary should not: finalization calls belong to `SessionService`, not `PromptTurnContext` or concrete Flow nodes.

`PromptTurnContext` should expose a way for the owner to recover the active transaction after graph execution:

```rust
impl PromptTurnContext {
    pub(crate) fn take_transaction(&mut self) -> Option<PromptTurnTransaction>;
    pub(crate) fn has_active_transaction(&self) -> bool;
}
```

After this slice, context methods such as `commit_transaction()` and `fail_transaction()` should be removed or kept private only for tests that explicitly cover the transaction primitive. Flow nodes should not call them.

## Prompt Flow Changes

`FinalizeTurn` remains in the graph because it is a useful explicit boundary, but its behavior changes.

It should validate:

- a final assistant message exists;
- a turn transaction exists;
- required prompt stages have completed.

It should not:

- call `TurnTransaction::commit()`;
- update `session.json`;
- append `operation.committed`;
- emit session persistence product events.

The owner finalizes after the graph returns successfully:

```text
PromptTurnFlow success
  -> SessionService::commit_prompt_transaction(...)
  -> EventService emits SessionWritePending
  -> EventService emits SessionWriteCommitted
  -> EventService emits PromptCompleted
  -> PromptTurnOutcome::Success
```

For graph failure:

```text
PromptTurnFlow error
  -> SessionService::fail_prompt_transaction(...)
  -> EventService emits SessionWritePending if a transaction exists
  -> EventService emits SessionWriteCommitted or SessionWriteSkipped
  -> EventService emits PromptFailed
  -> PromptTurnOutcome::Failed
```

The name `SessionWriteCommitted` means the write operation reached a final durable marker, including failed/aborted operation markers. The low-level session event data distinguishes `operation.committed`, `operation.failed`, and `operation.aborted`.

## Event Ordering

The product stream should preserve this order for persistent prompt paths:

```text
PromptStarted
AgentTurnStarted
assistant/tool events
SessionWritePending
SessionWriteCommitted | SessionWriteSkipped
PromptCompleted | PromptFailed | PromptAborted
```

`SessionWritePending` should include the operation id before the transaction flush is attempted.

`SessionWriteCommitted` should include:

- operation id;
- session id.

This slice does not add `leaf_id` to the event. Active leaf and branch semantics are reserved for the Rust-native fork/branch slice.

`SessionWriteSkipped` should include:

- operation id when known;
- a clear reason.

If the existing event variant does not allow an optional operation id, the implementation can use the current operation id from `PromptTurnContext` when skipping after a prompt starts.

## Error Handling

Successful graph, successful commit:

- append pending events and `operation.committed`;
- emit `SessionWritePending`;
- emit `SessionWriteCommitted`;
- return `PromptTurnOutcome::Success`.

Successful graph, failed commit:

- emit `SessionWritePending`;
- attempt `fail_prompt_transaction` if the transaction is still recoverable;
- emit `SessionWriteSkipped` if no reliable final marker can be written;
- return `PromptTurnOutcome::Failed`;
- do not emit `PromptCompleted`.

Graph failure with active transaction:

- call `fail_prompt_transaction`;
- append diagnostic and `operation.failed`;
- emit session write product events;
- return `PromptTurnOutcome::Failed`.

Graph failure before transaction exists:

- emit `SessionWriteSkipped` with a clear reason;
- return `PromptTurnOutcome::Failed`.

Abort support can use the same boundary when a cancellation handle exists. This slice should not add a public abort API unless it is already required by current tests.

## Adapter Impact

RPC enabled-session prompt already receives `CodingAgentEvent`. Existing behavior can remain: final RPC state may still be updated from `PromptTurnOutcome` while this slice lands.

New coverage should prove that the product event stream contains session persistence events. A later Phase 3 adapter slice can move RPC state updates from outcome inference to `SessionWriteCommitted`.

Print mode does not need new rendering behavior for session write events.

JSON mode is not changed by this slice because JSON execution still uses the old runner.

## Tests

Focused tests should cover:

- `SessionService` commit emits pending and committed product events.
- `SessionService` failure finalization emits pending and committed product events for `operation.failed`.
- `SessionService` skip emits `SessionWriteSkipped` when no transaction exists.
- `FinalizeTurn` validates readiness but does not flush `events.jsonl`.
- owner-level `CodingAgentSession::prompt()` still writes `operation.committed`.
- owner-level prompt subscriber receives `SessionWritePending` before `SessionWriteCommitted` before `PromptCompleted`.
- provider failure records `operation.failed` and emits `PromptFailed` after session write finalization.
- enabled-session RPC prompt tests continue to pass and still persist Rust-native session logs.

Suggested focused checks:

```text
cargo fmt --check
cargo test -p pi-coding-agent coding_session
cargo test -p pi-coding-agent --test protocol_sessions
cargo test -p pi-coding-agent --test rpc_mode
cargo check --workspace
```

## Acceptance

The slice is complete when:

- no concrete `PromptTurnFlow` node calls transaction commit, abort, or fail;
- `SessionService` owns prompt transaction finalization;
- persistent prompt paths emit `SessionWritePending` and either `SessionWriteCommitted` or `SessionWriteSkipped`;
- final prompt events are emitted only after session finalization policy runs;
- existing enabled print/RPC Rust-native persistence still works;
- old no-session/disabled/interactive paths remain explicitly transitional.
