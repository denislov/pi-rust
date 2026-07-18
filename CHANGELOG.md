# Changes

## 0.4.0 - Unreleased

### Architecture And Planning

- Added the dependency-reviewed `0.4.x` release train and independent `0.4.0`
  through `0.4.5` release contracts.
- Split the monolithic architecture document into normative principles, runtime,
  extension-platform, dependency, and testing contracts, plus versioned current
  state, ADRs, and migration evidence.
- Accepted runtime-owner/finalization, `RecoveryPending`, extension grant/lease,
  state/fact, Workbench protocol, and isolated Wasm invocation decisions. Their
  disposable evidence now includes locked real-engine cancellation, epoch, fuel,
  memory/disposal, and TypeScript-to-WIT Component fixtures.

### Runtime Integrity

- Converged PluginLoad on the admitted operation identity and typed
  Completed/Failed/Aborted ProductEvent terminal evidence, with completion after
  capability generation installation.
- Completed `RIF-001` with immutable permit-owned `OperationExecution` identity,
  freezing descriptor revision, origin, capability/session association, and
  resolved root/parent lineage at admission. Durable PluginLoad,
  SelfHealingEdit, and BranchSummary transactions now reuse that admitted
  identity; Agent/Team contexts receive it at construction and obtain nested
  child IDs through the scheduler. Root, child, session-copy correlation, and
  recovery allocation are explicitly separated, while delegation approval facts
  reuse the admitted approval operation ID. Submission and terminal projection
  retain the admitted execution instead of detached identity/descriptor copies.
- Replaced duplicated internal operation metadata values with one exhaustive
  descriptor table and orthogonal validated claims for lineage, session/runtime
  access, priority, capacity, durability, cancellation, children, outcomes, and
  terminal policy.
- Completed `RIF-007` by making scheduler/capability admission consume the
  execution-owned descriptor directly, enforcing descriptor-declared structured
  children, and removing the transitional operation metadata registry.
- Moved external API compile fixtures and their Cargo targets under the project
  `target/api-boundary-fixtures/` tree.
- Began `RIF-008` by making `CodingAgentSession` a facade over one `RuntimeHost`
  with explicit OperationSupervisor, SessionCoordinator, EventHub, and
  ClientProjectionCoordinator ownership. Added the identity-bearing session
  writer command/reply protocol and routed default-profile, fork, active-leaf,
  tree-label, and delegation approval/rejection mutations through it.
- Removed raw SessionLogStore/SessionHandle authority from TurnTransaction;
  workflow-local staging now reaches persistence through typed checkpoint and
  finalize writer commands over a shared bounded transaction actor, preserving
  checkpoint and PartialCommit semantics while rejecting queue saturation and
  closed writers. Authorization request/resolution facts now use the same
  bounded actor instead of retaining raw repository handles. Last writer-handle
  drop and drained RuntimeHost shutdown close and join the actor before shutdown
  publication.
- Made the bounded writer own and refresh its mutable session handle after
  manifest commits. Tree labels, active-leaf changes, default-profile changes,
  delegation durable facts, and startup recovery now submit event-plus-manifest
  mutations through that writer; the live `SessionService` no longer appends or
  patches its own repository handle directly.
- Routed `SessionCreated` and copy/fork target payload installation through
  typed writer commands. Copy provenance, copied facts, and the target manifest
  now form one writer commit, and failed target writers are closed before
  session cleanup.
- Added a canonical-session writer registry with per-open owner leases. Separate
  `SessionService::open()` calls for one session reuse one actor; shutting down
  one owner leaves other owners usable, and the last owner closes and joins the
  actor. Closed actors are never reused by a later open.
- Added a writer-owned manifest snapshot for independent-open reads. Session
  view, tree, summary, active-leaf, and default-profile reads now observe
  successful mutations made by another open owner without relying on a stale
  local manifest handle.
- Removed SessionService handle replacement after writes. Repository handles are
  now read/path authority only; mutable manifest owner state remains in the
  writer, and deterministic tests cover independent-session concurrency while a
  different writer is blocked.
- Closed `RIF-D007`: the owner graph and boundary evidence now show that runtime,
  session, and event mutation authority no longer overlaps through mutable
  service handles.
- Closed `RIF-008`: the per-session writer owner, cross-open registry, immutable
  read authority, bounded pressure behavior, shutdown drain, independent-session
  concurrency, navigation projection ordering, and workspace release gates now
  pass. The next planned slice is `RIF-009` outbox/snapshot consistency.
- Completed `RIF-009-001`: added the typed `DurableOutboxRecord` contract with
  schema/version, semantic identity, session/operation correlation, record kind,
  and structured product draft payload. The retained EventService window remains
  explicitly process-local replay state, not durable outbox proof.
- Completed `RIF-009-002`: session manifests now provision `outbox.jsonl`, writer
  mutation batches accept typed outbox records, and prompt success/failure/abort
  plus non-leaf commit/failure paths write a `SessionWriteCommitted` draft
  correlated to source session event IDs.
  Outbox intent is written before session facts under one append lock, so a
  later fact failure returns `PartialCommit` while leaving restart-visible
  recovery evidence.
- Activated `RIF-009-003` for the committed projection/snapshot cursor. Durable
  operation-terminal and recovery records remain sequenced behind the
  supervisor state machine in `RIF-002`.
- Added the first `RIF-009-003` cursor slice: outbox schema v2 records the
  `committed_through_session_sequence`; transaction code passes an uncommitted
  candidate, and only the repository assigns its durable cursor after validating
  the candidate session and source event IDs against the sequenced fact batch.

### Release Status

- `RIF-001`, `RIF-005`, `RIF-006`, `RIF-007`, and `RIF-008` are complete.
  Workspace packages remain at `0.3.1` until all `0.4.0` implementation, debt,
  and release gates close.

## 0.3.1 - 2026-07-18

### Added

- Added explicit known/unknown model-cost state and propagated it through
  transcripts, product events, replay, and RPC session statistics.
- Added AWS standard credential-chain and official SigV4 support for Bedrock,
  including named profiles, session credentials, cancellation, and redaction.
- Added a deterministic cross-provider contract matrix for every built-in API.

### Changed

- Provider streams now fail closed on truncation, malformed terminals, timeout,
  cancellation, and provider-declared failure; all HTTP providers share one
  retry, hook, option-validation, cancellation, and deadline implementation.
- SSE and OpenAI Responses parsing now support chunk-safe protocol processing,
  multiple interleaved outputs, structured failures, and safe unknown events.
- Compatibility metadata now exposes explicit request/runtime versus
  catalog-only dispositions, and generated catalog records are validated.
- RPC advances additively to protocol `2.1`; ProductEvent advances additively
  to `2.2`; UI Snapshot remains `2.1`.

### Removed

- Removed the DTO-only standalone image-generation category from the stable
  `pi_ai::api` facade; multimodal conversation images remain supported.
- Removed unused Codex WebSocket helpers. Explicit WebSocket or unknown
  transport selection now fails before network I/O.

### Migration

- See `docs/0.3.1-migration-guide.md` for public API, terminal, cost, transport,
  compatibility, and Bedrock credential changes.

### Release Status

- `PAIR-001` through `PAIR-012` and all completion criteria in
  `docs/0.3.1-pi-ai-remediation-plan.md` are closed.
- All workspace packages use version `0.3.1`; formatting, strict Clippy, full
  workspace tests, and the `0.3.1` public API freeze pass.
- The offline tmux TUI smoke suite was rerun successfully on 2026-07-18; no live
  provider credentials were used.

## 0.3.0 - 2026-07-17

### Added

- Added model-visible delegation target inventories, uniform asynchronous
  operation cancellation, durable session-tree labels, executable plugin UI
  actions, effective configuration handling, and runtime-backed tool
  authorization.
- Added a full-screen interactive application shell with responsive context,
  tips, composer, status, transcript interaction, mouse support, and focused
  overlays while retaining the supported inline path.
- Added truthful runtime-backed RPC capabilities, commands, state, statistics,
  multimodal prompts, compaction, session forks, cancellation, and
  authorization controls.

### Runtime Contracts

- Tool authorization is capability-bounded, cancellation-aware, reconnectable,
  redacted, and durably audited; unresolved requests recover as interrupted
  rather than approved.
- Product events and UI snapshots now project operation, change, delegation,
  usage, cost, context, and pending-authorization state from runtime owners.
- RPC remains protocol family `2.0`; ProductEvent and UI Snapshot advance
  additively to protocol family `2.1`. The durable session writer remains
  version `1`.

### Release Status

- All tasks and execution-debt entries in `docs/0.3-plan.md` are complete and
  closed.
- All workspace packages use the `0.3.0` workspace version.

## 0.2.0 - 2026-07-16

### Changed

- Unified all workspace packages under the root workspace version policy.
- Completed the breaking architecture convergence release train.
- Added reproducible architecture, public API snapshot, compatibility, and
  release gates.

### Boundaries

- The root `pi-rust` binary remains a placeholder and does not own provider,
  agent-runtime, session, product, or terminal UI behavior.
- The user-facing executable remains `pi-coding-agent`.

### Release Artifacts

- RPC, ProductEvent, and UI Snapshot protocol families are version `2.0`.
- The durable session writer remains version `1`.
- Public API freeze manifests are stored under `docs/api-snapshots/`.
- The completed architecture migration and release evidence are summarized in
  `docs/0.2-architecture-convergence-record.md`.
