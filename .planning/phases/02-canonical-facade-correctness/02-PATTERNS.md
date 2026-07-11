# Phase 2: Canonical Facade Correctness - Pattern Map

**Mapped:** 2026-07-11
**Files analyzed:** 6 likely modified files; no new production files expected
**Analogs found:** 6 / 6

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/pi-coding-agent/src/lib.rs` | public API facade/configuration | compile-time type projection | Existing curated `api` re-export block in the same file | exact |
| `crates/pi-coding-agent/src/coding_session/public_operation.rs` | public model/transform boundary | request-response transform | Existing exhaustive `into_internal` and `from_internal` matches in the same file | exact |
| `crates/pi-coding-agent/src/coding_session/operation.rs` | private operation model/metadata provider | request classification | Existing exhaustive `Operation::metadata` match in the same file | exact |
| `crates/pi-coding-agent/src/coding_session/mod.rs` | session owner/controller plus owner tests | async, sync request-response, event-driven, durable file I/O | Existing canonical fork/switch/branch-summary tests and dispatcher tests in the same module | exact |
| `crates/pi-coding-agent/tests/public_api.rs` | downstream integration/API test | compile-time API closure plus async request-response | Existing facade-only imports and canonical `run` tests in the same file | exact |
| `crates/pi-coding-agent/tests/api_boundary_guards.rs` | architecture/boundary test | source inspection and compile boundary | Existing root visibility and canonical-dispatch guards in the same file | exact |

No separate helper or fixture file is implied by CONTEXT/RESEARCH. Existing tempfile, faux-provider, plugin, profile, delegation, replay, and failure-injection fixtures should remain at their current owning layers.

## Pattern Assignments

### `crates/pi-coding-agent/src/lib.rs` (public API facade, compile-time type projection)

**Analog:** the current stable facade block in `crates/pi-coding-agent/src/lib.rs:60-118`.

**Core export pattern** (`lib.rs:60-82`):

```rust
/// Stable library facade for embedding or scripting `pi-coding-agent`.
///
/// The root modules remain public during the migration, but downstream crates
/// should prefer this module for APIs that are intended to stay stable.
pub mod api {
    pub use crate::coding_session::{
        AgentInvocationOptions, AgentInvocationOutcome, AgentProfile, AgentTeamMemberOutcome,
        AgentTeamOptions, AgentTeamOutcome, BranchSummaryReusePolicy, CapabilityRevocationPolicy,
        CapabilityStatus, CodingAgentCapabilities, CodingAgentClientConnection,
        CodingAgentClientId, CodingAgentEvent, CodingAgentOperation, CodingAgentOperationOutcome,
        CodingAgentPluginDiagnostic, CodingAgentPluginLoadOutcome, CodingAgentProductEvent,
        CodingAgentProductEventReceiver, CodingAgentSession, CodingAgentSessionExport,
        // ...curated caller-facing contracts only...
    };
}
```

Copy this pattern by adding only types proven missing by the signature-closure ledger. Keep implementation modules, `Operation`, metadata/dispatch types, services, Flow nodes, and raw `PluginLoadOptions` private. Do not replace or remove crate-root compatibility exports in this phase.

**Closest privacy analog:** root exports immediately above the facade are explicitly deprecated (`lib.rs:55-58`), while stable consumers receive direct `api` exports. Preserve that separation.

---

### `crates/pi-coding-agent/src/coding_session/public_operation.rs` (public model/transform boundary)

**Analog:** the existing exhaustive conversion and projection matches.

**Public variant shape** (`public_operation.rs:41-83`):

```rust
#[derive(Debug)]
pub enum CodingAgentOperation {
    Prompt(PromptTurnOptions),
    Compact(PromptTurnOptions),
    // ...all 15 public variants...
    ForkSession { target_leaf_id: Option<String> },
    SwitchActiveLeaf { target_leaf_id: String },
    ExportCurrent,
    ExportCurrentHtml(PathBuf),
}
```

**Public-to-private transform** (`public_operation.rs:106-156`):

```rust
impl CodingAgentOperation {
    pub(crate) fn into_internal(self, plugin_load: PluginLoadOptions) -> Operation {
        match self {
            Self::Prompt(options) => Operation::Prompt(options),
            Self::Compact(options) => Operation::ManualCompaction(options),
            Self::PluginLoad => Operation::PluginLoad(plugin_load),
            Self::ForkSession { target_leaf_id } => Operation::ForkSession { target_leaf_id },
            Self::ExportCurrent => Operation::Export(ExportOptions::view()),
            Self::ExportCurrentHtml(path) => Operation::Export(ExportOptions::html(path)),
            // exhaustive: no wildcard arm
        }
    }
}
```

**Private-to-public projection** (`public_operation.rs:160-180`):

```rust
impl CodingAgentOperationOutcome {
    pub(crate) fn from_internal(outcome: OperationOutcome) -> Self {
        match outcome {
            OperationOutcome::Prompt(outcome) => Self::Prompt(outcome),
            OperationOutcome::SetDefaultAgentProfile => Self::DefaultAgentProfileChanged,
            OperationOutcome::ForkSession => Self::SessionForked,
            OperationOutcome::SwitchActiveLeaf => Self::ActiveLeafSwitched,
            OperationOutcome::Export(outcome) => match outcome.path {
                Some(path) => Self::ExportHtml(path),
                None => Self::Export(outcome.export),
            },
            // exhaustive: no wildcard arm
        }
    }
}
```

Keep the two export projections distinct. The best test analog is the colocated test module beginning at `public_operation.rs:201`: owner tests can inspect crate-private internal variants without exposing them publicly. Extend this module with the independent mapping/outcome ledger rather than adding production instrumentation.

---

### `crates/pi-coding-agent/src/coding_session/operation.rs` (private operation model/metadata provider)

**Analog:** `Operation` and `Operation::metadata` in the same file.

**Privacy and metadata pattern** (`operation.rs:11-49`, `operation.rs:72-92`):

```rust
#[derive(Debug)]
pub(crate) enum Operation {
    Prompt(PromptTurnOptions),
    ManualCompaction(PromptTurnOptions),
    PluginLoad(PluginLoadOptions),
    PluginCommand { command_id: String, args: serde_json::Value },
    // ...private internal variants...
}

pub(crate) fn metadata(&self) -> OperationMetadata {
    match self {
        Self::Prompt(_) => OperationMetadata::new(
            Some(OperationKind::Prompt),
            OperationOrigin::ClientRoot,
            OperationClass::SessionWriteRoot,
            OperationDispatchMode::Async,
        ),
        Self::PluginLoad(_) => OperationMetadata::new(
            Some(OperationKind::PluginLoad),
            OperationOrigin::ClientRoot,
            OperationClass::RuntimeWrite,
            OperationDispatchMode::Async,
        ),
        // exhaustive: each internal variant owns one metadata row
    }
}
```

Do not generate expected test values from this method. The Phase 02 contract ledger must state expected dispatch modes independently, then compare them to `metadata()`. Preserve the special distinction that delegation approval has dynamic kind resolution and async dispatch, while rejection is static sync-mutable.

---

### `crates/pi-coding-agent/src/coding_session/mod.rs` (session owner/controller and owner tests)

**Dispatcher analog:** `CodingAgentSession::run` at `mod.rs:249-260`.

```rust
pub async fn run(
    &mut self,
    operation: CodingAgentOperation,
) -> Result<CodingAgentOperationOutcome, CodingSessionError> {
    let operation = operation.into_internal(self.default_plugin_load_options.clone());
    let dispatch_mode = operation.metadata().dispatch_mode;
    let outcome = match dispatch_mode {
        OperationDispatchMode::Async => self.run_operation(operation).await?,
        OperationDispatchMode::SyncReadOnly => self.run_sync_operation(operation)?,
        OperationDispatchMode::SyncMutable => self.run_sync_mut_operation(operation)?,
    };
    Ok(CodingAgentOperationOutcome::from_internal(outcome))
}
```

This is already the desired production pattern. Phase work should primarily strengthen owner evidence around it.

**Canonical mutation test analog:** canonical fork preserves runtime and event continuity (`mod.rs:3366-3425`):

```rust
session.run(CodingAgentOperation::ForkSession {
    target_leaf_id: Some(target_leaf_id),
}).await.unwrap();

let command = session.run(CodingAgentOperation::PluginCommand {
    command_id: "plugin.say_hello".into(),
    args: serde_json::Value::Null,
}).await.unwrap();

assert!(emitted.windows(2)
    .all(|events| events[0].sequence() < events[1].sequence()));
```

Use this outcome/state/event sequence for plugin, profile, and delegation canonical tests: execute through `run`, assert the exact public outcome, assert owner state, collect semantic product events, and verify monotonic sequences.

**Partial-commit analog:** canonical switch failure after durable append (`mod.rs:3430-3494`):

```rust
let error = session.run(CodingAgentOperation::SwitchActiveLeaf {
    target_leaf_id: target_leaf_id.clone(),
}).await.unwrap_err();

assert!(matches!(
    &error,
    CodingSessionError::PartialCommit { operation_id, .. }
        if operation_id.starts_with("op_")
));
assert_eq!(error.code(), "partial_commit");
assert_eq!(session.persistent_session_service().replay().unwrap()
    .active_leaf_id.as_deref(), Some(target_leaf_id.as_str()));
```

Reuse this exact boundary pattern only for operations that append a durable fact before manifest/publication can fail. Pair it with existing transaction failure injection for the pre-append/no-mutation case; do not invent a new durability simulator.

**Plugin error analog:** `mod.rs:3498-3530` asserts stable error code/message and releases the operation guard. Add a successful canonical `PluginCommand` projection beside it rather than replacing error coverage.

**Delegation metadata analogs:** `mod.rs:3562-3624` separately prove dynamic approval admission and static rejection admission. Canonical behavior tests should retain this asymmetry and add queue/decision state effects through public `run`.

**Branch-summary reuse analog:** `mod.rs:4200-4265` records the event log before calling canonical `run` with `ReuseExisting`. Extend the existing sequence to assert the public outcome, unchanged summary/event count, no duplicate durable record, no unexpected emitted events, and reopen/replay equivalence.

---

### `crates/pi-coding-agent/tests/public_api.rs` (downstream integration/API test)

**Analog:** facade-only import block and current public operation visibility test at `public_api.rs:8-29` and `public_api.rs:127`.

Use a single explicit `use pi_coding_agent::api::{...};` block for every caller-facing type in the closure. Do not import supporting types from crate root or implementation modules. The closure ledger should cover:

- `CodingAgentSession::{create, open, open_or_create, non_persistent, list, run, snapshot, connect, subscribe_product_events_public}` signatures.
- Every nested payload of all 15 `CodingAgentOperation` variants.
- Every nested payload of `CodingAgentOperationOutcome` and `CodingSessionError` that downstream callers must name.
- Existing snapshot/query, event subscription, client connection, capabilities/view, and control contracts.

**Canonical integration analog:** the existing tempfile-backed public tests around `public_api.rs:177` create a real session and invoke public contracts asynchronously. New FACADE-01/FACADE-05 public evidence should follow that shape:

```rust
let temp = tempfile::tempdir().unwrap();
let mut session = CodingAgentSession::create(
    CodingAgentSessionOptions::new().with_session_log_root(temp.path()),
).await.unwrap();

let outcome = session.run(CodingAgentOperation:: /* variant */).await.unwrap();
assert!(matches!(outcome, CodingAgentOperationOutcome:: /* family */));
```

Keep provider work deterministic through the existing support guards and faux provider. Do not broadly migrate compatibility-path tests assigned to Phase 4.

---

### `crates/pi-coding-agent/tests/api_boundary_guards.rs` (architecture/boundary test)

**Analog 1:** root module visibility guard at `api_boundary_guards.rs:5-42`.

**Analog 2:** explicit compatibility re-export guard at `api_boundary_guards.rs:44-77`.

**Analog 3:** canonical dispatcher source guard at `api_boundary_guards.rs:79-155`, which locates the `run` body and asserts conversion, metadata selection, all three dispatchers, centralized projection, and absence of compatibility calls.

Prefer ordinary downstream compilation/type naming in `public_api.rs` for positive stable API closure. Use this source-guard file only where Rust cannot directly express the structural negative, such as requiring `#[doc(hidden)]` on migration modules or rejecting a forbidden stable-facade re-export token. Keep each guard narrow, use explicit source paths/tokens, and include failure messages that name the architectural contract.

## Existing Fixture Analogs

These files are likely read/reused, not modified broadly in Phase 02:

| Concern | Closest Existing Analog | Pattern to Reuse |
|---|---|---|
| Persistent session and profile reopen | `crates/pi-coding-agent/tests/agent_profile_session.rs` | tempfile session root, mutate profile, reopen, assert replayed/default profile state |
| Plugin load and command | owner tests near `coding_session/mod.rs:2770` and `:3498` | first-party `PluginLoadCandidate`, local registry/provider, public diagnostic/output projection |
| Delegation approval/rejection | `crates/pi-coding-agent/tests/delegation_execution.rs` | deterministic pending-confirmation queue, exact operation/tool IDs, state and error assertions |
| Branch persistence/reuse | owner tests at `coding_session/mod.rs:4115` and `:4200` | faux provider, typed branch summary record, event-log count before/after, replay inspection |
| Failure before/after append | `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs:434-455` and tests near `:1060` | real append/manifest ordering, `InDoubt`, `PartialCommit`, replay as authority |
| Environment/provider isolation | `crates/pi-coding-agent/src/lib.rs:120-150` and `tests/support/mod.rs` | serialized guards that save and restore process-global state |

## Shared Patterns

### Independent Expectations

The 15-row owner matrix must use test-owned enums or match helpers for expected internal variant, dispatch family, and public outcome family. Never derive expected values from `Operation::metadata` or `from_internal`; otherwise the test repeats the implementation rather than checking it.

### Exhaustiveness

Production conversions remain wildcard-free exhaustive `match` expressions. Owner tests should make every current variant visible in an explicit ledger so adding a sixteenth variant creates both compiler work and a conspicuous test-ledger change.

### Durable Behavior Checklist

Apply, where relevant, in this order: public outcome; immediate owner state; unchanged error code/message; semantic product events and monotonic sequence; durable log/replay state; reopen equivalence; explicit `PartialCommit` only after durable append. Non-durable plugin commands should not be forced into persistence assertions.

### Error Handling

Assert typed `CodingSessionError` variants plus stable `code()` and meaningful message text. For operation-control tests, also assert guard release on error or preservation of the existing active operation when admission rejects work.

### Scope Control

Do not migrate production adapters, delete compatibility methods, redesign product events, expose private runtime types, add dispatcher instrumentation, or broaden failure testing into crash-consistency redesign. Those changes are outside Phase 02.

## No Analog Found

None. Every likely Phase 02 modification has an exact current analog in the owning file or test layer. No new file or abstraction is justified by the research.

## Metadata

**Analog search scope:** `crates/pi-coding-agent/src/lib.rs`, `src/coding_session/{public_operation.rs,operation.rs,mod.rs,session_service.rs,session_log/transaction.rs}`, `tests/{public_api.rs,api_boundary_guards.rs,agent_profile_session.rs,delegation_execution.rs,support/mod.rs}`

**Pattern extraction method:** repository `.codegraph/` index first, followed by targeted line-numbered reads of the returned symbols and owning tests.

**Pattern extraction date:** 2026-07-11
