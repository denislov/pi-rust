# Rust-Native Active Leaf Commit Design

## Context

Phase 3 has moved primary print, JSON, RPC, and interactive prompt paths onto
`CodingAgentSession` and Rust-native session logs. Interactive resume, `/session`,
and read-only `/tree` now hydrate through `SessionService` for Rust-native
sessions.

The remaining Phase 3 session-action work depends on a reliable branch/leaf
basis. The session manifest and event envelope already reserve
`active_branch_id`, `active_leaf_id`, `branch_id`, and `leaf_id`, and
`operation.committed` already accepts `new_leaf_id`. However, successful prompt
commits currently pass `None`, so persisted sessions do not establish a real
active leaf. That makes fork, clone, navigation, and future session compaction
ambiguous.

This slice adds active leaf commit semantics for successful persistent prompt
turns. It is a convergence step, not the full fork/branch implementation.

## Goals

- Generate a new Rust-native `leaf_id` for every successful persistent prompt
  commit.
- Persist that leaf through the existing transaction finalization path.
- Keep `SessionService` as the only component that commits canonical session
  writes.
- Make manifest, replay hydration, session summaries, `/session`, and read-only
  `/tree` observe the same active leaf after a successful prompt.
- Leave fork, clone, compact, and tree navigation explicitly unsupported until
  dedicated session-action APIs exist.

## Non-Goals

- Do not implement `/fork`, `/clone`, `/compact`, branch switch, or tree
  navigation in this slice.
- Do not add TypeScript session JSONL compatibility.
- Do not expose `SessionService` as public API.
- Do not make adapters read or write `events.jsonl` directly.
- Do not introduce `AgentTurnFlow`; that remains Phase 4.

## Architecture

`CodingAgentSession` remains the product owner for prompt execution. On a
successful persistent prompt outcome, it asks the persistent session layer for a
new leaf id and finalizes the prompt transaction with that id. The existing
`TurnTransaction::commit(Some(leaf_id))` path records
`operation.committed { new_leaf_id }` and updates the manifest `active_leaf_id`.

The leaf id is allocated before commit finalization and only becomes durable if
the transaction commit succeeds. Failed and aborted prompt outcomes continue to
finalize without a new leaf. Non-persistent sessions continue to report
`leaf_id: None` and emit `SessionWriteSkipped`.

The recommended implementation keeps leaf allocation inside `SessionService`,
not in adapters. A small method such as `next_leaf_id()` or a commit helper can
use `SystemIdGenerator` internally, preserving the owner/service boundary while
avoiding adapter knowledge of event-log internals.

## Components

`CodingAgentSession::finalize_prompt_transaction`

- For persistent success, request or supply a fresh leaf id to
  `SessionService::commit_prompt_transaction`.
- Preserve existing failure and abort behavior.
- Preserve event ordering: product events from the flow, then
  `SessionWritePending`, then `SessionWriteCommitted`, then final prompt
  outcome.

`SessionService`

- Own leaf id allocation for persistent sessions.
- Continue to delegate durable writes to `TurnTransaction`.
- Return `FinalizedSessionWrite.leaf_id = Some(new_leaf_id)` for successful
  prompt commits.

`TurnTransaction`

- No new transaction model is required for this slice.
- Existing `commit(Some(leaf_id))` should remain the only path that updates the
  manifest active leaf for a prompt operation.

`SessionReplay` and hydration

- Existing replay already observes `operation.committed { new_leaf_id }` and
  `active_leaf.changed`. No new replay event kind is required.
- `CodingAgentSessionHydration.summary.active_leaf_id` should match the updated
  manifest after a successful prompt.

Interactive session actions

- `/session` and read-only `/tree` should benefit from hydration without direct
  schema access.
- Rust-native fork, clone, compact, and navigation remain explicit unsupported
  capability boundaries in this slice.

## Data Flow

1. Adapter invokes `CodingAgentSession::prompt()` with runtime-backed
   `PromptTurnOptions`.
2. `PromptTurnFlow` records user/assistant/tool events into the active
   `TurnTransaction`.
3. On success, `CodingAgentSession` finalization requests a new leaf id from
   `SessionService`.
4. `SessionService::commit_prompt_transaction` calls
   `TurnTransaction::commit(Some(new_leaf_id))`.
5. The transaction appends pending events plus
   `operation.committed { new_leaf_id }` to `events.jsonl`.
6. The transaction updates `session.json.active_leaf_id`.
7. `PromptTurnOutcome::Success.leaf_id`, `FinalizedSessionWrite.leaf_id`,
   hydration, and session summaries report the same leaf id.

## Error Handling

- If leaf allocation cannot happen, treat the prompt as a session finalization
  failure and follow the existing prompt failure fallback path.
- If transaction commit fails after leaf allocation, do not report the leaf as
  committed. Preserve the existing `SessionWriteSkipped` fallback with a clear
  reason.
- Do not emit `active_leaf.changed` as a separate event in this slice. The
  committed operation remains the canonical active leaf change.
- Failed and aborted prompt transactions must not advance active leaf.

## Testing

Focused tests should prove:

- A successful persistent prompt returns `PromptTurnOutcome::Success` with
  `leaf_id: Some(...)`.
- The session event log contains `operation.committed` with the same
  `new_leaf_id`.
- The manifest `active_leaf_id`, replay `active_leaf_id`, hydration summary, and
  list summary agree after success.
- Failed or invalid prompt setup does not advance active leaf.
- Non-persistent prompts still return `leaf_id: None` and emit
  `SessionWriteSkipped`.
- Interactive `/session` output for a Rust-native prompt session includes the
  committed active leaf.
- Rust-native fork, clone, compact, and navigation remain explicit unsupported
  boundaries until their own session-action slice.

Suggested checks:

```text
cargo fmt --check
cargo test -p pi-coding-agent coding_session
cargo test -p pi-coding-agent --test interactive_sessions
cargo test -p pi-coding-agent --test interactive_mode
cargo check --workspace
```

Run `cargo test --workspace` if implementation touches shared session-log
behavior beyond the focused finalization path.

## Handoff

After this slice, Phase 3 can implement Rust-native fork/clone/navigation against
a real active leaf instead of an empty or inferred base. The next session-action
slice should add typed branch/fork events and shared `SessionService` methods,
then wire interactive commands through those owner APIs.
