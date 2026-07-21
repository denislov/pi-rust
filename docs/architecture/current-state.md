# Current Architecture Evidence

## Evidence Stamp

Baseline version: `0.3.1`, released as annotated tag `v0.3.1`.

Source baseline: commit `870d4bb`; dated release record: `180f219`; post-baseline
`0.4.0` through completed `0.5.3` convergence evidence is recorded below. Last
refreshed: 2026-07-21.

This file records implementation facts, not desired behavior. Cargo manifests,
compiled source, tests, and CodeGraph call paths outrank this summary when they
disagree. Every task that changes a listed fact must refresh the stamp and item.

## Workspace

- Active dependency edges are `pi-agent-core -> pi-ai` and
  `pi-coding-agent -> {pi-agent-core, pi-ai, pi-tui}`.
- `pi-ai` and `pi-tui` have no workspace dependencies.
- `pi-mom`, `pi-pods`, and `pi-web-ui` are placeholder crates.
- All workspace packages inherit version `0.5.3` from the root manifest.
- The reduced 0.4.x train ends at `0.4.2`; reserved Extension release plans
  `0.4.3` through `0.4.5` are Skip records and did not produce package versions.
- `pi-rust` is a placeholder binary; `pi-coding-agent` is user-facing.

## AI Provider Runtime

- `pi-ai` registers eight scoped built-in APIs. The Amazon Bedrock provider,
  AWS credential/SigV4 implementation, dependencies, authentication/options,
  and 90 generated catalog records are deleted in 0.5.0.
- The generated catalog contains 831 retained records. Source guards reject the
  retired provider/API identities across registration, auth, transport, and
  catalog implementation files.
- Retained provider wire parsing remains provider-owned. Private shared SSE
  mechanics own exactly-once terminal construction; OpenAI Completions and
  Mistral also share start emission, tool-argument assembly, and required
  terminal-marker validation.
- The test-only OpenRouter image-generation DTO/mapper is deleted. Multimodal
  conversation image input remains part of the provider-neutral content model.
- `pi_ai::api` remains the stable embedding facade. Its item-level 0.5.0 audit
  is recorded in `docs/0.5.0-migration-guide.md`.

## Runtime And Operations

- Production Agent turns and fixed product workflows use typed state/pipeline
  runners. The generic `Flow<C>` engine and compatibility wrappers are deleted.
- The zero-state `WorkflowService` is deleted. Fixed operations invoke their
  dedicated typed runners directly; only PromptTurn retains its dynamic
  provider/tool/control loop. Operation metadata, public/internal outcome
  projection, terminal evidence, and adapter extraction are derived from the
  exhaustive operation contract rather than repeated adapter matches.
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
  workspace gates pass; the runtime/session owner slice `RIF-008` and durable
  outbox/reconnect/snapshot-consistency slice `RIF-009` are complete.
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
- Print, JSON, RPC, and interactive adapters consume the same public
  ProductEvent/snapshot contract. The interactive adapter owns one exhaustive
  pending-command slot, one ordered `UiProjection`, and a separate
  `InteractiveLocalState`; it no longer mirrors shared projection facts across
  the event loop and root component.
- The fullscreen adapter owns one long-lived client connection generation and
  consumes the canonical snapshot/replay/live/ack path. Prompt tasks hand off
  that connection once and no longer create a parallel ProductEvent bridge.
  Input and task/control queues are bounded, running/idle modes share one typed
  select loop, and Unix resize delivery is signal-driven with a quiet fallback.
- Fullscreen transcript rendering uses cached cumulative row metadata to locate
  viewport-intersecting blocks before cloning lines. Unchanged 1,000- and
  10,000-block frames touch five visible blocks and write zero bytes in the
  frozen 24-row baseline.
- A durable ProductEvent outbox now shares the bounded writer commit point with
  its source SessionEvents. Prompt, Compact, PluginLoad, and SelfHealingEdit
  terminal records persist and publish only after the corresponding commit.
  BranchSummary is intentionally outcome-acknowledged. Standalone
  AgentInvocation and AgentTeam terminal ProductEvents publish after
  finalization without creating session outbox rows; delegated child
  publication remains child-admission owned. The supervisor-owned terminal and
  recovery state machine is complete under `RIF-002`.
- The retained broadcast window is live delivery/replay state, not durable
  evidence. The `events::outbox` record has a typed semantic identity, and the
  writer persists outbox intent before session facts under one append lock so a
  later fact failure remains restart-visible uncertainty evidence. Recovery
  publication persists `Recovery` outbox records atomically with non-terminal
  `OperationRecoveryPending` facts. Outbox schema v2 stores
  `committed_through_session_sequence`; only the repository may turn a validated
  candidate into a cursor-bearing durable record. `SessionReplay` derives the
  same cursor from sequenced facts, public snapshots expose
  `last_session_sequence`, and the bounded writer returns a typed
  `SessionCommitReceipt` that SessionService retains monotonically without a
  replay refresh. Session open rejects malformed, regressed, or duplicate
  outbox records before runtime startup, and EventService redelivers startup
  records once per runtime by semantic record ID. Offline duplicate, retained
  gap, replay ordering, reconnect, manifest-failure/reopen, startup recovery,
  and redelivery matrices are complete under `RIF-009`. Provider/live pressure
  and reconnect stress remain a later hardening concern.
- The current transaction may append facts and then fail a manifest refresh,
  producing partial-commit uncertainty that startup recovery can inspect.

## Agent, Workflows, And Extensions

- The production Agent turn uses `AgentTurnRunner` with exhaustive typed states
  and a dedicated decision enum. Per-state transition functions enumerate every
  decision variant, so new variants require a compile-time transition review.
  Agent events have one stream owner, turn write-back consumes messages and
  queues, and live steering/follow-up insertion remains preserved. The core
  Branch Summary, Session Context/Memory, and Harness/Proxy alternatives are
  deleted; product BranchSummary remains owned by `pi-coding-agent`. Fixed
  product workflows invoke operation-specific runners directly; the generic
  graph engine and zero-state workflow pass-through service are absent.
- The legacy Rust contribution-provider registry and Lua/`mlua` runtime are
  deleted. The replacement path has candidate
  Manifest/WIT/schema contracts, immutable packages, grant-backed activation,
  lease-only Host handles, an offline TypeScript Component harness, and the
  accepted core/extension handler target boundary, and a minimum production
  Wasmtime Component invocation path with a real lease-backed Host call.
  Contribution productization is Skipped; no PluginCommand/UI compatibility
  surface, Lua/native compatibility runtime, or automated migration layer is
  retained.
- The 0.5.2 retained-contract gate passes all 33 Extension unit contracts,
  TypeScript/WIT conformance, the production Wasm vertical slice (182.23 second
  debug cold path), the four-case Wasmtime isolation/resource prototype, public
  trusted-host reload, and durable PluginLoad restart/outbox coverage.
  Wasmtime remains pinned to `46.0.1`; skipped contribution productization is
  not claimed.
- The 0.5.3 dead-code audit retains only the reasoned ADR-002 Extension runtime
  scopes and core handler boundary in default production. Provider wire fields
  are either validated/consumed or removed; the complete source inventory has
  23 reasoned occurrences and zero unreasoned exceptions.
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

The supported `pi_coding_agent::api` facade now contains runtime, operation,
event, client, required view, protocol, Extension, authorization, and
high-level CLI runner contracts. Low-level CLI parser/config/input/resource/
theme implementation categories are private; migration details are recorded in
`docs/0.5.3-migration-guide.md` and the 0.5.3 API snapshot.

The archived detailed `0.3.1` inventory is preserved in
[`migrations/0.3.1-monolithic-architecture.md`](migrations/0.3.1-monolithic-architecture.md).
It remains useful historical evidence but is no longer the normative contract.
