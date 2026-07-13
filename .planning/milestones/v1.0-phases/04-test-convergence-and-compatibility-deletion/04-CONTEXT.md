# Phase 4: Test Convergence and Compatibility Deletion - Context

**Gathered:** 2026-07-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 4 migrates owner, public API, and integration behavior tests from replaced
workflow-specific `CodingAgentSession` methods to
`CodingAgentSession::run(CodingAgentOperation)`, then deletes each replaced broad
method after every production and test caller is gone. Existing assertions for
agents, teams, profiles, delegation, export, branch summaries, self-healing edits,
durability, events, replay, and errors remain authoritative.

This phase preserves construction, create/open/resume, snapshots, queries, event
subscriptions, control paths, and static repository helpers because they are not
operation-facade replacements. It does not perform Stage 10 ProductEvent payload or
compatibility-subscription convergence, and it does not absorb Phase 5's recursive
parser-complete guard hardening or final Stage 9 closure work.

</domain>

<decisions>
## Implementation Decisions

### Test Migration Boundary
- **D-01:** Use a public-first rule: every test that verifies a product workflow must import stable contracts through `pi_coding_agent::api` where applicable and execute the workflow through `CodingAgentSession::run`.
- **D-02:** Public operation outcomes become the primary assertion entry point, but migration must retain all existing behavior, state, error, event, replay, and persistence assertions. Only demonstrably duplicate implementation-detail assertions already covered through the public contract may be removed.
- **D-03:** Co-located owner tests may use crate-private operation paths only when the public operation cannot express required custom internal options, metadata or dispatcher state, or a deterministic fault boundary. Each exception must state why the public path is insufficient; convenience alone is not sufficient.
- **D-04:** Do not widen the public operation or option surface merely to support an owner white-box test.

### Migration And Deletion Order
- **D-05:** Organize work by risk and caller layer rather than deleting all methods at once. Migrate lower-risk owner/public/integration callers first, then persistence-, navigation-, and delegation-sensitive methods.
- **D-06:** Before deleting any method definition, prove that both production and test callers are zero, then run the method group's focused behavior tests, relevant boundary guards, and `cargo check -p pi-coding-agent`.
- **D-07:** Delete the definition only after the zero-caller and focused gates pass. A missed caller must be migrated; do not restore the method or introduce an equivalent wrapper under a new name.
- **D-08:** After deletion, convert existing closed-method ledgers into explicit absence checks covering old definitions, old calls, and synonymous compatibility wrappers. Phase 5 remains responsible for recursive/parser-complete hardening.

### Public Workflow Method Removal
- **D-09:** Treat `CodingAgentSession::set_default_agent_profile_id`, `approve_delegation_confirmation`, and `reject_delegation_confirmation` as replaced operation entry points. Migrate all first-party public API and integration callers to `CodingAgentOperation` plus `run`, then delete these public methods.
- **D-10:** This milestone intentionally converges the stable API and accepts the resulting breaking change for external callers of those old methods. Do not retain deprecated shims or a transition facade.
- **D-11:** Preserve public construction, create/open/resume, snapshot/query, event subscription, control, and static repository helpers. Their survival is an explicit contract boundary, not an exception to operation convergence.

### Test Helper Ownership
- **D-12:** Keep private conversion, metadata, dispatcher, custom-internal-option, and fault-boundary fixtures in co-located `coding_session` tests. Put reusable public-behavior fixtures in `crates/pi-coding-agent/tests/support`.
- **D-13:** Shared integration helpers may perform exhaustive typed outcome extraction only. They must not select operations, create or own sessions, hide errors, or hold runtime services.
- **D-14:** Fault injection remains directly `cfg(test)`, crate-private, and action-specific. Do not expose generic failure selectors, internal services, queues, registries, or production hooks.
- **D-15:** No helper may recreate the deleted broad workflow facade, even if its immediate callers are tests.

### Agent's Discretion
- Choose the exact low-to-high risk method groups and focused Cargo commands, provided every group satisfies D-05 through D-08.
- Decide when a pure typed outcome extractor removes meaningful repetition; local exhaustive matches remain acceptable when sharing would obscure the contract.
- Choose test names and file-local fixture organization while preserving existing behavioral assertions and deterministic offline execution.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Milestone And Phase Authority
- `.planning/PROJECT.md` - Defines the canonical runtime goal, deletion order, behavior/durability constraints, and Stage 10 exclusions.
- `.planning/REQUIREMENTS.md` - Defines TEST-01 through TEST-04 and DELETE-01 through DELETE-04 as the complete Phase 4 requirement set.
- `.planning/ROADMAP.md` - Fixes the Phase 4 goal and five observable success criteria.
- `.planning/STATE.md` - Records Phase 4 as the current focus and carries forward completed Phase 2/3 decisions.
- `.planning/phases/04-test-convergence-and-compatibility-deletion/04-CONTEXT.md` - Locks the migration, deletion, public API, and helper decisions from this discussion.

### Prior Phase Contracts And Evidence
- `.planning/phases/01-evidence-based-baseline/01-AUDIT.md` - Source-backed operation, test-caller, and compatibility-method inventories; current source still outranks historical checklist state.
- `.planning/phases/02-canonical-facade-correctness/02-CONTEXT.md` - Locks public facade completeness, owner-vs-integration test responsibilities, and durable canonical behavior requirements.
- `.planning/phases/02-canonical-facade-correctness/02-VERIFICATION.md` - Confirms the facade and high-risk behavior that migrated tests must continue to prove.
- `.planning/phases/03-production-adapter-convergence/03-CONTEXT.md` - Locks production adapter convergence and reserves broad test migration and method deletion for Phase 4.
- `.planning/phases/03-production-adapter-convergence/03-VERIFICATION.md` - Confirms production callers have converged and records the behavior baseline Phase 4 must preserve.
- `.planning/phases/03-production-adapter-convergence/03-SECURITY.md` - Records closed adapter/runtime threats and test-only fixture constraints relevant to deletion safety.

### Stage 9 Design And Architecture
- `docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md` - Historical caller/deletion inventory and verification ideas; use as design input, not completion authority.
- `docs/superpowers/specs/2026-07-10-canonical-operation-runtime-convergence-design.md` - Defines the intended one-facade convergence contract and non-goals.
- `docs/superpowers/specs/2026-07-07-operation-runtime-reference-architecture.md` - Defines runtime ownership, adapter boundaries, persistence, events, and control contracts.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/pi-coding-agent/src/coding_session/public_operation.rs` - Exhaustive public operation input and outcome contracts used by migrated tests.
- `crates/pi-coding-agent/src/coding_session/mod.rs` - Owner tests, canonical `run`, retained broad methods, action-specific test fixtures, and deletion targets.
- `crates/pi-coding-agent/tests/public_api.rs` - External-consumer compile/behavior tests that currently include callers of public profile/delegation methods.
- `crates/pi-coding-agent/tests/support/mod.rs` - Existing home for reusable integration fixtures and guards.
- `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` - Closed method ledger, adapter checks, and the natural location for post-deletion absence rules.

### Established Patterns
- Integration tests verify stable public behavior through `pi_coding_agent::api`; co-located tests inspect private conversion, metadata, dispatcher, and fault boundaries.
- Deterministic faux providers, tempfile sessions, typed outcomes, exact ProductEvent assertions, replay checks, and structured PartialCommit IDs are established evidence patterns.
- Existing behavior assertions are retained through migration; workflow-specific methods are not used as differential oracles once scheduled for deletion.
- Rust visibility and exhaustive enum matching are preferred over source scanning; textual absence guards remain appropriate for banning method names and local wrappers.

### Integration Points
- `crates/pi-coding-agent/tests/delegation_execution.rs` contains remaining public approval/rejection compatibility callers.
- `crates/pi-coding-agent/tests/agent_profile_session.rs` contains a remaining public profile-mutation compatibility caller.
- `crates/pi-coding-agent/tests/public_api.rs` compiles and calls the three public workflow methods selected for removal.
- `crates/pi-coding-agent/src/coding_session/mod.rs` contains owner-test callers and broad method definitions for profile, delegation, plugin, fork, and branch-summary workflows.
- `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` already inventories `set_default_agent_profile_id`, delegation decisions, plugin methods, fork, and branch-summary compatibility methods.

</code_context>

<specifics>
## Specific Ideas

- Treat every deletion group as a small migration proof: zero callers, focused behavior gate, boundary guard, crate check, then definition removal.
- Outcome helpers are allowed only as pure exhaustive extractors; an operation-running helper would recreate the compatibility surface and is forbidden.
- The three old public profile/delegation entry points are deliberately removed without shims once first-party callers migrate.

</specifics>

<deferred>
## Deferred Ideas

- Recursive and parser-complete adapter/source boundary hardening and final workspace closure audits - Phase 5.
- Typed `ProductEvent` payload convergence and compatibility event-subscription deletion - Stage 10.

</deferred>

---

*Phase: 4-Test Convergence and Compatibility Deletion*
*Context gathered: 2026-07-13*
