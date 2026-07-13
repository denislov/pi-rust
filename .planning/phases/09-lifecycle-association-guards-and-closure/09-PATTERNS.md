# Phase 9: Lifecycle Association, Guards, and Closure - Pattern Map

**Mapped:** 2026-07-14
**Scope:** Expected production, adapter, guard, test, and contract files identified by `09-CONTEXT.md` and `09-RESEARCH.md`
**Method:** Current on-disk source inspected with CodeGraph first, then targeted line-numbered reads

## File Classification

| New/Modified File | Role | Data Flow | Closest Existing Analog | Match |
|---|---|---|---|---|
| `src/coding_session/snapshot_coordinator.rs` | store/state machine | event-driven, pub-sub | its generation-gated client registry and retained-event state | exact extension |
| `src/coding_session/client_service.rs` | service facade | request-response | its thin delegation to `SnapshotCoordinator` | exact extension |
| `src/coding_session/public_projection.rs` | public model/facade | streaming, request-response | reconnect receiver plus scoped Prompt control | exact extension |
| `src/coding_session/error.rs` | public error model | transform | existing typed variants plus stable `code()` mapping | exact extension |
| `src/coding_session/mod.rs` | runtime owner/controller | event-driven, async dispatch | `SubmissionCommitGuard`, canonical `run`, prompt finalization | exact extension |
| `src/coding_session/operation_control.rs` | admission/control service | request-response | active-operation RAII guard and Prompt control binding | role/data-flow match |
| `src/coding_session/event_service.rs` | event service | pub-sub, streaming | atomic replay/live boundary and commit-then-broadcast `emit` | exact extension |
| `src/coding_session/public_operation.rs` | public operation model | transform | exhaustive 15-case operation/outcome contract table | exact extension |
| `src/coding_session/operation.rs` | internal metadata model | transform | exhaustive `Operation::metadata()` dispatch classification | exact extension |
| `src/coding_session/event.rs` | internal event model | event-driven, transform | `terminal_operation()` root-vs-family classification | exact extension |
| `src/coding_session/public_event.rs` | public event projection | transform, serialization | closed typed event families and explicit Serde projection | exact extension |
| `src/lib.rs` | curated public barrel | transform | existing `api` re-export list | exact extension |
| `src/protocol/types.rs` | wire model | serialization, request-response | tagged `RpcCommand`, `RpcResponse`, typed capability statuses | exact extension |
| `src/protocol/rpc.rs` | transport controller | streaming, request-response | single RPC event loop and EOF drain | exact extension |
| `src/protocol/rpc/state.rs` | adapter state/controller | async owner transfer, request-response | `ensure_client_connection`, moved-owner task result, idempotency ledger | exact extension |
| `src/protocol/rpc/commands.rs` and `wire.rs` | command router/wire utility | request-response, serialization | existing command arms and `write_rpc_response` | exact extension |
| `src/interactive/app.rs`, `loop.rs`, `prompt_task.rs` | UI adapter/controller | streaming, async owner transfer | `PromptTaskResult` always returns the live `CodingAgentSession` owner | exact extension |
| `tests/public_api.rs` plus focused lifecycle/association tests | contract tests | event-driven, streaming | Phase 8 takeover/reconnect/lease tests and snapshot writer ordering guards | exact extension |
| `tests/rpc_mode.rs` | adapter compatibility tests | JSONL streaming | pipe-backed exact JSON assertions and deterministic provider gates | exact extension |
| `tests/product_runtime_boundary_guards.rs` | source guard | file I/O, static transform | `SourceScan`, source sanitizer, recursive Rust walker, fixture matrices | exact extension |
| `tests/api_boundary_guards.rs`, `tests/fixtures/api_boundary/**` | compile guard | subprocess, structured diagnostics | temporary external crate, copied lockfile, offline Cargo check, positive fixture | exact extension |
| `docs/product-event-contract.md` | contract documentation | inventory | existing event-family/variant inventory | exact update if lifecycle event is added |

Paths above are relative to `crates/pi-coding-agent/` unless otherwise stated. Prefer extending these files over creating parallel lifecycle registries, adapter state stores, dispatchers, or event ledgers.

## Pattern Assignments

### Lifecycle authority: `snapshot_coordinator.rs` and `client_service.rs`

**Analog:** `SnapshotCoordinator` is already the sole mutex-protected client, projection, event-retention, and recovery authority (`snapshot_coordinator.rs:98-127`). `ClientService` is deliberately a thin internal facade (`client_service.rs:9-68`).

**State placement pattern** (`snapshot_coordinator.rs:98-120`):

```rust
pub(crate) struct SnapshotState {
    pub(crate) clients: HashMap<ClientConnectionId, ClientRecord>,
    pub(crate) projection: Option<SnapshotProjection>,
    pub(crate) capability_generation: CapabilityGeneration,
    pub(crate) next_event_sequence: u64,
    pub(crate) retained_product_events: VecDeque<ProductEvent>,
    pub(crate) dropped_before: Option<ProductEventSequence>,
    pub(crate) recovery_revision: u64,
}
```

Put runtime lifecycle state, detached-generation state, and lifecycle notification/index data under this authority. Preserve reconnectable client records; detach changes connection validity, not durable client-local contents.

**Generation gate pattern** (`snapshot_coordinator.rs:530-555`):

```rust
fn record<'a>(state: &'a mut SnapshotState, handle: &ClientHandle)
    -> Result<&'a mut ClientRecord, ClientRegistryError>
{
    let record = state.clients.get_mut(&handle.id)
        .ok_or(ClientRegistryError::StaleClient)?;
    if record.generation != handle.generation {
        return Err(ClientRegistryError::StaleClient);
    }
    Ok(record)
}
```

Every lifecycle-sensitive read or mutation should pass through one shared validator that returns distinct detached, stale-generation, shutting-down, and shut-down outcomes. Do not open-code partial checks in adapters.

**Takeover preservation pattern** (`snapshot_coordinator.rs:355-375`): same-id takeover increments only `generation`; it does not replace `ClientRecord`, so acknowledgement, drafts, submitted state, and receipts survive. Detach must preserve this property.

**Transition validation pattern** (`snapshot_coordinator.rs:639-710`): submitted states advance through explicit `Accepted -> Running -> Terminal`; regressions return `SubmittedRegression`. Model lifecycle and typed terminal anchors with similarly exhaustive transition methods, never public field mutation.

**Service pattern** (`client_service.rs:18-68`): add one-line methods that delegate typed inputs/results to the coordinator. Do not give `ClientService` a second `Mutex`, client map, or lifecycle flag.

### Public lifecycle facade: `public_projection.rs`, `error.rs`, and `lib.rs`

**Analog:** `CodingAgentClientConnection` carries only an opaque coordinator/event-service binding plus public client identity/generation/snapshot (`public_projection.rs:264-296`). Its methods derive the private handle internally and map private errors to stable public types.

**Scoped authority pattern** (`public_projection.rs:284-311`):

```rust
fn handle(&self) -> ClientHandle { /* derives id + generation from self */ }

pub fn acknowledge(&self, sequence: u64) -> Result<u64, CodingSessionError> {
    self.coordinator
        .acknowledge(&self.handle(), sequence)
        .map_err(|error| registry_error(&self.client_id, error))
}
```

`detach`, `acknowledge_outcome`, replay, drafts, submission, and control must derive client/generation from `self`. No public mutator should accept an arbitrary client id or generation. Stable lifecycle outcomes/rejections should be public enums, not string parsing.

**Atomic reconnect wrapper pattern** (`public_projection.rs:314-359`; `event_service.rs:198-253`): connection recovery acquires a live receiver and retained replay under the same coordinator lock, then exposes a wrapper carrying the private handle and last sequence. Add detach/shutdown notification selection to this wrapper; keep raw `broadcast::Receiver` private.

**Typed recovery pattern:** retained gap maps to `FreshSnapshotRequired`; live lag is separately projected by the reconnect receiver. Lifecycle closure should follow the same typed projection style, not become a generic resource/input error.

**Stable error-code pattern** (`error.rs:6-70+`): add explicit variants and explicit `code()` match arms. Do not derive wire codes from `Debug` or expose internal generation/receipt/durability details in messages.

**Curated export pattern** (`lib.rs:60-109`): add only stable lifecycle, association, terminal-anchor, and acknowledgement contracts to `api`. Keep `SnapshotCoordinator`, services, raw channels, internal operation metadata, maps, queues, and control transports private.

### Shutdown orchestration: `mod.rs`, `operation_control.rs`, and `event_service.rs`

**Analog:** `CodingAgentSession::run` is the one public dispatcher (`mod.rs:343-357`); `OperationControl` owns active-operation exclusivity; event publication commits coordinator state before broadcasting.

**Canonical dispatcher pattern** (`mod.rs:343-357`): preserve conversion and dispatch through `CodingAgentSession::run`. Shutdown closes admission but must not add another ordinary-operation entry point on the session or connection.

**RAII pattern** (`mod.rs:197-245`): `SubmissionCommitGuard` owns submitted-state cleanup across success/error/drop. Extend its finalization to accept exact terminal evidence, outcome-only evidence, or uncertainty. Keep Drop as fail-closed cleanup, but do not let Drop invent a fake product-event sequence.

**Commit-before-broadcast pattern** (`event_service.rs:278-296`):

```rust
let mut state = self.snapshot_coordinator.state.lock().unwrap();
let sequence = ProductEventSequence::new(state.next_event_sequence);
state.next_event_sequence += 1;
let product_event = ProductEvent::new(/* ... */);
self.retain_product_event(&mut state, product_event.clone());
drop(state);
let _ = self.product_sender.send(product_event.clone());
```

Publish the final lifecycle shutdown event through this path after the admitted operation's root terminal publication, then transition to shut down and wake/close receivers. Never hold the standard mutex across `.await` or channel send.

**Prompt durability/order analog** (`mod.rs:1489-1507`): transaction finalization, session-write events, then root prompt outcome event. Two-phase shutdown tests should assert this existing terminal boundary remains before the lifecycle event and receiver closure.

**Ownership landmine:** `run` takes `&mut self`; RPC/interactive move the owner into a task and recover it in the task result. A concurrent shutdown request needs shared admission/lifecycle state visible to `run`, not a cloned owner or secondary dispatcher.

### Operation association: `public_operation.rs`, `operation.rs`, `event.rs`, and `public_event.rs`

**Analog:** `public_operation.rs:42-104` defines exactly 15 public operations and 15 outcomes; its test table at `public_operation.rs:264-290+` already enforces exhaustive construction, internal mapping, dispatch mode, and outcome family.

Extend that table/descriptor with:

- stable submitted kind for every operation;
- `TerminalAssociated` or `OutcomeOnly` class for all 15 entries;
- optional Prompt draft fingerprint only for textual Prompt;
- expected outcome family and allowed root terminal evidence.

Use exact set/cardinality assertions so a new enum variant cannot compile or pass without classification. `ApproveDelegation` cannot rely on internal `metadata().static_kind`, which is currently optional.

**Root terminal pattern** (`event.rs:1025-1084`): `SessionCompactionCompleted` is a root terminal operation, while `ToolCallCompleted` and `SessionWriteCommitted` may have terminal-like status but return `terminal_operation() == None`. Association must use `terminal_operation()` plus exact operation id/kind, never generic `terminal_status()`.

**Typed event projection pattern:** add a lifecycle family/variant through the existing closed internal-to-public mapping and explicit Serde representation in `public_event.rs`. A runtime shutdown event has no operation id and is not a root operation terminal.

**Exact evidence pattern:** `EventService::emit` returns the sequenced `ProductEvent`; propagate that evidence or record a coordinator index keyed by operation id at emission time. Do not scan bounded retained history as authority.

### Submitted terminal anchors and PartialCommit: `snapshot_coordinator.rs`, `public_projection.rs`, and `mod.rs`

**Analog to replace:** current `TerminalAcknowledgementAnchor` contains only `terminal_sequence` (`snapshot_coordinator.rs:23-27`), and `acknowledge` clears terminal submitted state once the event cursor passes it (`snapshot_coordinator.rs:582-600`).

Follow the existing typed-enum state model, but make the anchor exhaustive:

- product-event anchor: exact sequence plus durability certainty;
- outcome-only anchor: stable outcome acknowledgement identity;
- terminal-uncertain anchor: original operation id plus typed recovery state.

Add a separate generation-scoped outcome acknowledgement method. Event acknowledgement must not clear outcome-only or uncertain state.

**Critical landmine** (`mod.rs:224-232`): `SubmissionCommitGuard::finish` currently passes `current_event_sequence()`. That sequence may belong to an unrelated progress/family event and is invalid for OutcomeOnly. Remove this guessing pattern completely.

**PartialCommit analog:** `session_log/transaction.rs:434-455` marks an append-success/manifest-failure transaction `InDoubt` and returns `CodingSessionError::PartialCommit` with the original operation id. Preserve that id and whether exact terminal event evidence exists; never retry under the same id to manufacture another terminal event.

### RPC lifecycle projection: `protocol/types.rs`, `rpc.rs`, and `rpc/*`

**Wire model analog:** `RpcCommand` uses a tagged Serde enum (`types.rs:444+`); response/status types use explicit names and stable status tags (`types.rs:695-702`). Add independent detach/shutdown commands and typed response/status payloads. Do not append lifecycle fields to existing prompt/state/replay/control payloads.

**Shared cleanup analog to improve:** `RpcState::clear_client_state` currently only drops `client_connection` (`rpc/state.rs:149-151`). Replace it with one idempotent cleanup method that calls public detach; both explicit detach and transport exit use it.

**Loop cleanup pattern** (`rpc.rs:20-108`): all EOF paths converge through the main loop. Ensure cleanup runs after every normal/error loop exit, including after an admitted operation is drained, rather than only one match arm.

**Moved-owner pattern** (`rpc/state.rs:59-80`): `CodingOperationTaskResult` returns the `CodingAgentSession` owner with the outcome. Runtime-owner shutdown occurs only after this owner is restored; connection detach can use shared public connection authority while the operation continues.

**Compatibility test analog:** `rpc_mode.rs:2531-2642` uses exact JSON field assertions for state/hello and stable error codes. Add full snapshots/values for existing response shapes before and after lifecycle commands, plus dedicated new lifecycle responses.

### Interactive lifecycle projection: `interactive/app.rs`, `loop.rs`, and `prompt_task.rs`

**Owner restoration pattern** (`prompt_task.rs:27-109`): every success/failure task result carries `CodingAgentSession` back to the loop. Preserve this invariant; normal UI exit detaches its stable connection, while only `run_interactive_mode`/the explicit top-level owner calls shutdown once the owner is restored.

**Boundary pattern** (`interactive/app.rs:63-84`): `run_interactive_mode` is the process-facing owner boundary; `run_interactive_loop_with_input` is the client/UI boundary. Place shutdown at the former and detach at the latter. A loop exit must not shut down a shared embedded runtime.

Keep lifecycle projection separate from transcript workflow projection. Old handles return typed lifecycle state; adapters must not silently reconnect, retarget Prompt control, or hide mutation rejection.

### Adapter discovery guard: `product_runtime_boundary_guards.rs`

**Reusable machinery:** existing `SourceScan`, `sanitize_rust_source`, `production_lines`, `rust_files_under`, and multiline/receiver-aware fixture helpers. Continue stripping comments, strings, and test-only code before structural scanning.

**Analog to replace** (`product_runtime_boundary_guards.rs:1455-1547`): `FIRST_PARTY_ADAPTERS` recursively walks three known roots, but cannot discover a new sibling entrypoint. Implement:

1. candidate discovery over production `src/**/*.rs` using structural signals (mode/transport entrypoint, session ownership, operation construction, public event projection, connection/replay/control, wire/output boundary);
2. explicit classification ledger with `CanonicalOperationCaller`, `StateReplayControlConsumer`, or `ApprovedNonRuntimeAdapter` and non-empty rationale;
3. exact set equality between candidates and ledger;
4. existing prohibited-call receiver-aware scan over classified files.

**Fixture analog** (`product_runtime_boundary_guards.rs:1549-1569`): use inline source matrices containing comments, strings, `#[cfg(test)]`, multiline calls, parenthesized receivers, legitimate non-session receivers, positive candidates, and near misses. Assert both discovery and classification failures.

### Diagnostic-bound compile fixtures: `api_boundary_guards.rs` and fixture tree

**Harness to retain** (`api_boundary_guards.rs:81-153`): create a temporary external crate, point it at local `pi-coding-agent`, copy workspace `Cargo.lock`, reuse one target directory, and run Cargo offline. Keep an adjacent stable-facade positive fixture.

**Harness to strengthen:** current `CompileFixture` has only category/path/source and accepts any E0432/E0603 (`api_boundary_guards.rs:11-16,122-140`). Extend each case with expected source line/span, forbidden symbol/path, accepted error code(s), and required diagnostic fragments.

Run `cargo check --offline --message-format=json`, deserialize compiler-message JSON with existing `serde_json`, and assert:

- primary error target is generated `src/main.rs`;
- primary span intersects the declared forbidden use;
- error code is the expected E0432/E0603;
- rendered diagnostic identifies the forbidden symbol/path;
- no earlier unrelated compiler error occurred;
- the adjacent stable alternative compiles.

Do not add `trybuild` and do not accept raw nonzero exit status or whole-stderr substring matching as proof.

### Behavior tests and deterministic synchronization

**Public contract analogs:** `public_api.rs:488-552` tests reconnect acknowledgement and RAII lease release through only `pi_coding_agent::api`; `public_api.rs:554+` tests canonical run clearing the draft and producing terminal submitted state. Extend these scenarios for every lifecycle rejection category and anchor class.

**Ordering/source guard analogs:** `public_api.rs:697-770` asserts lock release and ordering using focused source checks; preserve these and add behavioral ordering tests for shutdown. Source checks alone do not prove runtime drain behavior.

**Event-count analog:** `public_api.rs:1428-1485` drains emitted events and counts/matches exact typed variants. Use this style to assert exactly one matching root terminal event and zero root terminals for OutcomeOnly operations.

**Synchronization pattern:** use Tokio `oneshot`, `Notify`, or barriers/faux provider queues. Existing event-service concurrency tests use `Barrier`; RPC prompt tests gate completion with a notification. Do not use sleeps for detach-during-operation, blocked receiver wake-up, or shutdown drain ordering.

## Shared Patterns

### Locking and publication

- Mutate one authority under `SnapshotCoordinator.state`.
- Clone/return owned projection values while locked.
- Drop `std::sync::MutexGuard` before broadcast, callback, or `.await`.
- Establish replay plus live receiver atomically under the same lock.
- Wake blocked receivers explicitly on detach/shutdown; dropping one connection clone is insufficient.

### Authority and error handling

- Public connection methods derive private authority from the connection itself.
- Internal coordinator errors map to explicit public typed rejections/codes at the facade.
- Detached, stale generation, shutting down, and shut down are different states.
- Ordinary execution remains exclusively `CodingAgentSession::run`.
- Prompt control remains client + generation + submitted operation scoped.

### Closed ledgers

- Use exhaustive match/table coverage for all 15 operation variants and outcomes.
- Use exact set equality for adapter discovery/classification and compile-fixture categories.
- Use `ProductEvent::terminal_operation`, not generic terminal status, for root association.
- Update the documented event inventory from 45 to 46 only if the lifecycle event is actually added.

### Serialization compatibility

- Add independent lifecycle commands/responses/events.
- Preserve existing JSON keys, omission rules, error codes, and event ordering.
- Snapshot new public values to ensure generations, receipt signatures, retained capacities, internal recovery markers, and durability implementation strings are absent.

## Landmines

1. `current_event_sequence()` is not terminal evidence; it can point at unrelated progress, tool, session-write, or family-terminal events.
2. Detach is not takeover: it must invalidate the current generation and wake its receiver without deleting reconnectable client state or incrementing generation by itself.
3. A raw broadcast receiver blocked in `recv()` will not observe client detach unless the wrapper selects on lifecycle notification.
4. Clearing Prompt control on detach would prevent the new generation from controlling an operation that correctly continues running.
5. Holding the coordinator mutex across shutdown drain or broadcast can deadlock the runtime.
6. `CodingAgentSession` cannot be cloned around the `&mut self` owner constraint; use shared admission state and restore the owner from adapter task results.
7. OutcomeOnly operations must not receive fake product events/sequences; event ack and outcome ack are separate contracts.
8. `PartialCommit` retains the original operation id and uncertainty; retrying to create a second terminal event violates exactly-one association.
9. `terminal_status()` alone promotes tool/session-write/delegation family completion incorrectly; root association requires `terminal_operation()`.
10. A fixed adapter-root allowlist does not detect a new adapter sibling; discovery and classification must be separate exact sets.
11. Any E0432/E0603 is not sufficient compile-fixture evidence; bind code, symbol, target file, and primary span.
12. Adding lifecycle fields to old RPC state/control responses breaks the locked wire-shape decision even if Serde consumers ignore unknown fields.
13. Interactive loop exit is a client detach boundary; top-level runtime owner exit is the shutdown boundary.
14. Do not weaken or replace existing deterministic assertions, compatibility snapshots, or workspace gates with compile-only checks.

## Planner Handoff

The safest plan decomposition follows authority dependencies: public types/closed ledgers, coordinator lifecycle/receiver wake-up, generic submission and exact association, two-phase shutdown, adapters, then guards/security/docs/full verification. Runtime authority files overlap heavily, so do not schedule `snapshot_coordinator.rs`, `public_projection.rs`, or `mod.rs` in parallel plans. RPC and interactive projection can run in parallel only after the public lifecycle contract and shutdown ownership API are stable.
