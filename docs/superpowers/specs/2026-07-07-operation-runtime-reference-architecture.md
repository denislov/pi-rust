# Operation Runtime Reference Architecture

## Status

This document records a proposed reference architecture for the next simplification stage of `pi-rust`. It is not an implementation plan and does not claim the current code already has this final shape.

The proposal extends the existing Flow-centered runtime instead of replacing it. The goal is to make the architecture easier to explain, harder to bypass, and smaller at the public API boundary.

## Core Thesis

`pi-rust` should evolve into an event-sourced operation runtime:

```text
Command or Operation in
Typed Product Events out
Flow graphs orchestrate work
Typed operation contexts carry temporary state
Services own side effects
Session logs persist facts
Adapters render events
```

The short version:

```text
Command -> OperationRuntime -> Flow -> Context -> Services -> EventLog/ProductEvents -> Adapters
```

## Five Principles

### 1. Command In, Event Out

External callers should not need to know which internal service or Flow implements a product action.

The stable product-facing surface should converge toward a small set of verbs:

```text
open/create runtime
run operation
subscribe to product events
query capabilities
read view/export state
```

Instead of growing a wide `CodingAgentSession` method set for every workflow, product actions should be represented as typed operations.

Conceptual API:

```rust
let mut runtime = CodingAgentRuntime::open(options).await?;
let mut events = runtime.subscribe();
let outcome = runtime.run(Operation::Prompt(prompt)).await?;
```

### 2. Flow Orchestrates, Services Own Side Effects

Flow nodes should describe operation steps and state transitions. Durable side effects should remain behind services.

Allowed responsibilities:

```text
Flow node:
  validate local preconditions
  move operation context forward
  choose next action
  call capability/service handles

Service:
  write session events
  publish product events
  build provider/runtime snapshots
  collect plugins
  enforce capability policy
  run filesystem/shell/provider side effects
```

Disallowed drift:

```text
Flow node directly commits final session storage
Flow node constructs adapter protocol events
Adapter runs low-level Agent directly
Plugin receives raw session/runtime/provider internals
```

### 3. Session Is a Fact Container, Not a Business Object

The long-lived runtime needs session identity, active leaf, replay, pending operation state, and durable event log ownership. It should not personally implement every product workflow.

The target split is:

```text
CodingAgentRuntime / CodingAgentSessionOwner
  owns lifecycle, invariants, active operation guard, event bus, service container

Operation modules
  own prompt, compaction, plugin load, export, delegation, team, self-healing edit workflow logic

SessionService
  owns session persistence, event log append, replay/fold, active leaf updates
```

This keeps the owner from becoming a product-wide god object.

### 4. Event Families, Not One Infinite Enum

A single product event stream is still the right adapter boundary, but the event model should be internally grouped by family.

Conceptual shape:

```rust
pub struct ProductEvent {
    pub operation_id: OperationId,
    pub session_id: Option<SessionId>,
    pub sequence: EventSequence,
    pub kind: ProductEventKind,
}

pub enum ProductEventKind {
    Prompt(PromptEvent),
    Agent(AgentProductEvent),
    Tool(ToolProductEvent),
    Session(SessionProductEvent),
    Plugin(PluginProductEvent),
    Delegation(DelegationProductEvent),
    Workflow(WorkflowProductEvent),
    Diagnostic(DiagnosticEvent),
    Capability(CapabilityEvent),
}
```

Adapters still consume one stream, but event growth is localized by domain.

### 5. Capability Is the Permission Language

Plugins, profiles, teams, workflows, tools, and adapters should not receive raw services by default. They should receive scoped capabilities.

Conceptual capabilities:

```rust
pub struct CapabilitySet {
    pub model: Option<ModelCapability>,
    pub tools: ToolCapability,
    pub commands: CommandCapability,
    pub session_read: Option<SessionReadCapability>,
    pub session_write: Option<SessionWriteCapability>,
    pub filesystem: Option<FilesystemCapability>,
    pub shell: Option<ShellCapability>,
    pub ui: Option<UiCapability>,
    pub plugin: PluginCapability,
}
```

Capabilities answer both questions:

```text
Can this actor do the thing?
Through which narrow interface can it do it?
```

## Target Module Shape

The intended end state is four layers.

```text
crates/pi-ai
  provider/model/auth/transport runtime
  scoped AiClient / ProviderRegistry
  no product session or adapter semantics

crates/pi-agent-core
  Flow<C>
  Agent
  AgentTurnFlow
  AgentTool / AgentEvent / ExecutionEnv
  no CodingAgentSession, ProductEvent, or adapter state

crates/pi-coding-agent
  api/
    CodingAgentRuntime or narrowed CodingAgentSession facade
    Operation
    OperationOutcome
    ProductEvent
    CapabilityStatus

  runtime/
    OperationRuntime
    OperationRegistry
    ServiceContainer
    EventBus
    OperationControl

  operations/
    prompt/
    agent_invocation/
    team/
    delegation/
    plugin_load/
    manual_compaction/
    branch_summary/
    export/
    self_healing_edit/

  services/
    SessionService
    RuntimeService
    EventService
    CapabilityService
    PluginService

  session_log/
    manifest
    typed event envelope
    transaction
    replay/fold

  adapters/
    print
    json
    rpc
    interactive

crates/pi-tui
  generic terminal/input/render/component primitives
  no coding-agent product semantics
```

## Core Data Structures, Pseudocode

### Stable API

```rust
pub struct CodingAgentRuntime {
    owner: RuntimeOwner,
}

impl CodingAgentRuntime {
    pub async fn open(options: RuntimeOptions) -> Result<Self, ProductError>;
    pub async fn run(&mut self, op: Operation) -> Result<OperationOutcome, ProductError>;
    pub fn subscribe(&self) -> ProductEventReceiver;
    pub fn capabilities(&self) -> RuntimeCapabilities;
    pub fn view(&self) -> RuntimeView;
}

pub enum Operation {
    Prompt(PromptRequest),
    ManualCompaction(ManualCompactionRequest),
    BranchSummary(BranchSummaryRequest),
    Export(ExportRequest),
    PluginLoad(PluginLoadRequest),
    AgentInvocation(AgentInvocationRequest),
    TeamInvocation(TeamInvocationRequest),
    SelfHealingEdit(SelfHealingEditRequest),
}

pub enum OperationOutcome {
    Prompt(PromptOutcome),
    ManualCompaction(ManualCompactionOutcome),
    BranchSummary(BranchSummaryOutcome),
    Export(ExportOutcome),
    PluginLoad(PluginLoadOutcome),
    AgentInvocation(AgentInvocationOutcome),
    TeamInvocation(TeamOutcome),
    SelfHealingEdit(SelfHealingEditOutcome),
}
```

### Runtime Owner

```rust
struct RuntimeOwner {
    session: SessionHandleOrTransient,
    services: ServiceContainer,
    operations: OperationRegistry,
    event_bus: ProductEventBus,
    control: OperationControl,
}

struct ServiceContainer {
    session: SessionService,
    runtime: RuntimeService,
    events: EventService,
    capabilities: CapabilityService,
    plugins: PluginService,
}

struct OperationRegistry {
    prompt: PromptOperation,
    compaction: ManualCompactionOperation,
    branch_summary: BranchSummaryOperation,
    export: ExportOperation,
    plugin_load: PluginLoadOperation,
    agent_invocation: AgentInvocationOperation,
    team: TeamOperation,
    self_healing_edit: SelfHealingEditOperation,
}
```

### Operation Trait

```rust
trait ProductOperation {
    type Request;
    type Context;
    type Outcome;

    fn kind(&self) -> OperationKind;

    fn prepare(
        &self,
        request: Self::Request,
        owner: &mut RuntimeOwner,
    ) -> Result<Self::Context, ProductError>;

    async fn run_flow(
        &self,
        ctx: &mut Self::Context,
        services: &ServiceContainer,
    ) -> Result<FlowOutcome, ProductError>;

    fn finish(
        &self,
        ctx: Self::Context,
        services: &mut ServiceContainer,
    ) -> Result<Self::Outcome, ProductError>;
}
```

### Prompt Operation

```rust
struct PromptOperation {
    flow: PromptTurnFlowFactory,
}

struct PromptContext {
    ids: OperationIds,
    request: PromptRequest,
    resolved: Option<ResolvedPromptRequest>,
    runtime: Option<RuntimeSnapshot>,
    resources: Option<ResourceSnapshot>,
    agent: Option<Agent>,
    replay: Option<SessionReplay>,
    tx: Option<TurnTransaction>,
    assistant: AssistantAccumulator,
    tool_results: Vec<ToolObservation>,
    events: ProductEventBuffer,
    capabilities: CapabilitySet,
    abort_reason: Option<String>,
}

struct PromptTurnFlow {
    graph: Flow<PromptContext>,
}
```

### Event Model

```rust
pub struct ProductEvent {
    pub operation_id: OperationId,
    pub turn_id: Option<TurnId>,
    pub session_id: Option<SessionId>,
    pub timestamp: Option<Timestamp>,
    pub kind: ProductEventKind,
}

pub enum PromptEvent {
    Started,
    InputPrepared { summary: PreparedInputSummary },
    RuntimeResolved { model: ModelId },
    Completed { output: String, usage: Option<Usage> },
    Failed { error: ProductErrorView },
    Aborted { reason: String },
}

pub enum SessionProductEvent {
    Opened { session_id: SessionId },
    WritePending,
    WriteCommitted { new_leaf_id: Option<LeafId> },
    WriteSkipped { reason: String },
    ActiveLeafChanged { leaf_id: LeafId },
}

pub enum ToolProductEvent {
    Started { call_id: ToolCallId, name: String },
    Updated { call_id: ToolCallId, message: String },
    Completed { call_id: ToolCallId, output: ToolOutputView },
    Failed { call_id: ToolCallId, error: ProductErrorView },
}
```

### Session Log

```rust
struct SessionManifest {
    schema: String,
    version: u32,
    session_id: SessionId,
    active_leaf_id: Option<LeafId>,
    event_log: RelativePath,
    metadata: SessionMetadata,
}

struct SessionEventEnvelope {
    schema: String,
    version: u32,
    event_id: EventId,
    operation_id: OperationId,
    turn_id: Option<TurnId>,
    branch_id: Option<BranchId>,
    leaf_id: Option<LeafId>,
    parent_event_id: Option<EventId>,
    created_at: Timestamp,
    data: SessionEventData,
}

struct TurnTransaction {
    operation_id: OperationId,
    turn_id: TurnId,
    base_leaf: Option<LeafId>,
    pending: Vec<SessionEventEnvelope>,
    staged_blobs: Vec<BlobWrite>,
}
```

## Module Relationships

```text
Adapter
  depends on: api::CodingAgentRuntime, api::Operation, api::ProductEvent
  does not depend on: Flow node IDs, SessionLogStore internals, AgentTurnFlow internals

CodingAgentRuntime
  depends on: OperationRegistry, ServiceContainer, EventBus
  owns: operation guard, session handle, product invariants

ProductOperation
  depends on: Flow<C>, typed context, scoped services
  owns: operation-specific business flow

ServiceContainer
  owns side effects:
    SessionService -> session log/replay/finalization
    RuntimeService -> provider/model/tool runtime snapshot
    EventService -> product event construction/publication
    PluginService -> plugin capability collection/execution
    CapabilityService -> feature/permission status

pi-agent-core
  provides: Flow<C>, AgentTurnFlow, Agent, AgentTool, ExecutionEnv

pi-ai
  provides: AiClient, ProviderRegistry, provider streams

pi-tui
  provides: generic terminal primitives only
```

## Operation Concurrency Model

The operation runtime should use per-session single-writer semantics, multi-reader snapshots, and structured child concurrency.

The model is deliberately not globally single-threaded and not fully free-form concurrent. It is based on the resource being protected:

```text
Session event log -> single writer
Committed session view -> many readers
Runtime snapshot -> immutable per operation
Child operations -> owned by parent operation
Control commands -> delivered to active operation, not queued as operations
```

Short rule:

```text
One session may have at most one active root write operation.
Read/query operations may run concurrently.
Child operations may run concurrently only inside a parent operation scope.
Control commands are signals to the active operation, not standalone operations.
```

### Operation Classes

Every operation should declare its concurrency class.

```rust
pub enum OperationClass {
    Query,
    ReadOnly,
    SessionWrite,
    RuntimeWrite,
    Child,
    Control,
}

impl Operation {
    pub fn class(&self) -> OperationClass {
        match self {
            Operation::Prompt(_) => OperationClass::SessionWrite,
            Operation::ManualCompaction(_) => OperationClass::SessionWrite,
            Operation::BranchSummary(_) => OperationClass::SessionWrite,
            Operation::Export(_) => OperationClass::ReadOnly,
            Operation::PluginLoad(_) => OperationClass::RuntimeWrite,
            Operation::AgentInvocation(_) => OperationClass::Child,
            Operation::TeamInvocation(_) => OperationClass::Child,
            Operation::SelfHealingEdit(_) => OperationClass::SessionWrite,
        }
    }
}
```

Class meanings:

```text
Query
  capability query, state view, current UI snapshot, list commands
  always allowed

ReadOnly
  export, replay read, tree view, committed transcript read
  allowed during a root writer but reads committed state only

SessionWrite
  prompt, manual compaction, branch summary, switch active leaf, session mutation
  exclusive per session

RuntimeWrite
  plugin reload, profile/settings mutation, runtime capability mutation
  does not mutate already-running operation snapshots

Child
  delegated agent, team member, or scoped sub-operation
  can only be spawned by a parent operation

Control
  abort, steer, follow-up
  routed to active operation through a control channel
```

### Scheduler Shape

Each session has one active root operation slot.

```rust
pub struct OperationScheduler {
    active_root: Option<ActiveRootOperation>,
    runtime_generation: RuntimeGeneration,
}

pub struct ActiveRootOperation {
    pub operation_id: OperationId,
    pub kind: OperationKind,
    pub cancel: CancellationToken,
    pub control: OperationControlSender,
    pub children: ChildOperationSet,
}

pub enum OperationPermit {
    Query,
    ReadCommittedSnapshot,
    RootWriter { operation_id: OperationId },
    RuntimeWriter { generation: RuntimeGeneration },
    Child { parent_operation_id: OperationId },
}
```

Scheduling rules:

```rust
impl OperationScheduler {
    pub fn start_operation(
        &mut self,
        op: &Operation,
    ) -> Result<OperationPermit, BusyError> {
        match op.class() {
            OperationClass::Query => Ok(OperationPermit::Query),
            OperationClass::ReadOnly => Ok(OperationPermit::ReadCommittedSnapshot),
            OperationClass::SessionWrite => {
                if self.active_root.is_some() {
                    return Err(BusyError::session_writer_active());
                }
                let active = ActiveRootOperation::new(op.kind());
                let operation_id = active.operation_id.clone();
                self.active_root = Some(active);
                Ok(OperationPermit::RootWriter { operation_id })
            }
            OperationClass::RuntimeWrite => {
                if self.runtime_write_is_blocked() {
                    return Err(BusyError::runtime_write_active());
                }
                Ok(OperationPermit::RuntimeWriter {
                    generation: self.runtime_generation.next(),
                })
            }
            OperationClass::Child => Err(BusyError::child_requires_parent()),
            OperationClass::Control => Err(BusyError::use_control_channel()),
        }
    }
}
```

### Prompt Semantics

A normal prompt is a `SessionWrite` root operation.

While a prompt is running, the runtime allows:

```text
capability queries
UI snapshots
committed-state export/tree/replay reads
abort/steer/follow-up control commands
delegated child operations owned by the prompt
```

While a prompt is running, the runtime rejects or defers:

```text
another root prompt on the same session
manual compaction on the same session
active leaf switch on the same session
branch summary write on the same session
runtime mutation that would alter the prompt's frozen RuntimeSnapshot
```

Prompt write flow:

```text
start root writer permit
create PromptContext
open TurnTransaction
run PromptTurnFlow
optionally spawn child operations
collect assistant/tool/child results
finalize TurnTransaction through SessionService
emit session write and prompt outcome events
release root writer permit
```

### Child Operation Semantics

Delegation and team member runs should use structured child concurrency. A child operation cannot outlive its parent operation unless explicitly converted into a durable pending request.

```rust
pub struct ChildOperation {
    pub child_operation_id: OperationId,
    pub parent_operation_id: OperationId,
    pub isolation: ChildIsolation,
    pub cancel: CancellationToken,
}

pub enum ChildIsolation {
    NonPersistent,
    ScratchSession,
    ReadOnlyParentSnapshot,
}
```

Default policy:

```text
child agent/team operation does not write the parent session directly
child receives an isolated runtime/session view
parent collects the child outcome
parent decides what summary/result enters the parent transaction
parent abort cancels child operations unless policy says otherwise
```

This keeps the parent prompt's session transaction as the only write path into the parent session.

### Runtime Mutation And Generations

Runtime-changing operations should use a generation model.

```rust
pub struct RuntimeSnapshot {
    pub generation: RuntimeGeneration,
    pub model: ModelConfig,
    pub tools: ToolRegistrySnapshot,
    pub plugins: PluginSnapshot,
    pub capabilities: CapabilitySet,
}
```

Rules:

```text
operation start freezes RuntimeSnapshot
plugin reload/profile changes affect future operations
running operations do not see hot-swapped tools or model config
CapabilityChanged events describe future availability, not mutation of active snapshots
```

This avoids an operation seeing its tools, plugins, model, or auth context change halfway through execution.

### ReadOnly Snapshot Semantics

Read-only operations should not block an active root writer. They read committed state only.

```text
SessionService replay -> committed truth
ProductEvent stream -> live truth
UiState -> committed snapshot plus live projection
```

For example, if a prompt is currently streaming assistant deltas, export reads the latest committed leaf unless explicitly asked for a live UI projection export. The live assistant text is visible through `ProductEvent`, not through committed replay until the prompt finalizes.

### Control Commands

Abort, steer, and follow-up are not queued as ordinary operations. They target the active root operation.

```rust
pub enum OperationControlCommand {
    Abort { reason: String },
    Steer { text: String },
    FollowUp { text: String },
}

impl CodingAgentRuntime {
    pub async fn control(
        &mut self,
        command: OperationControlCommand,
    ) -> Result<(), ProductError> {
        let active = self
            .owner
            .scheduler
            .active_root
            .as_ref()
            .ok_or(ProductError::NoActiveOperation)?;
        active.control.send(command).await?;
        Ok(())
    }
}
```

Control commands may produce product events, but they do not own session transactions.

### Concurrency Invariants

The runtime should preserve these invariants:

```text
one parent session event log writer at a time
operation runtime snapshots are immutable after operation start
child operations are cancelled or finalized through their parent scope
parent operation owns merge policy for child results
committed reads never observe half-written session transactions
UI live state comes from ProductEvent projection, not direct storage reads
runtime writes do not mutate active operation snapshots
```

This gives the system useful concurrency without making session persistence, active leaf movement, plugin reload, and delegation merge behavior nondeterministic.

## One Prompt, End-to-End

### Conceptual Flow

```text
User submits prompt
Adapter builds Operation::Prompt
Runtime starts guarded operation
PromptOperation prepares PromptContext
PromptTurnFlow runs nodes
RuntimeService builds Agent runtime
AgentTurnFlow streams provider/tool events
PromptContext records pending session facts
SessionService finalizes transaction
EventService publishes product events
Adapter renders ProductEvent stream
Runtime returns PromptOutcome
```

### Pseudocode

```rust
async fn adapter_prompt(runtime: &mut CodingAgentRuntime, text: String) -> Result<(), Error> {
    let mut events = runtime.subscribe();

    spawn(async move {
        while let Some(event) = events.recv().await {
            render_product_event(event);
        }
    });

    let op = Operation::Prompt(PromptRequest {
        input: PromptInput::Text(text),
        mode: PromptMode::Normal,
        overrides: RequestOverrides::default(),
    });

    let outcome = runtime.run(op).await?;
    render_final_outcome(outcome);
    Ok(())
}

impl CodingAgentRuntime {
    async fn run(&mut self, op: Operation) -> Result<OperationOutcome, ProductError> {
        let _guard = self.owner.control.start(op.kind())?;

        match op {
            Operation::Prompt(request) => {
                let outcome = self.owner.operations.prompt.execute(request, &mut self.owner).await?;
                Ok(OperationOutcome::Prompt(outcome))
            }
            Operation::PluginLoad(request) => { /* same shape */ }
            Operation::SelfHealingEdit(request) => { /* same shape */ }
            _ => todo!(),
        }
    }
}

impl PromptOperation {
    async fn execute(
        &self,
        request: PromptRequest,
        owner: &mut RuntimeOwner,
    ) -> Result<PromptOutcome, ProductError> {
        let mut ctx = self.prepare(request, owner)?;

        owner.services.events.emit(ProductEvent::prompt_started(&ctx.ids));

        let flow_result = self.run_flow(&mut ctx, &owner.services).await;

        let outcome = match flow_result {
            Ok(_) => ctx.finish_success(),
            Err(error) if ctx.abort_reason.is_some() => ctx.finish_abort(),
            Err(error) => ctx.finish_failure(error),
        };

        let finalized = owner
            .services
            .session
            .finalize_prompt_transaction(ctx.tx.take(), &outcome)?;

        owner.services.events.emit_session_write_events(&finalized);
        owner.services.events.emit_prompt_outcome(&outcome);

        Ok(outcome)
    }
}
```

### PromptTurnFlow Nodes

```rust
fn build_prompt_turn_flow() -> Flow<PromptContext> {
    Flow::linear([
        node("start_prompt_turn", start_prompt_turn),
        node("resolve_request", resolve_request),
        node("prepare_input", prepare_input),
        node("resolve_runtime", resolve_runtime),
        node("load_resources", load_resources),
        node("open_session", open_session),
        node("build_agent_runtime", build_agent_runtime),
        node("record_user_input", record_user_input),
        node("run_agent_turn", run_agent_turn),
        node("finalize_turn", finalize_turn),
        node("emit_completion", emit_completion),
    ])
}

async fn build_agent_runtime(ctx: &mut PromptContext, services: &ServiceContainer) -> Action {
    let snapshot = services.runtime.resolve_snapshot(&ctx.request, &ctx.capabilities)?;
    let agent = services.runtime.build_agent(snapshot, ctx.replay.as_ref())?;
    ctx.runtime = Some(snapshot);
    ctx.agent = Some(agent);
    Action::next()
}

async fn run_agent_turn(ctx: &mut PromptContext, services: &ServiceContainer) -> Action {
    let agent = ctx.agent.as_mut().expect("agent runtime prepared");
    let mut stream = agent.run();

    while let Some(agent_event) = stream.next().await {
        let product_events = services.events.map_agent_event(&ctx.ids, &agent_event);
        ctx.events.extend(product_events);

        let session_events = map_agent_event_to_pending_session_events(&ctx.ids, &agent_event);
        ctx.tx.as_mut().record(session_events)?;

        ctx.assistant.apply(&agent_event);
    }

    if ctx.abort_reason.is_some() {
        Action::abort()
    } else {
        Action::next()
    }
}
```

## Event Ordering And Sequence Semantics

The runtime should use two sequence systems plus explicit causality references.

A single global sequence should not try to solve live UI delivery, durable replay, audit, child operation concurrency, and reconnect semantics at the same time. These are different concerns.

```text
ProductEvent sequence
  live stream delivery order
  used by UI, GUI, RPC, adapters, and reconnect within retention

SessionEvent sequence
  durable session log order
  used by replay, resume, fork, clone, export, and audit

Causality references
  logical relationship across operations, child operations, tool calls, and session writes
```

Short rule:

```text
ProductEvent sequence answers: what did the user-facing stream deliver, and in what order?
SessionEvent sequence answers: what durable facts does the session log contain, and in what order?
Causality answers: why are these events related?
```

### ProductEvent Sequence

`ProductEvent.sequence` should be assigned by the live `EventBus` for a specific event stream.

```rust
pub struct ProductEvent {
    pub stream_id: EventStreamId,
    pub sequence: EventSequence,
    pub operation_id: OperationId,
    pub parent_operation_id: Option<OperationId>,
    pub session_id: Option<SessionId>,
    pub causality: Vec<EventRef>,
    pub durability: EventDurability,
    pub kind: ProductEventKind,
}

pub struct EventRef {
    pub stream_id: Option<EventStreamId>,
    pub sequence: Option<EventSequence>,
    pub event_id: Option<EventId>,
    pub operation_id: Option<OperationId>,
    pub relation: EventRelation,
}

pub enum EventRelation {
    CausedBy,
    ParentOperation,
    ChildOperation,
    ToolCall,
    SessionWrite,
    RuntimeGeneration,
}
```

Semantics:

```text
sequence is strictly increasing within one EventStreamId
sequence is a live delivery order, not a durable session order
sequence does not imply global ordering across runtimes
sequence does not imply business merge order for child operations
```

A TUI, GUI, or RPC client can use `(stream_id, sequence)` to resume a live subscription while retained events are still available.

### SessionEvent Sequence

`SessionEventEnvelope.session_sequence` should be assigned by `SessionLogStore` when durable events are appended.

```rust
pub struct SessionEventEnvelope {
    pub session_id: SessionId,
    pub session_sequence: SessionSequence,
    pub event_id: EventId,
    pub operation_id: OperationId,
    pub turn_id: Option<TurnId>,
    pub data: SessionEventData,
}
```

A committed transaction receives a contiguous durable range.

```text
session seq 101  operation.started
session seq 102  turn.started
session seq 103  turn.input.recorded
session seq 104  message.completed
session seq 105  operation.committed
```

The final lifecycle event closes the durable operation state:

```text
operation.committed
operation.aborted
operation.failed
```

A replay process treats an operation without one of these closing markers as incomplete and applies recovery policy.

### Event Durability

Product events need to say whether the event is live-only, pending persistence, durable, or failed to persist.

```rust
pub enum EventDurability {
    LiveOnly,
    PendingSessionWrite,
    Durable {
        session_id: SessionId,
        sequence_range: std::ops::RangeInclusive<SessionSequence>,
    },
    FailedToPersist {
        reason: String,
    },
}
```

This handles the important case where live UI has already displayed output but session commit later fails.

```text
assistant delta emitted live
assistant text visible in UI projection
session commit fails
runtime emits PromptFailed and/or SessionWriteFailed product event
UI may keep the live output but mark it uncommitted or failed
session replay will not show it as durable history
```

Live product events do not become durable facts merely because they were shown to a user.

### Ordering Invariants

The product event stream should guarantee these local orderings:

```text
OperationStarted before other events for that operation
PromptStarted before assistant/tool/session-write events for that prompt
ToolCallStarted before ToolCallUpdated/Completed/Failed for that tool call
SessionWritePending before SessionWriteCommitted/Skipped/Failed
PromptCompleted/Failed/Aborted after finalization policy runs
CapabilityChanged after the new runtime generation is installed
parent operation completes only after child operations finish, fail, abort, or become durable pending requests
```

These are semantic invariants. They should be tested at adapter/protocol boundaries so UI and RPC clients do not infer state from internal Flow nodes.

### Concurrent Child Event Ordering

Concurrent child operations should not be forced into a fake business order.

```text
ProductEvent.sequence = actual live delivery order
parent_operation_id / child_operation_id = causality
parent merge result = stable business order chosen by parent policy
```

Example:

```text
product seq 40  member_b started
product seq 41  member_a started
product seq 42  member_a completed
product seq 43  member_b completed
product seq 44  team merge completed
```

This is valid even if the team profile lists `member_a` before `member_b`. Live order shows what happened. The parent team operation decides final merge order explicitly.

### Reconnect Semantics

UI/RPC/GUI reconnect should use snapshot plus retained product events.

```text
1. Client requests UiSnapshot.
2. Runtime returns UiSnapshot { last_sequence }.
3. Client subscribes to ProductEvent after last_sequence.
4. If retained events still cover that range, EventBus replays missed product events.
5. If retention no longer covers that range, client requests a fresh snapshot and continues from there.
```

The live event stream is not the durable audit log. It only offers replay within configured retention.

```rust
pub enum SubscribeResult {
    Streaming {
        from: EventSequence,
    },
    Gap {
        requested_after: EventSequence,
        earliest_available: EventSequence,
    },
}
```

If a gap exists, the client must rebuild UI state from a new snapshot.

### Session Replay Semantics

Session replay ignores live-only product events. It reads only durable `SessionEventEnvelope` values.

```text
ProductEvent stream -> live UI truth
SessionEvent log -> durable session truth
UiSnapshot -> projection over durable session truth plus current live runtime state
```

Therefore, ProductEvent and SessionEvent do not need a one-to-one mapping.

Examples:

```text
AssistantMessageDelta ProductEvent
  may be live-only and coalesced
  does not need a durable SessionEvent per token

MessageCompleted SessionEvent
  durable fact recorded at transaction finalization
  may map to one or more ProductEvents

SessionWriteCommitted ProductEvent
  indicates durable sequence range became available
  references SessionEvent sequence range
```

### Ordering Model Summary

The recommended model is:

```text
ProductEvent.sequence
  per EventStreamId live order for UI/RPC/GUI delivery and short reconnect

SessionEvent.session_sequence
  per SessionId durable order for replay/resume/audit

EventRef causality
  cross-links operations, child operations, tool calls, runtime generations, and session writes

EventDurability
  tells consumers whether a live event is durable, pending, live-only, or failed to persist
```

This avoids coupling UI delivery order to session storage order while still making operation causality explicit.

## Snapshot And Event Projection Consistency

A snapshot is a complete UI projection at a declared product-event sequence boundary. Event projection advances from that boundary.

Core formula:

```text
UiState at sequence N
  = Snapshot at N + ProductEvents after N applied in order
```

A snapshot must not be an arbitrary read of current memory. It must declare exactly which live event stream position it includes.

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

Required invariant:

```text
snapshot.cursor.last_product_sequence means:
  the snapshot includes all ProductEvent effects up to and including that sequence

client behavior:
  subscribe only to ProductEvent values after last_product_sequence

if runtime cannot guarantee the invariant:
  return a snapshot error or force the client to retry from a fresh snapshot
```

### Split Durable And Live State

The snapshot should keep durable committed state separate from live running state.

```rust
pub struct CommittedSessionView {
    pub session_id: SessionId,
    pub active_leaf_id: Option<LeafId>,
    pub transcript: TranscriptWindow,
    pub last_session_sequence: Option<SessionSequence>,
}

pub struct LiveOperationView {
    pub operation_id: OperationId,
    pub kind: OperationKind,
    pub status: RunningOperationStatus,
    pub transcript_overlay: Vec<LiveTranscriptBlock>,
    pub tool_calls: Vec<LiveToolCallView>,
    pub uncommitted: bool,
}

pub struct PendingView {
    pub delegation_confirmations: Vec<DelegationConfirmationView>,
    pub plugin_dialogs: Vec<PluginDialogView>,
    pub diagnostics: Vec<DiagnosticView>,
}
```

Semantics:

```text
committed
  built from durable SessionEvent replay or SessionService committed view
  safe for export, resume, fork, clone, and audit

live
  built from active operation memory or ProductEvent projection
  visible to UI but not durable until session finalization succeeds

pending
  user-facing unresolved runtime state such as confirmations and dialogs

cursor
  tells client exactly where to continue the ProductEvent stream
```

### Snapshot Creation

Snapshot creation should establish a short consistency point. It does not need to stop the world for long, but it must choose an event cursor and build a view that includes all effects through that cursor.

```rust
impl CodingAgentRuntime {
    pub fn ui_snapshot(&self) -> Result<UiSnapshot, ProductError> {
        let event_mark = self.owner.event_bus.mark_cursor();

        let committed = self.owner.services.session.committed_view()?;
        let live = self.owner.operation_control.live_view();
        let capabilities = self.owner.services.capabilities.snapshot();
        let runtime_generation = self.owner.services.runtime.generation();
        let pending = self.owner.pending_view();

        Ok(UiSnapshot {
            cursor: SnapshotCursor {
                stream_id: event_mark.stream_id,
                last_product_sequence: event_mark.sequence,
                session_sequence: committed.last_session_sequence,
                runtime_generation,
            },
            committed,
            live,
            pending,
            capabilities,
        })
    }
}
```

Implementation can use a short event-bus mark, read lock, or projection checkpoint. The important property is the externally visible invariant, not the exact locking strategy.

### Projection Must Be Sequential And Idempotent

UI projection must apply events in strict sequence order for a stream. Duplicate events are ignored. Gaps force a new snapshot.

```rust
pub enum ApplyResult {
    Applied,
    DuplicateIgnored,
    GapDetected {
        expected: EventSequence,
        got: EventSequence,
    },
    WrongStream,
}

impl UiProjection {
    pub fn apply(&mut self, event: ProductEvent) -> ApplyResult {
        if event.stream_id != self.cursor.stream_id {
            return ApplyResult::WrongStream;
        }

        if event.sequence <= self.cursor.last_product_sequence {
            return ApplyResult::DuplicateIgnored;
        }

        let expected = self.cursor.last_product_sequence.next();
        if event.sequence != expected {
            return ApplyResult::GapDetected {
                expected,
                got: event.sequence,
            };
        }

        self.apply_event_kind(event.kind);
        self.cursor.last_product_sequence = event.sequence;
        ApplyResult::Applied
    }
}
```

Rule:

```text
Do not guess across gaps.
Do not reconstruct missing internal state in the UI.
On gap, request a fresh snapshot.
```

### Live To Committed Reconciliation

A running prompt produces live UI state before durable session commit.

```text
AssistantMessageDelta ProductEvent
  updates LiveOperationView transcript overlay

AssistantMessageCompleted ProductEvent
  marks live message completed but still uncommitted

SessionWriteCommitted ProductEvent
  tells UI which durable session sequence range was written
  allows UI to reconcile live overlay with committed transcript
```

Stable identifiers are required for reconciliation:

```text
operation_id
turn_id
message_id
tool_call_id
session_id
session_sequence_range
```

Recommended event shape:

```rust
pub enum SessionProductEvent {
    WriteCommitted {
        operation_id: OperationId,
        session_id: SessionId,
        sequence_range: std::ops::RangeInclusive<SessionSequence>,
        committed_items: Vec<CommittedItemRef>,
    },
    WriteFailed {
        operation_id: OperationId,
        reason: String,
    },
}

pub struct CommittedItemRef {
    pub live_id: Option<LiveBlockId>,
    pub session_sequence: SessionSequence,
    pub kind: CommittedItemKind,
}
```

Projection behavior:

```text
on live assistant/tool events:
  update live overlay

on SessionWriteCommitted:
  move or reconcile matching live blocks into committed transcript
  clear pending write status
  update snapshot cursor's session_sequence

on SessionWriteFailed:
  keep live blocks visible if useful
  mark them failed/uncommitted
  do not add them to committed transcript
```

### Transcript Windows And Large Objects

Snapshots should not require loading the whole session into UI memory.

```rust
pub struct TranscriptWindow {
    pub items: Vec<TranscriptBlock>,
    pub has_before: bool,
    pub has_after: bool,
    pub cursor_before: Option<TranscriptCursor>,
    pub cursor_after: Option<TranscriptCursor>,
}

pub enum TranscriptBlockContent {
    InlineText(String),
    BlobRef(BlobRef),
    AttachmentRef(AttachmentRef),
    DiffSummary(DiffSummary),
}
```

Rules:

```text
long transcript -> windowed/paged
large tool output -> BlobRef
images/files -> AttachmentRef
large diffs -> summary plus lazy full content
```

This keeps TUI and GUI responsive and gives the same projection model to local and remote clients.

### Snapshot And Projection Rules

The consistency model is:

```text
Snapshot gives UI a trusted starting point.
ProductEvent advances UI state from that point.
Projection applies events only in sequence.
Projection treats duplicate events as harmless.
Projection treats gaps as fatal to incremental state and requests a fresh snapshot.
Live and committed state remain separate until SessionWriteCommitted reconciles them.
Large durable content is referenced, not embedded by default.
```

This makes reconnect, slow clients, live prompt rendering, and durable session replay share one clear model.

## Capability Authorization Lifecycle

Capabilities should be modeled as generated, scoped, operation-local authorization snapshots. They should not be global booleans read dynamically by running operations.

The lifecycle has three layers:

```text
Declared Capability
  what a provider, model, plugin, tool, adapter, or runtime feature says it can do

Granted Capability
  what current policy, user trust, workspace trust, settings, auth, and runtime state allow

Operation Capability Snapshot
  the frozen capability view available to one operation after it starts
```

Short rule:

```text
Declaration means "can".
Grant means "may".
Snapshot means "may for this operation".
Capability handles define "how".
Generation defines "when this authorization state applies".
Revocation policy defines "whether active operations are affected".
```

### Registry And Snapshot Shape

```rust
pub struct CapabilityRegistry {
    declarations: CapabilityDeclarations,
    grants: CapabilityGrantStore,
    generation: CapabilityGeneration,
}

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

pub enum ActorId {
    User,
    Runtime,
    Plugin(PluginId),
    AgentProfile(ProfileId),
    TeamProfile(ProfileId),
    DelegatedOperation(OperationId),
}
```

An operation receives one `OperationCapabilitySnapshot` during prepare/runtime resolution and uses only handles from that snapshot.

### Authorization Lifecycle

```text
1. Declare
   providers, plugins, tools, profiles, and adapters register available capability declarations

2. Grant
   policy evaluates declarations against trust, user settings, workspace settings, auth, and runtime state

3. Snapshot
   operation start freezes the granted capabilities for the operation actor and scope

4. Use
   nodes, tools, plugins, and workflows use narrow capability handles from the snapshot

5. Revoke
   global grant changes create a new generation and affect future operations by default

6. Cancel or interrupt
   high-risk revocations may cancel or interrupt matching active operations explicitly
```

This avoids a running prompt seeing its tool set, filesystem permissions, model config, or plugin host surface change halfway through execution.

### Why Active Operations Do Not Hot-Update By Default

Hot-updating active capability snapshots creates ambiguous behavior.

```text
prompt starts with tool A available
plugin reload removes tool A
model emits a tool A call after reload
runtime must decide whether to execute, reject, or rewrite the call
```

The stable rule is:

```text
running operation sees its frozen snapshot
runtime mutation creates a new capability generation
future operations see the new generation
active operations change only through explicit revoke/cancel policy
```

This makes operation behavior explainable and reproducible.

### Revocation Policy

Not all revocations have the same risk. Capability changes should declare how they affect active operations.

```rust
pub enum RevokePolicy {
    FutureOnly,
    CancelMatchingOperations,
    DenyNextUse,
}

pub struct CapabilityRevocation {
    pub generation: CapabilityGeneration,
    pub capability: CapabilityKey,
    pub actor_filter: ActorFilter,
    pub policy: RevokePolicy,
    pub reason: String,
}
```

Recommended defaults:

```text
plugin reload/profile/settings/model changes
  FutureOnly

auth secret removed/workspace trust revoked/filesystem permission revoked
  CancelMatchingOperations

temporary quota exceeded/rate limit/soft budget exceeded
  DenyNextUse or CancelMatchingOperations depending on operation kind
```

`DenyNextUse` is useful for capabilities that can fail safely at the next call boundary. `CancelMatchingOperations` is required when continued access would violate trust or security expectations.

### Narrow Capability Handles

A capability snapshot should not expose raw services. It should expose narrow handles that enforce scope.

```rust
pub struct FilesystemCapability {
    root: PathScope,
    access: AccessMode,
    token: CapabilityToken,
}

impl FilesystemCapability {
    pub async fn read(&self, path: RelativePath) -> Result<Bytes, CapabilityError>;
    pub async fn write(
        &self,
        path: RelativePath,
        content: Bytes,
    ) -> Result<(), CapabilityError>;
}
```

Other examples:

```text
ShellCapability
  allowed commands, cwd, timeout, environment policy

SessionReadCapability
  view/replay/query only, no raw store access

SessionWriteCapability
  transaction append/finalize only, no arbitrary manifest mutation

ModelCapability
  selected model and provider streamer, no raw auth secret access

PluginCapability
  stable host functions only, no RuntimeService or SessionService access

UiCapability
  controlled UI actions/dialog/keybinds, no raw TUI or GUI runtime access
```

Capability handles are the permission boundary. Services remain internal implementation owners.

### CapabilityChanged Events

`CapabilityChanged` describes a new generation. It does not imply active operations silently changed.

```rust
pub enum CapabilityEvent {
    Changed {
        generation: CapabilityGeneration,
        affects_future_operations: bool,
        active_operations_cancelled: Vec<OperationId>,
        reason: String,
    },
}
```

UI wording should reflect this distinction:

```text
future operations may have different capabilities
current operation continues with its original snapshot unless cancelled
cancelled operations are listed explicitly
```

This prevents UI from implying that an active prompt's tools or model changed while it was running.

### Capability Invariants

```text
operations use only their OperationCapabilitySnapshot
capability snapshots are immutable after operation start
runtime mutations create new CapabilityGeneration values
FutureOnly revocations do not affect active operations
CancelMatchingOperations revocations emit cancellation/failure events for affected operations
DenyNextUse revocations fail at the next capability handle call boundary
plugins never receive raw session/runtime/provider/auth internals
capability changes are visible through CapabilityChanged product events
```

This turns authorization into a versioned operation-local contract instead of scattered runtime conditionals.

## Error, Failure, And Recovery Strategy

Failures should be modeled as first-class operation outcomes. They should not leak as unstructured exceptions from providers, tools, storage, plugins, or UI adapters.

The runtime goal is simple:

```text
no ambiguous half-success
no silent durable corruption
no UI success before required persistence succeeds
no incomplete transcript promoted to normal history
```

An operation may stream partial live output, but its final meaning is defined only by its terminal outcome.

### Operation Outcome Model

```rust
pub enum OperationOutcome {
    Succeeded {
        persistence: PersistenceOutcome,
    },
    Aborted {
        reason: AbortReason,
    },
    Failed {
        error: ProductError,
    },
    RecoveryRequired {
        reason: RecoveryReason,
    },
}

pub enum PersistenceOutcome {
    NotRequired,
    Skipped,
    Committed {
        session_sequence: Option<SessionSequence>,
        product_sequence: ProductSequence,
    },
    Failed {
        error: ProductError,
    },
    InDoubt {
        recovery_id: RecoveryId,
    },
}
```

A user interrupt is `Aborted`, not `Failed`. A provider crash, tool error, plugin failure, capability denial, projection error, or storage error is `Failed` unless it leaves the runtime unable to classify durable state, in which case the operation becomes `RecoveryRequired` or `InDoubt`.

### Error Shape

Errors should be typed at the product boundary.

```rust
pub struct ProductError {
    pub category: ErrorCategory,
    pub phase: FailurePhase,
    pub retry: RetryAdvice,
    pub user_message: String,
    pub diagnostic: DiagnosticInfo,
}

pub enum ErrorCategory {
    Input,
    Config,
    Auth,
    Capability,
    Provider,
    Tool,
    Plugin,
    SessionStore,
    Projection,
    Concurrency,
    Cancelled,
    Internal,
}

pub enum FailurePhase {
    Prepare,
    Run,
    Finalize,
    Commit,
    Project,
    Recover,
}

pub enum RetryAdvice {
    DoNotRetry,
    RetryAfterBackoff,
    RetryAfterUserAction,
    RetryAfterRecovery,
}
```

`user_message` is safe for UI. `diagnostic` is for logs, traces, and support bundles. Secrets, raw provider payloads, and auth material must not enter product events.

### Operation State Machine

```text
Pending
  -> Started
  -> Running
  -> Finalizing
  -> Succeeded

Running / Finalizing
  -> Aborted
  -> Failed
  -> RecoveryRequired
```

The runtime should emit exactly one terminal product event for each started operation.

```rust
pub enum OperationTerminalEvent {
    Succeeded {
        operation_id: OperationId,
        persistence: PersistenceOutcome,
    },
    Aborted {
        operation_id: OperationId,
        reason: AbortReason,
    },
    Failed {
        operation_id: OperationId,
        error: ProductError,
    },
    RecoveryRequired {
        operation_id: OperationId,
        reason: RecoveryReason,
    },
}
```

Live streaming events may precede the terminal event. Consumers must treat the terminal event as the authoritative outcome.

### Session Transaction Semantics

Session persistence needs its own transaction state because live product events and durable session events have different failure modes.

```rust
pub enum SessionTransactionState {
    NotStarted,
    Open {
        operation_id: OperationId,
        first_sequence: Option<SessionSequence>,
    },
    Finalizing,
    Committed {
        last_sequence: SessionSequence,
    },
    Aborted,
    Failed {
        error: ProductError,
    },
    InDoubt {
        recovery_id: RecoveryId,
    },
}
```

Recommended behavior:

```text
failure before session transaction starts
  emit operation failed product event
  do not write session events

failure while transaction is open
  append operation.failed or operation.aborted if possible
  close open message/tool-call families as failed or cancelled
  then finalize the transaction as failed/aborted

failure while committing
  if no bytes were committed, fail the operation
  if commit definitely succeeded but manifest/index update failed, recover indexes from log
  if commit state is unknown, mark InDoubt and require recovery

failure after durable commit but before live projection catches up
  keep durable log as source of truth
  replay/project until snapshot reaches the committed cursor
```

If durable session write is required for the operation, the runtime must not emit `Succeeded` before durable commit succeeds.

### Partial Output Rules

Streaming makes partial output visible before an operation is durable. The projection must keep that distinction explicit.

```text
live assistant deltas
  may appear as pending live output

committed assistant message
  appears only after message family closes and commit succeeds

failed or aborted assistant output
  may remain visible in live status/history diagnostics
  must not become normal committed transcript content unless explicitly marked
```

Tool calls follow the same rule: an opened tool call must eventually close with success, failure, cancellation, or recovery marker.

### Recovery On Startup

On startup, session load, or explicit repair, the runtime should scan for incomplete durable state.

```rust
pub struct RecoveryScanResult {
    pub session_id: SessionId,
    pub incomplete_operations: Vec<OperationId>,
    pub open_message_families: Vec<MessageId>,
    pub open_tool_calls: Vec<ToolCallId>,
    pub manifest_out_of_sync: bool,
    pub staged_blobs: Vec<BlobRef>,
}
```

Recovery should be conservative.

```text
if session log is complete but manifest/index is stale
  rebuild manifest/index from the session log

if an operation has no terminal marker
  append a recovered failure/abort marker when possible
  close open event families with recovery markers

if staged blobs are unreferenced by committed events
  clean or quarantine them

if commit status is unknown
  keep the operation InDoubt
  require explicit repair or replay evidence before presenting it as committed
```

Recovery never silently promotes incomplete output to committed transcript history.

### Retry Semantics

Retries must respect idempotency and side effects.

```text
input/config/auth/capability errors
  do not auto-retry; require user or policy change

provider transient/network/rate errors
  retry with bounded backoff only while operation side effects remain safe

tool errors
  retry only if the tool declares an idempotency key and retry policy

session write errors
  retry only when the storage layer can prove no partial commit occurred
  otherwise mark InDoubt and recover

plugin hook errors
  fail open or fail closed according to hook policy
```

Operations, tool calls, provider requests, and staged writes should carry idempotency keys.

```rust
pub struct IdempotencyScope {
    pub operation_id: OperationId,
    pub request_id: RequestId,
    pub tool_call_id: Option<ToolCallId>,
    pub attempt: AttemptNumber,
}
```

A retry must not duplicate durable session events or external tool side effects.

### Abort And Cancellation

Cancellation is a controlled terminal path.

```text
user abort
  OperationOutcome::Aborted

capability revoke with CancelMatchingOperations
  OperationOutcome::Aborted or Failed depending on policy reason

shutdown with graceful drain
  allow finalization up to timeout, then abort active operations

shutdown without durable finalization
  recovery scan resolves incomplete operations on next startup
```

The UI should present abort separately from failure. Abort means the runtime obeyed a control command; failure means an operation could not complete as requested.

### Projection And UI Behavior

Projection should make recovery state visible without exposing internal storage details.

```rust
pub enum UiOperationStatus {
    Pending,
    Running,
    Finalizing,
    Succeeded,
    Aborted,
    Failed,
    RecoveryRequired,
}
```

UI projection rules:

```text
show live deltas as pending while operation runs
replace pending output with committed transcript after commit/projection
mark failed output as failed, not committed
mark aborted output as aborted/cancelled
surface recovery_required when durable state is ambiguous
```

Reconnect uses the snapshot cursor and terminal operation events to reconcile pending live state with committed state.

### Failure And Recovery Invariants

```text
each started operation emits exactly one terminal product event
user abort is not provider/tool/runtime failure
required durable commit must happen before operation success is emitted
open session event families must close with success, failure, abort, or recovery marker
session log is the source of truth for committed history
manifest, indexes, and projections are rebuildable from session log
recovery never silently promotes incomplete output to committed transcript
retries require explicit idempotency and retry policy
unknown commit state becomes InDoubt rather than guessed success
UI displays failed, aborted, and recovery-required states distinctly
```

This makes failure a runtime protocol rather than an accidental exception path.

## Protocol Versioning

Protocol versioning should be scoped by protocol family, not represented as one global runtime version. Different boundaries change at different speeds and carry different compatibility obligations.

The core rule:

```text
session event protocol optimizes for long-term readability
product event and UI command protocols optimize for negotiated compatibility
snapshot protocol optimizes for rebuildability
plugin, capability, and tool schema protocols optimize for stable extension boundaries
```

### Protocol Families

```rust
pub enum ProtocolFamily {
    SessionEvent,
    ProductEvent,
    UiCommand,
    Snapshot,
    PluginHost,
    Capability,
    ToolSchema,
}

pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

pub struct ProtocolEnvelope<T> {
    pub family: ProtocolFamily,
    pub version: ProtocolVersion,
    pub features: FeatureSet,
    pub payload: T,
}
```

Patch versions do not belong in durable or wire protocols. Patch-level changes are crate/internal implementation details.

### Compatibility Rules

```text
major version change
  incompatible protocol contract change

minor version change
  backward-compatible addition or optional feature

feature flag
  named opt-in behavior within a compatible major/minor range

unknown required feature
  reject with ProtocolUnsupported

unknown optional feature
  ignore if the family allows forward-compatible decode
```

A consumer must never guess across major versions.

```rust
pub struct ProtocolUnsupported {
    pub family: ProtocolFamily,
    pub requested: ProtocolVersion,
    pub supported: VersionRange,
    pub missing_features: Vec<FeatureKey>,
}
```

### SessionEvent Versioning

`SessionEvent` is the most conservative protocol because it is durable history.

```rust
pub struct SessionLogHeader {
    pub session_id: SessionId,
    pub protocol: ProtocolVersion,
    pub created_by: RuntimeBuildId,
    pub features: FeatureSet,
}

pub struct SessionEventRecord {
    pub session_sequence: SessionSequence,
    pub event_id: EventId,
    pub protocol: ProtocolVersion,
    pub kind: SessionEventKind,
}
```

Rules:

```text
old session logs must remain readable
historical facts are not rewritten by default
readers decode old event records into the current domain model
migration may write a new snapshot/checkpoint, not mutate old facts
unsupported required features prevent opening the session for normal use
```

This keeps the durable log as a stable source of truth while still allowing the current projection model to evolve.

### ProductEvent And UiCommand Negotiation

Live UI/RPC protocols should negotiate versions at connection time.

```rust
pub struct ProtocolHello {
    pub client: ClientIdentity,
    pub supported: Vec<SupportedProtocol>,
}

pub struct ProtocolAccepted {
    pub selected: Vec<SelectedProtocol>,
    pub runtime: RuntimeIdentity,
}

pub struct SupportedProtocol {
    pub family: ProtocolFamily,
    pub versions: VersionRange,
    pub features: FeatureSet,
}

pub struct SelectedProtocol {
    pub family: ProtocolFamily,
    pub version: ProtocolVersion,
    pub features: FeatureSet,
}
```

Example:

```text
client supports ProductEvent v1.0..v1.3, Snapshot v1.1, UiCommand v1.0
runtime selects ProductEvent v1.2, Snapshot v1.1, UiCommand v1.0
```

If no compatible version exists, the runtime rejects the connection with `ProtocolUnsupported` instead of silently degrading behavior.

### Snapshot Versioning

Snapshots are rebuildable products of event projection. Their versioning can be more aggressive than session logs.

```text
snapshot major version mismatch
  discard snapshot and rebuild from session events when possible

snapshot minor version mismatch
  upgrade in memory if rules are known

snapshot references unsupported event features
  mark snapshot unusable and recover from event log
```

Snapshots should include their source cursor.

```rust
pub struct VersionedSnapshot<T> {
    pub family: ProtocolFamily,
    pub version: ProtocolVersion,
    pub source: SnapshotSourceCursor,
    pub payload: T,
}
```

The event log remains authoritative; snapshot compatibility failures should not corrupt committed history.

### Plugin, Capability, And Tool Schema Versions

Extension boundaries need explicit protocol contracts.

```rust
pub struct PluginHostProtocol {
    pub host_api: ProtocolVersion,
    pub capability_protocol: ProtocolVersion,
    pub tool_schema_protocol: ProtocolVersion,
    pub features: FeatureSet,
}
```

Rules:

```text
plugin host API major mismatch
  plugin cannot load

capability protocol major mismatch
  grants cannot be interpreted safely, plugin cannot receive those capabilities

tool schema major mismatch
  tool cannot be exposed to model/runtime

tool schema minor addition
  allowed only if unknown fields are optional or ignored by contract
```

Capability keys should be stable semantic identifiers, not Rust type names or internal module paths.

### Migration Policy

Protocol migration should be explicit and mostly lazy.

```text
read old durable events through versioned decoders
project into current in-memory domain types
write new events using the current protocol version
write checkpoints/snapshots in the current snapshot protocol
avoid rewriting old session facts unless a dedicated repair/migration command is invoked
```

If an upgrade needs destructive or lossy migration, it should require an explicit operation and emit migration events.

### Protocol Versioning Invariants

```text
no global runtime version substitutes for family-specific protocol versions
major mismatch is rejected unless a decoder explicitly supports it
minor versions only add backward-compatible behavior
session event history remains readable across supported versions
snapshots are disposable and rebuildable from durable events
UI/RPC connections negotiate ProductEvent, UiCommand, and Snapshot protocols
plugins load only when host, capability, and tool schema protocols are compatible
unsupported required features fail closed with ProtocolUnsupported
```

This keeps compatibility rules local to the boundary that actually changed.

## Backpressure And Flow Control

Backpressure is a runtime protocol, not a channel buffer size. Every asynchronous boundary needs an explicit capacity, priority, overflow policy, downgrade policy, and terminal behavior.

The default rule:

```text
facts are never silently dropped
derived state can be rebuilt
high-frequency live streams can be coalesced
slow consumers must not stall session commit
pressure beyond budget must become an explicit event, rejection, disconnect, or cancellation
```

### Event Classes And Pressure Policy

```text
SessionEvent
  durable fact; never drop; if it cannot be written, the operation fails or enters recovery

operation terminal ProductEvent
  semantic terminal outcome; never silently drop; slow live subscribers reconnect from snapshot

token delta / progress / loader frame
  live display stream; may be throttled, coalesced, or drop-old-keep-new

snapshot
  derived projection; latest wins; old snapshots may be discarded

tool stdout/stderr/log stream
  bounded live stream; may be truncated but must emit a truncation marker

UI command
  bounded input queue; reject with Busy/Backpressure instead of growing forever
```

This classification keeps durable truth separate from presentation traffic.

### Flow Control Boundaries

```text
UI command queue
  controls how fast users, TUI, RPC, and future GUI can submit runtime commands

operation scheduler permits
  controls concurrent Operation and child Operation execution

model stream aggregator
  absorbs high-frequency provider deltas and emits paced ProductEvent updates

session writer queue
  controls durable writes and forces failure/recovery when storage cannot keep up

product event bus
  isolates live subscribers so a slow UI/RPC client does not block runtime progress

projection/snapshot builder
  coalesces repeated projection requests and keeps the latest requested cursor

plugin/tool stream bridge
  bounds tool/plugin output and converts overflow into semantic truncation events
```

Each boundary should be small enough to expose pressure early and large enough to avoid normal jitter becoming failure.

### Flow Control Policy Shape

```rust
pub struct FlowControlPolicy {
    pub queue_capacity: usize,
    pub high_watermark: usize,
    pub low_watermark: usize,
    pub overflow: OverflowPolicy,
    pub priority: PriorityPolicy,
}

pub enum OverflowPolicy {
    RejectNew,
    DropOldest,
    DropNewest,
    Coalesce,
    DisconnectSubscriber,
    CancelOperation,
}

pub enum EventPriority {
    DurableFact,
    TerminalOutcome,
    Control,
    UserVisibleStatus,
    LiveDelta,
    Diagnostic,
}
```

`DurableFact`, `TerminalOutcome`, and `Control` events should not be displaced by ordinary live deltas.

### Coalescing Rules

Coalescing must preserve meaning.

```text
multiple token deltas
  may merge into one text delta with a range/cursor

multiple progress ticks
  may keep the latest progress value

multiple loader frames
  may keep only the latest frame

multiple snapshot builds for increasing cursors
  may keep the latest cursor request

multiple terminal outcomes for one operation
  invalid; this is a runtime bug

session events
  never coalesce after they become facts
```

If a stream is truncated or coalesced in a way visible to the user, the runtime should emit an explicit marker.

```rust
pub enum StreamPressureEvent {
    Coalesced {
        stream: StreamId,
        dropped_count: u64,
    },
    Truncated {
        stream: StreamId,
        reason: TruncationReason,
    },
    SubscriberDisconnected {
        subscriber: SubscriberId,
        reason: DisconnectReason,
    },
    CommandRejected {
        command_id: CommandId,
        reason: BackpressureReason,
    },
}
```

### Slow Subscriber Semantics

Live subscribers are replaceable. Runtime progress is not.

```text
if a TUI/RPC/GUI subscriber falls behind
  first coalesce live deltas when allowed
  then send pressure markers when possible
  then disconnect that subscriber
  subscriber reconnects using snapshot cursor and replayable events
```

A slow subscriber must never block:

```text
session commit
operation terminal event creation
capability revocation
abort/cancel control path
recovery markers
```

This keeps UI failure isolated from runtime correctness.

### Session Writer Pressure

Session writer pressure is different from UI pressure because durable facts cannot be dropped.

```text
if session writer queue reaches high watermark
  stop accepting new SessionWrite operations for that session
  allow abort/control/recovery operations through priority path

if current operation requires durable commit and writer fails
  operation fails or enters InDoubt/RecoveryRequired

if current operation is live-only
  operation may continue with PersistenceOutcome::Skipped only if policy allows it
```

The runtime must not hide storage backpressure behind unbounded memory growth.

### UI Command Pressure

UI command pressure should be visible to callers.

```rust
pub enum CommandAdmission {
    Accepted {
        operation_id: OperationId,
    },
    Rejected {
        reason: BackpressureReason,
    },
    Deferred {
        queue_position: usize,
    },
}
```

Recommended defaults:

```text
prompt submission while session writer is pressured
  reject or defer according to product policy

abort/cancel command
  priority admission even under pressure

settings/plugin reload/profile switch
  admit only if the scheduler can serialize it safely

high-frequency UI typing/editing state
  coalesce before crossing runtime boundary
```

### Provider And Tool Stream Pressure

Provider and tool streams should be consumed through runtime-owned adapters.

```text
provider token stream
  aggregate and pace into ProductEvent deltas

tool stdout/stderr
  store bounded tail/head policy or blob reference if product requires inspection

plugin emitted status
  bounded and namespaced by plugin id
```

A provider or tool that outpaces the runtime should be paused if the protocol supports it; otherwise the runtime should bound memory and eventually cancel the operation with a structured failure.

### Backpressure Invariants

```text
no unbounded runtime channel exists by default
slow UI subscribers cannot block session commit
session commit cannot be delayed by token rendering
terminal events are never displaced by progress events
all dropping, truncation, coalescing, rejection, and disconnects have semantic markers
abort/cancel/recovery paths retain priority under pressure
reconnect can restore a consistent view from snapshot and replayable events
storage pressure becomes operation failure/recovery, not silent data loss
```

This lets the runtime remain correct under load while sacrificing only presentation fidelity where the protocol allows it.

## Persistent Facts, Product Semantics, And UI Projection

The runtime should separate durable facts, product semantics, and UI state as different layers.

```text
SessionEvent = fact
  durable history, replayable, auditable, and the source of session truth

ProductEvent = semantic event
  runtime-facing meaning for TUI/RPC/GUI; may be durable-derived or live-only

UiState = projection
  current view state derived from snapshots and product events; disposable and rebuildable
```

`ProductEvent` is not automatically persisted. `UiState` is never a source of truth. Persistence is decided by whether the runtime must be able to acknowledge the event as historical fact after restart, replay, audit, recovery, or reconnect.

### Layer Responsibilities

```rust
pub struct SessionEventRecord {
    pub session_sequence: SessionSequence,
    pub event_id: EventId,
    pub operation_id: OperationId,
    pub kind: SessionEventKind,
}

pub struct ProductEventRecord {
    pub product_sequence: ProductSequence,
    pub stream_id: EventStreamId,
    pub operation_id: Option<OperationId>,
    pub durability: EventDurability,
    pub kind: ProductEventKind,
}

pub struct UiState {
    pub cursor: ProjectionCursor,
    pub operations: OperationViewSet,
    pub transcript: TranscriptView,
    pub pending: PendingLiveState,
    pub diagnostics: DiagnosticView,
}
```

The direction is one-way:

```text
SessionEvent -> ProductEvent -> UiState
Runtime live state -> ProductEvent -> UiState
UiState does not write SessionEvent
ProductEvent does not imply SessionEvent unless mapped explicitly
```

### Durability Classification

```rust
pub enum EventDurability {
    DurableFact {
        session_sequence: SessionSequence,
    },
    DerivedFromFact {
        source: EventRef,
    },
    LiveOnly,
    ProjectionOnly,
}

pub enum PersistencePolicy {
    MustPersist,
    LiveOnly,
    ConditionalPersist,
    DerivedProjection,
}
```

A `ProductEvent` should carry enough durability metadata for UI/RPC clients to distinguish committed history from pending live display.

### Must Persist As SessionEvent

These facts must be written to the session log when they are part of a session operation:

```text
user prompt/message
assistant committed message
assistant message boundary and final content reference
tool call request
tool call result/failure/cancellation
operation started/finished/failed/aborted/recovered terminal marker
session branch creation, active leaf change, and parent relation
model invocation metadata needed for replay/audit
selected model/provider profile and usage summary
capability generation/reference used by the operation
compaction, summary, and checkpoint facts
abort/failure/recovery marker
durable file/edit/artifact reference
migration or repair marker
```

The rule is: if losing the event changes what the session historically contains or what recovery must know, it is a `SessionEvent`.

### Live-Only ProductEvent

These events normally remain live-only:

```text
token delta before committed message close
progress tick
loader/spinner frame
typing, caret, focus, viewport, and selection state
transient toast or notification
queue depth, scheduler heartbeat, and runtime health pulse
provider raw stream chunk
retry attempt detail that does not affect durable outcome
UI command echo
autocomplete/menu highlight
hover/preview state
subscriber pressure or dropped-frame diagnostics unless needed for audit
```

Live-only events may affect `UiState`, but they should not become session history.

### Conditional Persistence

Some event families need explicit rules.

```text
assistant streaming delta
  ProductEvent live-only while streaming
  SessionEvent only when message family is committed or explicitly failed/aborted

tool stdout/stderr
  ProductEvent live stream while running
  SessionEvent stores final status, summary, truncation marker, and artifact/blob reference if needed

provider raw response
  not persisted by default
  persist normalized message/tool/usage/error facts only

error event
  live-only if it only explains transient UI state
  durable if it determines operation terminal outcome, recovery, audit, or transcript validity

snapshot
  not a fact
  may be cached/checkpointed but must remain rebuildable from SessionEvent

capability change
  ProductEvent announces live runtime semantic change
  SessionEvent records capability generation only when an operation/session fact depends on it

backpressure/truncation
  live-only for UI presentation pressure
  durable when it changes committed tool output, transcript content, or recovery state
```

### Mapping Shapes

A session fact may produce one or more product semantic events.

```rust
pub struct EventMappingRule {
    pub session_kind: SessionEventKindTag,
    pub product_kind: ProductEventKindTag,
    pub durability: EventDurabilityRule,
    pub projection_effect: ProjectionEffect,
}

pub enum ProjectionEffect {
    AppendTranscript,
    UpdateOperationStatus,
    UpdateToolView,
    UpdateCapabilityView,
    UpdateDiagnostics,
    NoUiEffect,
}
```

Example mapping:

```text
SessionEvent::UserMessageCommitted
  -> ProductEvent::Transcript(UserMessageCommitted)
  -> UiState.transcript append committed user message

ProductEvent::AssistantTokenDelta(live-only)
  -> UiState.pending assistant draft update
  -> no SessionEvent until commit/failure/abort

SessionEvent::ToolCallFinished
  -> ProductEvent::Tool(ToolCallFinished)
  -> UiState tool view closes with durable result
```

The projection layer should be deterministic and idempotent for durable-derived events. Live-only events may be dropped or coalesced according to backpressure rules.

### UI State Projection Rules

```text
committed transcript view
  built from SessionEvent-derived ProductEvents

pending live assistant output
  built from live-only ProductEvents and cleared/reconciled on commit/failure/abort

operation status
  built from product semantic events, terminal outcome wins

tool views
  show live progress while running, reconcile to durable final result when available

diagnostics
  may include live-only details, but durable failure/recovery facts are marked separately
```

A reconnecting client should load a snapshot at a cursor, then replay product events after that cursor. If live-only deltas were missed, the client should receive a coherent snapshot and continue, not require every old display delta.

### Persistence Decision Test

Before introducing a new event, answer these questions:

```text
must replay after restart know this happened?
would losing it change transcript/history/recovery/audit?
does it close or validate an event family?
does it determine operation terminal outcome?
does it describe external side effects or durable artifacts?
```

If the answer is yes, create or map to a `SessionEvent`. If the answer is no and it only improves current display, keep it live-only.

### Mapping Invariants

```text
SessionEvent is the only durable session fact source
ProductEvent expresses runtime semantics and may be durable-derived or live-only
UiState is a projection and never a source of truth
committed transcript content comes only from durable facts
token deltas are not durable transcript until message commit
terminal operation outcome is durable when the operation opened a session transaction
snapshots/checkpoints are rebuildable and cannot override event facts
live-only loss may reduce presentation fidelity but must not corrupt history
conditional event families must document their persistence boundary
```

This keeps the architecture honest about what the system remembers, what it tells clients, and what the UI merely displays.

## Multi-Client Runtime Model

The runtime owns sessions, operations, scheduling, persistence, and product event streams. UI clients own connections, subscriptions, local view state, and unsubmitted drafts.

The core rule:

```text
multiple UI clients may connect to one runtime
multiple clients may submit intents
all runtime-affecting intents pass through one IntentRouter and scheduler admission path
unsubmitted prompt input is client-local by default
submitted prompts become runtime-owned operations
aborting an operation affects all clients that observe that operation, but not every client is authorized to abort
```

This model supports today's TUI and leaves room for future GUI windows, RPC clients, and automation clients without turning UI state into shared mutable runtime state.

### Client Identity And Connection State

```rust
pub struct ClientConnection {
    pub client_id: ClientId,
    pub client_kind: ClientKind,
    pub actor: ActorId,
    pub protocol: Vec<SelectedProtocol>,
    pub capabilities: ClientCapabilitySet,
    pub subscriptions: SubscriptionSet,
    pub connected_at: RuntimeInstant,
}

pub enum ClientKind {
    Tui,
    GuiWindow,
    RpcClient,
    HeadlessAutomation,
    TestHarness,
}
```

A client connection is not a session owner. It is a participant with a subscription and a set of capabilities.

### Client-Local State

The following state should stay client-local unless a future feature explicitly promotes it to a shared runtime concept:

```text
prompt draft text
cursor position
selection
IME composition state
viewport/scroll position
focused panel
hover/preview state
menu highlight
autocomplete selection
window layout
local undo stack for unsubmitted text
```

This prevents multiple GUI windows from fighting over focus, cursor movement, input method composition, and draft edits.

### Prompt Input Ownership

Default behavior:

```text
each client has its own DraftBuffer
editing the prompt mutates only that client-local draft
submit captures a draft snapshot
runtime receives PromptSubmitted intent
runtime creates an Operation if admission succeeds
the operation records initiator_client_id but is owned by the runtime/session
```

Shape:

```rust
pub struct PromptSubmittedIntent {
    pub client_id: ClientId,
    pub actor: ActorId,
    pub session_id: SessionId,
    pub draft_snapshot: PromptDraftSnapshot,
    pub expected_session_cursor: Option<SessionCursor>,
}

pub struct PromptDraftSnapshot {
    pub text: String,
    pub attachments: Vec<AttachmentRef>,
    pub selected_model: Option<ModelSelection>,
    pub local_draft_id: DraftId,
}
```

Shared prompt editing should be a separate future feature, not the default input model. If needed, introduce an explicit lease:

```rust
pub struct PromptInputLease {
    pub lease_id: LeaseId,
    pub session_id: SessionId,
    pub owner: ClientId,
    pub expires_at: RuntimeInstant,
}
```

A lease should be reserved for true collaborative editing or remote-control workflows. It should not be required for ordinary multi-window use.

### Intent Routing And Admission

Runtime-affecting intents cross one boundary.

```rust
pub struct ClientIntentEnvelope<T> {
    pub client_id: ClientId,
    pub actor: ActorId,
    pub command_id: CommandId,
    pub observed_cursor: Option<ProjectionCursor>,
    pub intent: T,
}

pub enum IntentAdmission {
    Accepted {
        operation_id: Option<OperationId>,
    },
    Rejected {
        reason: IntentRejectReason,
    },
    Deferred {
        queue_position: usize,
    },
}
```

The `IntentRouter` performs:

```text
protocol validation
client capability authorization
session/operation existence checks
optimistic cursor checks when required
scheduler admission
backpressure handling
operation creation or control dispatch
```

No UI client bypasses this path by holding direct service references.

### Concurrent Intent Semantics

Different intent families have different concurrency rules.

```text
client-local UI intent
  stays inside the client; no runtime ordering needed

read-only runtime intent
  may run concurrently when capability and scheduler allow

session-write prompt intent
  follows per-session single-writer admission; concurrent prompts are queued, deferred, or rejected

control intent such as abort/cancel
  priority admission; targets an existing operation

runtime-write intent such as plugin reload/profile change/settings mutation
  serialized through RuntimeWrite rules

active leaf/session navigation mutation
  requires compare-and-set cursor or generation check
```

Concurrent clients do not create multiple writers to session state. They create multiple intent sources that the runtime serializes according to operation class.

### Abort And Detach Semantics

Abort semantics must distinguish local connection behavior from runtime operation control.

```rust
pub enum ClientControlIntent {
    DetachClient {
        client_id: ClientId,
    },
    AbortOwnOperation {
        operation_id: OperationId,
    },
    AbortOperation {
        operation_id: OperationId,
    },
    AbortSessionOperations {
        session_id: SessionId,
    },
}
```

Recommended meanings:

```text
DetachClient
  closes or unsubscribes one client; does not abort runtime operations

AbortOwnOperation
  allowed for the initiating client or actor when policy permits; aborts the runtime operation globally

AbortOperation
  requires OperationControl capability; aborts the targeted runtime operation globally

AbortSessionOperations
  requires stronger SessionControl capability; aborts matching operations for the session
```

Because operations are runtime/session-owned, aborting an operation changes the shared operation state. All clients observing that operation receive the resulting product event and projection update.

### Multi-Client Visibility

Shared runtime facts and semantic events are visible through subscriptions.

```text
session facts
  affect all clients subscribed to that session

operation product events
  affect all clients subscribed to that operation or session stream

client-local UI state
  visible only to that client unless explicitly shared

admission result
  sent directly to the submitting client and may also produce shared product events
```

Product events should include initiator metadata when useful.

```rust
pub struct EventInitiator {
    pub client_id: Option<ClientId>,
    pub actor: ActorId,
    pub command_id: Option<CommandId>,
}
```

This lets UIs explain which client/user initiated a shared operation without making the operation private to that client.

### Reconnect And Multiple Windows

A reconnecting or newly opened client should not depend on another client's local state.

```text
connect
  negotiate protocol
  authenticate/authorize client capabilities
  request snapshot at current cursor
  subscribe to session/operation streams
  rebuild UiState from snapshot and subsequent ProductEvents
```

Unsubmitted drafts remain local to the client that owns them. If draft persistence is desired later, it should be modeled as a separate user preference or workspace draft store, not as session fact history.

### Conflict Handling

For shared mutable session controls, use explicit compare-and-set semantics.

```rust
pub struct CursorGuard {
    pub expected_session_cursor: SessionCursor,
    pub expected_generation: Option<Generation>,
}
```

Examples:

```text
switch active leaf
  require expected active leaf generation

change session profile
  require expected profile generation or serialize as RuntimeWrite

submit prompt based on old transcript
  either allow with captured context or reject with StaleSessionCursor, depending on product policy
```

The runtime should reject ambiguous shared mutations instead of letting the last client silently win.

### Multi-Client Invariants

```text
runtime owns sessions and operations
clients own connections, subscriptions, local drafts, and local view state
all runtime-affecting intents go through IntentRouter admission
unsubmitted prompt input is client-local by default
submitted prompt input becomes a runtime-owned Operation
operation abort is global to observers of that operation, but authorization is capability-scoped
client detach does not imply operation abort
session-write intents remain serialized per session
shared mutable controls use cursor/generation guards
slow or disconnected clients recover through snapshot plus replayable product events
```

This keeps multi-client support as an extension of the operation runtime rather than a special case in each UI.

## Migration Path And Phased Targets

The architecture should not be migrated through a one-time rewrite. The safer path is to establish boundaries first, route existing behavior through those boundaries, then replace internal implementations behind stable contracts.

The migration principle:

```text
first unify external semantics
then unify operation admission
then unify durable facts
then move the prompt path fully through the new runtime
finally delete retired paths
```

A partial migration is acceptable only when the stop condition is explicit. Long-term dual paths are architectural debt.

### Phase 0: Architecture Contract Freeze

Goal: freeze the core contracts that future code will target.

Targets:

```text
OperationRuntime boundary
ClientIntent / RuntimeCommand shape
ProductEvent shape
SessionEvent shape
UiState / Snapshot shape
CapabilitySnapshot shape
Operation outcome and failure taxonomy
protocol family/versioning rules
```

Exit criteria:

```text
new architecture document is accepted as reference
core event families and durability classification are named
runtime boundary responsibilities are clear enough to create API skeletons
old direct service access patterns are documented as migration targets
```

This phase is documentation and API design, not behavior replacement.

### Phase 1: ProductEvent First

Goal: make external adapters consume one product semantic event stream.

Targets:

```text
normalize existing CLI/TUI/RPC visible events into ProductEvent concepts
introduce event durability metadata where possible
separate live-only display deltas from durable-derived semantic events
keep old agent loop if needed, but adapt its output through ProductEvent
```

Exit criteria:

```text
TUI/CLI behavior can be described in ProductEvent terms
new UI-facing behavior does not need direct access to internal agent state
terminal operation-like outcomes are visible as semantic events
```

This creates a stable outside surface before internal execution changes.

### Phase 2: IntentRouter And Operation Scheduler

Goal: make all runtime-affecting commands pass through one admission path.

Targets:

```text
route prompt submit, abort, cancel, plugin reload, settings change, profile switch through IntentRouter
classify operations as Query, ReadOnly, SessionWrite, RuntimeWrite, Child, or Control
apply per-session single-writer semantics for session writes
prioritize abort/cancel/recovery paths
return admission results instead of ad hoc command handling
```

Exit criteria:

```text
there is no UI path that starts a session-affecting operation by directly calling deep services
concurrent prompt/control behavior is governed by scheduler rules
multi-client intent semantics can reuse the same admission path
```

This centralizes concurrency before introducing a new durable store.

### Phase 3: SessionEvent Durable Fact Layer

Goal: introduce the Rust-native session event log as the fact source.

Targets:

```text
define SessionEvent records and event family boundaries
write session events transactionally for a narrow operation slice
project committed session facts into current transcript/session views
add recovery markers for incomplete operation families
keep snapshots/checkpoints rebuildable from SessionEvent
```

Recommended migration style:

```text
shadow-generate SessionEvent during early validation
compare projected view with existing behavior
then switch one operation slice to SessionEvent as the source of truth
do not keep permanent double-write/double-read behavior
```

Exit criteria:

```text
at least one meaningful session operation uses SessionEvent as durable truth
projection can rebuild the corresponding view from the log
old persistence is no longer authoritative for that slice
```

This phase must be conservative: durable facts are harder to change than live events.

### Phase 4: PromptTurnFlow Owns One Prompt

Goal: make one user prompt fully pass through the new operation runtime.

Target flow:

```text
ClientIntent::PromptSubmitted
  -> IntentRouter admission
  -> OperationRuntime starts SessionWrite operation
  -> PromptTurnFlow orchestrates model/tool/session work
  -> SessionEvent transaction commits durable facts
  -> ProductEvent stream reports semantic progress and terminal outcome
  -> UiState projection reconciles live and committed state
```

Exit criteria:

```text
a normal prompt can complete through the new path
abort/failure/recovery behavior follows OperationOutcome semantics
committed assistant/tool results come from SessionEvent-derived projection
TUI does not need to know whether old or new internals produced the event stream
```

This is the architectural turning point. After this phase, new behavior should target the operation runtime by default.

### Phase 5: Capability Snapshot Integration

Goal: replace scattered authorization checks with operation-local capability snapshots.

Targets:

```text
model selection uses ModelCapability
filesystem operations use FilesystemCapability
shell commands use ShellCapability
tool execution uses ToolCapabilitySet
plugin host calls use PluginCapabilitySet
session read/write access uses SessionReadCapability and SessionWriteCapability
capability changes emit CapabilityChanged product events
```

Exit criteria:

```text
prompt operation behavior is explainable by its OperationCapabilitySnapshot
active operations do not observe silent mid-run capability mutation
plugin/tool/model/filesystem access no longer receives raw runtime services
revoke policy determines whether active operations continue, fail, or abort
```

This turns authorization into a versioned contract rather than dynamic global conditionals.

### Phase 6: UI Adapter Convergence

Goal: make UI adapters thin clients of the runtime boundary.

Targets:

```text
TUI sends ClientIntent / RuntimeCommand
TUI receives Snapshot and ProductEvent
local TUI state stays local
future GUI/RPC clients reuse the same protocol families
slow-client and reconnect behavior uses snapshot cursor semantics
```

Exit criteria:

```text
TUI no longer mutates session/runtime internals directly
new GUI windows can be modeled as additional ClientConnection instances
client detach, abort, submit, and reconnect behavior follow documented semantics
```

This phase prepares the runtime for real multi-client usage.

### Phase 7: Recovery, Versioning, And Flow-Control Hardening

Goal: make the runtime production-tolerant rather than only functionally correct.

Targets:

```text
startup recovery scan for incomplete operation families
InDoubt commit handling
protocol negotiation for UI/RPC clients
snapshot rebuild and version handling
bounded queues and explicit backpressure events
slow subscriber disconnect/reconnect path
structured retry and idempotency policies
```

Exit criteria:

```text
restart after interrupted operation produces a coherent recovered state
unsupported protocol clients fail clearly
slow UI/RPC subscribers do not block session commit
storage pressure becomes failure/recovery, not silent memory growth
```

This phase hardens the semantics already introduced by earlier phases.

### Phase 8: Retire Old Paths

Goal: remove replaced architecture instead of preserving parallel systems.

Targets:

```text
remove old prompt runner paths once PromptTurnFlow covers them
remove old session persistence paths once SessionEvent is authoritative
remove UI direct-runtime mutation paths once adapters use intent/event protocols
remove compatibility shims after the last unmigrated caller is gone
remove obsolete tests that only assert retired internal structure
```

Exit criteria:

```text
there is one prompt operation path
there is one durable session fact source
there is one UI/runtime communication protocol
legacy code has documented deletion commits rather than hidden fallback behavior
```

A migration without deletion is not complete.

### Migration Risk Controls

```text
prefer narrow vertical slices over broad horizontal rewrites
keep old behavior observable through ProductEvent adapters during transition
use shadow projection to compare old and new session views before switching authority
write deterministic tests around event sequences, not internal call order
mark temporary adapters with explicit removal conditions
avoid permanent compatibility for TypeScript session JSONL unless a future decision reverses current policy
```

The migration should preserve product behavior while replacing the architectural source of truth.

### Phase Summary

```text
Phase 0
  freeze architecture contracts

Phase 1
  ProductEvent first

Phase 2
  IntentRouter and operation scheduler

Phase 3
  SessionEvent durable fact layer

Phase 4
  PromptTurnFlow owns one prompt

Phase 5
  capability snapshots guard tools/models/plugins/session access

Phase 6
  UI adapters converge on intent/event/snapshot protocols

Phase 7
  recovery, versioning, and backpressure hardening

Phase 8
  delete retired paths
```

The final shape is one operation runtime, one durable session fact model, one product semantic event stream, and many thin clients.

## UI Communication Boundary

The runtime/UI boundary should follow the same operation-runtime rule:

```text
UI intent in
Product events and snapshots out
```

The UI must not talk directly to session storage, Flow nodes, provider runtime, plugin internals, or low-level Agent state. TUI and future GUI surfaces should both communicate through an adapter/presenter layer.

Conceptual shape:

```text
TUI or GUI
  renders UiState
  emits UiIntent
        |
        v
UI Adapter / Presenter
  UiIntent -> Operation or ControlCommand
  ProductEvent -> UiState / ViewModel
  applies throttling, batching, reconnect, and projection policy
        |
        v
OperationRuntime
  runs Operations
  emits ProductEvent
  serves Snapshot and Capability views
```

### Direction From UI To Runtime

UI input should be expressed as stable UI intents. The adapter translates those intents into product operations or control commands.

```rust
pub enum UiIntent {
    SubmitPrompt { text: String },
    AbortCurrent,
    Steer { text: String },
    FollowUp { text: String },
    OpenSession { session_id: SessionId },
    SwitchLeaf { leaf_id: LeafId },
    RunPluginCommand { command_id: String, args: serde_json::Value },
    OpenPluginDialog { dialog_id: String },
}

pub enum RuntimeCommand {
    Run(Operation),
    Control(OperationControlCommand),
    Query(RuntimeQuery),
}

pub enum OperationControlCommand {
    Abort { reason: String },
    Steer { text: String },
    FollowUp { text: String },
}
```

Example adapter mapping:

```rust
async fn handle_ui_intent(
    intent: UiIntent,
    runtime: &mut CodingAgentRuntime,
) -> Result<(), ProductError> {
    match intent {
        UiIntent::SubmitPrompt { text } => {
            runtime
                .run(Operation::Prompt(PromptRequest::text(text)))
                .await?;
        }
        UiIntent::AbortCurrent => {
            runtime
                .control(OperationControlCommand::Abort {
                    reason: "user requested abort".to_owned(),
                })
                .await?;
        }
        UiIntent::Steer { text } => {
            runtime.control(OperationControlCommand::Steer { text }).await?;
        }
        UiIntent::FollowUp { text } => {
            runtime
                .control(OperationControlCommand::FollowUp { text })
                .await?;
        }
        UiIntent::RunPluginCommand { command_id, args } => {
            runtime
                .run(Operation::PluginCommand(PluginCommandRequest {
                    command_id,
                    args,
                }))
                .await?;
        }
        UiIntent::OpenPluginDialog { dialog_id } => {
            runtime
                .run(Operation::OpenPluginDialog(PluginDialogRequest { dialog_id }))
                .await?;
        }
        UiIntent::OpenSession { session_id } => {
            runtime.run(Operation::OpenSession { session_id }).await?;
        }
        UiIntent::SwitchLeaf { leaf_id } => {
            runtime.run(Operation::SwitchLeaf { leaf_id }).await?;
        }
    }
    Ok(())
}
```

The exact operation variants can evolve. The important boundary is that UI emits intent, not service calls.

### Direction From Runtime To UI

Runtime output should reach UI through semantic product events plus explicit snapshots. The UI should not read `events.jsonl`, session internals, or Flow node paths.

```rust
pub enum UiUpdate {
    Snapshot(UiSnapshot),
    Event(ProductEvent),
    CapabilityChanged(RuntimeCapabilities),
    CommandResult(CommandResult),
}

pub struct UiSnapshot {
    pub session: Option<SessionView>,
    pub transcript: Vec<TranscriptBlock>,
    pub active_operation: Option<RunningOperationView>,
    pub capabilities: RuntimeCapabilities,
    pub pending_confirmations: Vec<DelegationConfirmationView>,
    pub plugin_ui: PluginUiView,
    pub diagnostics: Vec<DiagnosticView>,
    pub last_sequence: EventSequence,
}

pub struct UiState {
    pub session: Option<SessionView>,
    pub transcript: Vec<TranscriptBlock>,
    pub active_operation: Option<RunningOperationView>,
    pub capabilities: RuntimeCapabilities,
    pub pending_confirmations: Vec<DelegationConfirmationView>,
    pub plugin_ui: PluginUiView,
    pub diagnostics: Vec<DiagnosticView>,
}
```

The adapter projects runtime events into UI state:

```rust
impl UiProjection {
    pub fn apply(&mut self, update: UiUpdate) {
        match update {
            UiUpdate::Snapshot(snapshot) => self.replace_from_snapshot(snapshot),
            UiUpdate::Event(event) => self.apply_product_event(event),
            UiUpdate::CapabilityChanged(capabilities) => {
                self.state.capabilities = capabilities;
            }
            UiUpdate::CommandResult(result) => self.apply_command_result(result),
        }
    }

    fn apply_product_event(&mut self, event: ProductEvent) {
        match event.kind {
            ProductEventKind::Prompt(prompt) => self.apply_prompt_event(prompt),
            ProductEventKind::Tool(tool) => self.apply_tool_event(tool),
            ProductEventKind::Session(session) => self.apply_session_event(session),
            ProductEventKind::Plugin(plugin) => self.apply_plugin_event(plugin),
            ProductEventKind::Delegation(delegation) => {
                self.apply_delegation_event(delegation);
            }
            ProductEventKind::Diagnostic(diagnostic) => {
                self.state.diagnostics.push(diagnostic.into());
            }
            ProductEventKind::Capability(capability) => {
                self.apply_capability_event(capability);
            }
            ProductEventKind::Agent(agent) => self.apply_agent_event(agent),
            ProductEventKind::Workflow(workflow) => self.apply_workflow_event(workflow),
        }
    }
}
```

### Initial Load And Reconnect

UI state should be reconstructed from a snapshot plus event stream position.

```text
1. UI opens connection.
2. Adapter requests `UiSnapshot` from runtime.
3. Runtime returns current view plus `last_sequence`.
4. UI subscribes to `ProductEvent` values after `last_sequence`.
5. Adapter applies events to `UiState`.
```

This keeps TUI, GUI, RPC clients, and tests on the same semantic model.

### TUI Transport

The current TUI can use in-process channels because it runs in the same process as the runtime.

```text
pi-tui input
  -> interactive adapter
  -> UiIntent
  -> OperationRuntime
  -> ProductEvent broadcast
  -> interactive adapter projection
  -> UiState
  -> pi-tui render
```

Possible Rust shape:

```rust
struct InProcessUiBridge {
    intents: mpsc::Sender<UiIntent>,
    updates: broadcast::Receiver<UiUpdate>,
}
```

`pi-tui` remains generic. It renders `UiState` through coding-agent interactive components, but the base `pi-tui` crate does not learn coding-agent product concepts.

### Future GUI Transport

A GUI should use the same semantic protocol over a remote transport.

```text
GUI client
  -> JSON/RPC/WebSocket command: UiIntent or RuntimeCommand
  -> runtime server
  -> ProductEvent stream / UiSnapshot response
  -> GUI projection store
  -> render
```

The GUI should not get a separate business API. It should share the same operation, event, snapshot, and capability model as TUI.

Transport-specific envelopes can be thin:

```rust
pub enum UiWireRequest {
    Intent(UiIntent),
    Command(RuntimeCommand),
    GetSnapshot,
    Subscribe { after: Option<EventSequence> },
}

pub enum UiWireResponse {
    Snapshot(UiSnapshot),
    Event(ProductEvent),
    CommandResult(CommandResult),
    Error(ProductErrorView),
}
```

### Three Event Layers

Keep these layers separate:

```text
SessionEvent
  durable facts in the Rust-native session log
  used for replay, fork, clone, resume, export, and audit

ProductEvent
  semantic runtime events emitted by operations
  used by adapters, RPC, TUI, GUI, and tests

UiUpdate / UiState
  presentation projection owned by UI adapters
  used for rendering and interaction state only
```

Rules:

```text
UI does not read SessionEvent directly.
UI does not infer state from Flow node IDs.
Runtime does not emit TUI-specific render commands.
Adapters do not mutate session storage directly.
GUI and TUI share ProductEvent semantics even if transports differ.
```

### Why This Boundary Matters

This boundary keeps future UI work from changing the runtime architecture.

A richer TUI, a web GUI, an IDE panel, or a remote controller should all be different presenters over the same runtime contract:

```text
UiIntent -> OperationRuntime -> ProductEvent -> UiProjection -> Render
```

The runtime stays product-semantic and UI-independent. The UI stays reactive and presentation-focused. The adapter is the only place that knows both sides.

## Why This Is Clearer Than The Current Shape

Current center of gravity:

```text
CodingAgentSession with many public methods
large CodingAgentEvent enum
many workflow-specific services under one module
adapters mostly converged but still tied to broad facade
```

Target center of gravity:

```text
small runtime facade
operation modules
service-owned side effects
event families
capability-scoped permissions
```

The existing architecture already points in this direction. The proposed reform mostly changes how the same ideas are named, grouped, and exposed.

## Migration Direction

This should be done incrementally.

1. Keep current `CodingAgentSession` working.
2. Introduce `Operation` and `OperationOutcome` as an internal API first.
3. Route one existing method, likely `prompt`, through `run(Operation::Prompt)` internally.
4. Split `CodingAgentEvent` into event families while preserving adapter compatibility.
5. Move operation-specific code out of `coding_session/mod.rs` into operation modules.
6. Narrow the stable `api` facade after adapters and tests use the operation entrypoint.
7. Keep boundary guards for adapters, plugin hosts, tool ingress, and TUI product isolation.

The goal is not a rewrite. The goal is to make the already Flow-centered system easier to reason about and harder to accidentally widen.
