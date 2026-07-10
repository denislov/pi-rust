# Phase 1: Evidence-Based Baseline - Context

**Gathered:** 2026-07-11
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 1 produces a trustworthy, source-backed Stage 9 baseline audit. It inventories every public live-session operation, its internal runtime path, outcomes, production and test callers, compatibility surfaces, evidence quality, and actual gaps. It does not migrate adapters, change runtime behavior, delete compatibility methods, or update Stage 9 closure documentation; those actions belong to Phases 2-5.

</domain>

<decisions>
## Implementation Decisions

### Audit Artifact Structure
- **D-01:** The formal Phase 1 deliverable is `.planning/phases/01-evidence-based-baseline/01-AUDIT.md`.
- **D-02:** The audit uses an exhaustive matrix with one row per public `CodingAgentOperation` variant.
- **D-03:** Broad live-session compatibility methods are tracked in a separate compatibility inventory rather than mixed into operation rows.
- **D-04:** A findings report accompanies the matrix to explain cross-operation gaps, contradictions, risks, and obsolete plan material.
- **D-05:** Every gap records exact evidence, affected requirement IDs, its target phase from Phase 2 through Phase 5, and relevant dependencies. Phase 1 does not write implementation tasks.

### Completion Evidence Threshold
- **D-06:** Every completed behavior claim requires current source evidence plus a focused behavior or public API test.
- **D-07:** Boundary claims additionally require Rust visibility/type evidence, a compile/API test, or a precise source guard when Rust cannot directly express the boundary.
- **D-08:** Full workspace verification is a Phase 5 closure gate, not a prerequisite for marking every individual operation implementation present in the Phase 1 inventory.
- **D-09:** The current tree is authoritative for what exists. Git history corroborates timing, rationale, scope, and old checklist credibility, but a missing clean commit does not invalidate current source evidence.
- **D-10:** Record implementation and verification separately. Verification uses `passed`, `failed`, `blocked`, or `not_run`; final completion requires both axes to satisfy the applicable evidence threshold.
- **D-11:** Source-scanning guards are sufficient only when the intended boundary cannot be expressed with Rust visibility, types, or compile/API tests. Replaceable textual guards become Phase 5 hardening findings rather than false implementation gaps.

### Status And Confidence Taxonomy
- **D-12:** `implementation` uses `complete`, `partial`, `missing`, or `not_applicable`.
- **D-13:** `disposition` uses `active`, `obsolete`, `deferred_stage_10`, or `retained_compatibility`.
- **D-14:** Every item records `confidence: high | medium | low` using consistent evidence rules. High requires aligned source, focused tests, and applicable boundary evidence; medium means source is clear but verification or history is incomplete; low means the conclusion is indirect and needs downstream verification.
- **D-15:** `evidence_gaps` reduce confidence but still permit an audit conclusion. `blockers` prevent a reliable conclusion or downstream planning; only blockers prevent Phase 1 verification from passing.
- **D-16:** Finding obligation uses `blocking`, `required`, `hardening`, or `informational`. Required findings map to Stage 9 phases; hardening findings normally map to Phase 5.

### History And Documentation Handling
- **D-17:** Apply layered authority. Current source, tests, and guards define what exists; current `PROJECT.md`, `REQUIREMENTS.md`, and `ROADMAP.md` define milestone requirements; Stage 9 design and reference architecture define target constraints; the old implementation plan and `docs/TODO.md` are historical execution clues.
- **D-18:** When authorities conflict, record an explicit finding rather than silently choosing one source.
- **D-19:** Git analysis focuses on Stage 9 commits, commits named by the old plan, changes around 2026-07-10, and current broad-method or adapter history. Use deeper `git show`, `git log -S/-G`, or blame only when evidence conflicts.
- **D-20:** Phase 1 does not modify the old implementation plan or `docs/TODO.md`. The audit records obsolete checkboxes, contradictions, and recommended update locations; Phase 5 updates closure documentation after final verification.
- **D-21:** The mandatory canonical reference set is fixed below. Stage 8 plans, additional documents, and specific commits are loaded conditionally when the core references or evidence conflicts require them.

### Agent Discretion
- Choose the exact Markdown column ordering, concise evidence notation, and supporting command appendix layout as long as the required fields and taxonomies above remain explicit and machine-scannable.
- Split large caller or compatibility inventories into supporting tables within `01-AUDIT.md` when that improves readability without creating a second source of truth.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Current Milestone Authority
- `.planning/PROJECT.md` - Defines the Stage 9 goal, constraints, validated baseline, active scope, and exclusions.
- `.planning/REQUIREMENTS.md` - Defines AUDIT-01 through AUDIT-03 and the downstream requirements that audit findings must target.
- `.planning/ROADMAP.md` - Fixes the five-phase boundary and keeps implementation, adapter migration, deletion, and closure outside Phase 1.

### Stage 9 Architecture And History
- `docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md` - Historical implementation plan, expected operation inventory, adapter/test migration targets, commands, and checklist evidence to verify rather than trust.
- `docs/superpowers/specs/2026-07-10-canonical-operation-runtime-convergence-design.md` - Defines the intended canonical operation convergence contract and non-goals.
- `docs/superpowers/specs/2026-07-07-operation-runtime-reference-architecture.md` - Defines the broader runtime ownership, dispatch, service, event, and persistence architecture.
- `docs/TODO.md` - Historical Stage 9 tracking and the documentation that Phase 5 will update after final verification.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `.codegraph/` and `codegraph explore` - Primary mechanism for locating current operation symbols, callers, and dynamic dispatch paths before source scanning.
- `crates/pi-coding-agent/tests/public_api.rs` - Existing stable facade and public operation behavior evidence.
- `crates/pi-coding-agent/tests/api_boundary_guards.rs` - Existing public/private API and canonical dispatcher boundary evidence.
- `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` - Existing first-party adapter call and deprecation-suppression evidence.
- `.planning/codebase/TESTING.md` - Catalog of deterministic test conventions, focused suite locations, boundary guard patterns, and verification commands.
- `.planning/codebase/STRUCTURE.md` - Ownership map for the product runtime, adapters, tests, docs, and session internals.
- `.planning/codebase/CONCERNS.md` - Current known concern that operation convergence remains incomplete and identifies high-risk interactive and session areas.

### Established Patterns
- `CodingAgentSession::run` currently converts public operations, selects dispatch from operation metadata, and projects internal outcomes through `CodingAgentOperationOutcome::from_internal`.
- Tests use deterministic faux providers, tempfile-backed session fixtures, Rust integration tests, and source-scanning boundary guards rather than live providers.
- Public compatibility is evaluated through `pi_coding_agent::api`; private runtime contracts remain crate-owned.
- Durable navigation and mutation claims require evidence for session replay, event continuity, operation IDs, and `PartialCommit`, not only compile-level operation variants.

### Integration Points
- `crates/pi-coding-agent/src/coding_session/public_operation.rs` - Public-to-internal operation conversion and internal-to-public outcome projection.
- `crates/pi-coding-agent/src/coding_session/operation.rs` - Internal variants, metadata, dispatch modes, and outcomes.
- `crates/pi-coding-agent/src/coding_session/mod.rs` - Canonical `run`, dispatcher implementations, broad compatibility methods, owner tests, and session transitions.
- `crates/pi-coding-agent/src/protocol/json_mode.rs` and `crates/pi-coding-agent/src/print_mode.rs` - One-shot production callers.
- `crates/pi-coding-agent/src/protocol/rpc/` - Streaming RPC callers, control multiplexing, and command mutations.
- `crates/pi-coding-agent/src/interactive/` - Interactive background operations, mutations, navigation, event subscriptions, and projection refresh.
- `crates/pi-coding-agent/tests/` - Public API, adapter, owner, integration, and boundary evidence used by the audit.

</code_context>

<specifics>
## Specific Ideas

- The audit must be useful both as a completeness ledger and as direct input to planners. The operation matrix prevents omissions; the findings report explains system-level implications.
- A capability can be implemented but not yet verified. The audit must preserve that distinction instead of collapsing every uncertainty into `partial`.
- Retained compatibility can be intentional and still complete for the current tree, while remaining a required deletion finding for a later phase.
- The old plan remains valuable for expected tests, commands, and commit boundaries, but its checked boxes are never completion evidence by themselves.

</specifics>

<deferred>
## Deferred Ideas

None - discussion stayed within the Phase 1 audit scope.

</deferred>

---

*Phase: 1-Evidence-Based Baseline*
*Context gathered: 2026-07-11*
