# Operation Runtime Reference Architecture

## Status And Scope

This document records the next simplification target for `pi-rust` as of 2026-07-07. It is a reference architecture, not a step-by-step implementation plan.

The current project already has a Flow-centered runtime foundation. This document does not describe a greenfield migration. It describes how the existing `CodingAgentSession`, Flow services, Rust-native session log, product event stream, plugin capability surface, and adapter convergence should be narrowed into one operation runtime contract.

This document is normative where it says "must", "should", "target", "invariant", or "contract". Code snippets are conceptual and exist to clarify boundaries. If a future plan needs exact Rust signatures, it should derive them from the contracts below and from the current code, not from incidental snippet names.

The intended result is:

```text
Intent or Operation in
Typed ProductEvent stream and snapshots out
Flow graphs orchestrate work
Operation contexts carry temporary state
Services own side effects
SessionEvent log persists durable facts
Adapters project events into UI/protocol state
```

Short form:

```text
ClientIntent -> IntentRouter -> OperationRuntime -> Flow -> Context -> Services
             -> SessionEvent/ProductEvent -> Snapshot/Projection -> Adapter
```

## Current Baseline

The project state matters because this architecture is a refinement of working pieces, not a replacement of absent pieces.

Already in place:

```text
CodingAgentSession
  current product owner and public API surface

PromptTurnFlow
  product-level prompt orchestration in pi-coding-agent

AgentTurnFlow
  low-level agent loop graph in pi-agent-core

FlowService
  runner/factory boundary for prompt, compaction, export, plugin load,
  branch summary, agent/team invocation, and self-healing edit flows

SessionService and Rust-native session_log
  session manifest, typed SessionEventEnvelope, TurnTransaction,
  replay/fold, fork/clone/tree/export support

CodingAgentEvent
  current canonical product event stream for print, JSON, RPC,
  and interactive adapters

CapabilityService and PluginService
  current capability reporting, plugin tool/command/hook/UI/keybind/dialog
  collection, Lua host metadata, and guarded plugin execution surfaces

Advanced workflow slices
  manual compaction, branch summary, plugin reload, one-off agent/team
  invocation, delegation, pending delegation confirmation, and self-healing edit
```

Still not in the final shape:

```text
CodingAgentSession has a broad method set.
Runtime-affecting commands are not all admitted through one IntentRouter.
Operation classes are implicit and scattered across methods/services.
CodingAgentEvent is still a large flat enum rather than grouped event families.
OperationControl is an active-operation guard, not a full scheduler.
ProductEvent sequence, snapshot cursor, replay retention, and backpressure
  semantics are not yet one explicit protocol.
Capability snapshots are partially modeled but not the only way operations
  receive permission handles.
SessionEvent recovery/versioning/sequence hardening remains future work.
```

The architecture below keeps the working baseline intact while defining the convergence target.

## Design Goals

1. Keep `CodingAgentSession` working during migration while narrowing the stable facade.
2. Express product actions as typed operations instead of growing more public methods.
3. Route all runtime-affecting client intents through one admission path.
4. Keep Flow nodes focused on orchestration and operation-context mutation.
5. Keep durable side effects behind services.
6. Preserve Rust-native `SessionEvent` as the only durable session fact source.
7. Preserve one adapter-facing product event stream while grouping event families internally.
8. Use operation-local capability snapshots instead of dynamic raw-service access.
9. Make UI/RPC reconnect and multi-client behavior a first-class runtime contract.
10. Delete retired paths after replacement paths are adopted.

## Non-Goals

This architecture does not require:

```text
a one-time rewrite
a TypeScript session JSONL compatibility layer
raw FlowEvent as a UI/RPC protocol
raw AgentEvent as a UI/RPC protocol
Lua plugins registering arbitrary Flow nodes or subflows
pi-agent-core owning coding-agent product sessions
pi-tui learning coding-agent product semantics
```

## Layer Contract

### pi-ai

`pi-ai` owns model/provider/auth/transport behavior.

It may expose:

```text
AiClient
ProviderRegistry
provider request/response/stream types
model metadata
provider auth resolution
transport/retry helpers
```

It must not know about:

```text
CodingAgentSession
ProductEvent or CodingAgentEvent
SessionEvent
CLI/RPC/TUI adapters
product Flow graphs
```

### pi-agent-core

`pi-agent-core` owns low-level agent runtime behavior.

It may expose:

```text
Flow<C>
Agent and AgentConfig
AgentTurnFlow
AgentTool and AgentEvent
ExecutionEnv, filesystem, shell, hooks, resources
low-level transcript/session-context primitives when product-neutral
```

It must not depend on:

```text
CodingAgentSession
ProductEvent families
adapter state
coding-agent session persistence policy
```

### pi-coding-agent

`pi-coding-agent` owns product runtime behavior.

Target module shape:

```text
api/
  CodingAgentSession or CodingAgentRuntime facade
  Operation
  OperationOutcome
  ProductEvent
  RuntimeCapabilities
  RuntimeSnapshot / UiSnapshot views

runtime/
  OperationRuntime
  IntentRouter
  OperationScheduler
  OperationRegistry
  ServiceContainer
  EventBus

operations/
  prompt/
  manual_compaction/
  branch_summary/
  export/
  plugin_load/
  plugin_command/
  agent_invocation/
  team_invocation/
  delegation/
  self_healing_edit/
  session_navigation/

services/
  SessionService
  RuntimeService
  FlowService
  EventService
  CapabilityService
  PluginService

session_log/
  manifest
  typed event envelope
  transaction
  replay/fold
  recovery/versioning helpers

adapters/
  print
  json
  rpc
  interactive
```

The current `coding_session` module can remain the migration container, but new work should move operation-specific business logic into focused operation modules or services instead of adding more facade methods.

### pi-tui

`pi-tui` remains a generic terminal/input/render/component crate.

It may expose:

```text
terminal primitives
input normalization
layout/render components
generic editor/menu/dialog primitives
```

It must not own:

```text
CodingAgentSession
ProductEvent interpretation
agent profiles or teams
session tree/fork/clone business logic
plugin host policy
```

Coding-agent interactive code may use `pi-tui`, but product semantics stay in `pi-coding-agent` adapters and projections.

## Core Runtime Contract

### Stable Product Verbs

The stable product-facing surface should converge toward a small verb set:

```text
open/create runtime or session
run operation
send control command
subscribe to product events
query capabilities
read snapshot/view/export state
close/detach client
```

Near-term compatibility:

```text
CodingAgentSession remains the working owner and public facade.
Existing methods remain until adapters and tests move to Operation.
run(Operation) can start as an internal method behind existing public methods.
CodingAgentRuntime may later become a narrowed facade or type alias/wrapper.
```

Target conceptual API:

```rust
let mut runtime = CodingAgentRuntime::open(options).await?;
let mut events = runtime.subscribe();
let outcome = runtime.run(Operation::Prompt(request)).await?;
```

Equivalent migration API:

```rust
let mut session = CodingAgentSession::open(options).await?;
let outcome = session.run(Operation::Prompt(request)).await?;
```

The name is less important than the boundary: callers ask the runtime to run a typed operation instead of choosing an internal service or Flow directly.

### Runtime Owner

The runtime owner coordinates state and invariants. It does not personally implement every workflow.

Target split:

```text
Runtime owner
  session/transient handle
  service container
  operation registry
  scheduler/control state
  event bus
  client/intent admission state

Operation modules
  prompt, compaction, export, plugin, delegation, team,
  self-healing edit, session navigation business logic

Services
  durable writes, runtime snapshots, event publication,
  capability policy, plugin execution, Flow construction/running
```

The owner can be `CodingAgentSession` during migration. The final public facade should be smaller even if the internal owner remains a rich struct.

### Service Responsibilities

```text
SessionService
  owns session manifest, event-log append, transactions, replay/fold,
  active leaf updates, fork/clone/tree/export session views, and recovery policy

RuntimeService
  builds immutable per-operation runtime snapshots: model/provider, auth,
  tools, resources, execution environment, stream options, plugin registry view

FlowService
  constructs and runs product Flow graphs and low-level subflows

EventService
  maps internal events into ProductEvent/CodingAgentEvent families and publishes them

CapabilityService
  evaluates declarations/grants and creates runtime/operation capability views

PluginService
  collects and executes plugin tools, commands, hooks, UI actions, keybinds,
  dialogs, and future restricted extension points
```

Services are internal implementation owners. Plugins, adapters, and Flow nodes should receive scoped handles or snapshots, not raw services by default.

### Flow Node Responsibilities

Flow nodes may:

```text
validate local preconditions
move the operation context forward
choose the next graph action
call scoped capability or service handles
record pending facts into the operation transaction
emit or buffer semantic runtime events through EventService
```

Flow nodes must not:

```text
directly commit final session storage
construct adapter wire events
read or mutate raw adapter UI state
start low-level Agent execution from adapters
hand raw session/runtime/provider internals to plugins
```

A Flow node may call a service that performs side effects. The service owns the side-effect policy and error shape.

## Operation Contract

### Operation Identity

Every admitted operation receives:

```text
operation_id
operation_kind
operation_origin
initiator metadata when available
capability snapshot generation
runtime snapshot generation when applicable
optional session_id
optional turn_id
optional parent_operation_id
```

`operation_id` is the stable correlation key across ProductEvent, SessionEvent, snapshots, child operations, and protocol responses.

### Operation Shape

The target `Operation` set should cover all runtime-affecting product actions. The exact Rust enum can evolve, but the contract is:

```text
Prompt
ManualCompaction
BranchSummary
Export
PluginLoad
PluginCommand
OpenPluginDialog
AgentInvocation
TeamInvocation
DelegationRequest / DelegationApproval / DelegationRejection
SelfHealingEdit
SwitchActiveLeaf
SessionOpen / SessionCreate / SessionResume when modeled after runtime startup
RuntimeSettingsChange / DefaultProfileChange
```

Some current actions can stay as facade methods during migration. New runtime-affecting actions should declare their operation contract before being exposed through adapters.

### Operation Origin

Agent/team workflows need an explicit origin. This resolves the ambiguity between user-started one-off invocations and child delegations.

```text
ClientRoot
  submitted by a UI/RPC/headless client as a top-level operation

ParentChild
  spawned by an active parent operation, usually prompt/delegation/team

RuntimeInternal
  created by recovery, startup, or maintenance policy
```

A user-started one-off `AgentInvocation` or `TeamInvocation` is a root operation. A delegated helper agent/team run is a child operation. The scheduler must not reject root invocations merely because child invocations exist.

### Operation Classes

Every operation declares an admission class. Classes describe scheduler and side-effect rules, not UI labels.

```text
Query
  capability query, current view, list commands, profile/team listing
  never opens a session transaction and does not require the root operation slot

ReadOnly
  export, replay read, tree view, committed transcript read
  reads committed state only and may run while a root writer is active

SessionWriteRoot
  prompt, manual compaction, branch summary, self-healing edit,
  switch active leaf, session mutation
  exclusive per session

NonSessionRoot
  user-started agent/team invocation or plugin command that does not write
  the parent session but still consumes runtime execution/control resources
  exclusive where it shares the same runtime root slot

RuntimeWrite
  plugin reload, profile/settings mutation, capability generation changes
  installs a new generation and affects future operations by default

Child
  delegated agent, delegated team member, or scoped sub-operation
  requires a parent operation scope and cannot outlive it unless converted
  into a durable pending request

Control
  abort, steer, follow-up, revoke/cancel
  targets an active operation through its control channel
```

The class is part of the operation contract and should be tested at adapter/protocol boundaries.

### Scheduling Rules

The scheduler preserves these rules:

```text
one session may have at most one active SessionWriteRoot
one runtime root slot may reject or defer incompatible NonSessionRoot operations
ReadOnly operations read committed state and do not observe half-written transactions
Query operations are always allowed unless the runtime is shutting down
Child operations can only start through a parent operation scope
Control commands are priority signals, not ordinary queued operations
RuntimeWrite creates a new generation and must not mutate active snapshots
```

Runtime writes have two cases:

```text
FutureOnly runtime write
  may install a new generation while an operation is active if it does not
  mutate handles already captured by that operation

Interrupting runtime write
  applies a revocation/cancel policy to active operations before or during
  installation of the new generation
```

If a runtime mutation cannot be made generation-safe, it must be blocked while incompatible operations are active.

### Operation Outcome

Every started operation emits exactly one terminal product event and returns one terminal outcome.

Conceptual shape:

```rust
pub struct OperationOutcome {
    pub operation_id: OperationId,
    pub kind: OperationKind,
    pub status: OperationStatus,
    pub persistence: PersistenceOutcome,
    pub payload: OperationPayload,
}

pub enum OperationStatus {
    Succeeded,
    Aborted,
    Failed,
    RecoveryRequired,
}

pub enum PersistenceOutcome {
    NotRequired,
    Skipped { reason: String },
    Committed { session_range: Option<SessionSequenceRange> },
    Failed { error: ProductErrorView },
    InDoubt { recovery_id: RecoveryId },
}
```

Typed per-operation outcomes may wrap or project this shape. They must not contradict it.

A durable session write required for success must commit before `Succeeded` is emitted. Live streaming output can be visible before commit, but terminal success cannot precede required persistence.

## Product Event Contract

### ProductEvent Is The Adapter Boundary

Adapters consume one product semantic event stream. Internally, event growth should be grouped by family instead of one unbounded flat enum.

Target shape:

```rust
pub struct ProductEvent {
    pub stream_id: EventStreamId,
    pub sequence: EventSequence,
    pub operation_id: Option<OperationId>,
    pub parent_operation_id: Option<OperationId>,
    pub session_id: Option<SessionId>,
    pub initiator: Option<EventInitiator>,
    pub durability: EventDurability,
    pub causality: Vec<EventRef>,
    pub kind: ProductEventKind,
}

pub enum ProductEventKind {
    Operation(OperationEvent),
    Prompt(PromptEvent),
    Agent(AgentProductEvent),
    Team(TeamProductEvent),
    Tool(ToolProductEvent),
    Session(SessionProductEvent),
    Plugin(PluginProductEvent),
    Delegation(DelegationProductEvent),
    Workflow(WorkflowProductEvent),
    Capability(CapabilityEvent),
    Diagnostic(DiagnosticEvent),
    Pressure(StreamPressureEvent),
}
```

`CodingAgentEvent` can remain the concrete compatibility enum while this model is introduced. Splitting the enum is a migration step, not a behavior change by itself.

### ProductEvent Sequence

`ProductEvent.sequence` is assigned by the live `EventBus` for one `EventStreamId`.

Rules:

```text
sequence is strictly increasing within one stream
sequence is live delivery order, not durable session order
sequence does not imply global ordering across runtimes
sequence does not define child-operation business merge order
```

Clients use `(stream_id, sequence)` for reconnect within retention. If retained events no longer cover a requested sequence, the client must request a fresh snapshot.

### Event Durability

Use one durability model:

```rust
pub enum EventDurability {
    LiveOnly,
    PendingSessionWrite { operation_id: OperationId },
    Durable {
        session_id: SessionId,
        session_range: SessionSequenceRange,
    },
    DerivedFromSession {
        session_id: SessionId,
        source: EventRef,
    },
    FailedToPersist {
        reason: String,
    },
}
```

Meaning:

```text
LiveOnly
  presentation or in-flight runtime state; not replayed as history

PendingSessionWrite
  shown live, but not yet committed as durable session truth

Durable
  corresponds to committed SessionEvent facts

DerivedFromSession
  semantic event projected from durable facts, possibly coalesced or transformed

FailedToPersist
  live output existed, but required durability failed
```

ProductEvent and SessionEvent do not need a one-to-one mapping.

### Product Event Ordering Invariants

The product event stream must preserve these local orderings:

```text
OperationStarted before other events for that operation
PromptStarted before assistant/tool/session-write events for that prompt
ToolCallStarted before ToolCallUpdated/Completed/Failed for that call
SessionWritePending before SessionWriteCommitted/Skipped/Failed
CapabilityChanged after the new generation is installed
terminal Operation event after finalization policy runs
parent operation terminal event after child operations finish, fail, abort,
  or become durable pending requests
```

Consumers must treat the terminal operation event as the authoritative runtime outcome.

### Child Event Ordering

Concurrent child operations keep live delivery order separate from business merge order.

```text
ProductEvent.sequence = live stream delivery order
parent_operation_id / child_operation_id = causality
parent merge result = stable business order chosen by parent policy
```

This lets a team operation stream whichever member finishes first while still producing a deterministic final merge chosen by the parent operation.

## Session Event Contract

### SessionEvent Is Durable Truth

`SessionEvent` is the only durable session fact source.

It is used for:

```text
replay
resume
fork
clone
export
audit
recovery
transcript/tree/stat rebuild
```

`ProductEvent` is not automatically persisted. `UiState` is never persisted as truth.

### SessionEventEnvelope Target Shape

The current project already has `SessionEventEnvelope`. The target hardening adds explicit durable sequencing while preserving schema/version identity.

```rust
pub struct SessionEventEnvelope {
    pub schema: String,
    pub version: u32,
    pub session_id: SessionId,
    pub session_sequence: SessionSequence,
    pub event_id: EventId,
    pub operation_id: Option<OperationId>,
    pub turn_id: Option<TurnId>,
    pub branch_id: Option<BranchId>,
    pub leaf_id: Option<LeafId>,
    pub parent_event_id: Option<EventId>,
    pub created_at: Timestamp,
    pub data: SessionEventData,
}
```

During migration, existing append order and event IDs remain authoritative where `session_sequence` is not yet present. Plans that add `session_sequence` must include migration/replay compatibility for existing Rust-native logs.

### Required Session Facts

When part of a persistent session operation, these facts must be durable:

```text
session created/opened metadata needed for replay
operation started
operation committed/aborted/failed/recovered terminal marker
turn started/input recorded/completed/aborted/failed where applicable
committed user content
committed assistant content or cancellation/failure marker
committed tool call request and final result/failure/cancellation
branch/leaf creation and active leaf changes
model/provider/profile/capability generation references needed for audit
compaction, branch summary, export-relevant summary facts
plugin load facts that affect replay diagnostics or capability explanation
self-healing edit lifecycle, repair attempts, and durable artifact references
pending delegation confirmation and approval/rejection facts
migration and recovery markers
```

If losing a record changes historical transcript, recovery, fork/clone/export behavior, or auditability, it belongs in `SessionEvent`.

### Transaction Semantics

Session writes are transactional at the operation level.

```text
1. create operation transaction
2. record pending typed SessionEvent values
3. stage blobs/artifacts if needed
4. append pending events and terminal marker
5. update manifest/active leaf only after append succeeds
6. emit ProductEvent session-write result
7. rebuild or update projections from durable facts
```

An operation without a terminal durable marker is incomplete. Replay must apply recovery policy instead of presenting it as normal committed history.

### Runtime And Session Compaction

Keep two compaction concepts separate:

```text
runtime compaction
  affects provider context for an active or future model call
  may produce ProductEvent progress and optional session metadata

session compaction
  changes long-term session history or transcript projection
  must produce SessionEvent facts
```

The current project already records session compaction as a workflow fact. Future hardening should preserve the distinction.

## Snapshot And Projection Contract

### Three State Layers

```text
SessionEvent
  durable facts and replayable history

ProductEvent
  semantic runtime events, live or durable-derived

UiState
  disposable projection for rendering and interaction
```

Direction is one-way:

```text
SessionEvent -> ProductEvent -> UiState
Runtime live state -> ProductEvent -> UiState
UiState never writes SessionEvent
ProductEvent maps to SessionEvent only through explicit persistence policy
```

### UiSnapshot Shape

A snapshot is a complete UI projection at a declared event stream boundary.

```rust
pub struct UiSnapshot {
    pub cursor: SnapshotCursor,
    pub committed: CommittedSessionView,
    pub live: Option<LiveOperationView>,
    pub pending: PendingView,
    pub capabilities: RuntimeCapabilities,
}

pub struct SnapshotCursor {
    pub stream_id: EventStreamId,
    pub last_product_sequence: EventSequence,
    pub session_sequence: Option<SessionSequence>,
    pub runtime_generation: RuntimeGeneration,
}
```

Required formula:

```text
UiState at sequence N = Snapshot at N + ProductEvents after N applied in order
```

Required invariant:

```text
snapshot.cursor.last_product_sequence means the snapshot includes all ProductEvent
projection effects up to and including that sequence.
```

Implementation must satisfy this with a projection checkpoint, a short read lock, or another proven consistency point. It must not mark an event cursor and then independently read mutable state in a way that can include effects after the cursor or miss effects before it.

### Projection Rules

```text
apply events only for the snapshot stream_id
ignore duplicate events at or before the cursor
detect gaps and request a fresh snapshot
keep live and committed transcript state separate
reconcile live output only after SessionWriteCommitted or terminal failure/abort
reference large durable content by blob/artifact IDs instead of embedding by default
```

Live assistant/tool output can be shown before commit. It becomes normal committed transcript only after durable session facts are available and projection reconciles them.

## Capability Contract

Capabilities are generated, scoped, operation-local authorization snapshots.

Lifecycle:

```text
Declare
  providers, plugins, tools, profiles, adapters, and runtime features declare what exists

Grant
  policy evaluates declarations against trust, settings, auth, workspace, and runtime state

Snapshot
  operation admission freezes granted capabilities for one actor/scope/generation

Use
  operations, Flow nodes, tools, and plugins use narrow handles from the snapshot

Revoke
  policy creates a new generation and optionally cancels or interrupts active operations
```

Target shape:

```rust
pub struct OperationCapabilitySnapshot {
    pub generation: CapabilityGeneration,
    pub operation_id: OperationId,
    pub actor: ActorId,
    pub model: Option<ModelCapability>,
    pub tools: ToolCapabilitySet,
    pub commands: CommandCapabilitySet,
    pub filesystem: Option<FilesystemCapability>,
    pub shell: Option<ShellCapability>,
    pub session_read: Option<SessionReadCapability>,
    pub session_write: Option<SessionWriteCapability>,
    pub ui: Option<UiCapability>,
    pub plugin: PluginCapabilitySet,
}
```

Capabilities answer both:

```text
May this actor do the thing?
Through which narrow interface may it do it?
```

Revocation defaults:

```text
plugin reload/profile/settings/model changes
  FutureOnly unless policy says otherwise

auth secret removed/workspace trust revoked/filesystem permission revoked
  CancelMatchingOperations

temporary quota/rate/budget pressure
  DenyNextUse or CancelMatchingOperations according to operation policy
```

Active operations do not hot-update capability snapshots by default. If active behavior must change, the runtime emits explicit cancellation/failure/revocation events.

## Error And Recovery Contract

Failures are first-class operation outcomes, not unstructured exceptions leaking through adapters.

Error shape should include:

```text
category
phase
retry advice
safe user message
diagnostic details without secrets
```

Suggested categories:

```text
Input
Config
Auth
Capability
Provider
Tool
Plugin
SessionStore
Projection
Concurrency
Cancelled
Internal
```

Suggested phases:

```text
Prepare
Run
Finalize
Commit
Project
Recover
```

Failure rules:

```text
user abort is Aborted, not Failed
provider/tool/plugin/storage errors are Failed unless durable state is ambiguous
unknown durable commit state becomes InDoubt or RecoveryRequired
required durable commit must complete before success is emitted
open message/tool/session event families must close with success, failure,
  abort, cancellation, or recovery markers
recovery never silently promotes incomplete output to committed transcript history
```

Startup/session-open recovery should scan for:

```text
operations without terminal markers
open message families
open tool calls
manifest/index out of sync with event log
staged blobs not referenced by committed facts
unknown commit state
```

The event log is authoritative. Manifest, indexes, snapshots, and UI projections are rebuildable.

## Backpressure And Flow Control Contract

Backpressure is part of the runtime protocol.

Default rule:

```text
durable facts are never silently dropped
derived state can be rebuilt
high-frequency live streams can be coalesced
slow consumers must not stall session commit
pressure beyond budget becomes an explicit event, rejection, disconnect, or cancellation
```

Pressure policies by class:

```text
SessionEvent
  never drop; write failure becomes operation failure or recovery

operation terminal ProductEvent
  never silently drop; slow subscribers reconnect from snapshot

token/progress/live display events
  may throttle, coalesce, or drop-old-keep-new with semantic markers

tool stdout/stderr/log stream
  bounded; truncation must be explicit and inspectable where product requires it

snapshot requests
  derived and coalescible; latest cursor wins when safe

UI command queue
  bounded; reject or defer with visible admission result
```

Slow subscribers must not block:

```text
session commit
operation terminal event creation
capability revocation
abort/cancel control path
recovery markers
```

## UI And Multi-Client Contract

### Boundary Rule

```text
UI intent in
Snapshot and ProductEvent out
```

The UI must not talk directly to session storage, Flow nodes, provider runtime, plugin internals, or low-level Agent state.

Conceptual boundary:

```text
TUI/RPC/GUI client
  owns local input, subscriptions, viewport, drafts, and projection state
        |
        v
Adapter / Presenter
  maps UiIntent or wire command to ClientIntent / Operation / Control
  maps ProductEvent + Snapshot to UiState/ViewModel
        |
        v
OperationRuntime
  admits intents, runs operations, emits events, serves snapshots/capabilities
```

### Client-Local State

These stay client-local unless a future collaboration feature explicitly promotes them:

```text
prompt draft text
cursor and selection
IME composition
viewport/scroll/focus/menu highlight
autocomplete state
window layout
local undo stack for unsubmitted text
```

Submitted prompt input becomes runtime-owned operation data. Unsubmitted drafts are not session history.

### IntentRouter

All runtime-affecting client actions pass through one admission path.

The router performs:

```text
protocol validation
client identity and capability authorization
operation/session existence checks
optimistic cursor/generation checks where required
scheduler admission
backpressure handling
operation creation or control dispatch
admission response to initiating client
```

No UI/RPC adapter should hold direct service references that bypass admission for runtime-affecting actions.

### Abort And Detach

Detach and abort are different:

```text
DetachClient
  close/unsubscribe one client; operation continues unless policy says otherwise

AbortOwnOperation
  initiating actor requests global abort of its operation, if authorized

AbortOperation
  stronger control capability; aborts targeted operation globally

AbortSessionOperations
  session-control capability; aborts matching operations for a session
```

Aborting an operation changes shared runtime state. All subscribed clients observe the terminal event or projection update.

### Conflict Handling

Shared mutable session controls need explicit compare-and-set guards.

Examples:

```text
switch active leaf
  require expected active leaf/cursor generation

change default profile/settings
  require expected generation or serialize as RuntimeWrite

submit prompt based on old transcript
  either allow with captured context or reject with StaleSessionCursor,
  according to product policy
```

The runtime should reject ambiguous shared mutations instead of letting the last client silently win.

## Protocol Versioning Contract

Protocol versions are scoped by family, not by one global runtime version.

Families:

```text
SessionEvent
ProductEvent
UiCommand / ClientIntent
Snapshot
PluginHost
Capability
ToolSchema
```

Rules:

```text
major version change means incompatible contract change
minor version change means backward-compatible addition or optional feature
unknown required feature fails closed
unknown optional feature may be ignored only where the family contract allows it
patch versions are crate/internal implementation details, not protocol fields
```

Durable `SessionEvent` is the most conservative family. Old supported Rust-native logs should remain readable through versioned decoders. Historical facts should not be rewritten by default; migration may write new checkpoints, repair markers, or explicit migration events.

Live UI/RPC protocols negotiate compatible versions at connection time. If no compatible version exists, the runtime rejects the connection clearly instead of silently degrading behavior.

## Migration Path From Current Code

This is the current-state-aware migration path. It intentionally does not say to introduce pieces that already exist.

### Stage 0: Contract Normalization

Goal: make this reference architecture the single coherent contract for the next simplification stage.

Targets:

```text
name Operation, ProductEvent, SessionEvent, Snapshot, Capability, and Outcome contracts
remove conflicting duplicate pseudocode from architecture docs
record which current APIs are compatibility surfaces
record stop conditions for retiring broad facade methods
```

Exit criteria:

```text
design docs describe current baseline accurately
new implementation plans can target one contract without resolving contradictions first
```

### Stage 1: Internal Operation API

Goal: introduce `Operation` and `OperationOutcome` behind the existing `CodingAgentSession` facade.

Targets:

```text
keep public CodingAgentSession methods working
route prompt through run(Operation::Prompt) internally first
add operation metadata: id, kind, origin, class, capability/runtime generation
move method-specific business logic toward operation modules
```

Exit criteria:

```text
at least prompt and one workflow operation run through the internal operation path
adapters see unchanged behavior through existing events/protocols
new runtime-affecting work has an operation contract before implementation
```

### Stage 2: Product Event Family Convergence

Goal: split the current flat `CodingAgentEvent` model into product event families while preserving adapter compatibility.

Targets:

```text
introduce family-oriented ProductEvent concepts internally
map existing CodingAgentEvent variants to families
add durability metadata where meaningful
keep RPC/interactive/print JSON compatibility while adapters migrate
```

Exit criteria:

```text
adapter-visible behavior is describable in ProductEvent family terms
Flow node IDs and AgentEvent internals do not leak into product protocols
terminal outcome events are normalized by operation id/kind/status
```

### Stage 3: IntentRouter And Scheduler

Goal: centralize admission for runtime-affecting commands.

Targets:

```text
route prompt submit, abort, steer, follow-up, plugin reload,
profile/settings changes, session navigation, delegation confirmation,
and root agent/team invocation through one admission path
classify operations by Query, ReadOnly, SessionWriteRoot, NonSessionRoot,
RuntimeWrite, Child, or Control
replace ad hoc busy checks with scheduler decisions and admission results
```

Exit criteria:

```text
no UI/RPC path starts session-affecting work by directly calling deep services
root vs child agent/team invocation is unambiguous
control commands retain priority under busy/backpressure conditions
```

### Stage 4: SessionEvent Hardening

Goal: harden the already-existing Rust-native session event log.

Targets:

```text
add or formalize durable session sequence semantics
record capability/runtime generation references where required
add recovery markers for incomplete operation families
make replay/projection tolerant of existing Rust-native logs
strengthen transaction/idempotency behavior around partial commit uncertainty
```

Exit criteria:

```text
replay can distinguish complete, failed, aborted, recovered, and in-doubt operations
committed views are rebuildable from SessionEvent without hidden adapter state
old Rust-native logs remain readable through compatibility decoders
```

### Stage 5: Capability Snapshot Integration

Goal: make operation-local capability snapshots the only permission language for operations, tools, plugins, and workflows.

Targets:

```text
model/provider access uses ModelCapability
filesystem access uses FilesystemCapability
shell access uses ShellCapability
tool execution uses ToolCapabilitySet
plugin host calls use PluginCapabilitySet
session read/write uses SessionReadCapability and SessionWriteCapability
runtime mutations emit CapabilityChanged with generation and revocation semantics
```

Exit criteria:

```text
prompt and workflow behavior is explainable by operation capability snapshots
active operations do not observe silent mid-run capability changes
plugins/tools do not receive raw runtime/session/provider/auth services
```

### Stage 6: Snapshot, Reconnect, And Multi-Client Boundary

Goal: make adapters thin clients of snapshot plus product event projection.

Targets:

```text
add consistent UiSnapshot cursor semantics
make TUI/interactive projection consume Snapshot + ProductEvent
model RPC/GUI clients as ClientConnection instances
separate client-local drafts from runtime-owned submitted operations
implement gap handling and fresh-snapshot recovery
```

Exit criteria:

```text
TUI no longer mutates session/runtime internals directly
new GUI/RPC windows can be modeled as clients over the same semantic protocol
slow or disconnected clients recover through snapshot plus retained events
```

### Stage 7: Backpressure, Versioning, And Recovery Hardening

Goal: make the runtime production-tolerant.

Targets:

```text
bounded queues and explicit overflow policy
slow subscriber disconnect/reconnect semantics
protocol family negotiation
startup recovery scan
InDoubt commit handling
structured retry and idempotency keys
snapshot/version rebuild policy
```

Exit criteria:

```text
storage pressure becomes operation failure/recovery, not memory growth
unsupported protocol clients fail clearly
restart after interrupted operation produces coherent recovered state
terminal events and recovery markers are not displaced by live deltas
```

### Stage 8: Public Facade Narrowing And Deletion

Goal: remove replaced architecture instead of preserving parallel systems.

Targets:

```text
promote run(Operation), control, subscribe, capabilities, and snapshot/view verbs
retire broad public methods after adapters and tests use operation entrypoints
remove compatibility shims after the last caller is migrated
remove obsolete tests that assert retired internal structure
keep TypeScript session JSONL compatibility rejected unless a future decision reverses it
```

Exit criteria:

```text
there is one operation admission path
there is one durable session fact source
there is one product semantic event stream
legacy paths have documented deletion commits, not hidden fallback behavior
```

## Implementation Guardrails

```text
Prefer vertical slices over broad rewrites.
Keep current public behavior observable through adapter tests during migration.
Use deterministic faux providers and offline fixtures.
Test event sequences and terminal outcomes at adapter/protocol boundaries.
Test transaction/recovery behavior at SessionService boundaries.
Do not expose raw services to plugins or adapters to speed up migration.
Do not use Flow node IDs as product protocol fields.
Document temporary compatibility shims with removal conditions.
Update TODO/spec/plan documents when phase boundaries or public API direction changes.
```

## Final Target

The final shape is:

```text
one operation runtime owner
one operation admission path
one durable Rust-native session fact model
one product semantic event stream grouped by family
one snapshot/projection model for UI/RPC/GUI clients
many thin adapters
capability-scoped plugins, tools, workflows, and clients
```

This is not a new architecture replacing the Flow-centered design. It is the closure step that makes the existing Flow-centered runtime smaller at the public boundary, clearer under concurrency, and harder to bypass.
