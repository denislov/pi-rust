# Non-Persistent Product Runtime Design

## Purpose

This design defines how prompt execution works when durable session persistence is disabled.

The Flow-centered runtime should not depend on the old `session_runner` just because the user selected `--no-session`, RPC was started with disabled session mode, or JSON mode needs a one-shot transcript stream. `CodingAgentSession` should still be the product runtime owner for prompt operations. The difference is persistence policy: persistent runs write Rust-native `session.json` and `events.jsonl`; non-persistent runs keep operation state in memory and emit `SessionWriteSkipped`.

This design is the next convergence step after session finalization ownership moves under `SessionService`.

## Scope

In scope:

- define a non-persistent `CodingAgentSession` mode;
- keep prompt execution on `PromptTurnFlow` for both persistent and non-persistent modes;
- define session-boundary behavior for `PromptTurnContext` and `OpenSession`;
- define replay/transcript policy for non-persistent runs;
- define `SessionWriteSkipped` semantics for disabled persistence;
- define how print, RPC, and JSON adapters should converge onto the product runtime;
- identify tests needed before old runner usage can shrink.

Out of scope:

- implementing JSON adapter migration in this spec;
- interactive prompt migration;
- Rust-native fork/branch semantics;
- `AgentTurnFlow`;
- public plugin APIs;
- deleting old `session_runner`;
- TypeScript session import/export or compatibility.

## Current Gaps

The current architecture has two runtime paths:

```text
persistent enabled print/RPC session targets
  -> CodingAgentSession
  -> PromptTurnFlow
  -> Rust-native session log
  -> CodingAgentEvent

no-session / disabled print or RPC, plus JSON execution today
  -> old session_runner
  -> AgentEvent
  -> adapter-specific rendering
```

That keeps old runner behavior alive for primary prompt paths even after enabled print/RPC sessions migrate. It also makes JSON mode only partially converged: JSON rendering can use `CodingAgentEvent`, but execution still comes from `run_session_prompt`.

The missing abstraction is a product runtime session boundary that can say:

```text
this prompt has a product owner, operation ids, runtime snapshot, Flow graph,
and CodingAgentEvent stream, but no durable session log.
```

## Runtime Modes

`CodingAgentSession` should support two internal persistence modes.

```text
Persistent
  has SessionService
  has Rust-native session id
  has replay from events.jsonl
  has TurnTransaction
  finalizes durable operation markers through SessionService

NonPersistent
  has no SessionLogStore
  has no durable session id
  has optional in-memory transcript for the owner lifetime
  has no TurnTransaction
  finalizes to SessionWriteSkipped
```

Both modes still have:

- `RuntimeService`;
- `FlowService`;
- `EventService`;
- `CapabilityService`;
- `PromptTurnFlow`;
- operation id and turn id;
- product events;
- typed `PromptTurnOutcome`.

The product owner remains `CodingAgentSession` in both modes. Non-persistent mode is not a bypass around the product runtime.

## Public Construction

The existing persistent constructors should keep their meaning:

```rust
CodingAgentSession::create(options)
CodingAgentSession::open(options)
CodingAgentSession::open_or_create(options)
```

Add one explicit non-persistent constructor or option.

Recommended shape:

```rust
impl CodingAgentSession {
    pub async fn non_persistent(
        options: CodingAgentSessionOptions,
    ) -> Result<Self, CodingSessionError>;
}
```

`CodingAgentSessionOptions` may grow a persistence flag later, but a named constructor is clearer for the first slice because it prevents accidental calls to `open()` without a durable target.

Non-persistent construction should not create a session directory and should not inspect old `JsonlSessionStorage`.

## Owner Internals

`CodingAgentSession` should hold a persistence boundary rather than assuming `SessionService` is always present.

Recommended internal shape:

```rust
enum SessionPersistence {
    Persistent(SessionService),
    NonPersistent(TransientSessionState),
}

struct TransientSessionState {
    runtime_id: String,
    transcript: Vec<TranscriptItem>,
}
```

`runtime_id` is a process-local product identifier for diagnostics and event correlation. It is not a durable `session_id`, should not be accepted by `open()`, and should not be written to disk.

`TranscriptItem` can reuse the Rust-native replay transcript type internally. If that creates visibility problems, add a small owner-private transcript representation and convert it into agent messages through `RuntimeService`.

## PromptTurnContext Session Boundary

`PromptTurnContext` needs to distinguish persistent and non-persistent session boundaries.

Recommended shape:

```rust
enum PromptSessionBoundary {
    Persistent {
        session_id: String,
        replay: SessionReplay,
        transaction: PromptTurnTransaction,
    },
    NonPersistent {
        runtime_id: String,
        transcript: Vec<TranscriptItem>,
    },
}
```

Context methods should expose intent instead of raw enum mutation:

```rust
set_persistent_session(...)
set_non_persistent_session(...)
is_session_boundary_ready()
replay_for_runtime()
has_active_transaction()
take_transaction()
```

Nodes that only need replay should ask for `replay_for_runtime()` or an equivalent transcript view. Nodes that record session facts should no-op cleanly when there is no transaction.

## PromptTurnFlow Impact

`OpenSession` should validate that the owner prepared one valid boundary:

- persistent boundary with session id, replay, and transaction; or
- non-persistent boundary with runtime id and transcript.

It should not open files, inspect adapter flags, or create old JSONL sessions.

`RecordUserInput` and `RunAgentTurn` should continue to record pending session facts when a transaction exists. In non-persistent mode they should skip durable event recording and still emit product stream events.

`BuildAgentRuntime` should hydrate from:

- Rust-native replay in persistent mode;
- in-memory transcript in non-persistent mode;
- empty transcript for one-shot non-persistent print/JSON runs.

`FinalizeTurn` should not commit in either mode. Session finalization remains an owner/`SessionService` concern for persistent mode and an owner skip event for non-persistent mode.

Persistent failure still writes a durable `operation.failed` marker when finalization succeeds. Non-persistent failure does not write durable operation events and instead emits `SessionWriteSkipped` before `PromptFailed`.

## Event Semantics

Non-persistent prompt runs should emit normal product events for runtime and agent behavior:

```text
PromptStarted
AgentTurnStarted
AssistantMessage*
ToolCall*
SessionWriteSkipped
PromptCompleted | PromptFailed | PromptAborted
```

`SessionWriteSkipped` should include the operation id and reason:

```text
session persistence disabled
```

This event is not an error. It is the canonical signal that the prompt was owned by the product runtime but no durable session write was attempted.

`PromptTurnOutcome::Success` in non-persistent mode should have:

```text
session_id: None
leaf_id: None
final_text
final_message
diagnostics
```

Failure and abort outcomes should also use `session_id: None`.

## Transcript Policy

There are two non-persistent transcript modes:

```text
OneShot
  no prior transcript is hydrated
  final assistant output is not retained after the prompt

OwnerLifetime
  transcript is retained in memory inside CodingAgentSession
  follow-up prompts on the same owner can hydrate previous user/assistant/tool messages
  transcript is lost when the process/session owner is dropped
```

Print and JSON should use `OneShot` unless their adapter explicitly keeps a session owner across prompts.

RPC disabled-session mode may use `OwnerLifetime` because the RPC state already holds long-lived process state. This gives disabled-session RPC coherent multi-turn behavior without writing old JSONL.

No non-persistent transcript should be exported as a Rust-native session or opened later by id. If export is requested later, it should be an explicit export workflow, not implicit session persistence.

## Adapter Convergence

### Print

Disabled/no-session print should move from old `run_session_prompt` to:

```text
CodingAgentSession::non_persistent(...)
  -> PromptTurnOptions
  -> prompt()
  -> final_text
```

No session directory should be created. `SessionWriteSkipped` should be emitted internally but not rendered in normal print output.

### RPC

RPC with `SessionMode::Disabled` should move from `spawn_session_prompt` to a long-lived non-persistent `CodingAgentSession`.

RPC state should eventually become:

```text
session: CodingAgentSession
running: Option<RunningPrompt>
event_adapter: RpcCodingEventAdapter
```

When persistence is disabled:

- prompt command streams `CodingAgentEvent`;
- `get_state.sessionId` should report a stable non-durable value such as `in-memory`;
- `sessionFile` should be omitted;
- capabilities should report persistence-dependent session commands as disabled or unsupported;
- abort/steer/follow-up remain constrained by current `AgentTurnFlow` gaps.

This spec does not require RPC state to switch fully to `SessionWriteSkipped` handling in the first implementation slice, but new code should not add more raw `AgentEvent` dependencies.

### JSON

JSON mode should converge after non-persistent runtime exists.

Target path:

```text
JSON adapter
  -> CodingAgentSession::non_persistent(...)
  -> CodingAgentEvent stream
  -> CodingProtocolEventAdapter
```

The JSON wire shape should remain stable unless a later JSON-specific spec deliberately changes it. The session header currently emitted by JSON mode is a wire artifact, not proof of durable Rust-native session persistence.

### Interactive

Interactive prompt migration stays out of this spec. It should later choose between persistent `CodingAgentSession` and non-persistent `CodingAgentSession` using the same product runtime boundary.

## Session Actions

In non-persistent mode:

- fork is unsupported;
- clone is unsupported;
- switch/open durable session is unavailable until a persistent session is created;
- export is a future explicit workflow;
- compact remains unsupported until a dedicated compaction flow exists;
- tools and shell availability are independent of persistence mode.

`CapabilityService` should be able to report these differences without adapter-specific strings.

## Error Handling

Non-persistent mode should fail early when an operation requires durable session storage.

Examples:

- opening by session id in non-persistent mode is an input error;
- fork target in non-persistent mode is unsupported;
- session list/tree/stat commands in non-persistent mode return disabled/unsupported capability states;
- prompt persistence skip is not an error and should not change prompt success.

Provider/tool/runtime errors should behave the same as persistent mode, except durable `operation.failed` is not written. The product stream should still emit `PromptFailed` after `SessionWriteSkipped`.

## Tests

Focused tests should cover:

- `CodingAgentSession::non_persistent()` creates no `session.json` or `events.jsonl`.
- non-persistent prompt runs through `PromptTurnFlow` with a faux provider.
- non-persistent prompt emits `SessionWriteSkipped` before `PromptCompleted`.
- non-persistent `PromptTurnOutcome::Success` has no `session_id` or `leaf_id`.
- persistent prompt behavior remains unchanged.
- no-session print uses product runtime and does not create old JSONL.
- disabled-session RPC prompt streams product-event-derived protocol events.
- disabled-session RPC prompt does not create old JSONL.
- JSON adapter migration tests should be added in the later JSON convergence slice.

Suggested focused checks after implementation:

```text
cargo fmt --check
cargo test -p pi-coding-agent coding_session
cargo test -p pi-coding-agent --test print_mode
cargo test -p pi-coding-agent --test rpc_mode
cargo test -p pi-coding-agent --test protocol_sessions
cargo check --workspace
```

## Acceptance

This design is implemented when:

- `CodingAgentSession` has an explicit non-persistent construction path;
- disabled/no-session print no longer needs old `session_runner`;
- disabled-session RPC prompt can run through `CodingAgentSession`;
- non-persistent prompt paths emit `SessionWriteSkipped`;
- persistent prompt paths keep using Rust-native session logs;
- no non-persistent path writes old TypeScript-compatible JSONL as a product prompt operation;
- JSON execution convergence has a clear follow-up path onto the non-persistent runtime.

## Follow-Up Specs

After this design, the likely next specs are:

- JSON execution convergence onto `CodingAgentSession`;
- interactive prompt and event bridge convergence;
- Rust-native branch/leaf/fork semantics;
- session action convergence under `SessionService`.
