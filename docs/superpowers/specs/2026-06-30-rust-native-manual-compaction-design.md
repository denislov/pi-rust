# Rust-Native Manual Compaction Design

## Context

Phase 3 routes primary interactive prompts through `CodingAgentSession`, writes
Rust-native session logs, hydrates Rust-native sessions through
`SessionService`, and now supports Rust-native `/clone` and `/fork`. The main
remaining interactive prompt/session action still tied to the old runner is
manual `/compact`.

The old JSONL compaction path calls `pi_agent_core::compaction::summarize`,
appends a legacy compaction entry, and updates the interactive transcript via
`AgentEvent::SessionCompacted`. The Rust-native path should keep the product
behavior but move ownership under `CodingAgentSession` and `SessionService`.
Manual compaction should be a session-maintenance operation, not a prompt turn.

## Goals

- Add a Rust-native manual compaction operation owned by `CodingAgentSession`.
- Keep `PromptTurnFlow` focused on prompt turns; do not make
  `PromptInvocation::Compact` a prompt-flow request.
- Persist manual compaction in the Rust-native event log with typed events.
- Fold Rust-native compaction events during replay so future prompts hydrate
  from compacted history.
- Wire interactive `/compact [instructions]` for active Rust-native sessions.
- Keep legacy JSONL compaction behavior unchanged for legacy sessions.
- Keep runtime auto-compaction out of this slice.

## Non-Goals

- Do not implement tree navigation or leaf-aware branch switching.
- Do not implement print-mode `ForkTarget`.
- Do not delete the old `session_runner`; legacy JSONL compaction still uses it.
- Do not expose `SessionService` as a public API.
- Do not add TypeScript session compatibility.
- Do not make manual compaction cancellable unless the summarization boundary
  already supports it cleanly.

## Recommended Approach

Add a dedicated product operation:

```text
CodingAgentSession::compact(options)
  -> SessionService::compact_current(runtime, instructions)
  -> summarize compactable replay context
  -> append typed Rust-native compaction events
  -> return CodingAgentEvent stream + hydrated session state
```

This keeps compaction separate from `PromptTurnFlow`, matches the architecture
direction that reserves dedicated workflow types for session-maintenance
operations, and lets interactive mode use the same `CodingEventBridge` event
adapter already used for prompt events.

## Alternatives

### Add Compact To PromptTurnFlow

This would reuse the existing prompt task path, but it makes request resolution
and prompt-input preparation special-case manual compaction. It also weakens the
current boundary where `PromptTurnFlow` rejects manual compaction explicitly.

### Keep Old Runner For Rust-Native Compact

This is not viable for the migration goal. The old runner writes legacy JSONL
entries and cannot mutate Rust-native `events.jsonl` without crossing the new
session-service boundary.

## Session Events

Add typed events:

```text
session.compaction.started { first_kept_message_id, tokens_before }
session.compaction.completed { summary, first_kept_message_id, tokens_before }
```

`session.compaction.started` makes failed or interrupted compaction attempts
visible in the log if the operation later records a failure diagnostic. The
first implementation may append both events in one successful write if the
summarization step must finish before durable mutation begins.

`session.compaction.completed` is the replay signal. When replay sees it, it
should replace compacted transcript history with a synthetic compaction summary
item followed by items at or after `first_kept_message_id`. This makes later
prompt hydration compacted without preserving old history in the active context.

The event should preserve enough data for UI replay and future export:

- `summary`: model-generated compaction summary;
- `first_kept_message_id`: replay transcript id of the first non-compacted item;
- `tokens_before`: token estimate before compaction.

## Replay Model

Rust-native replay already produces transcript items with ids for assistant and
tool rows, and turn ids for user input. Manual compaction needs a stable
message-id namespace across replay.

The first slice should use these identifiers:

- user input: `turn_id`;
- assistant message: `message_id`;
- tool calls: `tool_call_id`.

The compaction operation chooses the last replay transcript item as the kept
tail and uses that item's replay id as `first_kept_message_id`. Replay folding
then truncates prior transcript items and prepends a `CompactionSummary`
transcript item before the kept tail. Later branch/tree work can make this more
expressive.

## Product Events

Use existing product events where possible:

- `SessionWritePending { operation_id }` before durable compaction events are
  appended;
- `RuntimeCompactionCompleted { operation_id, turn_id, summary,
  first_kept_message_id, tokens_before }` after summarization succeeds;
- `SessionWriteCommitted { operation_id, session_id }` after events are
  durable;
- `PromptFailed { operation_id, error }` for provider/session failures.

The event name `RuntimeCompactionCompleted` is already used by UI adapters for
compaction notices. In this slice it can represent the UI-visible product
compaction completion even though the durable mutation is session compaction.
A later event taxonomy cleanup can split runtime and session compaction events
if needed.

## Interactive Behavior

For an active Rust-native session:

- `/compact` starts Rust-native manual compaction and selects the same session.
- `/compact <instructions>` passes custom summarization instructions to
  `summarize`.
- On success, the transcript shows a compaction notice and the footer context
  token count becomes unknown until the next model response.
- The active session path, session id, and active leaf remain the same unless
  the event log later introduces a dedicated compaction leaf.
- If the session has fewer than two replay transcript items, report
  "Nothing to compact (no messages yet)".

For legacy JSONL sessions, keep the existing old-runner compaction path.

## Error Handling

- No active session: keep "Nothing to compact (no messages yet)".
- Active session is Rust-native but cannot hydrate: show a command/task failure
  through `UiEvent::AgentError`.
- Too little compactable history: return a typed session/input error and show it
  in the transcript.
- Summarization/provider failure: emit a product failure event and do not append
  `session.compaction.completed`.
- Unknown `first_kept_message_id` during replay: keep the transcript unchanged
  and add a diagnostic instead of dropping history.

## Tests

Focused coverage should prove:

- `SessionEventData` serializes stable compaction kind names.
- `SessionReplay` folds `session.compaction.completed` into a compacted
  transcript and diagnoses unknown kept ids.
- `SessionService` compacts a Rust-native session and persists typed events.
- `CodingAgentSession::compact()` emits session write and compaction events in
  a deterministic order.
- Interactive `/compact` after a Rust-native prompt no longer reports
  unsupported and leaves one Rust-native session directory.
- Legacy JSONL `/compact` coverage remains unchanged.

Suggested checks:

```text
cargo fmt --check
cargo test -p pi-coding-agent coding_session
cargo test -p pi-coding-agent --test interactive_event_bridge
cargo test -p pi-coding-agent --test interactive_mode
cargo test -p pi-coding-agent --test interactive_sessions
cargo check --workspace
```

Run `cargo test --workspace` because the replay schema change touches shared
interactive hydration behavior.

## Handoff

After this slice, Phase 3 still needs Rust-native tree navigation and any
print-mode branch target convergence. Phase 6 can later extract this operation
into a named `ManualCompactionFlow` once the Phase 3 adapter path is proven.
