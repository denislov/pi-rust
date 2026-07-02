# Rust-Native Session Format

Last verified: 2026-07-02

This document records the current `pi-coding-agent` session persistence format. It describes the Rust-native product session log, not the legacy TypeScript `pi` session JSONL format.

## Status

The TypeScript session JSONL compatibility requirement is removed. Product prompt/session paths should persist through `CodingAgentSession` and `SessionService` using this format, or run as explicit non-persistent sessions.

Legacy TypeScript JSONL files may remain useful as historical fixtures or rejected inputs, but they are not the current canonical storage format.

## Storage Root

The session log root is selected in this order:

1. Explicit adapter/runtime `session_log_root` or CLI `--session-dir`.
2. `PI_SESSION_DIR` when resolving CLI session options.
3. `$PI_RUST_DIR/sessions` when `PI_RUST_DIR` is set.
4. `~/.pi-rust/sessions` by default.

`PI_AGENT_DIR` is intentionally ignored for the default Rust-native session root.

## Directory Layout

Each session is a directory under the session log root:

```text
<session-root>/
  <session_id>/
    session.json
    events.jsonl
    blobs/
    index/
```

`session_id` must be non-empty and contain only ASCII letters, ASCII digits, `_`, or `-`.

`blobs/` and `index/` are reserved directories. The current canonical replay source is `events.jsonl` plus `session.json` metadata.

## Manifest

`session.json` uses schema `pi-rust.session` version `1`.

Required fields:

```json
{
  "schema": "pi-rust.session",
  "version": 1,
  "session_id": "sess_1",
  "created_at": "2026-06-29T00:00:00Z",
  "updated_at": "2026-06-29T00:00:01Z",
  "event_log": "events.jsonl"
}
```

Optional fields:

```json
{
  "active_branch_id": "branch_1",
  "active_leaf_id": "leaf_1"
}
```

`event_log` must be a contained relative path. Absolute paths, `.` segments, and `..` segments are rejected.

## Event Envelope

Each line in `events.jsonl` is one UTF-8 JSON event envelope using schema `pi-rust.session.event` version `2`.

Envelope fields:

```json
{
  "schema": "pi-rust.session.event",
  "version": 2,
  "session_id": "sess_1",
  "event_id": "evt_1",
  "created_at": "2026-06-29T00:00:01Z",
  "kind": "turn.started",
  "data": {}
}
```

Optional envelope fields:

```json
{
  "operation_id": "op_1",
  "turn_id": "turn_1",
  "branch_id": "branch_1",
  "leaf_id": "leaf_1",
  "parent_event_id": "evt_0"
}
```

The event `session_id` must match the owning manifest session id.

## Event Kinds

Current stable event kinds:

| Kind | Purpose |
|---|---|
| `session.created` | Records session creation and optional workspace cwd |
| `session.cloned` | Records cloned-session provenance |
| `session.forked` | Records forked-session provenance |
| `session.compaction.started` | Starts manual/session compaction metadata |
| `session.compaction.completed` | Persists compaction summary metadata |
| `branch.summary.created` | Persists a summary of abandoned branch work for replay into later context |
| `operation.started` | Starts a mutating operation such as prompt, manual compaction, branch summary, export, plugin load |
| `operation.committed` | Commits an operation and may assign `new_leaf_id` |
| `operation.aborted` | Records an aborted operation |
| `operation.failed` | Records failed operation code and message |
| `turn.started` | Marks a prompt turn boundary |
| `turn.input.recorded` | Persists normalized user input content |
| `message.started` | Opens a message id/role boundary |
| `message.completed` | Persists final message content and optional finish reason |
| `message.cancelled` | Cancels an open message |
| `tool.call.started` | Persists tool call name and JSON arguments |
| `tool.call.updated` | Persists tool execution update text |
| `tool.call.completed` | Persists tool result |
| `tool.call.failed` | Persists tool failure message |
| `tool.call.cancelled` | Persists tool cancellation reason |
| `diagnostic.emitted` | Persists diagnostic level/message |
| `metadata.updated` | Persists arbitrary metadata value |
| `active_leaf.changed` | Explicit active-leaf marker when needed |

## Content and Tool Data

Persisted content blocks use tagged JSON:

```json
{ "type": "text", "data": { "text": "hello" } }
{ "type": "thinking", "data": { "thinking": "...", "thinking_signature": "sig", "redacted": false } }
{ "type": "image", "data": { "mime_type": "image/png", "data": "base64-or-reference" } }
```

Persisted roles are `user`, `assistant`, `tool`, and `system`.

Tool results use tagged JSON:

```json
{ "kind": "text", "data": { "text": "ok" } }
{ "kind": "json", "data": { "value": { "ok": true } } }
{ "kind": "error", "data": { "message": "failed" } }
```

## Replay Rules

`SessionLogStore` reads `session.json`, validates the manifest, reads `events.jsonl`, validates each event envelope, and folds events into a replay transcript.

Current product behavior depends on these invariants:

- Completed assistant/user/tool content is restored from committed events, not from live UI deltas.
- Successful persistent prompt operations commit a new active leaf through `operation.committed { new_leaf_id }` and update `session.json.active_leaf_id`.
- Replay records each committed prompt leaf with its parent leaf plus transcript start/end range. The parent is the active leaf at commit time.
- `active_leaf.changed` updates replay's active leaf and is used by tree view and branch-summary range selection to model same-session branch returns.
- Failed or aborted operations must not advance the active leaf.
- Fork/clone/tree/compact/branch-summary actions use `SessionService` replay and leaf metadata instead of legacy JSONL assumptions; `/branch-summary <source-leaf-id> <target-leaf-id> [instructions]` persists a provider summary for the selected abandoned leaf range through the configured runtime model.
- Fork/clone copies committed history through the selected leaf. If a later complete branch-summary operation targets that leaf, the copy includes the operation boundary plus `branch.summary.created` event so replay hydrates the summary without copying abandoned prompt history.
- Interactive `/tree` navigation that leaves a different active leaf first summarizes the abandoned active branch, then forks to the selected target leaf with the created summary available in the new session transcript.
- Product paths should reject or ignore legacy TypeScript JSONL rather than silently importing it into this schema.

## Versioning Guidance

- Increment `session.json.version` only for manifest-breaking changes.
- Increment event `version` only for envelope or event-data breaking changes.
- Prefer adding optional fields over changing existing field semantics.
- Keep event kind strings stable; adapter and replay code should not depend on concrete Flow node IDs.
