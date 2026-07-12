# Phase 4: Test Convergence and Compatibility Deletion - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md; this log preserves the alternatives considered.

**Date:** 2026-07-13
**Phase:** 4-Test Convergence and Compatibility Deletion
**Areas discussed:** Test migration boundary, deletion order, public method disposition, test helper ownership

---

## Test Migration Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Public-first | Product workflow behavior tests use `api` plus `run`; private paths only cover contracts the public facade cannot express. | yes |
| Minimal migration | Migrate only direct callers of methods being deleted. | |
| Split by test layer | Require public paths only in integration tests and permit broad methods in owner tests. | |

**User's choice:** Public-first.
**Notes:** Public outcomes become the primary assertion entry, but the user confirmed the project constraint that existing behavior, error, event, replay, and persistence assertions cannot be weakened. Private paths are allowed only when public operations cannot express custom internal options, metadata/dispatcher state, or deterministic fault boundaries.

---

## Deletion Order

| Option | Description | Selected |
|--------|-------------|----------|
| Risk and caller layers | Migrate lower-risk caller groups before persistence/navigation/delegation-sensitive groups. | yes |
| Operation families | Complete Prompt, Profile, Delegation, Plugin, and Navigation families independently. | |
| One bulk deletion | Replace all callers and delete all definitions together. | |

**User's choice:** Risk and caller layers.
**Notes:** Every definition requires zero production and test callers plus focused behavior tests, boundary guards, and crate check before deletion. Existing ledgers become explicit absence checks; recursive/parser-complete hardening remains Phase 5.

---

## Public Method Disposition

| Option | Description | Selected |
|--------|-------------|----------|
| Delete old entry points | Migrate callers to typed operations and remove the three public workflow methods. | yes |
| Keep deprecated shims | Preserve wrappers for an external transition period. | |
| Reclassify as controls | Keep profile/delegation methods outside operation convergence. | |

**User's choice:** Delete old entry points within this milestone.
**Notes:** `set_default_agent_profile_id`, `approve_delegation_confirmation`, and `reject_delegation_confirmation` are removed without deprecated shims after first-party callers migrate. Construction, lifecycle, query, snapshot, subscription, control, and static helpers remain.

---

## Test Helper Ownership

| Option | Description | Selected |
|--------|-------------|----------|
| Ownership-based layering | Private fixtures remain co-located; reusable public fixtures live in integration support. | yes |
| Centralize all helpers | Move owner fixtures and fault injection into integration support. | |
| Keep all helpers local | Duplicate setup rather than share integration fixtures. | |

**User's choice:** Ownership-based layering with narrow test-only helpers.
**Notes:** Shared helpers may only extract exhaustive typed outcomes. They cannot choose or run operations, create/own sessions, hide errors, expose services, or recreate the broad facade. Fault injection remains specialized, crate-private, and directly `cfg(test)`.

## Agent's Discretion

- Exact risk-group composition and focused Cargo commands.
- Whether a pure typed outcome extractor removes enough duplication to justify sharing.
- Test names and local fixture organization within the locked ownership boundaries.

## Deferred Ideas

- Recursive/parser-complete guard hardening and final Stage 9 closure remain Phase 5.
- Typed ProductEvent and compatibility subscription convergence remain Stage 10.
