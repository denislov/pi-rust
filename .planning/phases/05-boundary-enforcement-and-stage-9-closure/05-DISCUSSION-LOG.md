# Phase 5: Boundary Enforcement and Stage 9 Closure - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md; this log preserves the alternatives considered.

**Date:** 2026-07-13
**Phase:** 5-Boundary Enforcement and Stage 9 Closure
**Areas discussed:** Recursive Adapter Scan Boundary, Same-Name Calls and Exception Policy, Stable API Negative Proof Strength, Stage 9 Closure Evidence and Documentation

---

## Recursive Adapter Scan Boundary

### Automatic Source Coverage

| Option | Description | Selected |
|--------|-------------|----------|
| Adapter ownership roots | Recursively scan declared adapter directories and explicitly register single-file entry points. | Yes |
| Entire crate source | Scan every Rust source file below `pi-coding-agent/src`. | |
| Current per-adapter lists | Keep fixed JSON/print files and current RPC/interactive directory lists. | |
| Implementer choice | Let planning select the boundary under the phase constraints. | |

**User's choice:** Adapter ownership roots.
**Notes:** New source below a registered adapter root must be included automatically without broad scanning of runtime internals.

### Test-Code Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Production only | Skip `#[cfg(test)]` items/modules; use the Phase 4 global absence ledger for tests. | Yes |
| Production and tests | Apply the same adapter rule to both source and test code. | |
| Split policy | Ban old calls in tests but check deprecation suppression only in production. | |

**User's choice:** Production only.
**Notes:** Phase 5 adapter guards remain production-boundary checks rather than duplicating the existing test-caller ledger.

### New Adapter Roots

| Option | Description | Selected |
|--------|-------------|----------|
| Central inventory | Register every first-party adapter and verify exactly one ownership root. | Yes |
| Source inference | Infer adapters from operation imports and process/UI/protocol behavior. | |
| Code-review maintenance | Scan only existing roots and rely on manual updates for new top-level adapters. | |

**User's choice:** Central inventory.
**Notes:** Inventory completeness is itself an executable boundary assertion.

### Discovery Failures

| Option | Description | Selected |
|--------|-------------|----------|
| Fail closed | Fail for missing roots, empty scans, read failures, duplicate ownership, or unowned known adapters. | Yes |
| Partial failure | Fail for missing roots but skip other discovery problems. | |
| Best effort | Check only files discovered and read successfully. | |

**User's choice:** Fail closed.
**Notes:** Diagnostics must identify the affected path and reason; an unsuccessful scan cannot produce a passing result.

---

## Same-Name Calls and Exception Policy

### Legitimate Same-Name Calls

| Option | Description | Selected |
|--------|-------------|----------|
| Receiver-aware exceptions | Match ownership scope, receiver shape, method, and reason in one central table. | Yes |
| String exclusions | Continue using `line.contains(...)` exclusions. | |
| No same-name calls | Rename every legitimate internal method sharing a deleted method name. | |
| Implementer choice | Let planning choose a fail-closed mechanism. | |

**User's choice:** Receiver-aware exceptions.
**Notes:** Unknown receivers fail closed; method-name substring matching is not sufficient authority.

### Exception Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Fixed scope and count | Limit path/root, receiver, method, and exact or maximum occurrences. | Yes |
| Receiver only | Allow a matching receiver anywhere and without a count limit. | |
| File only | Permit a method name throughout selected files without receiver analysis. | |

**User's choice:** Fixed scope and count.
**Notes:** Moving, copying, or increasing an allowed call forces explicit review.

### Formatting Independence

| Option | Description | Selected |
|--------|-------------|----------|
| Ordinary Rust syntax coverage | Recognize calls across lines, chains, parentheses, comments, and rustfmt changes; ignore non-code text. | Yes |
| Rustfmt-normalized calls | Assume normal single-line `.method(` output after formatting. | |
| Current shapes only | Cover only syntax currently present in the repository. | |

**User's choice:** Ordinary Rust syntax coverage.
**Notes:** Positive and negative fixtures must cover real calls, legal receivers, formatting variants, strings, comments, doc comments, and test-only code.

### Inline Suppression

| Option | Description | Selected |
|--------|-------------|----------|
| No inline suppression | The central exception table is the only permitted exception mechanism. | Yes |
| Line-level suppression | Allow adjacent reason comments to bypass a check. | |
| File-level suppression | Allow selected files to opt out. | |

**User's choice:** No inline suppression.
**Notes:** Production source cannot use comments, custom attributes, or file-level directives to bypass the guard.

---

## Stable API Negative Proof Strength

### External Compile Proof

| Option | Description | Selected |
|--------|-------------|----------|
| Compile-pass and compile-fail | Test both stable facade usability and internal-contract inaccessibility as an external crate. | Yes |
| Positive compile plus source scan | Keep external positive tests and source-based negative checks. | |
| Visibility only | Rely on ordinary workspace compilation and Rust visibility. | |

**User's choice:** Compile-pass and compile-fail.
**Notes:** Source inspection alone is not the final privacy proof.

### Negative Fixture Grouping

| Option | Description | Selected |
|--------|-------------|----------|
| Boundary categories | Separate operation/dispatch, services, plugin internals, and Flow contracts. | Yes |
| One combined fixture | Import every forbidden type in one compile-fail case. | |
| One fixture per identifier | Create an individual fixture for every prohibited name. | |

**User's choice:** Boundary categories.
**Notes:** Assert the nature of failure without locking complete rustc diagnostic wording.

### Access Paths

| Option | Description | Selected |
|--------|-------------|----------|
| Every external path | Attempt stable `api`, crate root, and migration-private hidden public modules. | Yes |
| Stable API only | Test only `pi_coding_agent::api`. | |
| API and crate root | Exclude hidden migration modules from negative proof. | |

**User's choice:** Every external path.
**Notes:** Internal contracts must be actually inaccessible, not merely absent from the curated facade documentation.

### Positive Contract Drift

| Option | Description | Selected |
|--------|-------------|----------|
| Independent explicit inventory | Import and use every operation variant, outcome family, and required support type. | Yes |
| Representative operations | Compile only a small set of common operations. | |
| Glob import | Use `pi_coding_agent::api::*` and construct common values. | |

**User's choice:** Independent explicit inventory.
**Notes:** Expected API closure must not be generated from production exports.

---

## Stage 9 Closure Evidence and Documentation

### Authoritative Closure Artifact

| Option | Description | Selected |
|--------|-------------|----------|
| Formal closure report | Create one authoritative Stage 9 evidence and handoff report. | Yes |
| Existing docs only | Update project, roadmap, design, and state without a dedicated report. | |
| GSD verification artifacts only | Rely on verification and plan summaries. | |

**User's choice:** Formal closure report.
**Notes:** Current documents should link to the report rather than duplicating competing closure claims.

### Verification Evidence Depth

| Option | Description | Selected |
|--------|-------------|----------|
| Structured reproducible summary | Record exact commands, time, status, key counts, audit scope, and verified tree identity. | Yes |
| Pass/fail checklist | Record only whether each required gate passed. | |
| Full raw output | Embed complete Cargo and audit output. | |

**User's choice:** Structured reproducible summary.
**Notes:** Keep the report high-signal; do not embed full Cargo logs.

### Historical Plan Authority

| Option | Description | Selected |
|--------|-------------|----------|
| Preserve and mark superseded | Keep the old plan body, add status and closure link, and update current authority docs. | Yes |
| Rewrite old checklist | Make the historical plan match final execution. | |
| Move to archive | Relocate the plan and retain only a closure-report reference. | |

**User's choice:** Preserve and mark superseded.
**Notes:** Historical disagreement remains visible for audit purposes; the old checklist is not completion authority.

### Stage 10 Handoff

| Option | Description | Selected |
|--------|-------------|----------|
| Bounded inventory | List remaining subscriptions, untyped payload families, deferral reasons, source locations, and behavior constraints. | Yes |
| One-sentence note | Name Stage 10 without an inventory. | |
| Preliminary implementation plan | Include sequencing and tasks for Stage 10. | |

**User's choice:** Bounded inventory.
**Notes:** Phase 5 records the handoff but does not plan or implement Stage 10.

---

## The Agent's Discretion

- Exact scanner/parser implementation and internal organization.
- Exact external compile-test harness and fixture layout.
- Adapter inventory and exception-table data structures and diagnostic wording.
- Closure report filename and section layout.
- Focused verification command ordering and batching, while retaining every mandatory gate.

## Deferred Ideas

- Typed `ProductEvent` payload convergence and compatibility event-subscription deletion remain Stage 10 work.
