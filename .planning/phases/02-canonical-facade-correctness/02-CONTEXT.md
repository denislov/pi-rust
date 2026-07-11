# Phase 2: Canonical Facade Correctness - Context

**Gathered:** 2026-07-11
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 2 establishes one complete, stable, and verifiable public operation facade. It ensures that first-party callers can import the required live-session contracts from `pi_coding_agent::api`, that every `CodingAgentOperation` passes through `CodingAgentSession::run` and the metadata-selected dispatcher, and that every internal outcome has an exhaustive public projection. It also proves the behavior-preserving durable semantics named by FACADE-05. This phase does not migrate production adapters, broadly migrate all tests, delete workflow-specific compatibility methods, redesign event payloads, or take on unrelated session-log reliability work; those remain assigned to later phases or out of scope.

</domain>

<decisions>
## Implementation Decisions

### Stable API Completeness
- **D-01:** Every public type that appears in the stable operation facade's signatures must be directly re-exported from `pi_coding_agent::api`. Callers must not need to know the type's implementation module or use crate-root compatibility exports.
- **D-02:** The stable API completeness boundary includes the full operation input/outcome/error type closure plus the existing live-session contracts required for create/open, snapshot/query, event subscription, and control.
- **D-03:** Session construction, observation, subscription, and control remain distinct contracts rather than being forced into `CodingAgentOperation`. Their inclusion in `api` is not permission to expose internal services, registries, dispatch metadata, Flow nodes, raw plugin options, or provider internals.
- **D-04:** Crate-root compatibility exports remain available during Phase 2, but all first-party code added or modified in this phase must use `pi_coding_agent::api`. Compatibility deletion remains Phase 4 work.
- **D-05:** Prove API completeness and privacy with Rust visibility plus positive and negative compile/API tests wherever possible. Use source-scanning guards only for boundaries the language cannot express.

### Dispatcher And Projection Exhaustiveness
- **D-06:** Maintain a per-variant contract matrix for all 15 current `CodingAgentOperation` variants. Each entry must independently prove public-to-internal mapping, expected metadata dispatch mode, and public outcome family.
- **D-07:** Keep `CodingAgentOperation::into_internal`, `Operation::metadata`, and `CodingAgentOperationOutcome::from_internal` as exhaustive matches so a new variant creates compiler-visible work.
- **D-08:** Keep the expected operation inventory independent from implementation metadata. Do not generate expected dispatch modes or outcome families from the implementation being tested.
- **D-09:** Use owner unit tests to inspect internal conversion, metadata, and projection. Use public integration tests that import contracts only through `pi_coding_agent::api` and execute work through `CodingAgentSession::run`.
- **D-10:** Prove that `run` follows metadata with direct metadata assertions plus dispatcher-specific behavior tests for async, sync-read-only, and sync-mutable paths. Do not add production dispatcher instrumentation for testing.

### Durable Mutation Verification
- **D-11:** Add focused canonical-facade behavior evidence for every FACADE-05 high-risk operation: session fork, active-leaf switch, branch-summary reuse, plugin load and command, default-profile mutation, and delegation approval and rejection.
- **D-12:** Apply a shared invariant checklist to those operations: correct public outcome, intended state effect, unchanged error semantics, reopen/replay persistence when applicable, semantic product events and sequence continuity when applicable, and explicit `PartialCommit` when a durable operation crosses a partial-commit boundary.
- **D-13:** Add operation-specific assertions beyond the shared checklist, including session-tree and active-leaf state, branch-summary reuse behavior, plugin registry/command effects, default-profile state, and delegation queue/decision effects as applicable.
- **D-14:** Reuse existing deterministic fixtures and behavior assertions, but make canonical correctness tests call `CodingAgentSession::run` directly. Do not make the canonical suite depend on differential execution against workflow-specific methods scheduled for deletion.
- **D-15:** For persistence-sensitive operations, reuse deterministic failure injection to cover failure before append with no durable mutation, failure after append but before manifest/publication with explicit `PartialCommit`, and reopen/replay recovery with the durable log as authority.
- **D-16:** Do not expand Phase 2 into torn JSONL recovery, interrupted manifest replacement, power-loss durability, atomic filesystem transactions, or other independent crash-consistency initiatives.

### Agent Discretion
- Choose the exact organization of the operation contract matrix and test helper names, provided expected values remain independent from implementation metadata and helpers stay test-only.
- Reuse or extend existing fixtures at the owning test layer without creating a new production compatibility facade.
- Select precise negative API-test mechanics according to what stable Rust and the existing test harness can express; retain narrowly scoped source guards only where compiler-visible enforcement is impractical.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Milestone Authority And Phase Evidence
- `.planning/PROJECT.md` - Defines the Stage 9 goal, architecture constraints, compatibility requirements, deletion order, and exclusions.
- `.planning/REQUIREMENTS.md` - Defines FACADE-01 through FACADE-05 and keeps adapter migration, broad test migration, deletion, guards, and closure assigned to later phases.
- `.planning/ROADMAP.md` - Fixes the Phase 2 boundary, dependencies, goal, and success criteria.
- `.planning/phases/01-evidence-based-baseline/01-AUDIT.md` - Current source-backed operation matrix, caller inventories, authority conflicts, and Phase 2 findings.
- `.planning/phases/01-evidence-based-baseline/01-CONTEXT.md` - Locks the evidence hierarchy, status taxonomy, history handling, and downstream phase boundaries inherited by Phase 2.

### Stage 9 Design And Runtime Architecture
- `docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md` - Historical execution plan and useful inventory of expected operation, adapter, test, and deletion work; evidence to verify rather than completion authority.
- `docs/superpowers/specs/2026-07-10-canonical-operation-runtime-convergence-design.md` - Defines the intended canonical facade contract and behavior-preserving convergence goals.
- `docs/superpowers/specs/2026-07-07-operation-runtime-reference-architecture.md` - Defines runtime ownership, dispatch, service, event, capability, and persistence boundaries.

### Current Canonical Facade Implementation
- `crates/pi-coding-agent/src/lib.rs` - Owns the stable `pi_coding_agent::api` facade and compatibility exports.
- `crates/pi-coding-agent/src/coding_session/public_operation.rs` - Defines public operations, public outcomes, public-to-internal conversion, and internal-to-public projection.
- `crates/pi-coding-agent/src/coding_session/operation.rs` - Defines internal operations, metadata, dispatch modes, admission facts, and internal outcomes that must remain private.
- `crates/pi-coding-agent/src/coding_session/mod.rs` - Defines `CodingAgentSession::run`, the three dispatchers, durable mutations, compatibility methods, and owner tests.

### Existing Contract And Boundary Tests
- `crates/pi-coding-agent/tests/public_api.rs` - Existing external-consumer tests for the stable facade and canonical operation behavior.
- `crates/pi-coding-agent/tests/api_boundary_guards.rs` - Existing stable/private API boundary assertions.
- `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` - Existing first-party canonical-runtime and deprecation-suppression source guards.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `CodingAgentSession::run` in `crates/pi-coding-agent/src/coding_session/mod.rs` already performs public conversion, reads `operation.metadata().dispatch_mode`, chooses the async/read-only/mutable dispatcher, and projects the internal outcome.
- `CodingAgentOperation::into_internal` and `CodingAgentOperationOutcome::from_internal` in `crates/pi-coding-agent/src/coding_session/public_operation.rs` already provide exhaustive conversion points suitable for owner contract tests.
- Existing faux-provider, tempfile-backed persistent-session, plugin, profile, delegation, branch, and failure-injection fixtures can be reused rather than introducing live-provider or production test hooks.
- Existing public API and boundary suites provide the correct integration-test ownership layer for facade completeness and privacy assertions.

### Established Patterns
- Public embedding contracts are curated through `pi_coding_agent::api`; implementation modules remain private or migration-only.
- Internal operation metadata is the source of dispatch selection, with `Async`, `SyncReadOnly`, and `SyncMutable` as the three execution modes.
- Product workflows retain behavior through typed outcomes, product events, replay-derived state, session transactions, recovery markers, and explicit partial-commit errors.
- Tests favor deterministic offline fixtures, behavior assertions, Rust visibility, and compile/API boundaries; textual guards are fallback enforcement.

### Integration Points
- Add or correct stable exports in `crates/pi-coding-agent/src/lib.rs` without exposing internal runtime contracts.
- Extend conversion/metadata/projection coverage around `public_operation.rs`, `operation.rs`, and the owner tests in `coding_session/mod.rs`.
- Extend `crates/pi-coding-agent/tests/public_api.rs` for external import and `run` usability evidence.
- Strengthen `crates/pi-coding-agent/tests/api_boundary_guards.rs` only where Rust visibility or compile/API checks cannot express the intended negative boundary.
- Reuse persistent session, branch navigation, plugin, profile, and delegation fixtures for FACADE-05 behavior and failure-path coverage.

</code_context>

<specifics>
## Specific Ideas

- Treat the operation inventory as an explicit contract ledger: every public variant must have an independently stated expected internal variant, dispatch mode, and public outcome family.
- Keep canonical behavior tests independent from workflow-specific methods scheduled for deletion; compatibility is preserved by retaining existing behavior assertions, not by making both APIs permanent test oracles.
- Apply strong durability assertions only to the FACADE-05 operations that carry state, event, or partial-commit risk; do not duplicate every later adapter and test-migration task in Phase 2.

</specifics>

<deferred>
## Deferred Ideas

None - discussion stayed within the Phase 2 facade-correctness scope.

</deferred>

---

*Phase: 2-Canonical Facade Correctness*
*Context gathered: 2026-07-11*
