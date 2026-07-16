# pi-rust Target Architecture

## 1. Document Status

This document defines the target architecture of the `pi-rust` workspace. It is
the normative source for crate ownership, dependency direction, runtime
boundaries, durable state, product events, adapters, and the intended source
tree.

The active `0.2.0` crates have converged to this architecture. Reserved product
crates and explicitly labeled future contracts remain target definitions, not
claims that their implementation already exists. To keep that distinction
explicit, this document uses:

- **Current** for behavior or structure present in the repository now.
- **Target** for the final structure and contract toward which code should move.
- **Reserved** for a boundary assigned to a placeholder crate but not yet
  implemented.
- **Invariant** for a rule that implementations and migrations must preserve.

The words **must**, **must not**, **should**, and **may** are normative. Code
snippets and type names are conceptual unless they match current public APIs.
Exact implementation plans should derive signatures from current code while
preserving the contracts in this document.

### 1.1 Scope

This document covers:

- the responsibility and dependency boundary of every workspace crate;
- the target source layout of every crate;
- the operation runtime owned by `pi-coding-agent`;
- the relationship between Flow, services, operations, durable session facts,
  product events, snapshots, and adapters;
- migration rules from the current source tree.

It does not prescribe a one-time rewrite, require compatibility with the former
TypeScript session format, or turn internal Flow/agent events into public
protocols.

### 1.2 Current Baseline

The active dependency chain is already:

```text
pi-coding-agent ───────> pi-agent-core ───────> pi-ai
        │
        ├─────────────────────────────────────> pi-ai
        └───────────────> pi-tui
```

`pi-mom`, `pi-pods`, and `pi-web-ui` are currently isolated placeholder crates.
The root `pi-rust` binary currently prints `Hello, world!`; the user-facing
executable is `pi-coding-agent`.

The repository already contains a Flow-centered runtime, a product session
owner, a Rust-native session event log, product events, plugins, terminal and
RPC adapters, and several advanced workflows. The target architecture narrows
and organizes those pieces; it does not replace them with a second runtime.

### 1.3 Evidence Policy

Statements marked **Current** in this document are derived from the repository
as of 2026-07-16, using the following evidence in descending order of authority:

1. workspace and crate `Cargo.toml` files;
2. compiled Rust source and public facade exports;
3. unit, integration, boundary, and public-API tests;
4. CodeGraph symbol and call-path data generated from that source.

Other design notes, reviews, and historical documentation are not evidence for
the current implementation. They may explain intent, but when they disagree
with source or tests, source and tests win. A migration should update this
section when it changes a fact listed below.

### 1.4 Current Implementation Inventory

The following inventory records code facts that matter to the target design:

- **Current — workspace dependencies:** `pi-agent-core -> pi-ai` and
  `pi-coding-agent -> {pi-agent-core, pi-ai, pi-tui}` are the only active
  workspace-internal dependency edges. `pi-ai` and `pi-tui` have no workspace
  dependencies.
- **Current — stable facades:** all four active library crates expose an `api`
  facade with nested categories. Their implementation modules are private;
  `pi-ai`, `pi-agent-core`, and `pi-tui` root/flat compatibility exports have
  been removed. `pi-coding-agent::api` is categorized by runtime, operation,
  event, client, view, protocol, CLI, and testing use scenario and has no flat
  exports.
- **Current — cross-crate imports:** `pi-agent-core` and `pi-coding-agent`
  use categorized `pi_ai::api` paths, and `pi-coding-agent` uses categorized
  `pi_agent_core::api` and `pi_tui::api` paths. Consolidated boundary tests
  reject flat paths, categories outside each declared dependency edge, and
  individual symbols outside the item-level edge allowlists.
- **Current — test-support isolation:** every `api::testing` category is gated
  by a non-default `test-support` feature. Production dependencies do not see
  faux providers, harnesses, in-memory environments, or virtual terminals.
  Core/TUI integration targets that use deterministic harnesses declare an
  explicit Cargo `required-features = ["test-support"]` contract; featureless
  builds omit those targets instead of widening the normal facade.
- **Current — product owner:** `CodingAgentSession` is a runtime facade under
  `runtime/facade.rs`. It owns the durable/transient session handle and
  long-lived runtime, Flow, event, capability, plugin, operation-control,
  snapshot, client, profile-registry, and pending-submission coordination
  state. Operation-specific execution lives under `operations/`; admission,
  dispatch, and submission live in sibling runtime modules. The former
  `coding_session/` structural concentration no longer exists.
- **Current — typed operations:** public `CodingAgentOperation` and
  `CodingAgentOperationOutcome` exist, and `CodingAgentSession::run` dispatches
  prompt, compaction, branch summary, self-healing edit, agent/team invocation,
  plugin load/command, default-profile change, delegation approval/rejection,
  session fork/leaf switch, and export operations. `CodingAgentSession::submit`
  now transfers PluginCommand, AgentInvocation, and AgentTeam NonSessionRoot
  execution to a runtime-owned `CodingAgentOperationTask`; the compatibility
  `run` path remains available for every operation and delegates PluginCommand
  to `submit(...).join()`.
- **Current — runtime-owned identity:** admitted PluginCommand,
  AgentInvocation, and AgentTeam roots use the scheduler operation ID for the
  task handle and client submission. Agent/Team root ProductEvents and public
  outcomes use the same ID. Their nested Prompt/agent work keeps a distinct
  child operation ID linked from the root event instead of replacing the root
  correlation key. PluginCommand explicitly declares the
  `OutcomeAcknowledgement` terminal policy and emits no invented root
  ProductEvent.
- **Current — plugin command usage:** PluginCommand is an async
  `NonSessionRoot` for a short-lived command against an already frozen plugin
  capability snapshot. It may coexist with the session writer, must call the
  capability-aware plugin service, and must not mutate durable session state,
  register arbitrary Flow work, or borrow adapter ownership. RPC awaits the
  task because its protocol response contains command output; interactive
  retains the session owner and gives only the task handle to its background UI
  waiter. First-use plugin discovery remains a separate RuntimeWrite
  PluginLoad operation before command submission.
- **Current — delegation-decision ownership:** ApproveDelegation and
  RejectDelegation are `SessionWriteRoot` operations, not NonSessionRoot/Control
  work. Approval records a durable confirmation transition before delegated
  execution and may adopt new pending confirmations afterward; rejection also
  records a durable decision. Neither may overlap Prompt or another session
  writer. An approved Agent/Team execution remains a guarded Child under the
  approval root. `submit` intentionally rejects both decisions; adapters retain
  the session-write owner until the operation and descendants finish. Control
  remains reserved for abort, steer, follow-up, and revocation signals that do
  not commit session facts.
- **Current — admission:** operation metadata already classifies origin,
  admission class, and async/read-only/mutable dispatch mode. `IntentRouter` and
  `OperationScheduler` exist. Query and Child classes are fail-closed on the
  generic path and use dedicated admission; ReadOnly and Control do not occupy
  a root slot. `OperationControl` now owns independent SessionWriteRoot,
  bounded NonSessionRoot, and RuntimeWrite slots: one session writer may coexist
  with non-session roots, non-session roots have an explicit default capacity
  of four, and current runtime writes are exclusive. It does not queue work.
  The public `CodingAgentSession::run(&mut self, ...)` dispatcher still
  serializes session-writing calls through the session owner. Runtime-owned
  PluginCommand, AgentInvocation, and AgentTeam tasks can now coexist with a
  SessionWriteRoot. These are the complete current set of true async
  NonSessionRoot operations; session/runtime writers and committed reads retain
  their dedicated dispatch owners.
- **Current — child lifetime:** `OperationControl` owns roots and children in
  one generation-scoped registry. Child admission requires an existing live
  parent, rejects duplicate IDs across the complete root/child set, and returns
  a guarded permit with a cancellation token. Delegated Agent/Team wrappers and
  their internal Prompt children are separate admitted lineage nodes. If a
  parent owner releases early, its identity remains in drain state, all
  descendants are cancelled, and root capacity/shutdown drain is released only
  after the last descendant guard exits. Prompt execution consumes this parent
  cancellation token. Parent terminal events are emitted only after child
  permits are released.
- **Current — controls:** prompt abort, steer, and follow-up use a typed control
  channel. Every active root identity stores kind, operation ID, and generation
  in production, and duplicate active operation IDs are rejected across slots.
  Prompt control registration must match the exact active Prompt ID before
  client binding. `CodingAgentOperationControl` exposes only exact-ID abort,
  while Prompt steer/follow-up remain on `CodingAgentPromptControl`. Submitted
  compaction binds its cancellation token to client owner generation and
  operation ID. Snapshot/client code also models typed control receipts and
  rejection reasons.
- **Current — capabilities:** operation capability snapshots, generations,
  actor lineage, filesystem/shell/session handles, plugin capabilities, and
  explicit future-only/cooperative-revocation policies exist. Session access is
  declared by the operation as `None`, `Read`, or `Write` before admission rather than
  inferred from dynamic OperationKind. Every current session-mutating operation
  owner requires the frozen `SessionWriteCapability` before touching
  persistence or transient session state. Plugin capability inventory retains
  exact provider IDs per contribution category rather than only provider
  counts. Tool collection, command execution, and Prompt hooks filter by those
  frozen identities before invoking provider registration or execution;
  possession of one provider grant cannot authorize another provider in the
  same category.
  `pi-agent-core::api::tool::ToolExecutionContext` is constructed for every
  actual tool invocation and carries a caller-owned generic scope ID, turn,
  tool-call ID/name, and the run cancellation token. `pi-coding-agent` sets the
  scope to the admitted operation ID; it does not let a later mutable runtime
  choose that identity. Long-running owned tools such as shell execution race
  their work against the supplied cancellation token.
  Admission derives filesystem/shell handles from the operation runtime's
  frozen cwd, including non-persistent operations. Product bootstrap may still
  declare built-in tools, but `RuntimeService` reconstructs every reserved
  filesystem/shell tool closure from the admitted handles after name policy is
  applied. A missing handle omits the tool rather than retaining its bootstrap
  closure. Provider streamer creation for Prompt, compaction, branch summary,
  and self-healing model repair requires the frozen `ModelCapability`; its
  granted profile must exactly match the RuntimeSnapshot profile. Default
  profile resolution therefore occurs before admission for model-backed product
  operations. Agent/Team wrappers receive the admitted parent snapshot and
  derive child handles by intersection; production child execution cannot fall
  back to a permissive snapshot. Active root/child identities retain their
  admitted capability generation through scheduler drain, and ProductEvent
  envelopes project that operation-bound value rather than the coordinator's
  current generation. Startup recovery events recover the same value from the
  durable OperationStarted fact. Direct Rust `AgentTool` closures supplied by
  an embedding host are explicitly trusted executable code: Rust cannot inspect
  or sandbox what an arbitrary closure captured. Untrusted extensions therefore
  enter through the product plugin host and frozen provider identities, not the
  raw embedding-tool constructor. Revocation beyond future-only installation
  is exposed only through a privileged capability control: it installs a new
  generation, rejects stale admission, requests cancellation for exact older
  root/child identities, and publishes those request targets. It does not claim
  that arbitrary trusted Rust code has been forcibly preempted.
- **Current — product events:** `CodingAgentProductEvent` is the single
  sequenced envelope stored, broadcast, replayed, and returned to public
  consumers; internal runtime code uses only a private `ProductEvent` type
  alias and typed cursor accessor. Owner emitters publish `ProductEventDraft`
  values through the single `EventService` sequencer. Session-write lifecycle
  is isolated in `events/session.rs::SessionWriteEvent`, session opening in
  `SessionLifecycleEvent`, Prompt lifecycle in `events/prompt.rs::PromptEvent`,
  default-profile changes in `events/profile.rs::ProfileEvent`, diagnostics in
  `events/diagnostic.rs::DiagnosticEvent`, and capability generation changes in
  `events/capability.rs::CapabilityEvent`. Agent invocation lifecycle is owned
  by `events/agent.rs::AgentInvocationEvent`, and team lifecycle by
  `events/team.rs::TeamEvent`. Prompt stream translation is bounded by
  `PromptStreamEvent`, whose variants delegate payload ownership to
  `AgentStreamEvent`, `MessageEvent`, `ToolEvent`, `DelegationEvent`, and
  `RuntimeEvent`. Session compaction is owned by `SessionCompactionEvent`,
  self-healing edit lifecycle by `SelfHealingEditEvent`, and startup recovery
  projection by `RecoveryEvent`. `FinalizedSessionWrite` cannot carry unrelated
  event families. Prompt Flow completion is explicit
  idempotent context state rather than a cached terminal event; the terminal
  ProductEvent is published exactly once from the typed `PromptTurnOutcome`
  boundary.
  The centralized `CodingAgentEvent` compatibility enum and its internal-to-
  public mapping are deleted. Every emitter now builds an owner-local typed
  `ProductEventDraft`; only `EventService` may assign stream identity, sequence,
  operation lineage, retention, and broadcast delivery. Typed families,
  durability projection, retained-event reconnect, live receiver lag
  handling, and fresh-snapshot recovery are implemented. The five
  terminal-associated root families resolve terminal
  operation metadata from the admitted `OperationKind` plus exact evidence in
  the operation descriptor; event variants alone cannot declare a root
  terminal. One admission-owned operation event context atomically carries
  kind, capability generation, direct parent, and stable root; these
  associations are projected by the public envelope. Session association is
  currently projected only from event-owned session facts. Recovery and
  partial-commit durability are explicit. Delivery is classified as data,
  terminal, control, or recovery; all classes share a bounded sequence and fail
  closed to fresh-snapshot recovery on a gap. The envelope, retained window,
  snapshot cursor, public reconnect API, and RPC `eventStreamId` now share one
  runtime-unique stream identity; session identity is intentionally separate.
  Session-write finalization distinguishes skipped, definitely failed, and
  persistence-uncertain outcomes. Every submitted operation explicitly chooses
  ProductEvent terminal publication or exact outcome acknowledgement. The
  P6.4's remaining work is the phase gate and any evidence-backed contract
  cleanup, not restoration of a compatibility event layer.
- **Current — snapshots and clients:** public connection generations, drafts,
  submitted-operation state, detach/shutdown behavior, snapshot cursors,
  reconnect delivery, terminal anchors, and recovery reasons are implemented.
- **Current — durable sessions:** `SessionEventEnvelope` already contains
  schema/version, optional `session_sequence`, operation/turn/branch/leaf
  association, parent event ID, timestamp, and typed event data. The log has
  manifest, store, transaction, and replay modules plus operation terminal and
  recovery facts.
- **Current — adapters:** print, JSON, RPC, and interactive paths live in
  `pi-coding-agent`; interactive rendering uses `pi-tui`. Product protocol and
  UI modules consume typed operations, ProductEvents, snapshots, and product
  views. Session bootstrap for headless, RPC, and interactive operation tasks
  is owned by `app/session.rs`; the same owner also handles interactive
  hydrate/list/clone/tree/export commands. Adapters project those product views
  into text, JSONL, RPC DTOs, SessionChoice, TranscriptItem, and TUI state.
  `app/cli/request.rs` owns config/model/auth/resource/profile resolution and
  `app/configuration.rs` owns auth/settings persistence. RPC capability output
  consumes product-owned `CodingAgentCapabilities` projections instead of
  scheduler/plugin internals. P6.5 adapter convergence is complete.
- **Current — RPC operation projection:** one bounded session-owned ProductEvent
  pump feeds one RPC event adapter and one applied-sequence cursor. Foreground
  Prompt/Delegation completion is separate from an operation-ID-keyed
  AgentInvocation/AgentTeam background registry. A background Agent/Team may
  coexist with a foreground Prompt; generic Prompt abort does not target a
  background root, and session replacement remains blocked until every active
  root drains. Agent/Team admission responses expose the scheduler operation ID
  used by their root ProductEvents and terminal outcomes.
- **Current — placeholders:** `pi-mom`, `pi-pods`, and `pi-web-ui` each contain
  only the generated `add` function and its trivial test, and have no
  dependencies. Their later sections are architecture reservations, not
  descriptions of implemented behavior. The crate names alone are not treated
  as evidence of product semantics.
- **Current — root package:** the root binary has no workspace dependency and
  only prints `Hello, world!`; `pi-coding-agent/src/main.rs` is the functional
  product entrypoint.

## 2. Workspace Architecture

### 2.1 Layering

The target workspace has four active layers and three reserved product/adapter
crates:

```text
┌──────────────────────────────────────────────────────────────────────┐
│ Products and deployment                                             │
│ pi-mom (reserved)   pi-pods (reserved)   pi-web-ui (reserved)       │
└───────────────────────────────┬──────────────────────────────────────┘
                                │ use stable product/protocol APIs
┌───────────────────────────────▼──────────────────────────────────────┐
│ Coding-agent product                                                │
│ pi-coding-agent: operations, sessions, policy, plugins, adapters    │
└──────────────────────┬─────────────────────────────┬─────────────────┘
                       │                             │
┌──────────────────────▼──────────────────┐  ┌──────▼──────────────────┐
│ Agent runtime                           │  │ Generic terminal UI     │
│ pi-agent-core                           │  │ pi-tui                  │
└──────────────────────┬──────────────────┘  └─────────────────────────┘
                       │
┌──────────────────────▼───────────────────────────────────────────────┐
│ Model/provider runtime                                              │
│ pi-ai                                                               │
└──────────────────────────────────────────────────────────────────────┘
```

Dependencies point downward or from a product to an independent adapter
library. Lower layers must not import product-layer types.

### 2.2 Allowed Workspace Dependencies

| Crate | May depend on | Must not depend on |
| --- | --- | --- |
| `pi-ai` | no workspace crate | every other workspace crate |
| `pi-agent-core` | `pi-ai` | `pi-coding-agent`, `pi-tui`, product crates |
| `pi-tui` | no workspace crate | `pi-ai`, `pi-agent-core`, `pi-coding-agent`, product crates |
| `pi-coding-agent` | `pi-ai`, `pi-agent-core`, `pi-tui` | reserved product crates |
| `pi-web-ui` | stable `pi-coding-agent` protocol/API surface when activated | `pi-agent-core` internals, provider implementations, session storage internals |
| `pi-mom` | stable `pi-coding-agent` API when activated | `pi-agent-core` internals, `pi-tui`, session storage internals |
| `pi-pods` | stable `pi-coding-agent` API/protocol when activated | `pi-agent-core` internals, `pi-tui`, provider implementations |
| root package | executable composition only | domain/runtime implementation |

An exception requires an architecture decision record (ADR), an updated table,
and a boundary test. Transitive access is not a reason to omit a direct Cargo
dependency when a crate names another crate's type.

Dependency review must distinguish three different relationships:

1. **Cargo dependency:** which crate may compile against another crate.
2. **Type/interface dependency:** which categorized facade items that specific
   edge may name, enforced by the allowlists in section 2.5.
3. **Runtime data flow:** how owned values, events, streams, and scoped handles
   move through the layers without granting ownership of the upstream service.

A valid Cargo edge does not authorize unrestricted use of the upstream facade,
and a runtime value flowing through a layer does not make that layer the owner
of its durable or external protocol representation.

### 2.3 Ownership Matrix

Every important concern has one owner:

| Concern | Owner | Non-owners consume through |
| --- | --- | --- |
| Model metadata and provider wire mapping | `pi-ai` | `pi_ai::api` |
| Provider auth inputs, HTTP/SSE transport, retry | `pi-ai` | scoped provider client interfaces |
| Provider-neutral agent loop | `pi-agent-core` | `pi_agent_core::api` |
| Generic Flow engine | `pi-agent-core` | `Flow<C>` and `FlowNode<C>` |
| Generic execution environment and agent tools | `pi-agent-core` | narrow traits/types |
| Generic terminal lifecycle and widgets | `pi-tui` | `pi_tui::api` |
| Coding-agent operation runtime and policy | `pi-coding-agent` | `pi_coding_agent::api` |
| Durable coding session facts | `pi-coding-agent` | session repository/service |
| Product semantic events and snapshots | `pi-coding-agent` | subscription and snapshot APIs |
| CLI, print, JSON, RPC, interactive adapters | `pi-coding-agent` | operation runtime facade |
| Coding tools and plugin policy | `pi-coding-agent` | capability-scoped product services |
| Browser presentation | `pi-web-ui` (reserved) | versioned product protocol |
| Cross-session personal orchestration | `pi-mom` (reserved) | stable coding-agent embedding API |
| Isolated/remote runtime hosting | `pi-pods` (reserved) | stable coding-agent API/protocol |

If a concern appears to need two owners, split the domain contract from its
product policy. For example, `pi-agent-core` owns a generic `FileSystem` trait;
`pi-coding-agent` owns which operation is permitted to use it.

### 2.4 Public API Rule

Each active library crate exposes one intentional facade:

```text
pi_ai::api
pi_agent_core::api
pi_tui::api
pi_coding_agent::api
```

New cross-crate code must import stable facade items through a category.
Root-level, flat-facade, and internal-module compatibility paths are removed and
must not be restored. Every new public item must have a clear downstream use
case and be represented by the owning public-API contract suite. This does not
require one compile-only test case per exported item.

The facade must be categorized. A flat module that re-exports every public item
still creates unrestricted coupling even when it is named `api`. Target imports
should communicate their dependency category:

```rust
use pi_ai::api::conversation::{ContentBlock, Context, Message};
use pi_ai::api::model::{Model, ThinkingConfig};
use pi_ai::api::stream::{AssistantMessageEvent, StreamOptions};

use pi_agent_core::api::agent::{Agent, AgentEvent};
use pi_agent_core::api::flow::{Flow, FlowNode};
use pi_agent_core::api::transcript::StoredAgentMessage;

use pi_tui::api::input::{InputEvent, Key};
use pi_tui::api::terminal::{Terminal, TerminalSize};
```

All current production imports use categorized paths. A flat re-export added as
a migration convenience is itself an architecture regression.

### 2.5 Dependency-Edge Interface Contracts

Public availability and permission to depend are separate contracts:

```text
provider facade = everything intentionally supported for all consumers
edge allowlist  = the subset one specific downstream crate may name
```

Adding an item to a provider facade does not add it to an edge allowlist.
Extending an allowlist requires a concrete downstream use case, the smallest
stable interface that serves it, a boundary-test update, and confirmation that
the consumer is not bypassing a higher-level abstraction.

Every exposed item belongs to one interface kind:

| Kind | Shape | Exposure rule |
| --- | --- | --- |
| value/data | owned enums/structs such as `Model`, `ContentBlock`, `Usage` | may cross a public boundary only when its semantics are stable and it contains no client, secret, transport, or mutable service state |
| function | pure conversion/calculation or a narrow async entrypoint | expose the highest-level useful operation; do not expose helper chains that force the consumer to reproduce orchestration |
| behavior | trait/callback such as provider streaming, filesystem, terminal | keep methods minimal, object-safe where useful, cancellation-aware, and independent of concrete implementations |
| context/handle | scoped client, capability, transaction, connection | expose a purpose-specific handle; never pass a whole service container or mutable runtime context |
| event/stream | typed event plus bounded/cancellable delivery contract | define ordering, terminal, error, lag, and backpressure semantics with the type |
| testing | faux provider, virtual terminal, builders, fixtures | non-default/test-only facade; must not enter production signatures |

An upstream type appearing in a downstream public signature becomes a
transitive compatibility commitment. Such leakage is allowed only for
explicitly approved value/behavior contracts in the edge tables. Internal-use
functions and handles must not be re-exported merely for convenience. At a
durable or external wire boundary, the owning crate normally converts upstream
types into its own versioned DTO.

Wildcard imports from another workspace crate are forbidden in production.
Imports from another crate's private, doc-hidden, `compat`, provider-specific,
or root compatibility modules are forbidden. Tests follow the same rule except
for an explicit `api::testing` facade.

#### 2.5.1 `pi-agent-core -> pi-ai`

Core needs the provider-neutral AI vocabulary and streaming contract. It does
not need provider-client, registry, credential, HTTP, or concrete-provider
ownership.

| `pi_ai::api` category | Allowed items | Core use scenario |
| --- | --- | --- |
| `model` | `Model`, `ThinkingConfig` | `AgentConfig`, model limits/reasoning, provider request snapshots |
| `conversation` | `Context`, `Message`, `ContentBlock`, `Tool`, `AssistantMessage`, `StopReason`, `Usage`, `Cost` | construct model context; represent agent/tool messages; usage and transcript values |
| `stream` | `AssistantMessageEvent`, `EventStream`, `StreamOptions`, `complete` | consume provider output, expose `AgentEvent`, propagate options/cancellation, collect summaries |
| `hooks` | `ProviderStreamHooks`, `ProviderResponseInfo` | generic provider-request/response observation in the harness |
| `stream::json` | `parse_streaming_json` | generic proxy stream decoding; the only allowed low-level parser |

Approved transitive public types are the model/conversation/stream value types
needed by `AgentConfig`, `AgentMessage`, `AgentEvent`, `AgentToolOutput`, and
`ProviderStreamer`. Hook types may appear only in the corresponding public hook
contract. `complete`, `parse_streaming_json`, and other imported functions are
internal implementation dependencies and must not be re-exported by core.

`ModelCost`, `ModelInput`, and deterministic model builders are allowed only
through `pi_ai::api::testing::model` for tests; core algorithms must not depend
on them.

Core must not depend on:

```text
AiClient, ProviderRegistry, or ApiProvider
auth resolvers, environment key lookup, or credential material
all_models, lookup_model, get_providers, or product model selection
provider-specific modules and wire types
HTTP/SSE/retry/header implementations
compatibility payload formats or image-generation clients
FauxProvider outside api::testing
```

The provider call boundary uses dependency inversion:

```text
pi-coding-agent owns AiClient and provider selection
        │
        └── creates a ProviderStreamer callback with pi-ai vocabulary
                    │
                    ▼
             pi-agent-core AgentConfig
```

Local usage is also constrained:

- `agent/` may use model, conversation, and stream categories.
- `compaction/` may build conversation context and use `stream::complete`; it
  must not select a model or resolve authentication.
- `hooks/` may use only the explicit hook contracts.
- `transcript/` may retain stable content/stop/usage values, never clients,
  authentication, headers, or provider wire payloads.
- generic `flow/`, `execution/`, and `resources/` should not import `pi-ai`
  unless an AI type is intrinsic to that domain's public contract.

#### 2.5.2 `pi-coding-agent -> pi-ai`

The product directly uses `pi-ai` because it is the composition root: it selects
models, creates scoped clients, resolves product credential sources, constructs
prompt input, and maps AI events into product-owned events and DTOs.

| `pi_ai::api` category | Allowed items/functions | Product use scenario |
| --- | --- | --- |
| `client` | `AiClient` | bootstrap one scoped provider runtime and inject it into runtime services |
| `model` | `Model`, `ModelInput`, `ModelCost`, `ThinkingConfig`; `all_models`, `lookup_model`, `get_providers` | model listing/selection/validation/display and runtime construction |
| `conversation` | `AssistantMessage`, `ContentBlock`, `Context`, `Message`, `StopReason`, `Usage`, `Cost` | prompt/tool content, durable conversion, product event/protocol projection |
| `stream` | `AssistantMessageEvent`, `EventStream`, `StreamOptions` | adapt `AiClient` to core `ProviderStreamer` and map live events |
| `auth` | `ProviderAuthDiagnostic`, `env_api_key` | credential source selection and secret-free diagnostics |
| `provider` | built-in registration function at bootstrap only | install providers into the scoped client |
| `testing` | `FauxProvider` and deterministic responses | offline tests only; never normal production modules |

`AiClient`, provider/stream handles, auth diagnostics, and catalog functions are
internal product dependencies and must not leak through product event, session,
snapshot, or protocol types. `Model` and provider-neutral content values may
appear in an embedding request only when the product intentionally adopts their
compatibility; external RPC/durable formats still use product-owned DTOs.

`ApiProvider` may be named only by an explicit provider-extension boundary or
test fixture. Ordinary runtime code uses `AiClient`. The product must not import
concrete provider modules, wire structs, transport internals, compatibility
conversion internals, registry storage internals, raw secrets, or doc-hidden
`pi-ai` modules.

Product-local permissions are narrower:

- `app/` and `config/` may use client, model, and auth categories.
- `runtime/` and prompt operations may use allowed model, conversation, and
  stream contracts.
- `tools/` normally uses only `ContentBlock`; a tool must not call providers.
- `session/` uses AI values only at an explicit persistence conversion boundary
  and prefers product-owned durable DTOs where upstream evolution is risky.
- `events/` and `adapters/` translate to product-owned DTOs and never expose a
  client, stream, credential, or provider wire type in public protocols.

#### 2.5.3 `pi-coding-agent -> pi-agent-core`

The product consumes the following core capabilities. These categories form
the edge allowlist; individual agent-turn nodes are not implicitly allowed.

| `pi_agent_core::api` category | Allowed items/functions | Product use scenario |
| --- | --- | --- |
| `agent` | `Agent`, `AgentConfig`, `AgentEvent`, `AgentMessage`, `AgentStream`, `AgentResources`, `ProviderStreamer`, `ThinkingLevel`, `QueueMode`, compaction settings | build/run the low-level agent and translate its events |
| `tool` | `AgentTool`, `AgentToolOutput`, `AgentToolResult`, `ToolFn`, `ToolExecutionContext`, `ToolUpdateCallback`, `ToolExecutionMode` | implement coding tools and plugin-tool adapters; receive generic scope/turn/call/cancellation context at invocation |
| `flow` | `Flow`, `FlowNode`, `Action`, `FlowOutcome`, `FlowError`, `FlowRunOptions` | build product operation graphs using the generic engine |
| `execution` | `ExecutionEnv`, `ExecOptions`, `FileSystem`, `FileError`, execution outputs | capability-scoped filesystem/shell work and self-healing edits |
| `resources` | `Skill`, `PromptTemplate`, `AgentResources`, diagnostics and parsing/loading/substitution helpers | discover product paths, then parse provider-neutral resources |
| `compaction` | token estimation and summarization entrypoints | product compaction/branch summary with product persistence policy |
| `transcript` | `SessionEntry`, `SessionHeader`, `SessionTreeNode`, `StoredAgentMessage`, `StoredUsage`, `StoredUsageCost`, `TreeFilterMode`, ID/timestamp helpers | current provider-neutral transcript, tree, export and protocol values |

Agent/Flow/execution handles are internal product dependencies and never part
of `pi_coding_agent::api`. Selected configuration values may be re-exported only
under an explicit compatibility commitment. Transcript values currently cross
several product boundaries; the target external protocol/view surface converts
them to product-owned DTOs so core transcript evolution does not silently
change product wire compatibility.

The product must not depend on individual `AgentTurnFlow` node structs/node IDs,
implementation modules for harness/queue/context, `FlowEvent` as a product
protocol, in-memory session storage in production, private proxy/conversion
details, test environments outside `api::testing`, or deprecated root exports.

Usage rules:

- only runtime and prompt/agent operations construct or drive `Agent`;
- only operation Flow modules use the `flow` category;
- adapters may use stable transcript/config values but not `AgentEvent`,
  `AgentStream`, `Flow`, or execution handles;
- `EventService` is the single mapping boundary from `AgentEvent` to product
  events;
- product session envelopes/transactions remain product-owned even when they
  embed an allowed core transcript value;
- tools implement core tool contracts, while capability policy remains in the
  product.

The currently broad export of individual agent-turn nodes should become
crate-private or `api::advanced` unless a second real embedder demonstrates a
stable use case. Current `pi-coding-agent` production code does not require
those node exports.

#### 2.5.4 `pi-coding-agent -> pi-tui`

| `pi_tui::api` category | Allowed items | Product use scenario |
| --- | --- | --- |
| `terminal` | `Terminal`, `ProcessTerminal`, `TerminalSize`, capability/color/image negotiation | initialize and safely restore an interactive terminal |
| `input` | `InputEvent`, `Key`, key event/modifier types, `StdinBuffer`, keybinding manager/matching | normalize input and map it to product intents |
| `component` | `Component`, editor, markdown, list, dialog, loader, text/image components | build product-owned views and menus |
| `render` | `Tui`, scheduler, style/color, width/wrap/truncate and paint helpers | render product view models |
| `theme` | generic component themes and palettes | map product theme resources to widget themes |
| `testing` | `VirtualTerminal`, `TerminalOp` | deterministic lifecycle/render tests only |

After migration, product code must not use doc-hidden or root compatibility
exports. `VirtualTerminal`/`TerminalOp` do not belong in normal runtime paths.
Generic components receive rendered values/view models, never sessions, product
events, profiles, or repositories.

#### 2.5.5 `pi-coding-agent -> Downstream Products`

The target product facade is smaller than the current broad
`pi_coding_agent::api` re-export list:

| `pi_coding_agent::api` category | Exposed contract | Intended consumer |
| --- | --- | --- |
| `runtime` | open/create options, runtime/session handle, runtime-owned operation task, shutdown/detach | embedders such as future `pi-mom` or a pod host |
| `operation` | `CodingAgentOperation`, typed request options, common/typed outcomes | every embedder that submits product work |
| `event` | public `ProductEvent` envelope/families, receiver, sequence/durability views | headless clients, higher products, adapters |
| `client` | connection generation, snapshot, reconnect, exact-operation abort, prompt control, command receipt/rejection | multi-client TUI/web/RPC integrations |
| `view` | committed session/export/capability views without service handles | read-only presentation and automation |
| `protocol` | versioned RPC/JSON wire DTOs and negotiation only | out-of-process clients such as future web or pod transport |
| `cli::{command,configuration,input,print,resources,runtime,runner,theme}` | explicitly supported scripting values and high-level process entrypoints | the product binary, offline examples, and deliberate CLI embedders only |
| `testing` | deterministic product builder/faux runtime | downstream tests under non-default `test-support` only |

The facade must not expose `ServiceContainer`, repositories, Flow nodes,
`AgentEvent`, `AgentStream`, provider clients, plugin host internals, adapter
view state, or concrete TUI components. CLI parsing/config convenience APIs are
not automatically embedding contracts; they stay under an explicit `cli`
category if external scripting genuinely requires them.

Allowlist by reserved consumer:

- `pi-web-ui` defaults to `event`, `client`, `view`, and `protocol`; it does not
  embed services or operate session storage.
- `pi-mom` defaults to `runtime`, `operation`, `event`, `client`, and `view`; it
  orchestrates through product verbs rather than agent/core primitives.
- `pi-pods` defaults to `runtime` for in-process hosting or `protocol` for
  out-of-process hosting, plus lifecycle-neutral event/health views. It does
  not receive provider or session repository internals.

No reserved consumer may depend on `api::testing` in production.

#### 2.5.6 Reserved Product Edges

`pi-mom`, `pi-pods`, and `pi-web-ui` currently have no dependency edge, so no
code-derived allowlist exists. An activation ADR must define one before adding
a Cargo dependency. The default is the narrow `pi_coding_agent::api`
operation/protocol facade; direct dependencies on lower crates require separate
demonstrated use cases.

### 2.6 Enforcement

The target facade uses nested categories while implementation modules remain
private:

```rust
pub mod api {
    pub mod model { /* selected pub use */ }
    pub mod conversation { /* selected pub use */ }
    pub mod stream { /* selected pub use */ }
    pub mod testing { /* cfg/feature-gated deterministic support */ }
}
```

Boundary tests check forbidden Cargo edges, categorized production imports,
forbidden path prefixes, external-consumer compilation, test-facade isolation,
and forbidden upstream types leaking through downstream public signatures.
Prefer Rust visibility and compile-fail checks over source-text scanning. A
source guard is acceptable as a temporary migration check only.

## 3. Crate Boundaries And Target Layouts

The trees below show ownership-oriented target layouts. They are not a demand
that every leaf become a separate file. Prefer one cohesive module over many
one-type files; split a module when it has multiple reasons to change, not when
it crosses an arbitrary line count.

### 3.1 `pi-ai`: Model And Provider Runtime

#### Responsibility

`pi-ai` converts provider-independent model requests into provider wire
protocols and streams provider results back as provider-independent AI events.

It owns:

- model metadata and model lookup;
- provider registry and scoped `AiClient` instances;
- authentication inputs and provider credential resolution interfaces;
- provider-independent messages, content, tools, usage, stop reasons, and
  stream options;
- provider request/response conversion and streaming parsers;
- HTTP, SSE, headers, retries, and provider error classification;
- provider compatibility settings and image-generation protocol types;
- deterministic faux providers used by downstream tests.

It does not own:

- agent turns, tool execution, hooks around coding operations, or compaction;
- sessions, transcripts, operation IDs, product events, or projections;
- CLI configuration precedence, project trust, plugins, RPC, or UI behavior;
- coding-agent retry/admission policy beyond transport-level retry semantics.

#### Dependency and type boundary

`pi-ai` has no workspace dependencies. Its public types may be embedded in
`pi-agent-core` types where they are genuinely part of the low-level agent
protocol. Provider-specific wire structs stay private.

#### Target source tree

```text
crates/pi-ai/
├── src/
│   ├── lib.rs                 # module declarations only
│   ├── api.rs                 # stable facade and testing facade
│   ├── client.rs              # AiClient; scoped provider access
│   ├── model/
│   │   ├── mod.rs             # Model and lookup API
│   │   ├── catalog.rs         # generated/built-in catalog loading
│   │   ├── cost.rs            # usage-cost calculation
│   │   └── generated.json
│   ├── protocol/
│   │   ├── mod.rs
│   │   ├── content.rs
│   │   ├── message.rs
│   │   ├── request.rs         # Context, StreamOptions, Tool
│   │   ├── response.rs        # AssistantMessage/Event, StopReason
│   │   └── usage.rs
│   ├── registry/
│   │   ├── mod.rs
│   │   ├── provider.rs        # ApiProvider contract
│   │   └── auth.rs            # resolver contracts, never product config
│   ├── providers/
│   │   ├── mod.rs             # built-in registration only
│   │   ├── common.rs          # truly shared provider helpers
│   │   ├── anthropic/
│   │   │   ├── mod.rs
│   │   │   ├── wire.rs
│   │   │   ├── convert.rs
│   │   │   └── stream.rs
│   │   └── <provider>/        # same internal pattern
│   ├── transport/
│   │   ├── mod.rs
│   │   ├── http.rs
│   │   ├── sse.rs
│   │   ├── retry.rs
│   │   ├── headers.rs
│   │   └── error.rs
│   ├── compatibility/         # cross-provider request compatibility only
│   ├── images/                # image protocol/client support
│   └── testing/               # faux provider and deterministic fixtures
└── tests/
    ├── provider_contract.rs
    ├── transport_contract.rs
    └── public_api.rs
```

Provider folders must use the same vocabulary (`wire`, `convert`, `stream`)
unless a provider has a documented reason to differ. A provider module must not
become a home for product-specific model selection or CLI behavior.

### 3.2 `pi-agent-core`: Provider-Neutral Agent Runtime

#### Responsibility

`pi-agent-core` runs a model/tool loop in a provider-neutral environment. It is
embeddable without the coding-agent product.

It owns:

- `Agent`, `AgentConfig`, `AgentEvent`, and the agent turn loop;
- the generic `Flow<C>` engine and Flow primitives;
- generic tool definitions, updates, results, and execution modes;
- execution environment traits for filesystem and shell access;
- provider-request, tool, and agent-loop hooks;
- provider-neutral resources such as parsed skills and prompt templates;
- runtime-context compaction and generic branch summarization algorithms;
- in-memory conversation context and low-level transcript vocabulary;
- cancellation, input queues, truncation, and shell-output capture.

It does not own:

- coding session directories, manifests, durable operation transactions, or
  product replay policy;
- product operations, profiles, teams, delegation policy, or plugins;
- product event families, RPC DTOs, CLI commands, or UI state;
- provider-specific wire payloads or credential-store policy;
- concrete coding tools such as the product's `read`, `edit`, or `bash` policy.

#### Flow boundary

The Flow engine is a reusable orchestration mechanism. A `FlowEvent` and node ID
are diagnostic internals, not adapter protocols. Product workflows may compose
core Flow nodes, but product semantics remain in `pi-coding-agent`.

#### Transcript boundary

Core transcript types describe provider-neutral agent messages and trees. They
must not acquire product session manifests, operation scheduling, plugin facts,
or adapter state. Durable coding-session storage remains a product concern.

#### Target source tree

```text
crates/pi-agent-core/
├── src/
│   ├── lib.rs
│   ├── api.rs
│   ├── agent/
│   │   ├── mod.rs             # Agent facade
│   │   ├── config.rs
│   │   ├── event.rs
│   │   ├── message.rs
│   │   ├── queue.rs
│   │   └── turn/
│   │       ├── mod.rs         # AgentTurnFlow
│   │       ├── context.rs
│   │       ├── nodes.rs
│   │       └── runtime.rs
│   ├── flow/
│   │   ├── mod.rs             # small public Flow vocabulary
│   │   ├── graph.rs
│   │   ├── node.rs
│   │   ├── action.rs
│   │   └── error.rs
│   ├── tool/
│   │   ├── mod.rs
│   │   ├── definition.rs
│   │   └── result.rs
│   ├── execution/
│   │   ├── mod.rs
│   │   ├── environment.rs
│   │   ├── filesystem.rs
│   │   ├── shell.rs
│   │   └── capture.rs
│   ├── hooks/
│   │   ├── mod.rs
│   │   ├── agent.rs
│   │   ├── provider.rs
│   │   └── tool.rs
│   ├── context/
│   │   ├── mod.rs
│   │   ├── memory.rs
│   │   └── conversion.rs
│   ├── compaction/
│   │   ├── mod.rs
│   │   ├── estimate.rs
│   │   ├── prepare.rs
│   │   └── summarize.rs
│   ├── resources/
│   │   ├── mod.rs
│   │   ├── frontmatter.rs
│   │   ├── skills.rs
│   │   ├── templates.rs
│   │   └── system_prompt.rs
│   ├── transcript/
│   │   ├── mod.rs
│   │   ├── id.rs
│   │   └── types.rs
│   ├── branch_summary.rs
│   ├── cancellation.rs
│   ├── truncate.rs
│   └── error.rs
├── tests/
│   ├── agent_turn.rs
│   ├── flow.rs
│   ├── boundaries.rs
│   └── public_api.rs
└── examples/
    └── loop_example.rs
```

The current `harness`, `loop_runtime`, and `agent_turn_flow` concepts should
converge under `agent/` rather than remain competing top-level runtime names.
There must be one authoritative agent-loop implementation.

### 3.3 `pi-tui`: Generic Terminal UI Toolkit

#### Responsibility

`pi-tui` provides terminal lifecycle, input normalization, rendering, and
generic reusable components. It should remain useful to a non-agent terminal
application.

It owns:

- terminal setup, restore, resize, cursor, capability negotiation, and images;
- normalized key/input events and configurable keybindings;
- render surfaces, overlays, display width, ANSI handling, and virtual terminals;
- generic editor, input, markdown, menu, list, dialog, loader, text, and image
  components;
- generic component themes, fuzzy matching, undo, kill ring, and word motion.

It does not own:

- prompts, models, tools, sessions, profiles, teams, or delegation;
- product commands, product keybinding actions, or plugin dispatch;
- `ProductEvent`, snapshots, RPC messages, or coding-agent projections;
- filesystem/session side effects initiated by UI actions.

#### Target source tree

```text
crates/pi-tui/
├── src/
│   ├── lib.rs
│   ├── api.rs
│   ├── terminal/
│   │   ├── mod.rs             # Terminal trait and ProcessTerminal
│   │   ├── lifecycle.rs
│   │   ├── capability.rs
│   │   ├── color.rs
│   │   └── image.rs
│   ├── input/
│   │   ├── mod.rs
│   │   ├── event.rs
│   │   ├── key.rs
│   │   ├── keybindings.rs
│   │   └── stdin.rs
│   ├── render/
│   │   ├── mod.rs
│   │   ├── surface.rs
│   │   ├── scheduler.rs
│   │   ├── overlay.rs
│   │   ├── style.rs
│   │   ├── ansi.rs
│   │   └── width.rs
│   ├── component/
│   │   ├── mod.rs             # Component and Container contracts
│   │   ├── editor/
│   │   │   ├── mod.rs
│   │   │   └── autocomplete.rs
│   │   ├── input.rs
│   │   ├── markdown.rs
│   │   ├── menu.rs
│   │   ├── dialog.rs
│   │   ├── loader.rs
│   │   ├── image.rs
│   │   └── text.rs
│   ├── editing/
│   │   ├── cursor.rs
│   │   ├── undo.rs
│   │   ├── kill_ring.rs
│   │   └── word.rs
│   ├── theme.rs
│   ├── fuzzy.rs
│   └── testing/
│       ├── mod.rs
│       └── virtual_terminal.rs
└── tests/
    ├── terminal_lifecycle.rs
    ├── rendering.rs
    ├── unicode_width.rs
    └── public_api.rs
```

Product naming in `pi-tui` is a boundary violation even when a component is
currently used only by `pi-coding-agent`. Product composition belongs in the
interactive adapter.

### 3.4 `pi-coding-agent`: Coding-Agent Product

#### Responsibility

`pi-coding-agent` owns the product. It assembles providers, the low-level agent
runtime, coding tools, durable sessions, operations, plugins, protocols, and
user-facing adapters.

It owns:

- the `pi-coding-agent` binary and CLI/configuration resolution;
- `CodingAgentSession` during migration and the target operation runtime facade;
- typed product operations, admission, scheduling, control, and outcomes;
- product Flow graphs and operation-local contexts;
- durable session events, manifests, transactions, replay, recovery, fork,
  clone, tree, export, and projections;
- product semantic events, snapshots, capabilities, and multi-client behavior;
- coding tools and product execution policy;
- skills/templates/context-file discovery and product resource precedence;
- plugins, profiles, teams, delegation, and self-healing edit behavior;
- print, JSON, RPC, and interactive adapters;
- product themes and TUI view models.

It does not own:

- provider wire protocols or generic AI transport;
- the reusable agent loop or generic Flow engine;
- terminal primitives or generic widgets;
- deployment-fleet policy, browser rendering, or cross-session personal
  orchestration reserved for higher product crates.

#### Target source tree

```text
crates/pi-coding-agent/
├── src/
│   ├── lib.rs
│   ├── main.rs                # process entry; delegates immediately
│   ├── api.rs                 # intentionally small embedding facade
│   ├── app/
│   │   ├── mod.rs
│   │   ├── bootstrap.rs       # composition root
│   │   ├── cli/               # mode router plus args/request/input/model subowners
│   │   │   ├── mod.rs
│   │   │   ├── args.rs
│   │   │   ├── request.rs
│   │   │   ├── input.rs
│   │   │   └── error.rs
│   │   └── shutdown.rs
│   ├── config/
│   │   ├── mod.rs
│   │   ├── paths.rs
│   │   ├── settings.rs
│   │   └── auth.rs            # product credential source selection
│   ├── runtime/
│   │   ├── mod.rs             # OperationRuntime internal owner
│   │   ├── facade.rs          # CodingAgentSession compatibility facade
│   │   ├── intent.rs          # ClientIntent and IntentRouter
│   │   ├── operation.rs       # common operation envelope/metadata
│   │   ├── admission.rs
│   │   ├── scheduler.rs
│   │   ├── control.rs
│   │   ├── outcome.rs
│   │   ├── capability.rs
│   │   ├── client/
│   │   │   ├── mod.rs
│   │   │   ├── state.rs       # internal client/draft snapshot state
│   │   │   └── projection.rs  # public connection/reconnect projection
│   │   └── snapshot.rs
│   ├── operations/
│   │   ├── mod.rs             # registry; no business logic
│   │   ├── prompt/
│   │   │   ├── mod.rs         # request/outcome and entrypoint
│   │   │   ├── context.rs
│   │   │   └── flow.rs
│   │   ├── compaction/        # same pattern
│   │   ├── branch_summary/
│   │   ├── export/
│   │   ├── plugin_load/
│   │   ├── agent_invocation/
│   │   ├── team_invocation/
│   │   ├── delegation/
│   │   ├── self_healing_edit/
│   │   └── session_navigation/
│   ├── services/
│   │   ├── mod.rs             # ServiceContainer composition
│   │   ├── runtime.rs
│   │   ├── flow.rs
│   │   ├── event.rs
│   │   ├── capability.rs
│   │   └── plugin.rs
│   ├── session/
│   │   ├── mod.rs
│   │   ├── id.rs
│   │   ├── manifest.rs
│   │   ├── event.rs           # SessionEvent envelope and data families
│   │   ├── transaction.rs
│   │   ├── repository.rs      # append/read storage boundary
│   │   ├── replay.rs
│   │   ├── recovery.rs
│   │   ├── tree.rs
│   │   ├── export.rs
│   │   └── projection.rs      # committed session views only
│   ├── events/
│   │   ├── mod.rs             # ProductEvent envelope
│   │   ├── prompt.rs
│   │   ├── prompt_stream.rs   # closed Prompt stream owner union
│   │   ├── agent.rs
│   │   ├── team.rs
│   │   ├── message.rs
│   │   ├── tool.rs
│   │   ├── session.rs
│   │   ├── profile.rs
│   │   ├── capability.rs
│   │   ├── plugin.rs
│   │   ├── delegation.rs
│   │   ├── runtime.rs
│   │   ├── workflow.rs
│   │   ├── recovery.rs
│   │   └── diagnostic.rs
│   ├── plugins/
│   │   ├── mod.rs
│   │   ├── manifest.rs
│   │   ├── registry.rs
│   │   ├── capability.rs
│   │   ├── error.rs
│   │   └── contributions/     # each contract owns its scoped registration host
│   │       ├── tool.rs
│   │       ├── command.rs
│   │       ├── hook.rs
│   │       ├── keybind.rs
│   │       └── ui.rs          # actions and dialogs
│   ├── tools/
│   │   ├── mod.rs
│   │   ├── filesystem/        # read/write/edit/find/grep/ls/path policy
│   │   ├── shell.rs
│   │   ├── mutation_queue.rs
│   │   └── output.rs
│   ├── profiles/
│   ├── resources/
│   ├── theme/
│   └── adapters/
│       ├── mod.rs             # adapter contracts only
│       ├── print.rs
│       ├── json/
│       ├── rpc/
│       │   ├── mod.rs
│       │   ├── wire.rs
│       │   ├── commands.rs
│       │   ├── events.rs
│       │   └── connection.rs
│       └── interactive/
│           ├── mod.rs
│           ├── app.rs
│           ├── intent.rs
│           ├── projection.rs
│           ├── render.rs
│           ├── components/
│           └── menus/
├── tests/
│   ├── public_api.rs
│   ├── boundaries.rs
│   ├── operation_contract.rs
│   ├── session_recovery.rs
│   ├── product_events.rs
│   ├── rpc_contract.rs
│   └── cli.rs
└── examples/
    └── embed.rs
```

#### File organization rules

1. `runtime/` owns admission and lifecycle mechanics, not workflow business
   logic.
2. `operations/<name>/` owns one product verb's request, outcome, context, and
   Flow orchestration. Small operations may stay in one `mod.rs`.
3. `services/` owns reusable side effects and durable boundaries. A file named
   `*_service.rs` must not live beside unrelated facade/event/Flow files.
4. `session/` contains durable session concepts only. Live product events and UI
   drafts do not belong there.
5. `events/` contains semantic product events only. RPC serialization belongs
   in `adapters/rpc/`.
6. `adapters/` may translate intents and project events; it may not call Flow
   nodes, repositories, or provider clients directly.
7. `api.rs` re-exports a deliberate embedding surface. It must not mirror every
   internal product type.
8. The former `coding_session/` migration container has been deleted. Do not
   recreate it or add a compatibility alias: runtime facade mechanics belong
   under `runtime/`, product verbs under `operations/`, reusable side effects
   under `services/`, and durable facts under `session/`.

### 3.5 `pi-web-ui`: Browser Client And Presentation (Reserved)

#### Reserved responsibility

When activated, `pi-web-ui` owns a browser-facing client and web presentation
for coding-agent product protocols. It is an adapter/client, not another
operation runtime.

It may own:

- browser connection/session handling;
- decoding versioned product events and snapshots;
- browser-local projections, drafts, layout, and view components;
- reconnect, cursor-gap recovery, and web-specific accessibility behavior.

It must not own:

- session storage, operation scheduling, provider calls, or coding tools;
- a second copy of operation/event business rules;
- imports from private `pi-coding-agent`, `pi-agent-core`, or `pi-ai` modules.

#### Activation rule and target tree

Until a dedicated ADR chooses the web technology and transport, this crate must
remain a placeholder. The reserved layout is intentionally shallow:

```text
crates/pi-web-ui/
├── src/
│   ├── lib.rs
│   ├── client/                # negotiated connection and reconnect
│   ├── protocol/              # decoding/encoding adapters, not canonical DTOs
│   ├── projection/            # ProductEvent + Snapshot -> WebState
│   ├── view/                  # browser presentation
│   └── input/                 # WebIntent construction
└── tests/
    ├── protocol_compat.rs
    └── projection.rs
```

Canonical wire contracts remain owned by `pi-coding-agent` unless a future ADR
extracts a dependency-free protocol crate for more than one real consumer.

### 3.6 `pi-mom`: Cross-Session Orchestration Product (Reserved)

#### Reserved responsibility

When activated, `pi-mom` may provide higher-level, user-facing orchestration
across multiple coding-agent runtimes or sessions: task intake, delegation to
independent runtimes, notification/inbox policy, and long-lived personal
automation.

It must use the stable `pi-coding-agent` facade as a client/embedding API. It
must not absorb the coding-agent operation scheduler, duplicate session logs,
or reach into agent/provider internals.

```text
crates/pi-mom/
├── src/
│   ├── lib.rs
│   ├── api.rs
│   ├── coordinator/           # cross-runtime policy only
│   ├── task/                  # Mom-owned task lifecycle
│   ├── connector/             # external inbox/channel adapters
│   ├── store/                 # Mom facts, never coding session facts
│   └── notification/
└── tests/
    ├── coordination.rs
    └── boundaries.rs
```

This boundary is reserved, not implemented. An activation ADR must define the
user model, durable facts, and failure semantics before production code lands.

### 3.7 `pi-pods`: Isolated Runtime Hosting (Reserved)

#### Reserved responsibility

When activated, `pi-pods` may own lifecycle and transport for isolated or remote
coding-agent runtime instances: provisioning, health, resource limits,
connection routing, and teardown.

It owns infrastructure lifecycle facts, not product session facts. A pod hosts
or connects to the product runtime through stable APIs/protocols; it must not
fork the provider/agent/session implementation.

```text
crates/pi-pods/
├── src/
│   ├── lib.rs
│   ├── api.rs
│   ├── spec.rs                # desired isolated-runtime specification
│   ├── lifecycle.rs           # provision/start/stop/remove
│   ├── backend/               # local/container/remote implementations
│   ├── transport/             # connection plumbing
│   ├── health.rs
│   └── error.rs
└── tests/
    ├── lifecycle.rs
    └── boundaries.rs
```

This boundary is reserved, not implemented. An activation ADR must define the
isolation and trust model before the crate gains dependencies or process-launch
capabilities.

### 3.8 Root `pi-rust` Package

The workspace root is not a shared domain crate. The target choice is either:

1. remove the root package and keep a virtual workspace; or
2. keep a thin launcher that immediately delegates to a product crate.

It must not accumulate runtime, provider, session, or UI implementation. There
must be no second coding-agent binary behavior hidden at the root.

## 4. Coding-Agent Runtime Contract

### 4.1 End-To-End Dataflow

The target runtime has one command path and two output fact streams:

```text
ClientIntent
    │
    ▼
IntentRouter ──validate/authorize──> OperationScheduler
                                          │ admitted Operation
                                          ▼
                                  OperationRuntime
                                          │
                              operation-specific Flow
                                  │               │
                                  ▼               ▼
                         OperationContext     scoped Services
                                  │               │
                                  └───────┬───────┘
                                          ▼
                         SessionEvent + ProductEvent
                                  │               │
                          durable replay      live delivery
                                  └───────┬───────┘
                                          ▼
                                  Snapshot/Projection
                                          │
                                          ▼
                              print / JSON / RPC / TUI
```

Invariant: adapters never bypass admission to start runtime-affecting work.

### 4.2 Stable Product Verbs

The final embedding facade should converge on a small set of verbs:

```text
open/create a runtime or session
submit a typed operation
send a typed control command
subscribe/reconnect to product events
query capabilities
read a consistent snapshot or committed view
detach/close/shutdown
```

`CodingAgentSession` may remain the compatibility facade while callers migrate.
Existing convenience methods should delegate to these verbs. A new public
method is justified only when it expresses a new stable product verb, not when
it exposes an internal service.

### 4.3 Runtime Owner

There is one `OperationRuntime` owner per runtime instance. It coordinates:

- runtime/session handles and immutable generations;
- operation registry, admission, scheduling, and control;
- service container and event bus;
- client connections and consistent snapshots;
- shutdown and recovery state.

The runtime owner does not implement every operation. It delegates workflow
business logic to operation modules and side effects to services.

Global mutable provider registries, global session state, and adapter-owned
runtime handles are outside the target architecture. Dependencies should be
scoped and injected at runtime construction.

## 5. Operation, Flow, And Service Contracts

### 5.1 Operation Envelope

Every admitted operation receives:

```text
operation_id             stable correlation key
operation_kind           typed product verb
operation_class          scheduling and side-effect class
origin                    ClientRoot, ParentChild, or RuntimeInternal
initiator                 authenticated actor/client when available
capability_generation     frozen authorization generation
runtime_generation        frozen runtime/configuration generation
session_id                optional target session
turn_id                   optional target turn
parent_operation_id       required for child operations
idempotency_key           where retry can duplicate external submission
```

The operation ID correlates outcomes, product events, session facts, child
operations, protocol responses, and diagnostics.

### 5.2 Operation Set

The typed set should cover all runtime-affecting actions, including:

```text
Prompt
ManualCompaction
BranchSummary
Export
PluginLoad / PluginCommand / OpenPluginDialog
AgentInvocation / TeamInvocation
DelegationRequest / Approval / Rejection
SelfHealingEdit
SwitchActiveLeaf
SessionCreate / Open / Resume when runtime-managed
RuntimeSettingsChange / DefaultProfileChange
```

Query-only API calls may use typed queries instead of full operations, but they
must still pass authorization and consistency checks.

### 5.3 Operation Classes And Admission

| Class | Meaning | Concurrency rule |
| --- | --- | --- |
| `Query` | capabilities, lists, current view | no transaction; allowed unless shutting down |
| `ReadOnly` | export, replay, tree, committed transcript | committed state only; may coexist with writer |
| `SessionWriteRoot` | prompt, compaction, summary, edit, navigation, durable delegation approval/rejection | at most one active per session |
| `NonSessionRoot` | root agent/team/plugin execution without parent session write | subject to runtime root execution limit |
| `RuntimeWrite` | plugin/profile/settings/capability generation mutation | generation-safe or exclusive |
| `Child` | delegated/scoped sub-operation | requires parent; cannot silently outlive it |
| `Control` | abort, steer, follow-up, revoke | priority signal to a target operation; never commits session facts |

The current scheduler state implements these root distinctions without a work
queue. A runtime instance defaults to four concurrent `NonSessionRoot` slots;
`RuntimeWrite` is currently exclusive with every root class. The default is an
implementation limit, not a protocol constant, and may become runtime
configuration when the runtime-owned executor is introduced.

A runtime write may install a future-only generation while work is active only
if it does not mutate handles captured by active operations. Otherwise it must
wait, reject, or explicitly revoke/cancel matching operations.

Read-only operations never observe half-committed session transactions. Control
commands are not placed behind ordinary work in a queue that can starve abort.

Root and child IDs share one uniqueness domain. Child admission is not a
lineage-shape check: the referenced parent must still be registered and owned.
Releasing an owner with live descendants transitions that node to draining,
cancels descendants, rejects new children and controls for the released owner,
and keeps runtime shutdown/root capacity blocked until the descendant tree is
empty. Intermediate children use the same deferred-release rule, so dropping a
parent before a grandchild cannot sever the ancestry chain.

### 5.4 Operation Outcome

Every started operation produces exactly one terminal outcome and one terminal
product event:

```text
status:
  Succeeded | Aborted | Failed | RecoveryRequired

persistence:
  NotRequired
  Skipped(reason)
  Committed(session sequence range)
  Failed(error)
  InDoubt(recovery id)
```

Typed operation payloads may extend this common envelope. They must not
contradict it. If durability is required for success, commit must finish before
the successful terminal event is published.

### 5.5 Operation Context

An operation context contains temporary state for one run:

- request and validated inputs;
- operation/capability/runtime metadata;
- cancellation and child-operation scope;
- transaction buffer or scoped write handle;
- intermediate Flow results;
- narrow service/capability handles.

It must not become a long-lived service locator or durable store. Dropping an
operation context must not silently lose facts already advertised as durable.

### 5.6 Flow Nodes

Flow nodes may validate local preconditions, mutate operation-local context,
select the next action, call scoped services, stage typed session facts, and
emit semantic progress through the event service.

Flow nodes must not:

- commit final session storage directly;
- construct RPC/JSON/TUI wire events;
- read or mutate adapter state;
- fetch global provider/auth/plugin/session singletons;
- expose Flow node IDs as product protocol fields.

Flow is orchestration. Durable and external side-effect policy belongs to a
service.

### 5.7 Services

The target services have non-overlapping responsibility:

| Service | Owns |
| --- | --- |
| `SessionService` | transaction creation, append, replay, manifest updates, recovery, session tree/views |
| `RuntimeService` | immutable per-operation model/provider/tool/resource/execution snapshots |
| `FlowService` | construction and execution of product Flow graphs and low-level subflows |
| `EventService` | mapping internal events to `ProductEvent` and publishing them |
| `CapabilityService` | declaration/grant evaluation and generation-scoped handles |
| `PluginService` | plugin discovery, guarded execution, and contribution collection |

Services are internal owners. Adapters and plugins receive typed commands,
snapshots, or scoped handles—not a raw `ServiceContainer`.

## 6. State And Event Contracts

### 6.1 Three State Layers

```text
SessionEvent   durable, replayable session facts
ProductEvent   semantic runtime facts for clients; live or durable-derived
UiState        disposable client-local projection
```

Allowed direction:

```text
SessionEvent ──project──> ProductEvent ──apply──> UiState
live runtime ───────────> ProductEvent ──apply──> UiState
```

`UiState` never writes session history. `ProductEvent` becomes durable only
through explicit session persistence policy; it is not automatically appended.

### 6.2 `SessionEvent`: Durable Truth

`SessionEvent` is the only durable source of coding-session facts. Replay,
resume, fork, clone, export, audit, recovery, transcript, tree, and statistics
must be rebuildable from it.

A target envelope includes:

```text
schema and version
session_id and strictly increasing session_sequence
event_id
operation_id / turn_id / branch_id / leaf_id where applicable
parent_event_id where applicable
created_at
typed SessionEventData
```

Existing Rust-native logs without explicit sequence fields remain readable by a
versioned decoder. Append order/event IDs remain authoritative for those logs.

Durable facts include, when applicable:

- session creation and replay-critical metadata;
- operation and turn start plus terminal status;
- committed user/assistant messages;
- tool request and terminal result/failure/cancellation;
- branch/leaf creation and active-leaf changes;
- model/provider/profile/capability generations needed for audit;
- compaction, branch summary, delegation, plugin, and self-healing workflow
  facts that affect recovery or historical behavior;
- migration, repair, and recovery markers.

If losing a record changes transcript history, resume, fork/clone/export,
recovery, or auditability, it belongs in `SessionEvent`.

### 6.3 Session Transactions

A session-writing operation follows one commit protocol:

```text
1. create an operation transaction
2. stage typed SessionEvent values and referenced artifacts
3. append facts and a terminal marker atomically or detect partial uncertainty
4. update manifest/index/active-leaf derived state only after append succeeds
5. publish the session-write result and terminal operation event
6. update or rebuild projections from committed facts
```

An operation without a terminal durable marker is incomplete. Replay applies
recovery policy; it never presents incomplete output as normally committed.
The event log is authoritative. Manifests, indexes, snapshots, and projections
are rebuildable.

Runtime context compaction and durable session-history compaction remain
different concepts. Only the latter necessarily changes durable history.

### 6.4 `ProductEvent`: Adapter Boundary

Adapters consume one semantic event stream. The event has a common envelope and
typed families:

```text
ProductEvent envelope:
  stream_id, sequence
  operation_id, parent_operation_id, root_operation_id, session_id
  capability_generation
  initiator, causality
  durability
  ProductEventKind

ProductEventKind families:
  Operation, Prompt, Agent, Team, Tool, Session,
  Plugin, Delegation, Workflow, Capability,
  Diagnostic, Pressure
```

Every semantic emitter constructs an owner-local `ProductEventDraft`. There is
no centralized compatibility event enum and no internal-to-public event
conversion. `CodingAgentProductEvent` is canonical from publication through
reconnect and projection. The Prompt owner may temporarily collect only the
closed `PromptStreamEvent` union; that union cannot carry session-write,
profile, capability, or root-operation lifecycle events. Adapters must not
consume raw owner events, `AgentEvent`, or `FlowEvent`.

Product sequence rules:

- sequence is strictly increasing within one `stream_id`;
- one runtime creates one opaque, runtime-unique `stream_id`; it is not a
  session ID and must not be reconstructed from session state;
- every ProductEvent, retained replay entry, and snapshot cursor from that
  runtime carries the same `stream_id`;
- sequence is live delivery order, not durable session order;
- no global order is implied across runtimes;
- child completion order does not dictate deterministic parent merge order;
- clients reconnect using `(stream_id, sequence)` within retention.

Durability is explicit:

```text
LiveOnly
PendingSessionWrite(operation_id)
Durable(session_id, session range)
DerivedFromSession(session_id, source reference)
PersistenceUncertain(operation_id)
PersistenceFailed(operation_id, reason)
```

Required local ordering includes:

- operation start before other events for that operation;
- prompt start before its assistant/tool events;
- tool start before updates and terminal tool status;
- session-write pending before committed/skipped/failed;
- capability change after the new generation is installed;
- parent terminal after children finish, abort, fail, or become durable pending
  requests;
- exactly one authoritative terminal association. `ProductEvent` policy
  operations publish exactly one normalized root terminal event;
  `OutcomeAcknowledgement` policy operations publish none and remain terminal
  until the exact outcome acknowledgement is accepted.

### 6.5 Snapshots And Projections

A snapshot is a complete projection at a declared product-event boundary:

```text
Snapshot:
  cursor(stream_id, last_product_sequence, session_sequence, runtime_generation)
  committed session view
  optional live operation view
  pending controls/confirmations
  capabilities
```

Invariant:

```text
UiState at N = Snapshot including N + ProductEvents after N applied in order
```

Creating the snapshot and cursor requires a proven consistency point. The
runtime must not capture a cursor and independently read mutable state in a way
that includes later effects or misses earlier effects.

Clients apply events only for the matching stream, ignore duplicates at or
before the cursor, detect gaps, and request a fresh snapshot when retention no
longer covers the gap. Live output is kept separate from committed transcript
until durable facts reconcile it.

## 7. Capability And Plugin Contracts

Capabilities are generation-scoped authorization snapshots, not booleans
scattered through UI and services:

```text
Declare -> Grant -> Snapshot at operation admission -> Use scoped handle
        -> Revoke by installing a new generation and optional cancellation
```

An operation snapshot identifies the actor, operation, generation, and narrow
handles for model, tools, filesystem, shell, session read/write, UI, commands,
and plugins. It answers both “may this actor do this?” and “through which
interface may it do it?”

Active operations do not silently hot-update snapshots. Configuration/plugin
changes are future-only by default. Trust, secret, or filesystem permission
revocation may explicitly cancel matching operations and must emit semantic
events.

Plugins may contribute guarded tools, commands, hooks, UI actions, keybindings,
and dialogs. They must not receive raw session repositories, provider clients,
runtime services, arbitrary Flow registration, or unrestricted adapter state.
Every contribution executes through a declared capability boundary.

Plugin UI contributions use a reference-only execution contract. Commands are
the executable leaves; a dialog submits to a registered command, and every UI
action references either a registered command or dialog. A keybinding must
reference the target exposed by a registered UI action. Plugin load validates
the complete contribution graph and rejects unresolved references before
installing the plugin service. Adapters may render actions and dialogs and
collect typed dialog arguments, but command execution still enters through the
canonical capability-aware `PluginCommand` operation. A transcript notice or
editor text that resembles a plugin command is not execution and must not be
used as an internal dispatch transport.

## 8. Adapter And Multi-Client Contracts

### 8.1 Adapter Rule

```text
UI/wire intent in
admission response, ProductEvent, and Snapshot out
```

Print, JSON, RPC, TUI, and future web clients are peers over the same semantic
runtime contract. An adapter may parse/validate syntax, construct a typed
intent, render/project events, and maintain client-local state. It must not
start Flows, append session events, execute tools, or resolve provider auth.

Machine-readable modes reserve stdout for their protocol. Diagnostics use
structured protocol events or stderr as defined by the adapter; they must not
corrupt JSONL/RPC output.

An adapter connection owns at most one live ProductEvent subscription and one
projection cursor for a runtime session. Per-operation subscriptions are
forbidden: every receiver observes the complete session stream, so one receiver
per concurrent root would duplicate events and split sequence ownership. A
bounded session event pump may use a completion barrier to place all events
emitted before an operation terminal outcome into the adapter queue before that
completion is retired.

Foreground interaction and background execution use different completion
owners. The foreground slot represents the operation that accepts prompt
control. Runtime-owned background roots are registered by exact operation ID
and report completion through a separate channel that carries outcomes only,
never ProductEvents. Session navigation and shutdown reason over the union of
both owners; Prompt steer/follow-up/abort reason only over the foreground Prompt
identity.

### 8.2 Client-Local State

Draft text, cursor, selection, IME composition, viewport, scroll, focus, menu
highlight, autocomplete, window layout, and unsubmitted undo history remain
client-local. Submitted prompt input becomes runtime-owned operation data.

### 8.3 Intent Router

All runtime-affecting client actions pass through one router that performs:

- protocol and input validation;
- client identity and capability checks;
- runtime/session/operation existence checks;
- optimistic cursor or generation checks where required;
- scheduler admission and pressure policy;
- operation creation or control dispatch;
- an explicit admission response to the initiating client.

### 8.4 Detach, Abort, And Conflict

Detaching a client does not abort its operation unless explicit product policy
says so. Abort is a shared runtime control and its terminal result is visible to
all authorized subscribers.

Shared mutations such as active-leaf switches and settings changes use expected
cursor/generation guards or serialization. The runtime rejects stale ambiguous
mutations rather than silently applying last-writer-wins behavior.

## 9. Failure, Pressure, And Versioning

### 9.1 Errors And Recovery

Errors are typed operation outcomes. A product error identifies category,
phase, safe user message, retry advice, and secret-free diagnostics.

Recommended categories are input, configuration, authentication, capability,
provider, tool, plugin, session store, projection, concurrency, cancellation,
and internal. Recommended phases are prepare, run, finalize, commit, project,
and recover.

Rules:

- user abort is `Aborted`, not `Failed`;
- unknown durable commit state is `InDoubt`/`RecoveryRequired`;
- success never precedes required durable commit;
- every open operation/message/tool family closes with a terminal marker;
- startup scans incomplete operations, open message/tool families, index/log
  mismatch, unreferenced staged artifacts, and unknown commit state;
- recovery never promotes incomplete live output to committed history.

### 9.2 Backpressure

Durable facts and terminal outcomes are never silently dropped. Derived views
can be rebuilt; high-frequency live deltas may be throttled or coalesced with an
explicit semantic marker.

Slow subscribers must not block session commit, terminal event creation,
capability revocation, abort, or recovery. When retention or queue budgets are
exceeded, the runtime emits pressure, rejects, disconnects, cancels, or requires
fresh-snapshot recovery according to the event class.

### 9.3 Protocol Versions

Version these families independently:

```text
SessionEvent
ProductEvent
ClientIntent / control
Snapshot
PluginHost
Capability
ToolSchema
```

Major changes are incompatible; minor changes are backward-compatible
additions; patch versions are implementation details. Unknown required features
fail closed. Durable session decoding is the most conservative family. Live
clients negotiate compatible versions and fail clearly when none exists.

## 10. Migration From The Current Tree

Migration is incremental and behavior-preserving. The former
`coding_session/` container was split by moving complete vertical slices; do
not recreate a second runtime or compatibility container beside the target
owners.

### Phase 1: Establish Facades And Boundary Tests

- Keep `pi_ai::api`, `pi_agent_core::api`, `pi_tui::api`, and
  `pi_coding_agent::api` as the supported cross-crate imports.
- Introduce categorized facade modules and migrate imports according to the
  dependency-edge allowlists, keeping deprecated flat re-exports temporarily.
- Add dependency-direction and forbidden-type tests before moving modules.
- Consolidate boundary assertions into the owning crate's existing contract
  target instead of creating a new integration binary for every rule.
- Record current compatibility exports and their removal conditions.
- Make the root package's future (virtual workspace or thin launcher) explicit.

Exit: new code cannot introduce a reverse dependency or a second public facade
without a failing boundary test.

### Phase 2: Normalize Lower-Crate Layouts

- Group `pi-ai` by model/protocol/registry/provider/transport.
- Normalize provider submodules to a common internal pattern.
- Converge `pi-agent-core` agent/harness/loop modules under one agent runtime.
- Group `pi-tui` by terminal/input/render/component/editing.

Use mechanical moves with unchanged public facade exports. Exit: directory
names describe owned concepts and downstream public paths remain stable.

### Phase 3: Split The `coding_session` Migration Container

Move one complete vertical slice at a time:

```text
common operation/admission/control -> runtime/
SessionEvent/store/replay          -> session/
ProductEvent families              -> events/
one workflow context + Flow        -> operations/<workflow>/
reusable side effects              -> services/
RPC/interactive projection         -> adapters/
```

Do not split request, Flow, service, and tests for several workflows in one
large move. This phase has reached its structural exit: `coding_session/` is
deleted, the facade lives under `runtime/facade`, and no temporary re-export
module remains. Adapter relocation continues independently under its target
owner.

### Phase 4: Converge Runtime Contracts

- Route prompt and workflow entrypoints through typed `Operation` submission.
- Centralize client intent admission and scheduler classes.
- Group the flat compatibility event enum into product event families.
- Add consistent snapshot cursors and reconnect/gap behavior.
- Integrate generation-scoped capabilities into operations and plugins.

Exit: every runtime-affecting adapter action uses one admission path; adapters
consume only product events/snapshots.

### Phase 5: Harden Durability And Recovery

- Formalize durable session sequence semantics and compatibility decoding.
- Add incomplete-operation recovery and idempotency behavior.
- Make manifests/indexes/projections demonstrably rebuildable.
- Test commit uncertainty, interruption, and recovery markers.

Exit: replay distinguishes committed, aborted, failed, recovered, and in-doubt
operations without hidden adapter state.

### Phase 6: Narrow The Product Facade And Delete Shims

- Promote only open/submit/control/subscribe/capabilities/snapshot/close verbs.
- Migrate internal adapters and external tests from broad convenience methods.
- Remove deprecated paths after the last caller and compatibility test move.
- Do not activate placeholder crates merely to relocate unwanted code.

Exit: one runtime owner, one admission path, one durable session fact source,
one semantic event stream, and no parallel legacy implementation.

## 11. Test Strategy And Value Budget

Tests exist to protect contracts and high-risk behavior, not to mirror the
source tree. The objective is maximum defect-detection value per compile, link,
runtime, and maintenance cost.

### 11.1 Current Cost Baseline

A final source-tree scan for `0.2.0` on 2026-07-16 finds:

| Crate | Independent `tests/*.rs` targets | Test cases in `src/` + `tests/` |
| --- | ---: | ---: |
| `pi-ai` | 3 | 188 |
| `pi-agent-core` | 8 | 223 |
| `pi-tui` | 8 | 245 |
| `pi-coding-agent` | 11 | 1,251 |

The case counts are source-level `#[test]`/`#[tokio::test]` counts and are
diagnostic, not quality scores. P1 already consolidated the integration targets;
P5 reduces semantic duplication inside those targets and owner-local test
modules. Every direct `tests/*.rs` file is a separate Cargo test binary and can
repeat compilation, monomorphization, and linking of a large dependency graph,
so target count remains a first-class budget.

The final tree contains no `#[ignore]` tests. The former local transcript render
timing probe was removed because it printed wall-clock statistics without a
regression threshold and duplicated cache-correctness assertions already owned
by deterministic tests.

Inline `#[cfg(test)]` modules share the crate's library-test binary, but very
large inline test modules still increase test compilation and obscure
production code. They should remain focused on private, local invariants.

### 11.2 Test Value Tiers

Every test must fit one tier:

| Tier | Keep/add when it protects | Examples |
| --- | --- | --- |
| P0 contract/safety | compatibility, durability, security, cancellation, concurrency, public boundary, or a costly regression | session recovery, event terminal ordering, path confinement, secret redaction, terminal restore |
| P1 representative behavior | one representative success plus materially different failure/edge classes | one tool success/error/cancel path; Unicode width boundaries; provider stream completion/error |
| P2 implementation detail | incidental structure or behavior already guaranteed elsewhere | getter tests, node-count tests, duplicate happy paths, formatting snapshots for private helpers |

P0 tests are mandatory and should be deterministic. P1 tests are selected by
equivalence class, not by enumerating every combination. P2 tests should not be
added and should be removed when encountered unless they document a real past
regression that cannot be protected at a better boundary.

A new test must answer:

1. Which defect could this catch?
2. Which crate owns that defect?
3. Is the behavior already covered at a lower, cheaper layer?
4. What distinct state transition, failure mode, or compatibility promise does
   this case add?
5. Can it join an existing test binary/table instead of creating a target?

### 11.3 Coverage Ownership By Crate

Only the owning crate exhaustively tests a concern. Downstream crates test the
integration seam and their own mapping/policy, not the upstream implementation.

| Owner | Must cover | Must not duplicate |
| --- | --- | --- |
| `pi-ai` | provider request/stream conversion, auth/header redaction boundary, transport retry/error classification, model catalog invariants, scoped registry/client behavior | agent turns, product sessions, CLI selection policy, TUI |
| `pi-agent-core` | agent-loop state transitions, tool ordering/concurrency/cancellation, hook patching, Flow semantics, compaction thresholds, generic resources/execution/transcript conversion | provider-specific wire matrices, coding-session durability, RPC/TUI behavior |
| `pi-tui` | terminal cleanup/resize/cursor, input normalization, Unicode width/wrap, editor transitions, component rendering on virtual terminal | product commands, sessions, model/tool behavior, coding-agent event projection |
| `pi-coding-agent` | operation admission/outcomes, capability policy, session transaction/replay/recovery, product-event ordering, protocol mapping/versioning, coding-tool safety, plugin policy, adapter integration | provider wire details, core Flow internals, generic editor/widget algorithms |
| reserved crates | their own contracts after activation | speculative tests for behavior that has no implementation |

Cross-layer examples:

- `pi-ai` tests that an Anthropic stream becomes canonical AI events.
- `pi-agent-core` uses one faux canonical stream to test the agent loop; it does
  not rerun the Anthropic/OpenAI/Google matrix.
- `pi-coding-agent` uses one faux agent/provider path to test product event and
  session persistence; it does not re-prove core tool-loop semantics.
- the interactive adapter tests that a product event updates a view model and a
  small number of terminal lifecycle scenarios; it does not duplicate every
  generic `pi-tui` component test.

### 11.4 Required Scenarios Per Crate

#### `pi-ai`

Keep focused coverage for:

- one table-driven contract suite per provider family covering representative
  text, tool, reasoning/multimodal capability when supported, terminal event,
  structured error, and auth redaction;
- shared transport retry, timeout/cancellation, SSE framing, and error mapping;
- scoped `AiClient`/registry isolation and authentication resolver contracts;
- model catalog uniqueness, required metadata, and cost calculation boundaries;
- public categorized facade compilation.

Do not create an integration target per provider or per request field. Pure
wire conversion belongs in provider-local unit/table tests; shared transport is
tested once. Do not test live provider availability in ordinary CI.

#### `pi-agent-core`

Keep focused coverage for:

- agent turn state transitions: stop, tool use, failure, abort, queued input,
  and context compaction;
- ordering and cancellation for sequential/parallel tool execution;
- hook application order and patch conflict/error behavior;
- generic Flow transition/error/cancellation semantics;
- compaction decision boundaries and summary integration using one faux stream;
- execution/resource/transcript round trips that cross a public boundary;
- the exact `pi-agent-core -> pi-ai` allowlist and public facade.

Do not assert private node topology, exact node count, private helper call order,
or every combination of queue/thinking/tool flags. Do not reproduce provider
mapping tests from `pi-ai`.

#### `pi-tui`

Keep focused coverage for:

- terminal enter/restore on success, error, panic-equivalent unwind boundary,
  resize, and capability negotiation;
- input/key normalization for materially different terminal encodings;
- editor state transitions, selection, undo, kill ring, and Unicode graphemes;
- display width, ANSI wrapping/truncation, cursor placement, and overlays;
- representative markdown/component/image rendering using `VirtualTerminal`;
- generic public facade and the absence of product types.

Do not snapshot every color/theme combination, terminal escape permutation, or
component constructor. Do not duplicate pure width/editor cases through the
coding-agent interactive adapter.

#### `pi-coding-agent`

Keep focused coverage for:

- every operation's admission class, dispatch mode, outcome family, and
  authoritative terminal association in table-driven contract tests;
- session transaction success, abort, storage failure, partial uncertainty,
  replay, recovery, fork, clone, and active-leaf invariants;
- ProductEvent local ordering, durability transitions, retention gap, reconnect,
  snapshot consistency, and slow-client pressure;
- capability generation/revocation and plugin/tool authorization boundaries;
- coding-tool path safety, mutation serialization, cancellation, truncation,
  and error projection;
- one representative success/failure/cancel flow for print, JSON, RPC, and
  interactive adapters, plus protocol version compatibility;
- configuration precedence and secret-free diagnostics at the product boundary;
- exact cross-crate allowlists and the narrow embedding facade.

Do not test the same operation independently through facade, print, JSON, RPC,
and TUI unless each adapter has distinct mapping behavior. A common runtime
contract suite proves runtime behavior once; adapter tests prove only input and
output translation. Do not duplicate provider matrices, core Flow mechanics,
or generic widget rendering.

### 11.5 Prohibited Or Low-Value Tests

The following are prohibited by default:

- tests for trivial getters/setters, constructors that only assign fields, or
  derived `Clone`, `Debug`, `Eq`, and ordinary Serde behavior;
- one test file per source file or one integration binary per feature;
- multiple tests differing only in literal strings, IDs, model names, colors,
  or equivalent enum variants;
- golden/snapshot tests for private formatting with frequent intentional churn;
- assertions on private module layout, Flow node IDs, incidental allocation,
  log wording, or internal helper call sequence;
- downstream repetition of behavior owned and tested by an upstream crate;
- tests that require live provider credentials, public network access, wall
  clock sleeps, developer configuration, or shared mutable home directories;
- compile-only tests repeated for every exported item when one representative
  external-consumer facade test suffices;
- public production APIs added only to make private implementation testable;
- `api::testing` fixtures used by production modules;
- exhaustive combinatorial tests without a documented risk model.

Serde tests are justified only for durable/session/protocol compatibility,
custom serialization logic, or security-sensitive omission/redaction. Snapshot
tests are justified for stable public wire/render contracts, with a small number
of representative fixtures.

### 11.6 Test Binary And Layout Budget

Target maximum independent integration binaries:

| Crate | Target `tests/*.rs` binaries | Suggested grouping |
| --- | ---: | --- |
| `pi-ai` | 4 | `api_contract`, `provider_contract`, `transport`, `registry` |
| `pi-agent-core` | 8 | `api_contract`, `agent`, `flow`, `tool_hooks`, `execution`, `resources`, `compaction`, `transcript` |
| `pi-tui` | 8 | `api_contract`, `terminal`, `input`, `editing`, `render`, `components`, `markdown_image`, `boundaries` |
| `pi-coding-agent` | 12 | `api_contract`, `operation`, `session`, `events_snapshot`, `capability_plugin`, `tools`, `config`, `print_json`, `rpc`, `interactive`, `boundaries`, `recovery` |
| each reserved crate | 0 until activation; then at most 2 initially | public contract and primary behavior |

These are budgets, not invitations to fill every slot. Exceeding one requires a
documented reason showing that the new binary needs different features, process
isolation, or a materially different test harness. File size alone is not a
reason: a target entry file can include domain modules stored below
`tests/<target>/`, which Cargo does not compile as independent binaries.

Recommended layout:

```text
tests/
├── operation.rs              # one Cargo integration target
├── operation/
│   ├── admission.rs          # module of operation.rs, not a target
│   ├── outcomes.rs
│   └── control.rs
├── session.rs
├── session/
│   ├── transaction.rs
│   ├── replay.rs
│   └── recovery.rs
└── support/
    ├── mod.rs
    ├── faux_runtime.rs
    └── fixtures.rs
```

Large reusable deterministic fixtures belong in one support module. Avoid
generic fixture frameworks that monomorphize many variants into every test
binary. Prefer simple builders returning product/core types.

### 11.7 Unit, Contract, Scenario, And Smoke Tests

Use the cheapest layer that can catch the defect:

```text
unit test
  private pure logic and small state transitions; same module; no full runtime

contract integration test
  public facade, dependency allowlist, durable/wire compatibility

scenario integration test
  one vertical product/runtime behavior crossing real module boundaries

smoke test
  process/terminal lifecycle and packaging; very few representative paths
```

Do not promote a unit case into an integration scenario merely to improve
nominal end-to-end coverage. Do not keep both after a regression is adequately
protected at the cheaper owner boundary.

### 11.8 Determinism And Test-Only Interfaces

- Provider behavior uses `pi_ai::api::testing` faux providers; no live network.
- `api::testing` should be behind a non-default `test-support` feature where
  downstream integration tests require it. Normal production builds must not
  expose or compile large fixture implementations unnecessarily.
- Filesystem/session tests use temporary roots and product environment guards.
- Time, IDs, cancellation, and scheduling use injected clocks/generators or
  controlled Tokio time; no arbitrary sleeps.
- Environment mutation uses the repository guard and restores previous values.
- Large fixture content should be data files loaded only by the owning target,
  not Rust literals duplicated across binaries.

### 11.9 Test Reduction Procedure

Test reduction is behavior-preserving work:

1. inventory cases by owned contract and failure class;
2. identify duplicates across unit, integration, and adapters;
3. retain the cheapest authoritative assertion for each behavior;
4. merge direct `tests/*.rs` files into domain targets before changing cases;
5. replace literal permutations with table-driven equivalence classes;
6. remove P2 structural/trivial cases;
7. compare targeted test results and compile/link time before and after;
8. keep regression cases whose failure history demonstrates value.

Raw coverage percentage is not a target. Coverage can help find untested P0
paths, but it must not justify low-value tests for trivial lines.

## 12. Validation And Architecture Guardrails

Every structural or cross-crate change must use the smallest relevant checks
while iterating and broaden validation with blast radius:

```text
cargo fmt --all --check
cargo check/test for the owning crate
public_api and boundary tests for changed cross-crate contracts
RPC/CLI adapter tests for product event/protocol changes
session replay/recovery tests for durable changes
scripts/tui-smoke.sh for terminal lifecycle or interactive behavior
full workspace clippy/test before completing a broad migration phase
```

Additional invariants:

- preserve async cancellation, operation association, and streaming order;
- never block async runtime paths with uncontrolled filesystem/process I/O;
- never print secrets in events, diagnostics, snapshots, or fixtures;
- use deterministic providers and isolated `PI_RUST_DIR` test state;
- keep public APIs small and update their contract tests intentionally;
- do not weaken boundary tests or Clippy rules to accommodate misplaced code;
- document temporary compatibility modules with a removal condition;
- do not create a shared crate until at least two real consumers need the same
  stable, dependency-neutral contract.

## 13. Final Target

The final architecture is intentionally simple:

```text
pi-ai             owns AI providers and protocol-neutral model I/O
pi-agent-core     owns the reusable agent loop and Flow primitives
pi-tui            owns generic terminal interaction and rendering
pi-coding-agent   owns the coding-agent product and operation runtime
pi-web-ui         may become a thin browser client
pi-mom            may become a cross-session orchestration product
pi-pods           may become an isolated runtime host
```

Inside the coding-agent product there is one operation admission path, one
runtime owner, one durable Rust-native session fact model, one product semantic
event stream, one consistent snapshot/projection contract, and many thin
adapters. That is the organizing rule for both source files and runtime
behavior.
