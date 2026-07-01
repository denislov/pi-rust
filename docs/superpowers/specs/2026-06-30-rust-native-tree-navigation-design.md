# Rust-Native Tree Navigation Design

## Context

Phase 3 already opens Rust-native sessions in interactive mode, tracks committed
`active_leaf_id`, supports read-only `/tree` projection, and can clone/fork
Rust-native sessions from committed leaves through `SessionService`.

The remaining gap was selector confirmation. Rust-native `/tree` could show a
projection, but choosing an entry still reported that navigation was not
implemented. The projection ids were temporary `rust_native_entry_N` values, so
accepting them as branch targets would have mixed UI-only ids with durable
session leaf ids.

## Goals

- Build `/tree` from real committed Rust-native `leaf_id` values.
- Keep tree construction owned by `SessionService`/`CodingAgentSession`, not the
  interactive UI.
- Let interactive confirmation select a committed leaf.
- If the selected leaf is the current active leaf, report that the session is
  already at that point.
- If the selected leaf is historical, create an independent Rust-native fork at
  that leaf and select the new session.
- Preserve legacy JSONL tree navigation behavior.

## Non-Goals

- Do not implement an in-place branch switch inside the same Rust-native
  session.
- Do not implement a full branch DAG in this slice.
- Do not make old projection ids valid targets.
- Do not add label persistence for Rust-native tree nodes.

## Design

`SessionService::tree_view` reads the Rust-native event log and creates a UI tree
from committed prompt operations:

- each `operation.committed { new_leaf_id: Some(...) }` for a prompt operation
  becomes one tree node;
- the node id is the real committed `leaf_id`;
- node text comes from the operation's recorded prompt input;
- prompt leaves are currently linked linearly in event order because the session
  log does not yet store an in-session branch DAG.

Interactive `/tree` asks `CodingAgentSession::tree_view` for the tree. It no
longer derives navigation ids from hydrated transcript projection rows.

When the selector confirms a Rust-native node:

- current leaf: show `Already at this point`;
- historical leaf: call `CodingAgentSession::fork_session(..., Some(leaf_id))`,
  apply the returned hydrated session, and show `Navigated to selected point`;
- invalid leaf: surface the typed session error as a tree navigation failure.

This keeps navigation semantically safe: selecting history creates a new
independent session instead of mutating active leaf metadata while replay remains
linear.

## Tests

Focused coverage should prove:

- `SessionService::tree_view` uses committed leaf ids as tree node ids;
- interactive `/tree` can select a historical Rust-native leaf;
- selecting that leaf creates a new Rust-native fork with `session.forked`;
- the forked session contains history up to the selected leaf and does not copy
  later source prompts;
- legacy JSONL tree behavior remains unchanged.

## Follow-Up

A later full branch-navigation slice can add durable parent/branch metadata and
support in-place branch switching. Until then, Rust-native tree navigation uses
fork-on-select for historical leaves.
