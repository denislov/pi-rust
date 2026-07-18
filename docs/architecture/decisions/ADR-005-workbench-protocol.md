# ADR-005: Workbench Semantic View Protocol

- Status: Accepted
- Date: 2026-07-18
- Owner: `0.4.0` `RIF-006`
- Implementation/final schema freeze: `0.4.4` `WAP-001`, `WAP-002`, `WAP-007`
- Prototype: `tools/architecture-prototypes/runtime-contracts.mjs`

## Context

Extensions need portable application-like views across TUI, RPC, and embedding
without access to raw `pi-tui` widgets, HTML/WebView, CSS, a virtual DOM, or an
arbitrary JavaScript UI runtime. Large views and multiple clients require bounded
incremental updates, deterministic gap handling, and strict separation of shared
business data from transient client UI state.

## Decision

Workbench is a host-rendered semantic retained tree:

```text
ViewSnapshot(view_instance_id, revision, root)
ViewPatch(view_instance_id, base_revision, typed operations)
```

Every client open creates a distinct ViewInstance and revision sequence. Nodes
have stable IDs and closed semantic types. The host validates schemas,
references, depth, node/text/row/log counts, actions, paging, patch rate, and
encoded size. It owns the accepted revision.

Patches use closed insert/remove/replace/update/replace-children operations. A
wrong view instance, stale base revision, retention gap, or invalid reference
causes `ViewResyncRequired` and a fresh snapshot; the host never heuristically
merges stale patches.

Lists, trees, tables, diffs, and logs use cursor/page/lazy loading,
virtualization, and bounded buffers. Every action/fetch/callback is an admitted
operation with a lease.

Focus, selection, scroll, viewport, expansion, menu highlight, IME, and
unsubmitted form input are client-local. Shared invalidation schedules separate
refreshes for each client; patches are not copied between ViewInstances.

TUI, RPC, and embedding are peer renderers of the same protocol. Extensions do
not receive adapter component types.

## Alternatives Rejected

- raw TUI component/widget API;
- HTML/WebView/CSS;
- virtual DOM or arbitrary JS UI runtime;
- server-durable focus/scroll/form drafts;
- unbounded full-list snapshots and patch queues;
- stale-patch merge or last-writer-wins revision repair.

## Prototype Evidence

The offline prototype defines materially different Review and Incident view
trees, applies a matching revision patch, rejects a stale revision with explicit
resync, and proves two clients retain independent transient state. Concrete
schemas remain candidates until `0.4.4` conformance applications pass.

## Verification

Shared fixtures across TUI and RPC; snapshot/patch revision and reference
matrices; gap/resync/reconnect; paging/virtualization/pressure; action admission
and revoke; multi-client local-state isolation; restart; and two materially
different conformance applications before final schema freeze.
