# Rust-Native Session Fork and Clone Design

## Context

Phase 3 now routes primary prompt paths through `CodingAgentSession`, persists
Rust-native session logs, hydrates Rust-native sessions in interactive mode, and
commits a real `active_leaf_id` for every successful persistent prompt. The next
session-action gap is `/clone` and `/fork`: both still return explicit
unsupported messages for Rust-native sessions.

The old JSONL implementation clones a branch into a sibling session file by
copying the path from the selected entry to the root. Rust-native sessions do
not use entry ids as prompt leaves. They use committed `leaf_id` values from
`operation.committed`. The first Rust-native fork/clone slice should therefore
operate on committed leaves, not on the temporary read-only `/tree` projection
ids.

## Goals

- Add `SessionService`-owned Rust-native clone/fork APIs.
- Create a new Rust-native session directory for clone/fork results.
- Copy durable source history up to a committed target leaf into the new
  session by rewriting source event envelopes to the new session id.
- Record typed provenance events for clone/fork in the new session log.
- Keep `CodingAgentSession` as the adapter-facing owner API; interactive UI
  must not read or write `events.jsonl` directly.
- Wire interactive `/clone` and `/fork` for active Rust-native sessions.
- Preserve explicit unsupported behavior for Rust-native compact and tree
  navigation.

## Non-Goals

- Do not implement full branch DAG navigation in this slice.
- Do not make `/tree` projection ids valid fork targets.
- Do not implement session compaction.
- Do not add TypeScript JSONL import/export compatibility.
- Do not expose `SessionService` as a public API.

## Approaches

Recommended: rewrite durable source events into a new session.

This produces an independent Rust-native session that can replay normally,
hydrate through existing `SessionService`, and continue prompting without a
special parent lookup path. It matches the product shape of the old clone/fork
action while staying Rust-native.

Rejected: store only a pointer to the source session and target leaf.

That would be cheap, but replay would need cross-session lookup and lifecycle
rules before follow-up prompts could work. It also makes export/compaction more
complex.

Deferred: implement full branch DAG and tree navigation first.

That is the longer-term direction, but it is too large for this Phase 3
adapter-convergence slice. Clone/fork can create independent sessions now and
leave in-session branch switching for a later action slice.

## Session Events

Add typed provenance events:

```text
session.cloned { source_session_id, source_leaf_id }
session.forked { source_session_id, source_leaf_id }
```

These events are informational for replay in this slice. They should be
ignored by transcript folding, but they make the session log explain why a new
session begins with copied history.

The copied history should exclude source `session.created`, `session.cloned`,
and `session.forked` events. Prompt operation events, message events, tool
events, diagnostics, metadata, and active leaf markers before the target leaf
remain useful history and should be copied.

When copying, the new event envelope must:

- use the target session id;
- get a fresh event id;
- preserve operation id, turn id, branch id, leaf id, and data;
- clear `parent_event_id`, because copied events no longer share the source
  event id space.

## Service API

`SessionService` should own the durable operation:

- choose the target leaf: explicit fork target or current active leaf;
- validate that the target leaf appears in a committed source operation;
- create a new sibling Rust-native session;
- write `session.created`;
- write `session.cloned` or `session.forked`;
- append rewritten source events through the target leaf commit;
- set the new manifest `active_leaf_id` to the target leaf;
- return a hydrated view or service handle for the new session.

`CodingAgentSession` should expose internal owner methods that adapters can
call, such as `clone_session(options)` and `fork_session(options, target_leaf)`.
Adapters pass session identity and cwd through `CodingAgentSessionOptions`; they
do not call `SessionService` or inspect event logs.

## Interactive Behavior

For an active Rust-native session:

- `/clone` clones from the current active leaf and selects the new session.
- `/fork` with no argument forks from the current active leaf and selects the
  new session.
- `/fork <leaf-id>` accepts a committed Rust-native `leaf_id` and selects the
  new session if found.
- `/fork <projection-entry-id>` remains invalid until `/tree` exposes real
  leaf-backed navigation.

After clone/fork, interactive state should update through the hydrated
Rust-native session choice: session label, active session path, active leaf, and
transcript should reflect the new session.

## Error Handling

- Missing active Rust-native session: keep the existing "Nothing to clone/fork
  yet" style message.
- Missing active leaf: report that there is no committed Rust-native leaf yet.
- Unknown fork leaf: report that the leaf id was not found in the source
  session.
- Any storage or parse failure should return a typed `CodingSessionError` and
  be shown as a command failure in interactive mode.

## Tests

Focused coverage should prove:

- `SessionService` can clone/fork a Rust-native session from active leaf.
- Explicit fork to an unknown leaf fails.
- The new session has its own `session_id`, `session.json`, and `events.jsonl`.
- The new session log contains `session.cloned` or `session.forked`.
- The new manifest and hydration summary active leaf equal the target leaf.
- Follow-up prompt hydration from the cloned/forked session includes copied
  source transcript.
- Interactive `/clone` and `/fork` after a Rust-native prompt select the new
  Rust-native session instead of reporting unsupported.
- Interactive `/compact` and tree navigation remain explicitly unsupported.

Suggested checks:

```text
cargo fmt --check
cargo test -p pi-coding-agent coding_session
cargo test -p pi-coding-agent --test interactive_sessions
cargo test -p pi-coding-agent --test interactive_mode
cargo check --workspace
```

Run `cargo test --workspace` if event schema changes touch shared folding or
adapter behavior beyond the focused slice.

## Handoff

After this slice, Phase 3 will still need Rust-native tree navigation and
session compaction. The later navigation slice should replace temporary
projection ids with leaf-aware tree nodes or another stable Rust-native branch
view before accepting arbitrary UI tree selections as fork targets.
