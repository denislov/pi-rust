# Current Architecture Evidence

## Evidence Stamp

Baseline version: `0.3.1`, released as annotated tag `v0.3.1`.

Source baseline: commit `870d4bb`; dated release record: `180f219`; post-baseline
`0.4.0` through `0.4.2` convergence evidence is recorded below. Last refreshed:
2026-07-20.

This file records implementation facts, not desired behavior. Cargo manifests,
compiled source, tests, and CodeGraph call paths outrank this summary when they
disagree. Every task that changes a listed fact must refresh the stamp and item.

## Workspace

- Active dependency edges are `pi-agent-core -> pi-ai` and
  `pi-coding-agent -> {pi-agent-core, pi-ai, pi-tui}`.
- `pi-ai` and `pi-tui` have no workspace dependencies.
- `pi-mom`, `pi-pods`, and `pi-web-ui` are placeholder crates.
- All workspace packages inherit version `0.4.2` from the root manifest.
- `pi-rust` is a placeholder binary; `pi-coding-agent` is user-facing.

## Runtime And Operations

- Production Agent turns and fixed product workflows use typed state/pipeline
  runners. The generic `Flow<C>` engine and compatibility wrappers are deleted.
- AgentTeam member execution is bounded to two structured child contexts and
  restores results in profile order.
- Persistent sessions disable ephemeral core automatic compaction and use the
  durable ManualCompaction transaction/outbox path.
- `CodingAgentSession` is a facade over one `RuntimeHost` composition root.
  Admission/capability authority resides in `OperationSupervisor`, session state
  in `SessionCoordinator`, product-event fan-out in `EventHub`, and client
  snapshots/controls in `ClientProjectionCoordinator`. An identity-bearing
  `SessionWriterCommand`/`SessionWriterReply` protocol owns default-profile,
  fork, active-leaf, and tree-label mutation; fork installation replaces
  persistence and replay-derived pending/recovery owner state as one coordinator
  action. Delegation approval/rejection durable facts and pending-queue changes
  also share one writer action, while child execution and EventHub publication
  occur after the writer reply. `TurnTransaction` no longer owns raw store/handle
  fields; checkpoint and finalization use typed `SessionTransactionWriter`
  commands while staged events remain workflow-local. Transactions created by
  one `SessionService` share a bounded writer actor and reject queue saturation.
  Authorization request/resolution facts use that same actor, and their scoped
  writer carries only the session identity plus the writer port. Last
  writer-handle drop closes the sender and joins the actor; closed writers
  reject commands. RuntimeHost shutdown drains active operations, closes/joins
  the writer, and only then publishes shutdown. The actor now owns a mutable
  session handle and refreshes it after manifest commits. Tree-label,
  active-leaf, default-profile, delegation-fact, and startup-recovery mutations
  use event-plus-manifest writer commands. `SessionCreated` uses a guarded
  initialize command, while copy/fork provenance, copied facts, and target
  manifest installation form one target-writer commit. `SessionService` no
  longer appends or patches repository handles directly. Independently opened
  same-session coordination now reuses a canonical-path writer actor through
  per-open owner leases: one RuntimeHost can release its lease without closing
  another open owner, while the final owner closes and joins the actor. Closed
  actors are excluded from later registry acquisition. The writer-owned
  manifest snapshot feeds session view, tree, summary, active-leaf, and
  default-profile reads across independent opens. Navigation refresh ordering,
  writer pressure, independent-session concurrency, startup recovery, and
  workspace gates pass; the runtime/session owner slice `RIF-008` is complete.
  Durable outbox, reconnect, and snapshot-consistency work belongs to the
  subsequent `RIF-009` plan.
  `SessionService` no longer replaces its repository handle after writes, so
  that handle is read/path authority only; deterministic coverage also proves
  one session writer can progress while another session writer is blocked.
- `IntentRouter`, `OperationScheduler`, `OperationControl`, typed operation
  metadata, root/child lineage, capability snapshots, and generation-scoped
  cancellation exist.
- Admission now freezes an internal `OperationExecution`, and operation permits
  retain that immutable identity. Root executions carry descriptor revision,
  origin, capability generation, admitted session identity, and root lineage;
  child executions retain resolved parent/root lineage. PluginLoad,
  SelfHealingEdit, and BranchSummary durable transactions consume the admitted
  snapshot identity; Agent/Team contexts receive that identity at construction,
  and scheduler-owned allocation supplies nested child IDs. Root, child,
  session-copy correlation, and recovery allocators are now explicit, and
  delegation approval facts reuse their admitted approval operation identity.
  Submission commit, terminal association, outcome acknowledgement, and drop
  cleanup retain the admitted execution as one value. Allocator ownership and
  dispatcher boundary tests close `RIF-001` and `RIF-D001`.
- The 16 public operation variants now share one exhaustive descriptor table.
  Internal operation payloads map to its contract keys, and internal metadata,
  capability session access, admission class, and dispatch mode are derived
  projections. Orthogonal lineage, session/runtime access, priority, capacity,
  durability, cancellation, child, outcome, and terminal claims are validated;
  scheduler and capability admission consume the descriptor directly, and only
  descriptors declaring structured children enter child admission. The former
  `OperationMetadata` projection has been deleted; `RIF-007` and `RIF-D006` are
  complete.
- SessionWriteRoot, NonSessionRoot, RuntimeWrite, Query, ReadOnly, Child, and
  Control admission classes exist; the scheduler has no general work queue.
- AgentInvocation and AgentTeam have runtime-owned submitted task paths. The
  unreachable legacy PluginCommand operation and protocol/presentation surface
  are absent; other operations still rely on the session facade as execution
  owner.
- `OperationSupervisor` now freezes immutable typed finalization decisions from
  the admitted execution and typed outcome across all four dispatch paths.
  Submission projection validates that decision instead of independently
  classifying a naked terminal status. Typed Prompt/Compact/BranchSummary
  failures are now classified as Failed rather than Completed. SessionCoordinator
  resolves durable evidence to `Committed`/`DefinitelyFailed`/`InDoubt`;
  partial commits with evidence enter non-terminal `RecoveryPending`, and
  EventHub emits typed `OperationRecoveryPending` evidence without terminal
  fields. Missing evidence preserves the original `PartialCommit`; bounded
  retry and authenticated remote control are implemented and audited.
  The public session facade now exposes a read-only `recovery_pending()` list
  with stable recovery IDs and persisted operation-kind hints. Its trusted-host
  `resolve_recovery()` control accepts only Failed/Aborted and rejects stale
  record/descriptor/capability evidence. Startup scan atomically persists a non-terminal
  `operation.recovery_pending` fact plus `Recovery` outbox record, keeps the
  replayed operation `InDoubt`, and reuses the same session/operation-derived
  recovery ID and outbox record across repeated opens. Recovery evidence v1
  carries record version, admitted descriptor revision, and capability
  generation through durable facts/outbox, public inspection/events, and the
  JSON/RPC protocol; legacy pending facts deserialize as v1. Successful local
  resolution writes a redacted audit fact, Failed/Aborted fact, terminal marker,
  and `OperationTerminal` outbox atomically, publishes only after commit, and
  redelivers the original family terminal after restart.
- Commit uncertainty is represented by `PartialCommit`, and durable
  `RecoveryPending` now survives caller exit/restart. Trusted-host
  `retry_recovery()` performs a durable, non-terminal facts/outbox inspection
  attempt with a three-attempt cap and restart-visible timestamps; it never
  reruns an external side effect. Scheduled retries use deterministic `+1s`,
  `+2s`, and `+4s` timestamps; session startup executes one elapsed retry
  inspection atomically and preserves non-terminal ownership. Authenticated RPC
  control/audit identity now covers authenticated inspect/retry/resolve RPC
  commands; resolve persists `rpc_token` as the audit authority. New `SessionWriteRoot` admission is
  fail-closed for unresolved recovery in the affected persistent session;
  Query/ReadOnly paths and explicit recovery controls remain available.
- AgentInvocation and AgentTeam definite failures now publish their root
  terminal drafts after finalization; child PromptFailed events no longer
  replace the root failure lifecycle evidence.
- Runtime-owned non-session submissions now finalize and publish terminal drafts
  independently of client submission leases, covering RPC background execution.
- Runtime-owned cancellation and drop paths retain the admitted operation
  identity through terminal projection, closing the post-admission ownership
  bypass tracked by `RIF-D002`.
- Client operation projection now fails closed for unknown local events; a new
  running root requires explicit admission/root evidence or terminal evidence.
- Durable session commit tests cross-check the final event cursor, replay cursor,
  manifest snapshot, and outbox cursor, including manifest-failure reopen and
  startup redelivery evidence.
- BranchSummary and SelfHealingEdit now have an explicit descriptor-level
  lifecycle matrix: outcome acknowledgement for branch summaries, and
  ProductEvent terminal evidence for self-healing edits.
- The 0.4.0 runtime baseline harness records provisional offline limits and
  current measurements for admission, writer pressure, session/outbox commit,
  snapshot/reconnect, and recovery scan under `target/perf-baseline/`.

## Events, Sessions, And Clients

- `SessionEventEnvelope`, transaction, append/replay, operation terminal facts,
  recovery markers, manifests, and snapshots exist.
- `CodingAgentProductEvent` is the typed client event envelope; `EventService`
  sequences and broadcasts it through a bounded retained stream.
- Snapshot/reconnect, stream identity, sequence gaps, capability generation,
  client projection, print/JSON, RPC, and interactive adapters exist.
- A durable ProductEvent outbox now shares the bounded writer commit point with
  its source SessionEvents. Session-write, startup-recovery, and the first
  Prompt, Compact, PluginLoad, and SelfHealingEdit `OperationTerminal` records
  are implemented; remaining supervisor-owned operation-terminal records remain
  open under `RIF-002`.
  Standalone AgentInvocation and AgentTeam terminal ProductEvents now publish
  after finalization; delegated child publication remains child-admission owned.
- The retained broadcast window is live delivery/replay state, not durable
  evidence. `RIF-009-001` must define an outbox record and semantic identity
  before a writer batch can claim session/outbox atomicity. The record contract
  now exists in `events::outbox`. Completed `RIF-009-002` added `outbox.jsonl`, a
  typed writer batch, outbox-first append ordering under the session append lock,
  source event correlation, and session-write outbox persistence for prompt
  success/failure/abort and non-leaf commit/failure paths. Remaining
  operation-terminal publication stays open under `RIF-002`; recovery
  publication now persists
  `Recovery` outbox records atomically with non-terminal
  `OperationRecoveryPending` facts. Offline restart/reconnect/redelivery
  reconciliation is complete under `RIF-009-004`; provider/live stress is a
  later hardening concern.
  Prompt terminal publication now occurs only after the terminal fact/outbox
  commit succeeds. Compact success/failure, PluginLoad success/failure/abort,
  and SelfHealingEdit success/failure/abort now use the same ordering and an
  outbox operation-kind hint for correct restart redelivery. BranchSummary is
  intentionally outcome-acknowledged. Standalone AgentInvocation/AgentTeam
  are non-session ProductEvent terminals and do not create session outbox rows.
  `RIF-009-004` owns the completed offline
  restart/reconnect/redelivery consistency slice. The completed cursor work:
  outbox schema v2 stores `committed_through_session_sequence`, while only the
  repository may turn a validated candidate into a cursor-bearing durable
  record. `SessionReplay` now derives the same cursor from sequenced facts, and
  public snapshots expose `last_session_sequence`. Completed `RIF-009-003`
  returns a typed `SessionCommitReceipt` from the bounded writer, retains that
  cursor monotonically in SessionService, and installs projection state from the
  receipt-derived cursor without replaying. `RIF-009-004` completed the recovery
  and redelivery matrix; session open rejects malformed, regressed, or duplicate
  outbox records before runtime startup, and EventService redelivers startup
  records once per runtime by semantic record ID. The first matrix slice covers
  duplicate suppression, retained gaps, replay ordering, and reconnect from the
  current cursor. A manifest-failure -> writer shutdown -> reopen integration
  test proves durable outbox evidence survives the restart boundary. Startup
  recovery idempotence coverage also verifies the recovery record is read back
  on subsequent opens, references the exact pending fact event, and never marks
  the operation terminal merely because the process restarted.
- The current transaction may append facts and then fail a manifest refresh,
  producing partial-commit uncertainty that startup recovery can inspect.

## Agent, Workflows, And Extensions

- The production Agent turn uses `AgentTurnRunner` with exhaustive typed states
  and a dedicated decision enum. Fixed product workflows use operation-specific
  runners behind `WorkflowService`; the generic graph engine is absent.
- The legacy Rust contribution-provider registry and Lua/`mlua` runtime are
  deleted. The replacement path has candidate
  Manifest/WIT/schema contracts, immutable packages, grant-backed activation,
  lease-only Host handles, an offline TypeScript Component harness, and the
  accepted core/extension handler target boundary, and a minimum production
  Wasmtime Component invocation path with a real lease-backed Host call.
  Contribution productization is Skipped; no PluginCommand/UI compatibility
  surface, Lua/native compatibility runtime, or automated migration layer is
  retained.
- PluginLoad uses the admitted snapshot operation ID and typed
  Completed/Failed/Aborted root terminal evidence. Its terminal draft now
  persists through the coordinator outbox and publishes only after commit.
- SelfHealingEdit terminal drafts persist through the same coordinator path;
  cancellation observed after runner success now records a failed/aborted
  transaction instead of losing the operation transaction.
- Workbench semantic views, extension state/facts, package update coordination,
  and background extension services do not exist.
- `tools/architecture-prototypes/runtime-contracts.mjs` is decision evidence for
  capability generation, state/fact boundaries, per-invocation memory isolation,
  and Workbench revision/resync. Locked standalone Wasmtime and TypeScript/Jco
  fixtures add real-engine interruption/limit/disposal and typed WIT Component
  evidence for accepted `ADR-003`. Wasmtime is now a pinned production runtime
  dependency for the reduced minimum Component vertical slice.

## Evidence Maintenance

The archived detailed `0.3.1` inventory is preserved in
[`migrations/0.3.1-monolithic-architecture.md`](migrations/0.3.1-monolithic-architecture.md).
It remains useful historical evidence but is no longer the normative contract.
