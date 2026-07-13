# Phase 9: Lifecycle Association, Guards, and Closure - Context

**Gathered:** 2026-07-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Close the v1.1 client lifecycle and operation-association contract built in Phases 6-8. This phase adds explicit, idempotent client detach and runtime shutdown behavior; closes operation id, submitted state, terminal outcome, and terminal event association; hardens first-party adapter discovery and external compile diagnostics; and runs the complete compatibility, security, source-audit, and workspace verification suite.

The phase preserves `CodingAgentSession::run(CodingAgentOperation)` as the only ordinary-operation dispatcher, keeps Prompt controls outside the ordinary operation queue, and preserves existing RPC, interactive, JSON/print, replay, ordering, durability, and `PartialCommit` behavior. A separately named runtime owner, multi-session daemon orchestration, new product workflows, and broad UI redesign remain out of scope.

</domain>

<decisions>
## Implementation Decisions

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

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Milestone Scope and Requirements

- `.planning/PROJECT.md` - v1.1 core value, architectural constraints, compatibility requirements, and deferred runtime-owner work.
- `.planning/ROADMAP.md` - Phase 9 goal, requirements, and blocking success criteria.
- `.planning/REQUIREMENTS.md` - authoritative `COMPAT-03`, `CLIENT-04`, `CONTROL-02`, `GUARD-01`, and `GUARD-02` definitions and traceability.
- `.planning/STATE.md` - completed Phase 6-8 decisions and current milestone position.

### Prior Contracts

- `.planning/phases/06-product-event-inventory-and-typed-contract/06-RESEARCH.md` - emitted event inventory, terminal association baseline, and typed event design evidence.
- `.planning/phases/08-client-connection-replay-and-scoped-control/08-CONTEXT.md` - locked connection generation, replay/live handoff, acknowledgement, drafts, submitted state, receipt, and scoped-control semantics.
- `.planning/phases/08-client-connection-replay-and-scoped-control/08-VERIFICATION.md` - verified public reconnect/control contract and Phase 8 gap-closure evidence.
- `.planning/phases/08-client-connection-replay-and-scoped-control/08-GAP-CLOSURE.md` - atomic public replay/live receiver boundary and end-to-end behavior closure.

### Existing Architecture and Testing

- `.planning/codebase/ARCHITECTURE.md` - product runtime ownership, adapter boundaries, data flow, and durable-session authority.
- `.planning/codebase/TESTING.md` - deterministic fixture, public API, source guard, and workspace verification conventions.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `crates/pi-coding-agent/src/coding_session/snapshot_coordinator.rs`: sole client registry, generation, acknowledgement, draft, submitted-state, event cursor, and recovery projection authority; lifecycle state must extend this authority rather than add another registry.
- `crates/pi-coding-agent/src/coding_session/public_projection.rs`: stable connection, reconnect receiver, snapshot, submitted state, draft, control receipt, and rejection contracts; lifecycle values belong on this curated projection surface.
- `crates/pi-coding-agent/src/coding_session/event_service.rs`: atomic retained replay/live receiver boundary and sequenced publication; shutdown ordering must preserve its commit-before-broadcast invariants.
- `crates/pi-coding-agent/src/coding_session/operation_control.rs`: active-operation guard and Prompt-control transport; shutdown closes admission/control without transferring operation ownership to a connection.
- `crates/pi-coding-agent/src/coding_session/event.rs` and `public_event.rs`: operation id, terminal status, durability, and current root-terminal association logic for the closed operation matrix.
- `crates/pi-coding-agent/tests/public_api.rs`: deterministic external behavior coverage for takeover, reconnect, lease, acknowledgement, control receipts, and typed recovery.
- `crates/pi-coding-agent/tests/api_boundary_guards.rs` and `product_runtime_boundary_guards.rs`: established fail-closed source scanning and public facade ledgers.

### Established Patterns

- Public contracts are exported only through `pi_coding_agent::api`; internal services, coordinators, operation metadata, senders, queues, and Flow nodes remain private.
- Ordinary operations enter through `CodingAgentSession::run`; public connections expose state, recovery, preparation, acknowledgement, lifecycle, and scoped control only.
- Client takeover and mutation authorization are generation-scoped and typed; lifecycle must use the same authority rather than introduce adapter-local flags.
- Snapshot-visible writers commit through the coordinator, release the standard mutex, then publish or perform async work. No standard mutex may be held across await.
- Durable uncertainty preserves operation identity through `PartialCommit`; recovery must not invent a replacement operation or duplicate terminal event.
- Integration tests use deterministic channels, faux providers, temp storage, exact event ordering, and source-level guards rather than live providers or timing sleeps.

### Integration Points

- Add public connection detach and runtime shutdown ownership in `crates/pi-coding-agent/src/coding_session/mod.rs` and the curated `api` facade in `crates/pi-coding-agent/src/lib.rs`.
- Coordinate lifecycle state with `snapshot_coordinator.rs`, `client_service.rs`, `event_service.rs`, `operation_control.rs`, and submitted-operation terminal anchors.
- Build the 15-operation association matrix from `public_operation.rs`, `operation.rs`, `event.rs`, `public_event.rs`, and canonical run outcomes.
- Project explicit lifecycle commands/events through `crates/pi-coding-agent/src/protocol/rpc/` while preserving all existing wire responses.
- Project client detach versus owner shutdown through `crates/pi-coding-agent/src/interactive/` and top-level process ownership in `main.rs`/`lib.rs`.
- Harden recursive adapter discovery and diagnostic-bound compile fixtures under `crates/pi-coding-agent/tests/`.

</code_context>

<specifics>
## Specific Ideas

- Lifecycle outcomes and rejections must be typed, exhaustive, and stable; error-string parsing is never authoritative.
- Detach is recoverable disconnection, not operation cancellation and not deletion of client-local recoverable state.
- Shutdown is a drain-and-publish boundary, not an immediate receiver drop and not an implicit Abort policy.
- Existing RPC/JSON shapes remain byte-for-byte stable; lifecycle is additive through separate commands, responses, and events.

</specifics>

<deferred>
## Deferred Ideas

None - discussion stayed within phase scope. A separately named runtime owner and multi-session daemon routing remain the already-recorded future requirements.

</deferred>

---

*Phase: 09-lifecycle-association-guards-and-closure*
*Context gathered: 2026-07-14*
