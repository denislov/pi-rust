# Phase 9: Lifecycle Association, Guards, and Closure - Research

**Researched:** 2026-07-14  
**Domain:** Rust/Tokio session lifecycle, operation/event correlation, adapter boundary enforcement  
**Confidence:** HIGH for repository architecture, behavior, and recommended implementation order

## User Constraints

### Locked Decisions

### Detach, Close, and Shutdown

- **D-01:** Detach ends the current connection generation and its live receiver while preserving acknowledgement cursor, drafts, submitted terminal state, and accepted control receipts for a later same-id reconnect.
- **D-02:** An operation belongs to the session runtime, not to the submitting connection lifetime. Detach never cancels an active Prompt or other canonical operation. The stale generation loses state/control authority immediately; reconnect restores observation and a new Prompt-scoped control handle when applicable.
- **D-03:** Detach is explicitly idempotent and returns a typed outcome that distinguishes `Detached`, `AlreadyDetached`, and `StaleGeneration`. Callers must not parse error strings to determine lifecycle state.
- **D-04:** Shutdown first closes admission and control, marks connection generations detached or shutting down, waits for the active operation to finish and publish/commit its terminal event, then publishes a final lifecycle shutdown event and closes live receivers. Repeated shutdown returns typed `AlreadyShutDown`.

### Operation and Event Association

- **D-05:** Maintain one fail-closed matrix classifying every public operation as `TerminalAssociated`, `OutcomeOnly`, or `NotApplicable`. A new, removed, renamed, duplicated, or unclassified operation fails the guard.
- **D-06:** Each admitted `TerminalAssociated` operation id has exactly one root terminal event. It may have any number of progress/tool/message events, but the root terminal event cannot be missing or duplicated and must be published/committed before the canonical outcome returns.
- **D-07:** Success, failure, and cancellation use the same operation-id and exactly-one-terminal-event rule for `TerminalAssociated` operations.
- **D-08:** `PartialCommit` preserves the original operation id. If a terminal event exists, submitted state retains its exact sequence plus explicit durability uncertainty. If no terminal event was established, record a `TerminalUncertain` recovery marker and never fabricate a second terminal event during retry or recovery.
- **D-09:** An `OutcomeOnly` operation terminates through its typed canonical outcome and submitted terminal state under the same operation id. Its terminal anchor is explicitly `OutcomeOnly` and clears through outcome acknowledgement rather than a nonexistent event sequence.

### Boundary Guards and Verification

- **D-10:** Recursively discover all first-party production adapter roots and entrypoints. Each must be explicitly classified as a canonical operation caller, state/replay/control consumer, or approved non-runtime adapter. Any unclassified new entrypoint fails closed.
- **D-11:** Every external negative compile fixture binds failure to the expected file/span, error code or forbidden symbol, and diagnostic fragment. An adjacent positive fixture proves the intended public API still compiles. Unrelated syntax, dependency, or privacy failures cannot satisfy the guard.
- **D-12:** Phase and milestone completion require every layer to pass: focused lifecycle/association/guard tests, `cargo fmt --all --check`, `cargo test -p pi-coding-agent`, `cargo test --workspace`, `cargo check --workspace`, security/source audits, positive/negative compile fixtures, and `git diff --check`. No layer is advisory.
- **D-13:** Security/source audits cover the entire public authority boundary: no raw sender/receiver, internal coordinator/service, queue/map, cross-client or cross-generation mutation, second ordinary-operation dispatcher, unvalidated Prompt control, or internal session/receipt/durability detail leakage through debug, Serde, or error text.
- **D-14:** After detach or shutdown, every mutation path fails closed with typed lifecycle state. Tests must independently prove state, acknowledgement, draft, submission, replay, and Prompt-control rejection.

### RPC and Interactive Projection

- **D-15:** RPC exposes an explicit detach command with a typed lifecycle outcome. EOF, transport closure, and RPC-loop exit invoke the same idempotent detach API as cleanup; normal and abnormal exits do not maintain separate lifecycle logic.
- **D-16:** Add independent typed detach/shutdown responses and lifecycle events with stable status codes. Existing prompt, state, replay, control, JSON/print responses, fields, and error codes remain byte-for-byte compatible.
- **D-17:** Interactive lifecycle behavior depends on ownership. A normal UI/client exit detaches only. A top-level process calls shutdown only when it explicitly owns the runtime and is performing final process exit.
- **D-18:** Use typed `Detached`, `StaleGeneration`, or `RuntimeShutDown` rejection after lifecycle termination. RPC maps stable lifecycle codes and Interactive applies one explicit lifecycle transition. Old receivers close; adapters never auto-reconnect, silently ignore a mutation, retry it implicitly, or retarget a Prompt control handle.

### the agent's Discretion

- Exact public Rust type and method names may follow existing `CodingAgent*` naming and facade conventions, provided the typed distinctions above remain exhaustive and stable.
- The internal two-phase locking/drain implementation and test fixture organization are left to research and planning, subject to the existing no-lock-across-await and single-`SnapshotCoordinator` authority rules.
- The exact membership of `TerminalAssociated` versus `OutcomeOnly` must be derived from the live 15-operation inventory and current observable behavior, not guessed from names or widened merely for symmetry.

### Deferred Ideas (OUT OF SCOPE)

None - discussion stayed within phase scope. A separately named runtime owner and multi-session daemon routing remain the already-recorded future requirements.

## Summary

Phase 9 should extend the existing session-owned authority rather than add a lifecycle manager beside it. `SnapshotCoordinator` already owns the only client registry, generation validation, acknowledgement, drafts, submitted state, retained cursor facts, and Prompt-control binding; `EventService` already serializes sequence allocation, retention, and broadcast. Lifecycle state therefore belongs in `SnapshotState`/`ClientRecord`, while receiver wake-up belongs in a coordinator-owned notification primitive wrapped by the public typed receiver. [VERIFIED: `crates/pi-coding-agent/src/coding_session/snapshot_coordinator.rs:63-141`; `event_service.rs:123-135,198-254,278-296`]

Two live gaps must drive the first implementation wave. First, `CodingAgentReconnectReceiver::recv()` currently waits only on a broadcast receiver, so marking a generation stale cannot wake a blocked receiver; detach/shutdown needs a lifecycle notification branch and every delivery must revalidate lifecycle state before projection. Second, client submission provenance is Prompt-only: `submission_fingerprint()` returns `None` for 14 variants and `prepare_submission()` rejects them, while the locked Phase 9 contract requires OutcomeOnly submitted terminal state. The submission lease must become operation-generic, with Prompt draft validation/clearing retained as a Prompt-specific admission rule. [VERIFIED: `public_projection.rs:406-428,431-483`; `public_operation.rs:106-117`; `mod.rs:343-357`]

The correct association baseline is conservative: the five operation kinds already represented by distinct root terminal events remain `TerminalAssociated`; the other ten public operations become `OutcomeOnly`. `NotApplicable` is intentionally empty for the current 15-variant `CodingAgentOperation` enum and exists as a closed taxonomy for future public values that are not admitted operations; Abort/Steer/FollowUp are outside the enum and must remain outside `run`. BranchSummary, PluginLoad, and ApproveDelegation currently emit related events but lack a distinct root terminal association, so they are OutcomeOnly rather than being promoted by symmetry. [VERIFIED: `docs/product-event-contract.md:124-153`; `event.rs:86-114`; `public_operation.rs:41-104`]

**Primary recommendation:** implement one coordinator-owned lifecycle state machine and one closed association ledger, make canonical `run` finalize submitted state from the actual outcome/terminal event evidence, then migrate adapter teardown and close guards in later waves.

## Project Constraints (from AGENTS.md)

- Use Chinese for user communication; technical documents may be fully English. [VERIFIED: `AGENTS.md`]
- Because `.codegraph/` exists, use `codegraph explore` before grep/find/direct source reads for code discovery. This research did so for lifecycle, operation/outcome, terminal events, adapters, and guards. [VERIFIED: `AGENTS.md`; `.codegraph/`]
- Preserve dependency direction `pi-coding-agent -> pi-agent-core -> pi-ai` and `pi-coding-agent -> pi-tui`; lifecycle and product semantics remain in `pi-coding-agent`. [VERIFIED: `AGENTS.md` Project constraints]
- Export new stable contracts only through `pi_coding_agent::api`; internal operations, metadata, services, queues, plugin options, and Flow nodes remain private. [VERIFIED: `AGENTS.md` Project constraints and conventions]
- Preserve JSON/print/RPC/interactive output, ordering, controls, replay, navigation, durability, recovery markers, and explicit `PartialCommit`. [VERIFIED: `AGENTS.md` Project constraints]
- Use deterministic offline fixtures and retain behavioral assertions; compile-only coverage cannot replace behavior coverage. [VERIFIED: `AGENTS.md` Project constraints]
- Required completion gates include formatting, focused crate tests, workspace test/check, source audits, and diff check. [VERIFIED: `AGENTS.md`; `09-CONTEXT.md` D-12]
- Stage 10 event compatibility expansion and unrelated reliability/security/performance work remain outside scope. [VERIFIED: `AGENTS.md` Project constraints]

No project-local `.codex/skills/` or `.agents/skills/` directory exists, so there are no additional project skill rules to apply. [VERIFIED: repository filesystem inspection]

## Standard Stack

### Core

| Component | Use | Reason |
|---|---|---|
| Rust 2024 enums and exhaustive `match` | Public lifecycle outcomes/rejections, terminal anchors, association classification | Existing stable contracts use typed enums and source guards can enforce closed set equality. [VERIFIED: `public_projection.rs`; `public_operation.rs`; `event.rs`] |
| `Arc<SnapshotCoordinator>` + `std::sync::Mutex` | Sole runtime/client lifecycle authority | All snapshot-visible client facts already live under one state mutex; splitting authority would violate Phase 8 atomicity. [VERIFIED: `snapshot_coordinator.rs:98-141`; `08-VERIFICATION.md`] |
| Tokio bounded broadcast plus lifecycle notification | Product delivery and detach/shutdown wake-up | Existing event transport is bounded broadcast. Add notification only for termination selection; do not duplicate retained events or business delivery. [VERIFIED: `event_service.rs:123-135`; `public_projection.rs:446-483`] |
| Existing `CodingSessionError` and stable `code()` | Session-level typed failures | Existing adapters already map stable codes; lifecycle outcomes that are not failures should remain result enums, while post-termination mutations use typed rejection/error variants. [VERIFIED: `coding_session/error.rs:1-81`; `09-CONTEXT.md` D-03/D-18] |
| Existing Cargo test harness, `tempfile`, offline Cargo fixture compilation | Behavior, source guards, external facade tests | The repository already builds a temporary external consumer with the workspace lockfile and `--offline`. [VERIFIED: `tests/api_boundary_guards.rs:81-153`] |

### New Dependencies

None. The workspace already has Tokio and `tokio-util`; the phase can implement lifecycle notification with existing synchronization dependencies. Do not alter `Cargo.toml` or `Cargo.lock`. [VERIFIED: workspace manifests and existing Phase 8 stack]

## Exact Operation Association Matrix

The planner should create one executable source-of-truth matrix and make documentation/tests derive or validate against it. The current 15 public variants and 15 outcomes are exhaustively listed in `public_operation.rs`. [VERIFIED: `public_operation.rs:41-104`]

| Public operation | Outcome | Phase 9 class | Evidence and required anchor |
|---|---|---|---|
| `Prompt` | `Prompt` | `TerminalAssociated` | Existing `PromptCompleted/Failed/Aborted -> Prompt`; exact event sequence anchor. [VERIFIED: `event.rs:89-93`] |
| `Compact` | `Compact` | `TerminalAssociated` | Existing `Session.CompactionCompleted -> Compact`; exact event sequence anchor. Failure/cancellation paths need explicit exactly-one closure tests because current mapping only has completed. [VERIFIED: `event.rs:108-110`; `docs/product-event-contract.md:136`] |
| `BranchSummary` | `BranchSummary` | `OutcomeOnly` | Uses Prompt workflow events but has no distinct BranchSummary root association; outcome acknowledgement anchor. [VERIFIED: `docs/product-event-contract.md:137`] |
| `SelfHealingEdit` | `SelfHealingEdit` | `TerminalAssociated` | Existing completed/failed mapping; exact event sequence anchor. [VERIFIED: `event.rs:104-107`] |
| `InvokeAgent` | `AgentInvocation` | `TerminalAssociated` | Existing completed/failed/aborted mapping; exact event sequence anchor. [VERIFIED: `event.rs:94-98`] |
| `InvokeTeam` | `AgentTeam` | `TerminalAssociated` | Existing completed/failed/aborted mapping; exact event sequence anchor. [VERIFIED: `event.rs:99-103`] |
| `PluginLoad` | `PluginLoad` | `OutcomeOnly` | Diagnostics/capability events are not plugin root terminal events; outcome acknowledgement anchor. [VERIFIED: `docs/product-event-contract.md:141`] |
| `PluginCommand` | `PluginCommand` | `OutcomeOnly` | Synchronous/eventless; outcome acknowledgement anchor. [VERIFIED: `docs/product-event-contract.md:142`] |
| `SetDefaultAgentProfile` | `DefaultAgentProfileChanged` | `OutcomeOnly` | Profile event is metadata-only and has no operation id; outcome acknowledgement anchor. [VERIFIED: `docs/product-event-contract.md:143`; `event_service.rs:328-332`] |
| `ApproveDelegation` | `DelegationApproved` | `OutcomeOnly` | Delegation events are event-terminal but not root-operation-associated; outcome acknowledgement anchor. [VERIFIED: `docs/product-event-contract.md:144`] |
| `RejectDelegation` | `DelegationRejected` | `OutcomeOnly` | Rejection event is not a root terminal association; outcome acknowledgement anchor. [VERIFIED: `docs/product-event-contract.md:145`] |
| `ForkSession` | `SessionForked` | `OutcomeOnly` | Typed navigation outcome without root terminal event; outcome acknowledgement anchor. [VERIFIED: `docs/product-event-contract.md:146`] |
| `SwitchActiveLeaf` | `ActiveLeafSwitched` | `OutcomeOnly` | Typed navigation outcome without root terminal event; outcome acknowledgement anchor. [VERIFIED: `docs/product-event-contract.md:147`] |
| `ExportCurrent` | `Export` | `OutcomeOnly` | Eventless read outcome; outcome acknowledgement anchor. [VERIFIED: `docs/product-event-contract.md:148`] |
| `ExportCurrentHtml` | `ExportHtml` | `OutcomeOnly` | Eventless path outcome; outcome acknowledgement anchor. [VERIFIED: `docs/product-event-contract.md:149`] |

`NotApplicable` has zero current `CodingAgentOperation` rows. Keep the enum variant and assert that every current operation maps exactly once; controls are independently guarded as non-operation public APIs and should not be inserted as fake matrix rows. [VERIFIED: `public_operation.rs:41-83`; `09-CONTEXT.md` phase boundary]

## Architecture Patterns

### Pattern 1: One Lifecycle State Machine Under Snapshot Authority

Add runtime lifecycle to `SnapshotState` (for example `Running`, `ShuttingDown`, `ShutDown`) and connection lifecycle to `ClientRecord` (for example current generation attached/detached plus the last detached generation). Do not delete the client record on detach because cursor, drafts, submitted state, and receipts must survive. [VERIFIED: `snapshot_coordinator.rs:63-119`; locked D-01]

Recommended semantics:

1. `connect` rejects `ShuttingDown`/`ShutDown`; otherwise it advances generation and marks that generation attached.
2. `detach(handle)` under the coordinator lock returns `Detached` for the current attached generation, `AlreadyDetached` for the same generation already detached, and `StaleGeneration` when a newer generation owns the id.
3. Every state/replay/ack/draft/submission/control method validates both generation and attached runtime state, returning distinct typed lifecycle rejection rather than collapsing everything to `StaleClientConnection`.
4. Detach clears only the Prompt control authority for that generation; it does not clear receipts, drafts, acknowledgement, submitted state, or the operation's internal channel. A same-id reconnect may rebind a new-generation control handle while the Prompt is active. [VERIFIED: locked D-01 to D-03; existing binding at `snapshot_coordinator.rs:328-356`]

Store enough state to distinguish same-generation `AlreadyDetached` from older-generation `StaleGeneration`; a boolean without generation history cannot implement D-03. [VERIFIED: current `ClientRecord` stores only current generation at `snapshot_coordinator.rs:63-85`]

### Pattern 2: Detach-Aware Receiver Wrapper

Keep `ProductEventReceiver` private and bounded. Extend the public reconnect receiver with a lifecycle notifier/epoch tied to its client handle. Its `recv()` should select event delivery against lifecycle change, and its `try_recv()` should validate lifecycle before and after reading. This closes both blocked-wait and event-after-detach races without exposing a sender or raw receiver. [VERIFIED: current receiver only awaits `inner.recv()` at `public_projection.rs:446-460`; locked D-18]

The ordering rule should be: coordinator transition first, notify after releasing the standard mutex, and receiver validates the handle before returning any event. This follows the existing commit-under-lock then broadcast-after-unlock pattern. [VERIFIED: `event_service.rs:278-296`; `08-CONTEXT.md` established patterns]

### Pattern 3: Two-Phase Shutdown and the `&mut` Owner Constraint

Shutdown must be implemented as a phase transition, not as `Drop`. Phase A atomically changes runtime state to `ShuttingDown`, rejects new connect/submission/control/mutations, detaches generations, and prevents new operation admission. Phase B waits for the admitted operation to reach its existing canonical terminal publication/commit boundary, emits one lifecycle shutdown event through `EventService`, sets `ShutDown`, then wakes/closes receivers. [VERIFIED: locked D-04; canonical terminal publication in `mod.rs:1489-1507`]

`CodingAgentSession::run` takes `&mut self`, and adapters move the owner into a background task and receive it back in the task result. Therefore top-level shutdown normally occurs after that owner returns, while a concurrently initiated shutdown signal needs shared admission/lifecycle state that `run` checks at admission boundaries. Do not create a second owner or ordinary dispatcher to work around Rust ownership. [VERIFIED: `mod.rs:343-357`; `protocol/rpc/state.rs:59-80`; interactive owner-restoration tests]

The planner should explicitly test both cases: shutdown while idle, and shutdown requested while an operation is already admitted using a deterministic gate/oneshot fixture. The latter proves admission closes immediately while the admitted operation still publishes its terminal event before the lifecycle shutdown event. [VERIFIED: locked D-04; repository deterministic channel pattern]

### Pattern 4: Generic Submission Provenance With Prompt-Specific Draft Rules

Replace Prompt-only `submission_fingerprint()` with exhaustive public operation classification (`kind`, association class, and optional Prompt draft fingerprint). Let `prepare_submission` accept every public operation; only Prompt requires a matching Prompt draft id/text and clears that draft after admission. This preserves the connection's lack of dispatch authority: preparation still installs an RAII lease, and execution still occurs only through `CodingAgentSession::run`. [VERIFIED: `public_operation.rs:106-168`; `public_projection.rs:406-428`; `mod.rs:343-380`]

All 15 operations need an admission operation id. Internal `Operation::metadata()` currently has `static_kind: None` for ApproveDelegation, so the matrix/descriptor must provide a public submitted kind without relying on an optional internal static kind. [VERIFIED: `operation.rs:98-103`; `public_operation.rs:63-66`]

### Pattern 5: Typed Terminal Anchors

Replace `TerminalAcknowledgementAnchor { terminal_sequence }` with an exhaustive anchor, conceptually:

```rust
enum SubmittedTerminalAnchor {
    ProductEvent {
        sequence: u64,
        durability: SubmittedDurability,
    },
    OutcomeOnly {
        acknowledgement: OutcomeAcknowledgementId,
    },
    TerminalUncertain {
        operation_id: String,
        recovery: TerminalUncertainty,
    },
}
```

Exact names are discretionary. The important constraints are that event acknowledgement cannot clear OutcomeOnly state, outcome acknowledgement cannot clear event-anchored state, and uncertainty remains explicit. Current `acknowledge(sequence)` clears every terminal state by comparing the current event cursor, which would incorrectly clear an OutcomeOnly operation if represented with a fake sequence. [VERIFIED: `snapshot_coordinator.rs:582-600,684-710`; locked D-08/D-09]

Add a separate typed `acknowledge_outcome(operation_id or stable acknowledgement id)` connection method. It must be generation/client scoped and only clear a matching OutcomeOnly terminal anchor. [VERIFIED: locked D-09/D-14]

### Pattern 6: Association Is Finalized From Evidence, Not `current_event_sequence()` Guessing

`SubmissionCommitGuard::finish` currently records the coordinator's current event sequence regardless of whether that sequence is the admitted operation's root terminal event. This is unsafe: unrelated progress or family-terminal events can occupy that sequence, and OutcomeOnly operations have no terminal event. [VERIFIED: `mod.rs:224-232`; event-level versus root-terminal distinction in `event.rs:1044-1084`]

For `TerminalAssociated`, capture the exact `ProductEvent` returned by the root terminal emission or query a coordinator-maintained terminal association index keyed by operation id. Validate kind/id/status and reject missing/duplicate associations. For OutcomeOnly, finalize directly from the typed `CodingAgentOperationOutcome`/error with the outcome anchor. Do not scan retained history as the primary runtime authority because retention can evict events. [VERIFIED: `event_service.rs:278-296`; bounded retention at `event_service.rs:256-276`]

### Pattern 7: PartialCommit Is a Terminal-Correlation State, Not a Retry Policy

Current persistence paths already retain the original operation id in `CodingSessionError::PartialCommit`, including manifest-update failure after append. Preserve that id through the submitted record. [VERIFIED: `session_service.rs:359-395`; `coding_session/error.rs:26-31`]

The implementation must distinguish:

- root terminal event already emitted: keep its exact sequence and set durability uncertainty on that anchor;
- durable write partially committed before root terminal emission: record `TerminalUncertain` under the original id;
- OutcomeOnly PartialCommit (for example navigation): retain an uncertain outcome anchor rather than inventing a product event.

Retry must not generate a replacement terminal event under the same operation id. Tests should inject append/manifest faults at existing test-only seams and assert the count of matching root terminal events remains zero or one as appropriate. [VERIFIED: existing fault seams in `session_service.rs:811-816`; locked D-08]

### Pattern 8: Additive Lifecycle Events and Wire Types

Add a typed lifecycle product-event family/variant only for the final runtime shutdown transition, with no operation id and no root-operation terminal association. Preserve all existing 45 event variants and serialized fields; update the executable/document inventory to 46 only when the new event exists. [VERIFIED: current 45-row inventory in `06-RESEARCH.md:153-171`; locked D-04/D-16]

RPC should add dedicated detach/shutdown commands and independent response types/status codes. Existing commands must serialize identically. EOF and all loop-return paths should call one `RpcState` cleanup method that invokes the public idempotent detach path; explicit detach uses that same method and returns the outcome. [VERIFIED: RPC loop exits at `protocol/rpc.rs:33-108`; current `clear_client_state()` merely drops the handle at `protocol/rpc/state.rs:149-151`]

Interactive should establish a stable client connection for the UI owner, detach it on normal loop exit, and let only `run_interactive_mode`/top-level process ownership invoke shutdown after the live owner is restored. Keep lifecycle state transition separate from transcript/product workflow projection. [VERIFIED: `interactive/app.rs:63-84`; `interactive/loop.rs:110-146`; locked D-17]

## Guard Architecture

### Adapter Discovery

The current guard recursively scans only three explicitly listed roots: `interactive`, `protocol`, and `print_mode.rs`. That proves recursion within known roots but does not discover a new sibling adapter under `src/`. [VERIFIED: `product_runtime_boundary_guards.rs:1455-1547`]

Implement two ledgers:

1. an automatic candidate discovery pass over production `src/**/*.rs` using structural signals such as public mode entrypoints, `CodingAgentSession` ownership, `CodingAgentOperation` construction, product-event projection, connection/replay/control use, and CLI output/wire boundaries;
2. an explicit classification table mapping every candidate entrypoint/file to `CanonicalOperationCaller`, `StateReplayControlConsumer`, or `ApprovedNonRuntimeAdapter` with a non-empty rationale.

Set equality between discovered and classified paths makes both an unclassified new adapter and a stale deleted entry fail. Keep receiver-aware call scanning after classification. [VERIFIED: locked D-10; existing source sanitizer/recursive walker in `product_runtime_boundary_guards.rs`]

Do not parse Rust with raw substring matching where the existing sanitizer cannot distinguish receiver identity. Reuse the structural scanner's multiline/function-body helpers and add fixture matrices for new discovery signals. [VERIFIED: existing sanitizer fixture at `product_runtime_boundary_guards.rs:1550-1569`]

### Diagnostic-Bound Compile Fixtures

The current external fixture harness correctly uses a temporary crate, copied lockfile, `cargo check --offline`, and a positive facade fixture, but each negative fixture accepts any E0432/E0603 diagnostic. It does not bind the error to its intended symbol or source span. [VERIFIED: `api_boundary_guards.rs:81-153`]

Extend `CompileFixture` with expected source line, forbidden symbol, accepted error code(s), and required diagnostic fragments. Parse rustc stderr into diagnostic blocks or use Cargo JSON messages (`--message-format=json`) and deserialize compiler-message spans. Assert:

- failing target is the generated `src/main.rs`;
- primary span line/column intersects the declared forbidden use;
- error code is the declared E0432/E0603 contract;
- rendered diagnostic names the forbidden symbol/path;
- no earlier unrelated compiler error exists;
- the adjacent positive fixture importing the intended stable alternative compiles.

Use structured Cargo JSON rather than brittle whole-stderr substring slicing; `serde_json` is already present. Do not add `trybuild` or another dependency. [VERIFIED: existing Serde stack and fixture harness]

### Security and Source Audits

Create blocking source tests that inspect the curated `api` region and public lifecycle type definitions for:

- no `Sender`, raw `broadcast::Receiver`, `SnapshotCoordinator`, `ClientService`, `EventService`, `OperationControl`, internal `Operation`, `HashMap`, `VecDeque`, or service/queue accessor exposure;
- no public connection method accepting arbitrary client id/generation/operation id to mutate another authority;
- no `run`/dispatch method on `CodingAgentClientConnection`;
- all Prompt control construction bound to the connection handle and submitted Prompt id;
- stable lifecycle error/status code mapping without Debug-derived codes;
- Serde payload snapshots exclude internal generations, receipt signatures, retained capacities, recovery internals, and durability implementation strings;
- every mutation/replay/ack/control/submission path rejects detached, stale, shutting-down, and shut-down state explicitly. [VERIFIED: locked D-13/D-14; existing facade guards]

Security review should cover ASVS-style input validation/error handling, access control, data protection, and communication/resource boundaries at repository ASVS Level 1 depth. No authentication, secret storage, network protocol, cryptography, browser, or database schema is introduced by this phase; those categories are not applicable. [VERIFIED: phase scope and existing `07-SECURITY.md` audit method]

## Test and Verification Architecture

### Focused Lifecycle Tests

- first detach returns `Detached`; repeat on same handle returns `AlreadyDetached`; old handle after takeover returns `StaleGeneration`;
- detach preserves cursor, all three draft kinds, terminal submitted state, and accepted receipts across same-id reconnect;
- active Prompt continues through detach and publishes its terminal event; old control rejects, new generation can obtain a Prompt control handle while active;
- every post-detach mutation category independently returns typed lifecycle rejection;
- shutdown idle ordering and active-operation drain ordering: admission/control close before completion, root terminal event before lifecycle shutdown event, receivers close after it;
- repeated shutdown returns `AlreadyShutDown`; connect and all mutation/replay/control paths return `RuntimeShutDown` afterward;
- blocked `recv()` wakes deterministically on detach/shutdown and never yields a later business event.

Use oneshot/barrier gates and faux provider queues, not sleeps. [VERIFIED: project testing convention; locked D-12/D-14]

### Association Tests

- source guard parses all 15 operation and 15 outcome variants and compares exact set equality with the classification matrix;
- each of the five TerminalAssociated operations has success/failure/cancellation coverage where the workflow supports each state, and asserts exactly one matching root terminal event before `run` returns;
- each of the ten OutcomeOnly operations asserts no matching root terminal event, terminal submitted state under the same operation id, and matching outcome acknowledgement semantics;
- tool/message/session-write/delegation family-terminal events never satisfy a root association;
- wrong operation id, wrong kind, missing root terminal, and duplicate root terminal fixtures fail the association validator;
- PartialCommit fixtures cover terminal-event-present and terminal-event-absent uncertainty without a second terminal event.

Compact currently exposes only a completed root mapping, so the planner must not promise an aborted event that does not exist; implement explicit failure/cancellation root closure only if current workflow behavior can emit a semantically correct event without changing compatibility. Otherwise the association validator must classify unsupported branches explicitly and keep the exactly-one rule for admitted terminal outcomes. [VERIFIED: `event.rs:108-110`; locked D-07 requires closure]

### Adapter Compatibility Tests

- exact JSON snapshots for all pre-existing RPC prompt/state/replay/control/error responses before and after lifecycle additions;
- explicit detach response status matrix and transport EOF cleanup using the same public detach call path;
- shutdown response/event additive behavior and event ordering;
- interactive normal exit detaches, top-level final exit shuts down, and a UI exit cannot terminate a shared runtime;
- existing `json_mode`, `rpc_mode`, `protocol_events`, interactive session, replay, navigation, and PartialCommit assertions stay unchanged.

### Blocking Commands

Run and record all of the following; the plan checker should reject any plan that marks one advisory:

```text
cargo test -p pi-coding-agent <focused lifecycle/association/guard targets>
cargo fmt --all --check
cargo test -p pi-coding-agent
cargo test --workspace
cargo check --workspace
<security/source audit integration targets>
<positive/negative external compile fixture target>
git diff --check
```

[VERIFIED: locked D-12]

## Suggested File Ownership

| Area | Primary files | Role |
|---|---|---|
| Lifecycle authority | `coding_session/snapshot_coordinator.rs`, `client_service.rs` | Runtime/client state machine, validation, detach result, notifications |
| Public lifecycle facade | `coding_session/public_projection.rs`, `coding_session/error.rs`, `src/lib.rs` | Stable outcomes/rejections, detach receiver semantics, curated exports |
| Shutdown orchestration | `coding_session/mod.rs`, `operation_control.rs`, `event_service.rs` | Admission close, drain, final lifecycle publication, receiver closure |
| Association ledger | `coding_session/public_operation.rs`, `operation.rs`, `event.rs`, `public_event.rs` | Closed 15-row classification and exact root terminal correlation |
| Submitted terminal anchors | `snapshot_coordinator.rs`, `public_projection.rs`, `mod.rs` | Event/outcome/uncertain anchors and acknowledgement APIs |
| RPC lifecycle | `protocol/rpc.rs`, `protocol/rpc/state.rs`, `commands.rs`, protocol types/wire files | Explicit commands, common cleanup, additive typed wire projection |
| Interactive lifecycle | `interactive/app.rs`, `interactive/loop.rs`, owner task/result modules | UI detach versus top-level shutdown ownership |
| Guards and fixtures | `tests/api_boundary_guards.rs`, `tests/product_runtime_boundary_guards.rs`, `tests/fixtures/api_boundary/**` | Auto-discovery, structured diagnostics, positive adjacency |
| Behavior contracts | `tests/public_api.rs`, `tests/rpc_mode.rs`, interactive/session tests, new focused lifecycle/association tests | Deterministic end-to-end evidence |
| Documentation | `docs/product-event-contract.md`, Phase 9 validation/security artifacts | 46-event inventory if lifecycle event added; final evidence |

## Recommended Plan and Wave Decomposition

### Wave 1: Freeze Public Types and Closed Ledgers

- define lifecycle states/outcomes/rejections and terminal anchor types;
- create exact 15-row association matrix plus set-equality guard;
- extend public API/privacy tests before runtime wiring;
- update validation plan with every locked D-12 gate.

### Wave 2: Coordinator Lifecycle and Receiver Wake-Up

- add runtime/client lifecycle state under the sole coordinator mutex;
- implement idempotent detach and typed validation for all state/mutation paths;
- add lifecycle notification and detach/shutdown-aware public receiver;
- prove preservation and blocked receiver wake-up.

### Wave 3: Generic Submission and Association Finalization

- generalize submission provenance to all 15 operations;
- implement event/outcome/uncertain terminal anchors and separate acknowledgements;
- replace `current_event_sequence()` guessing with exact terminal evidence;
- close PartialCommit correlation and exactly-one tests.

### Wave 4: Runtime Shutdown

- add two-phase admission close/drain/final lifecycle event/receiver close;
- bind operation control shutdown semantics without aborting active work;
- add idle, active, repeat, and post-shutdown tests.

### Wave 5: RPC and Interactive Projection

- add RPC explicit lifecycle commands/responses and one cleanup path for EOF/loop exit;
- add interactive client detach and explicit top-level owner shutdown;
- preserve all old wire/UI behavior with exact regression tests.

### Wave 6: Boundary and Milestone Closure

- replace fixed adapter-root inventory with auto-discovery plus explicit classification;
- upgrade compile fixtures to structured diagnostic/span/symbol contracts and adjacent positives;
- run security/source audits and every blocking workspace gate;
- update product-event contract, requirements, and final verification evidence only after all commands pass.

Keep waves sequential across authority boundaries. Parallelism is safe only within Wave 5 adapter projections after the runtime public contract is stable, and within Wave 6 independent guards/docs. [VERIFIED: shared-file/authority dependencies above]

## Don't Hand-Roll

| Problem | Do not build | Use instead | Why |
|---|---|---|---|
| Lifecycle storage | Adapter-local booleans or a second client registry | `SnapshotCoordinator` lifecycle fields | Single authority and atomic snapshots already exist. [VERIFIED: Phase 8 architecture] |
| Receiver termination | Polling, sleeps, or dropping only the connection clone | Typed receiver wrapper plus lifecycle notification | A blocked broadcast `recv()` otherwise cannot observe detach. [VERIFIED: current receiver implementation] |
| Shutdown cancellation | Implicit Abort on detach/shutdown | Admission closure plus drain | Locked semantics preserve operation ownership and terminal publication. [VERIFIED: D-02/D-04] |
| Terminal correlation | `current_event_sequence()` or retained-history scan | Exact root event returned/indexed by operation id | Current sequence may identify an unrelated event; retention is bounded. [VERIFIED: `mod.rs:224-232`; `event_service.rs:256-276`] |
| OutcomeOnly completion | Fake generic terminal product events | Typed outcome anchor and outcome acknowledgement | Locked D-09 explicitly forbids waiting for a nonexistent event. |
| PartialCommit recovery | New operation id or automatic retry | Original id plus uncertainty marker | Prevents duplicate terminal facts. [VERIFIED: D-08] |
| Compile diagnostics | Whole stderr category substring only | Cargo JSON compiler messages and primary spans | Proves intended symbol caused failure. [VERIFIED: D-11] |
| Adapter inventory | Only a fixed list of known roots | Automatic candidates plus explicit classification set equality | New sibling entrypoints otherwise bypass audit. [VERIFIED: current guard limitation] |

## Common Pitfalls

### Treating Detach as Stale Takeover

Current validation has one `StaleClient` bucket. Reusing it cannot distinguish first detach, repeated detach, takeover, or runtime shutdown and violates typed lifecycle decisions. Add explicit state before mapping public errors. [VERIFIED: `snapshot_coordinator.rs:530-555`; D-03/D-18]

### Closing a Receiver Without Waking It

Changing coordinator state alone leaves `recv().await` blocked until another event arrives. Tests must spawn a blocked receive and assert prompt deterministic wake-up after detach/shutdown. [VERIFIED: `public_projection.rs:446-452`]

### Losing the Active Prompt on Detach

Clearing the internal Prompt control channel would alter the running operation. Detach should revoke the connection binding/generation, while the runtime-owned receiver continues until operation completion; reconnect may rebind authority. [VERIFIED: `operation_control.rs:145-206`; D-02]

### Fake Terminal Sequence

The current guard uses the current global sequence, not a verified root terminal sequence. Do not carry this behavior into the new anchor model. [VERIFIED: `mod.rs:224-232`]

### Promoting Related Events to Root Terminal Events

BranchSummary Prompt events, delegation completion, profile change, capability change, tool completion, and session-write commit are not distinct root terminal associations. Keep them OutcomeOnly/event-level unless a locked product event is deliberately added; this phase's decisions favor preserving observable behavior. [VERIFIED: `docs/product-event-contract.md:121-149`]

### Holding the Coordinator Mutex Across Await

Lifecycle transitions and notification handle capture occur under lock; waiting/draining and broadcasting happen after release. Preserve the Phase 8 lock ordering. [VERIFIED: `event_service.rs:278-296`; `08-CONTEXT.md`]

### Shutdown API That Cannot Be Called During a Moved Owner Task

An async `&mut self` shutdown method alone cannot close admission while the owner is moved into a running adapter task. Put the admission/lifecycle flag in shared coordinator state and make the top-level method orchestrate final drain/publication. [VERIFIED: RPC task returns session via `CodingOperationTaskResult`; interactive owner restoration tests]

### Breaking Existing JSON by Adding Fields Everywhere

Lifecycle wire values must be separate commands/responses/events. Do not add lifecycle fields to existing state/prompt/control envelopes. Use exact pre-existing response snapshots. [VERIFIED: D-16]

### Compile Fixture False Positives

An unresolved import, missing dependency, syntax error, or different private symbol can currently satisfy the broad E0432/E0603 check. Structured primary-span assertions and adjacent positives are mandatory. [VERIFIED: `api_boundary_guards.rs:122-141`; D-11]

## Runtime State Inventory

This phase changes live in-memory runtime state but not durable external schemas.

| Category | Items Found | Required Action |
|---|---|---|
| Stored data | Rust-native session logs store `SessionEventEnvelope`; client cursor/drafts/receipts/submitted state are in-memory coordinator records. | Do not migrate session-log schemas. Preserve operation ids and existing append/manifest ordering. Lifecycle shutdown event remains a live product event unless a separate durable requirement exists. [VERIFIED: `session_log/event.rs`; `snapshot_coordinator.rs:63-119`] |
| Live service config | Event channel/retention and client/draft/receipt limits are internal constants. | No config migration; do not expose capacities as stable API. [VERIFIED: `snapshot_coordinator.rs:10-12`; `event_service.rs`] |
| OS-registered state | No OS service/registry entry participates in client lifecycle. | No action. [VERIFIED: repository-local CLI architecture] |
| Secrets and env vars | Auth/provider configuration is independent of lifecycle and association. | No action; lifecycle errors/events must not include credentials or internal debug dumps. [VERIFIED: phase scope and public error boundary] |
| In-flight state | Active operation permit, Prompt control sender/receiver, adapter-owned session task, broadcast receivers, pending submission lease, and coordinator client records. | Explicitly transition/rebind/drain each; this is the main migration surface. [VERIFIED: `mod.rs:156-205`; `operation_control.rs:145-206`; RPC/interactive owner tasks] |

## Security Threat Model

| Threat | Risk | Required mitigation |
|---|---|---|
| Cross-generation mutation after detach/takeover | High | Coordinator validates id + exact generation + attached runtime state on every mutation and replay path. [VERIFIED: D-13/D-14] |
| Cross-client Prompt control by operation-id knowledge | High | Control remains constructed from an authorized connection and verifies owner/generation/operation id before send. [VERIFIED: existing `snapshot_coordinator.rs:217-304`] |
| Second ordinary dispatcher on connection/lifecycle API | High | Public facade/source guard forbids `run`/dispatch and internal `Operation`; preparation remains lease-only. [VERIFIED: D-13] |
| Event after detach or shutdown | High | Lifecycle-aware receiver validation and notification closes delivery before returning a business event. [VERIFIED: D-18] |
| Duplicate or spoofed terminal association | High | Exact operation-id index and one-time insert; reject wrong kind/id and duplicate terminal root. [VERIFIED: D-06/D-07] |
| PartialCommit repudiation | High | Preserve original id and explicit uncertainty; no automatic retry/new id. [VERIFIED: D-08] |
| Internal authority/data disclosure | High | Curated facade ledger, Serde snapshots, stable codes, no Debug-derived wire payload. [VERIFIED: D-13] |
| Shutdown denial/deadlock | High | Never hold standard mutex across await; close admission before drain; deterministic drain tests. [VERIFIED: D-04 and Phase 8 lock rule] |
| Adapter bypass introduced later | High | Candidate discovery/classification set equality and recursive receiver-aware scans. [VERIFIED: D-10] |
| Compile guard false assurance | Medium | Structured diagnostic/span/symbol contract plus positive compile neighbor. [VERIFIED: D-11] |

## Package Legitimacy Audit

No external packages are proposed or installed. Package legitimacy and registry checks are not applicable. [VERIFIED: recommended stack uses existing workspace dependencies]

## Code Examples

### Exhaustive Association Descriptor

```rust
impl CodingAgentOperation {
    pub(crate) fn descriptor(&self) -> OperationDescriptor {
        match self {
            Self::Prompt(_) => OperationDescriptor::terminal(OperationKind::Prompt),
            Self::Compact(_) => OperationDescriptor::terminal(OperationKind::Compact),
            Self::BranchSummary { .. } => OperationDescriptor::outcome_only(OperationKind::BranchSummary),
            // Exhaust all 15 variants; no wildcard.
        }
    }
}
```

This should be the source used by submission preparation and association tests, not a parallel hand-maintained switch in each adapter. [VERIFIED: existing exhaustive `into_internal` pattern at `public_operation.rs:119-168`]

### Two-Phase Lifecycle Transition

```rust
let notification = {
    let mut state = coordinator.state.lock().unwrap();
    let outcome = transition_to_detached(&mut state, &handle)?;
    (outcome, state.lifecycle_notifier.clone())
};
notification.1.notify_waiters();
notification.0
```

The exact notifier may differ, but notification/broadcast must occur after releasing the standard mutex. [VERIFIED: existing EventService publication pattern]

### Exact Outcome Acknowledgement

```rust
match submitted.anchor {
    SubmittedTerminalAnchor::OutcomeOnly { ref acknowledgement_id }
        if acknowledgement_id == supplied => clear(),
    SubmittedTerminalAnchor::ProductEvent { .. }
    | SubmittedTerminalAnchor::TerminalUncertain { .. } => reject_wrong_anchor(),
    _ => reject_mismatch(),
}
```

This preserves the distinction locked by D-09 and prevents an unrelated high event sequence from clearing OutcomeOnly state.

## Open Questions for Planning

No user decision is required. The following are implementation details the planner must resolve explicitly in tasks:

1. Choose the existing-dependency notification primitive (`watch`, `Notify` plus epoch, or cancellation token) that can distinguish detach generation from runtime shutdown and avoid missed wake-ups. The behavior contract, not the type, is locked.
2. Decide whether the terminal association index is stored transiently in `SnapshotState` or returned directly from EventService emission paths. Prefer the smallest design that captures exact sequence without scanning retention.
3. Define the stable outcome acknowledgement id shape. It must be opaque, operation-scoped, and not reveal internal receipt/durability implementation details.
4. Resolve Compact failure/cancellation closure based on live workflow semantics while preserving exactly-one terminal association and compatibility.

## Sources

### Primary Repository Sources

- `.planning/phases/09-lifecycle-association-guards-and-closure/09-CONTEXT.md` - locked phase decisions and scope.
- `.planning/phases/06-product-event-inventory-and-typed-contract/06-RESEARCH.md` and `docs/product-event-contract.md` - 45-event inventory and five existing root terminal associations.
- `.planning/phases/08-client-connection-replay-and-scoped-control/08-CONTEXT.md`, `08-RESEARCH.md`, `08-PATTERNS.md`, `08-VERIFICATION.md`, and `08-GAP-CLOSURE.md` - connection/coordinator/replay/control contract and verified implementation.
- `crates/pi-coding-agent/src/coding_session/{mod.rs,snapshot_coordinator.rs,public_projection.rs,public_operation.rs,operation.rs,event.rs,public_event.rs,event_service.rs,operation_control.rs,session_service.rs,error.rs}` - current runtime implementation.
- `crates/pi-coding-agent/src/protocol/rpc.rs`, `src/protocol/rpc/**`, and `src/interactive/**` - adapter ownership and cleanup paths.
- `crates/pi-coding-agent/tests/{api_boundary_guards.rs,product_runtime_boundary_guards.rs,public_api.rs}` and `tests/fixtures/api_boundary/**` - current guard/fixture behavior.

### External Sources

None. This phase is governed by locked repository contracts and current implementation; no new library capability or unstable external standard was required.

## Research Verification Checklist

- [x] All locked decisions copied into the first content section.
- [x] Phase 6-8 context/research/verification evidence inspected.
- [x] CodeGraph used before direct code search/read.
- [x] Exact 15-operation association matrix derived from live public enum and current event contract.
- [x] Runtime State Inventory covers stored data, live config, OS state, secrets/env, and in-flight state.
- [x] Security domain and applicable ASVS-style categories included.
- [x] No external packages proposed.
- [x] Risks, pitfalls, file ownership, test architecture, blocking gates, and wave dependencies documented.

---

*Phase: 09-lifecycle-association-guards-and-closure*  
*Research mode: gsd-phase-researcher generic-agent workaround*
