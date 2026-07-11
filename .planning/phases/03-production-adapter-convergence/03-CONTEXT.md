# Phase 3: Production Adapter Convergence - Context

**Gathered:** 2026-07-11
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 3 migrates every first-party production adapter that performs live-session product
work to `CodingAgentSession::run(CodingAgentOperation)`. The migration covers JSON and
print prompt flows, RPC background and command work, interactive background operations
and mutations, and persistent interactive navigation. Existing output, error, session,
wire, event, control, subscriber, snapshot, projection, and UI behavior must remain
unchanged.

This phase does not broadly migrate owner or integration tests, delete workflow-specific
session methods, strengthen the final recursive source guards, redesign RPC or interactive
presentation, converge Stage 10 event payloads, or introduce a new lifecycle control
handle. Test convergence and method deletion belong to Phase 4; final guard and closure
hardening belongs to Phase 5.

</domain>

<decisions>
## Implementation Decisions

### Migration Axis And Risk Order
- **D-01:** Organize Phase 3 by adapter risk, not by operation family across multiple adapters.
- **D-02:** The required order is JSON/print first, RPC second, interactive ordinary operations third, and interactive navigation last.
- **D-03:** A lower-risk adapter boundary must complete its behavior-preservation gate before planning or execution advances to the next risk layer.

### JSON And Print Boundary
- **D-04:** JSON, persistent print, and transient print belong in one plan because they share the prompt operation contract.
- **D-05:** The three paths remain separate tasks with separate atomic commits so a regression can be isolated to one adapter/session mode.
- **D-06:** The plan closes with one combined JSON/print parity gate covering output, errors, and session effects.

### RPC Boundary
- **D-07:** Split RPC migration into two plans based on control model rather than one large RPC plan or one plan per operation family.
- **D-08:** The first RPC plan owns background/select-driven operations: prompt, agent, team, delegation approval, and any equivalent work whose correctness depends on `tokio::select!`, control delivery, and product-event forwarding.
- **D-09:** The second RPC plan owns mutation/command operations: self-healing edit, profile mutation, delegation rejection, plugin load, plugin command, and equivalent short-lived typed operations.
- **D-10:** Background control/event behavior must have an independent verification boundary so mutation outcome mapping cannot obscure multiplexing regressions.

### Interactive Boundary
- **D-11:** Split interactive migration into three risk-increasing plans.
- **D-12:** Interactive background plan: prompt, agent, team, manual compaction, self-healing edit, plugin actions, branch summary, and other background operations that retain the existing event/control loop.
- **D-13:** Interactive mutation plan: profile mutation and delegation decisions, preserving existing menus, dialogs, queue state, errors, and visible projections.
- **D-14:** Interactive navigation plan: fork, active-leaf switch, session/owner replacement, subscription continuity, event sequence continuity, snapshot refresh, projection refresh, and visible navigation behavior.
- **D-15:** Navigation is the final migration unit and must be independently accepted after all non-navigation interactive paths are canonical.

### Locked Cross-Phase Constraints
- **D-16:** Production adapters use contracts exported by `pi_coding_agent::api`; internal operations, metadata, plugin options, services, registries, and Flow nodes remain private.
- **D-17:** Session create/open, snapshot/query, subscription, and control remain distinct contracts and are not forced into `CodingAgentOperation`.
- **D-18:** Preserve current adapter semantics rather than using migration as permission to redesign output, wire protocol, TUI projection, event payloads, or control handling.
- **D-19:** Do not delete broad workflow methods in Phase 3. Phase 4 migrates tests and deletes methods only after all production callers are gone.
- **D-20:** Do not create an adapter compatibility facade or shared helper that merely recreates the deleted broad workflow surface under another name.

### the agent's Discretion
- Choose exact plan names and task-level file grouping within the six locked migration boundaries.
- Decide whether small crate-private typed outcome extraction helpers reduce duplication, provided they remain adapter-oriented, do not become a second operation facade, and preserve each adapter's current error/output ownership.
- Select focused test commands and fixture reuse for each task while retaining all existing behavior assertions and deterministic offline execution.
- Determine whether closely related operations within a locked plan require additional atomic commits when source ownership or rollback safety warrants it.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Milestone Authority
- `.planning/PROJECT.md` - Defines the Stage 9 goal, architecture constraints, compatibility requirements, deletion order, and out-of-scope work.
- `.planning/REQUIREMENTS.md` - Defines ADAPT-01 through ADAPT-04, RPC-01 through RPC-04, and INTER-01 through INTER-05 as the complete Phase 3 requirement set.
- `.planning/ROADMAP.md` - Fixes the Phase 3 goal and five observable success criteria.
- `.planning/phases/03-production-adapter-convergence/03-CONTEXT.md` - Locks the migration axis, plan boundaries, order, and cross-phase constraints from this discussion.

### Prior Phase Contracts
- `.planning/phases/01-evidence-based-baseline/01-AUDIT.md` - Source-backed operation inventory, production caller inventory, compatibility method inventory, and actual gap classification.
- `.planning/phases/01-evidence-based-baseline/01-CONTEXT.md` - Defines evidence authority, completion thresholds, and the rule that current code/tests/guards outrank historical checklist state.
- `.planning/phases/02-canonical-facade-correctness/02-CONTEXT.md` - Defines the stable facade, dispatcher, outcome projection, durability, and public/private boundaries that adapters must consume.
- `.planning/phases/02-canonical-facade-correctness/02-VERIFICATION.md` - Confirms the canonical facade and high-risk runtime semantics that Phase 3 must preserve.
- `.planning/phases/02-canonical-facade-correctness/02-SECURITY.md` - Records closed trust-boundary threats for facade privacy, dispatch, projection, durable mutation, plugin capability, and test-only fault controls.

### Stage 9 Design And Architecture
- `docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md` - Historical migration targets, expected callers, behavior tests, commands, and sequencing evidence; use as input, not authoritative completion state.
- `docs/superpowers/specs/2026-07-10-canonical-operation-runtime-convergence-design.md` - Defines the intended operation convergence contract and non-goals.
- `docs/superpowers/specs/2026-07-07-operation-runtime-reference-architecture.md` - Defines runtime ownership, adapter boundaries, event/control paths, persistence, and service layering.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/pi-coding-agent/src/coding_session/public_operation.rs` - Complete public operation inputs, typed public outcomes, public-to-private conversion, and exhaustive projection.
- `crates/pi-coding-agent/src/coding_session/mod.rs` - Canonical `run`, metadata-selected dispatchers, observation/control contracts, retained compatibility methods, and owner transition behavior.
- `crates/pi-coding-agent/tests/public_api.rs` - Facade-only operation and support-type closure evidence.
- `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` - Existing adapter ownership, low-level-agent prohibition, event projection, and canonical-facade boundary checks.
- Existing JSON, print, RPC, and interactive integration suites under `crates/pi-coding-agent/tests/` - Behavior-preservation assertions to retain rather than replace with compile-only checks.

### Established Patterns
- Production adapters translate process or UI inputs into product operations and project typed product outcomes/events back to their existing external contracts.
- RPC uses JSONL command/response handling plus product-event forwarding; long-running work coordinates abort, follow-up, and steering through existing `tokio::select!` paths.
- Interactive code owns transcript/projection state, background task coordination, dialogs/menus, product-event subscription, and snapshot refresh after persistent transitions.
- Durable navigation semantics are replay-authoritative and operation-ID-sensitive; Phase 2 tests already cover no-append, partial-commit, event sequence, and reopened state.
- Deterministic faux providers, tempfile sessions, serialized environment/provider guards, and focused Rust integration tests are the preferred verification tools.

### Integration Points
- `crates/pi-coding-agent/src/protocol/json_mode.rs` - JSON prompt adapter and output/error projection.
- `crates/pi-coding-agent/src/print_mode.rs` - Persistent and transient print prompt flows.
- `crates/pi-coding-agent/src/protocol/rpc/` - RPC command routing, state, background tasks, event queues/adapters, responses, and control multiplexing.
- `crates/pi-coding-agent/src/interactive/loop.rs` - Interactive event/control loop and background-operation completion handling.
- `crates/pi-coding-agent/src/interactive/session_actions.rs` - Interactive session mutations and persistent navigation actions.
- `crates/pi-coding-agent/src/interactive/event_bridge.rs` - Product-event subscription/projection continuity.
- `crates/pi-coding-agent/src/interactive/commands.rs` and related menus/dialogs - User command dispatch and visible mutation behavior.

</code_context>

<specifics>
## Specific Ideas

- The migration sequence itself is a safety mechanism: complete and verify one adapter/control boundary before touching the next risk tier.
- JSON and print are intentionally grouped for shared prompt semantics but retain per-path commits for diagnosis and rollback.
- RPC background work is separated from mutation commands because `tokio::select!` control handling and event forwarding deserve their own failure boundary.
- Interactive navigation is deliberately last; session replacement, subscriptions, snapshots, projections, and visible navigation form one atomic compatibility contract.

</specifics>

<deferred>
## Deferred Ideas

- Broad owner/public/integration test migration and compatibility method deletion - Phase 4.
- Parser-complete adapter/source boundary guards and final Stage 9 closure audits - Phase 5.
- Typed `ProductEvent` payload convergence and compatibility subscription deletion - Stage 10.

</deferred>

---

*Phase: 3-Production Adapter Convergence*
*Context gathered: 2026-07-11*
