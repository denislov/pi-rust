# Phase 2: Canonical Facade Correctness - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-07-11
**Phase:** 2-Canonical Facade Correctness
**Areas discussed:** Stable API completeness rules, Dispatcher and projection exhaustiveness proof, Durable mutation Phase 2 verification boundary

---

## Stable API Completeness Rules

| Decision | Options considered | Selected |
|----------|--------------------|----------|
| Support-type import policy | Direct `api` re-exports; accessible through other modules; new facade wrappers; agent discretion | Direct `api` re-exports |
| Crate-root compatibility exports | Retain but stop first-party use; delete duplicate exports now; allow both paths until Phase 4; agent discretion | Retain but stop first-party use |
| Stable API scope | Operation plus required session contracts; operation closure only; all public session capabilities; agent discretion | Operation plus required session contracts |
| Private-boundary proof | Rust visibility/API tests first; source scanning first; positive tests only; agent discretion | Rust visibility/API tests first |

**User's choices:** All four recommended options were selected.
**Notes:** Compatibility deletion remains in Phase 4. Existing create/open, snapshot/query, subscription, and control contracts remain public without being remodeled as operations. Source guards are fallback enforcement only.

---

## Dispatcher And Projection Exhaustiveness Proof

| Decision | Options considered | Selected |
|----------|--------------------|----------|
| Coverage granularity | Per-variant matrix; representative per dispatch mode; per-variant conversion with sampled dispatch; agent discretion | Per-variant matrix |
| Future-variant enforcement | Exhaustive matches plus independent inventory; declaration macro; exhaustive matches only; source-count guard | Exhaustive matches plus independent inventory |
| Test ownership | Owner and public integration layers; owner tests primarily; public integration tests primarily; new architecture suite | Owner and public integration layers |
| Actual dispatcher proof | Metadata plus path-specific behavior; test-only instrumentation; metadata only; full E2E for every variant | Metadata plus path-specific behavior |

**User's choices:** All four recommended options were selected.
**Notes:** The independent inventory must not derive expected dispatch or outcome values from implementation metadata. No production test hooks or compatibility facade may be added.

---

## Durable Mutation Phase 2 Verification Boundary

| Decision | Options considered | Selected |
|----------|--------------------|----------|
| Focused operation set | All FACADE-05 operations; persistent navigation only; confirmed gaps only; all 15 operations | All FACADE-05 operations |
| Required invariants | Shared checklist plus operation-specific assertions; final state/outcome only; old-method equivalence; reuse tests without a standard | Shared checklist plus operation-specific assertions |
| Compatibility evidence | Canonical path with migrated assertions; differential dual-path tests; old full tests plus canonical smoke; delete old tests now | Canonical path with migrated assertions |
| Failure-path depth | Deterministic key commit boundaries; typed errors only; every failure point; existing scenarios only | Deterministic key commit boundaries |

**User's choices:** All four recommended options were selected.
**Notes:** The shared checklist covers public outcome, state, error fidelity, replay, events, sequence continuity, and applicable `PartialCommit`. Torn writes, interrupted manifests, power loss, and filesystem atomicity remain outside this phase.

---

## the agent's Discretion

- Exact contract-matrix file and helper organization.
- Exact negative compile/API test technique supported by the existing harness.
- Fixture reuse and test naming within the locked owner/integration boundaries.

## Deferred Ideas

None.
