# Changes

## 0.4.1 - Unreleased

### Agent And Workflow Convergence

- Started the 0.4.1 workflow convergence plan and completed `AWC-001` active
  cancellation semantics. Provider streams, provider/context hooks, sequential
  and parallel tools, and tool hooks now have host-enforced cancellation waits.
- Added an active-tool cancellation regression proving a cancelled tool wait
  returns promptly even when the tool emits no progress updates.
- Accepted `ADR-013` and recorded the complete Flow inventory. Generic Flow is
  now explicitly non-durable and retained only for tests/compatibility and the
  temporary AgentTurn migration scaffold; fixed product workflows are assigned
  to typed pipelines or structured concurrency.
- Started `AWC-005` correctness convergence: concurrent Agent admission now
  returns typed `AgentAdmissionError` values without panicking, empty ToolUse
  terminals fail deterministically, runtime/queue/turn message IDs avoid
  collisions, and Unicode resource limits count characters rather than bytes.

## 0.4.0 - 2026-07-19

Workspace packages are versioned `0.4.0`; API/protocol snapshots and the offline
release gates passed on 2026-07-19.

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

- Fixed definite-failure terminal publication for AgentInvocation and AgentTeam:
  `DefinitelyFailed` results now retain and publish their root terminal draft,
  matching committed-success publication and preventing child-only failure
  projections.
- Fixed runtime-owned RPC submission finalization when no submission lease is
  present. Background AgentInvocation/AgentTeam operations now always freeze
  their terminal decision and publish the deferred root draft after finalization.
- Closed the post-admission exit debt for runtime-owned roots: success, definite
  failure, and cancellation now all retain supervisor-owned terminal ownership
  through lease-free submission and operation-identity cancellation paths.
- Closed projection root-invention debt: unknown local events no longer create
  running operation roots without admitted kind, terminal evidence, or explicit
  root identity.
- Closed durable publication/snapshot consistency debt: session commit, outbox
  cursor, manifest-failure reopen, and startup redelivery now have explicit
  cross-checks in the session matrix.
- Closed the BranchSummary/SelfHealingEdit lifecycle debt. BranchSummary is
  explicitly outcome-acknowledged without a ProductEvent terminal, while
  SelfHealingEdit requires ProductEvent Completed/Failed/Aborted root evidence.
- Added the reproducible offline `scripts/runtime-baseline.sh` harness and
  `docs/architecture/runtime-baseline-0.4.0.md` evidence for admission, writer
  pressure, session/outbox commit, snapshot/reconnect, and recovery scan paths.
- Refreshed the 0.4.x roadmap status: all current 0.4.0 implementation/debt
  rows, offline reconnect/lag matrices, API snapshots, and release gates are
  closed. Provider/live reconnect stress is deferred to later hardening.
- Corrected `scripts/release-gates.sh` defaults to the `0.4.0` workspace and API
  snapshot baseline; the script no longer points at retired pre-train defaults.

- Added a typed `RecoveryPending` admission rejection. `SessionWriteRoot`
  operations now inspect durable recovery evidence before
  `OperationScheduler::admit` and fail closed for the affected session, while
  read-only/query and explicit recovery-control paths remain usable.
- Added authenticated RPC `recovery_inspect` and `recovery_retry` commands;
  both require `PI_RUST_RPC_AUTH_TOKEN`, and retry honors the existing
  idempotency ledger while preserving durable evidence checks.
- Added authenticated RPC `recovery_resolve`; its durable recovery audit fact
  records `rpc_token` as the authorization subject while trusted-host Rust
  resolution records `trusted_host`.

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
- `SessionReplay` now derives and carries the last committed session fact cursor;
  public snapshot cursors expose `last_session_sequence`. Atomic writer to
  read-model handoff is now implemented through typed `SessionCommitReceipt`
  responses and a monotonic SessionService cursor cache; projection refresh no
  longer replays the session log. `RIF-009-003` is complete and `RIF-009-004`
  recovery/redelivery matrix work is active.
- Began the `RIF-009-004` restart slice: opening a session now validates durable
  outbox schema/version, session identity, monotonic commit cursors, source
  event IDs, and duplicate semantic identities before runtime startup.
- Added runtime-local idempotent outbox redelivery: startup records are emitted
  through EventService after `SessionOpened`, and duplicate record IDs are
  suppressed within the runtime.
- Added the first `RIF-009-004` matrix test covering duplicate suppression,
  retained-gap recovery, replay-through ordering, and current-cursor reconnect.
- Added a full restart failure case: manifest failure after outbox/fact append,
  writer shutdown, reopen, replay cursor validation, and startup outbox evidence
  for redelivery.
- Startup scan now commits a durable non-terminal `OperationRecoveryPending`
  fact and `Recovery` outbox record in the same writer batch. Recovery IDs are
  stable per session/operation, replay remains `InDoubt`, and repeated opens
  redeliver the same evidence without duplicating facts or records instead of
  speculatively marking incomplete work recovered. Recovery evidence v1 carries
  record version, descriptor revision, and capability generation through the
  durable fact/outbox, public Rust API/events, and JSON/RPC protocol, with
  defaults for pending facts written before these fields were introduced.
- Added trusted-host `resolve_recovery()` control for durable pending operations.
  Requests must match the recovery/operation identity, record version,
  descriptor revision, and capability generation, and may resolve only to
  Failed or Aborted. The bounded reason is secret-redacted before the writer
  atomically commits the audit fact, terminal fact, terminal marker, and
  `OperationTerminal` outbox; EventHub publishes only after commit and restart
  redelivery preserves the original operation family and terminal status.
- Added bounded trusted-host `retry_recovery()` inspection attempts. Each retry
  appends a pending snapshot and Recovery outbox record with durable attempt
  count and timestamps, caps attempts at three, and never reruns an external
  side effect. `with_backoff()` records deterministic `+1s`, `+2s`, and `+4s`
  next-attempt timestamps. Session startup executes one elapsed inspection
  attempt atomically, preserves the three-attempt cap, and never reruns external
  side effects.
- Began `RIF-002`: `OperationSupervisor` now owns typed immutable
  `FinalizationDecision` creation for every dispatch path. Decisions freeze the
  admitted identity, lineage, descriptor, capability generation, terminal
  policy/class, semantic event ID, and safe payload; submission projection
  validates the decision instead of classifying a detached status. Typed
  Prompt/Compact/BranchSummary failure outcomes now correctly resolve Failed
  rather than Completed.
- Partial commit paths with durable fact/outbox evidence now resolve to a stable
  `InDoubt(recovery_id)` result and project non-terminal `RecoveryPending`
  status. EventHub and protocol adapters emit `OperationRecoveryPending` with
  no `terminal_status` or `terminal_operation`; when evidence is absent, the
  original `PartialCommit` error is preserved and the handoff fails closed.
- Added the first normal terminal persistence slice for Prompt: the coordinator
  writes an `operation.terminal.recorded` SessionEvent and `OperationTerminal`
  outbox draft atomically, publishes the Prompt terminal only after that commit,
  and restart redelivery reconstructs the Prompt root terminal metadata.
  Compact success/failure now use the same terminal fact/outbox batch, publish
  only after commit, and reconstruct `Compact` terminal metadata after restart.
  Operation-terminal outbox records carry an optional operation-kind recovery
  hint so shared payloads retain their admitted family. PluginLoad
  success/failure/abort now use the same coordinator path and restart as typed
  PluginLoad terminals. SelfHealingEdit success/failure/abort now follow the
  same commit-before-publication and typed restart path; cancellation arriving
  after Flow success no longer drops its session transaction. BranchSummary is
  intentionally outcome-acknowledged, while SelfHealingEdit uses typed terminal
  evidence. Standalone
  AgentInvocation/AgentTeam roots now publish terminal ProductEvents after
  supervisor finalization while delegated child ownership remains explicit.
  Added a read-only `recovery_pending()` session inspection surface that reports
  stable recovery IDs and persisted operation kinds without synthesizing a
  terminal outcome. Authenticated inspect/retry/resolve operator controls now
  persist evidence and audit authority.

### Release Status

- `RIF-001`, `RIF-003`, `RIF-005`, `RIF-006`, `RIF-007`, `RIF-008`, and
  `RIF-010` are complete. `RIF-004` and `RIF-009` remain in progress.
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
