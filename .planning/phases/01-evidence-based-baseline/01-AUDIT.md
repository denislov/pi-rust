Audit Status: draft

<!-- 01-AUDIT.md - Phase 1 Evidence-Based Baseline audit artifact.
     This is a STRUCTURAL SCAFFOLD created by Plan 01-01.
     Evidence collection, assessment, and findings are populated by Plan 01-02.
     Final validation and traceability closure happen in Plan 01-03.
     The validate-audit.sh script enforces this contract mechanically. -->

# Phase 1 Audit: Canonical Operation Runtime Convergence

**Audit Status:** draft
**Created:** 2026-07-11
**Owner Plan:** 01-01 (scaffold), 01-02 (evidence), 01-03 (final)
**Authority:** `.planning/PROJECT.md`, `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`

---

## Audit Contract

This artifact is the single source of truth for the Stage 9 completion state. It is
machine-scannable and validated by `validate-audit.sh`. The taxonomies below are locked
by `01-CONTEXT.md` decisions D-10 and D-12 through D-16. No prose synonym may replace a
locked value.

### Locked Taxonomies

| Field | Allowed Values | Source |
|-------|---------------|--------|
| implementation | `complete` \| `partial` \| `missing` \| `not_applicable` | D-12 |
| verification | `passed` \| `failed` \| `blocked` \| `not_run` | D-10 |
| disposition | `active` \| `obsolete` \| `deferred_stage_10` \| `retained_compatibility` | D-13 |
| confidence | `high` \| `medium` \| `low` | D-14 |
| finding_obligation | `blocking` \| `required` \| `hardening` \| `informational` | D-16 |
| evidence_gaps | `none` \| comma-separated gap descriptions | D-15 |
| blockers | `none` \| comma-separated blocker descriptions | D-15 |

### Evidence Rules

- **D-06:** Every completed behavior claim requires current source evidence plus a focused
  behavior or public API test.
- **D-07:** Boundary claims additionally require Rust visibility/type evidence, a compile/API
  test, or a precise source guard when Rust cannot directly express the boundary.
- **D-14:** High confidence requires aligned source, focused tests, and applicable boundary
  evidence. Medium means source is clear but verification or history is incomplete. Low means
  the conclusion is indirect and needs downstream verification.
- **D-15:** Evidence gaps reduce confidence but permit an audit conclusion. Blockers prevent a
  reliable conclusion or downstream planning. Only blockers prevent Phase 1 verification passing.

### Scope Rules

- **D-05:** Every gap records exact evidence, affected requirement IDs, target Phase 2-5, and
  dependencies. Phase 1 does not write implementation tasks.
- **D-20:** Phase 1 does not modify the old implementation plan or `docs/TODO.md`.
- Findings route to Phase 2 (facade), Phase 3 (adapters), Phase 4 (tests/deletion), or
  Phase 5 (boundary/closure). Stage 10 work is recorded as `deferred_stage_10`.

### Known Requirement ID Prefixes

`AUDIT-*`, `FACADE-*`, `ADAPT-*`, `RPC-*`, `INTER-*`, `TEST-*`, `DELETE-*`, `GUARD-*`, `CLOSE-*`

---

## Authority Order

Per D-17, authorities are layered. When authorities conflict, record an explicit finding (D-18)
rather than silently choosing one source.

1. **Current source, tests, and guards** - define what exists in the live tree.
2. **Current `PROJECT.md`, `REQUIREMENTS.md`, `ROADMAP.md`** - define milestone requirements.
3. **Stage 9 design and reference architecture** - define target constraints.
4. **Historical implementation plan and `docs/TODO.md`** - historical execution clues only.

Per D-19, Git analysis focuses on Stage 9 commits, commits named by the old plan, changes around
2026-07-10, and current broad-method or adapter history. Per D-09, the current tree is
authoritative for what exists; Git corroborates timing, rationale, and scope.

---

## Evidence Index

Evidence IDs are stable references used by the Operation Matrix, Findings, and Command Ledger.
In this draft scaffold, no evidence has been collected yet. Plan 01-02 will populate evidence IDs.

### Evidence ID Namespaces

| Prefix | Category | Description |
|--------|----------|-------------|
| SRC-* | Source | File and symbol references from the live tree |
| TEST-* | Test | Focused behavior, public API, and integration test references |
| GUARD-* | Boundary Guard | Source-scanning or compile/API boundary guard references |
| GIT-* | Git History | Commit, diff, or blame references for corroboration |

### Registered Evidence IDs

| Evidence ID | Category | Reference | Collected In |
|-------------|----------|-----------|--------------|
| _(none registered yet - populated by Plan 01-02)_ | - | - | - |

---

## Command Ledger

Every focused test or source scan recorded as audit evidence must appear here with exact command,
date, exit status, executed-test count, result, and evidence ID. The validator rejects Cargo
ledger rows whose executed-test count is not a positive integer. Zero-test output is a validation
failure.

| Evidence ID | Command | Date | Exit Status | Test Count | Result |
|-------------|---------|------|-------------|------------|--------|
| _(none recorded yet - populated by Plan 01-02)_ | - | - | - | - | - |

---

## Operation Matrix

One row per public `CodingAgentOperation` variant. Structural columns are seeded from live source
(`public_operation.rs`, `operation.rs`). Assessment columns are populated by Plan 01-02.

**Column key:**
- `variant` - exact `CodingAgentOperation` spelling
- `internal` - exact `Operation` variant mapping
- `kind` - `static` if `static_kind` is `Some`, `dynamic` if `None`
- `origin` - `OperationOrigin` from metadata
- `class` - `OperationClass` from metadata
- `dispatch` - `OperationDispatchMode` from metadata
- `outcome` - exact `CodingAgentOperationOutcome` variant
- `prod_callers` - production caller files/symbols (populated by 01-02)
- `test_callers` - test caller files/symbols (populated by 01-02)
- `impl` - implementation status (D-12 taxonomy, populated by 01-02)
- `verify` - verification status (D-10 taxonomy, populated by 01-02)
- `disp` - disposition (D-13 taxonomy, populated by 01-02)
- `conf` - confidence level (D-14 taxonomy, populated by 01-02)
- `evidence` - comma-separated evidence IDs (populated by 01-02)
- `gaps` - evidence gaps; `none` when empty (D-15)
- `blockers` - blockers; `none` when empty (D-15)

| variant | internal | kind | origin | class | dispatch | outcome | prod_callers | test_callers | impl | verify | disp | conf | evidence | gaps | blockers |
|---------|----------|------|--------|-------|----------|---------|--------------|--------------|------|--------|------|------|----------|------|----------|
| Prompt | Operation::Prompt | static | ClientRoot | SessionWriteRoot | Async | Prompt(PromptTurnOutcome) | | | | | | | | none | none |
| Compact | Operation::ManualCompaction | static | ClientRoot | SessionWriteRoot | Async | Compact(PromptTurnOutcome) | | | | | | | | none | none |
| BranchSummary | Operation::BranchSummary | static | ClientRoot | SessionWriteRoot | Async | BranchSummary(PromptTurnOutcome) | | | | | | | | none | none |
| SelfHealingEdit | Operation::SelfHealingEdit | static | ClientRoot | SessionWriteRoot | Async | SelfHealingEdit(SelfHealingEditOutcome) | | | | | | | | none | none |
| InvokeAgent | Operation::AgentInvocation | static | ClientRoot | NonSessionRoot | Async | AgentInvocation(AgentInvocationOutcome) | | | | | | | | none | none |
| InvokeTeam | Operation::AgentTeam | static | ClientRoot | NonSessionRoot | Async | AgentTeam(AgentTeamOutcome) | | | | | | | | none | none |
| PluginLoad | Operation::PluginLoad | static | ClientRoot | RuntimeWrite | Async | PluginLoad(CodingAgentPluginLoadOutcome) | | | | | | | | none | none |
| PluginCommand | Operation::PluginCommand | static | ClientRoot | NonSessionRoot | SyncReadOnly | PluginCommand(String) | | | | | | | | none | none |
| SetDefaultAgentProfile | Operation::SetDefaultAgentProfile | static | ClientRoot | RuntimeWrite | SyncMutable | DefaultAgentProfileChanged | | | | | | | | none | none |
| ApproveDelegation | Operation::ApproveDelegationConfirmation | dynamic | ClientRoot | NonSessionRoot | Async | DelegationApproved | | | | | | | | none | none |
| RejectDelegation | Operation::RejectDelegationConfirmation | static | ClientRoot | Control | SyncMutable | DelegationRejected | | | | | | | | none | none |
| ForkSession | Operation::ForkSession | static | ClientRoot | SessionWriteRoot | SyncMutable | SessionForked | | | | | | | | none | none |
| SwitchActiveLeaf | Operation::SwitchActiveLeaf | static | ClientRoot | SessionWriteRoot | SyncMutable | ActiveLeafSwitched | | | | | | | | none | none |
| ExportCurrent | Operation::Export(ExportOptions::view()) | static | ClientRoot | ReadOnly | SyncReadOnly | Export(CodingAgentSessionExport) | | | | | | | | none | none |
| ExportCurrentHtml | Operation::Export(ExportOptions::html(path)) | static | ClientRoot | ReadOnly | SyncReadOnly | ExportHtml(PathBuf) | | | | | | | | none | none |

---

## Production Caller Inventory

Production caller inventory is populated by Plan 01-02 from live source scans. The expected
caller roots are:

- `crates/pi-coding-agent/src/protocol/json_mode.rs` - JSON one-shot prompt path
- `crates/pi-coding-agent/src/print_mode.rs` - Print persistent and transient prompt paths
- `crates/pi-coding-agent/src/protocol/rpc/` - Streaming RPC callers and control multiplexing
- `crates/pi-coding-agent/src/interactive/` - Interactive background operations and mutations

| Caller File | Caller Symbol | Operation | Canonical (run) | Compatibility Path | Evidence |
|-------------|---------------|-----------|-----------------|--------------------|---------|
| _(populated by Plan 01-02)_ | - | - | - | - | - |

---

## Test Caller Inventory

Test caller inventory is populated by Plan 01-02 from live source scans. The expected test roots are:

- `crates/pi-coding-agent/tests/public_api.rs` - Stable facade and public operation behavior
- `crates/pi-coding-agent/tests/api_boundary_guards.rs` - Public/private API and dispatcher boundary
- `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` - Adapter call and deprecation guards
- `crates/pi-coding-agent/src/coding_session/mod.rs` (owner tests) - Owner unit tests
- Integration tests under `crates/pi-coding-agent/tests/`

| Test File | Test Name | Operation | Canonical (run) | Compatibility Path | Evidence |
|-----------|-----------|-----------|-----------------|--------------------|---------|
| _(populated by Plan 01-02)_ | - | - | - | - | - |

---

## Compatibility Inventory

Per D-03, broad live-session compatibility methods are tracked separately from the operation matrix.
Per DELETE-04, construction/open/resume, snapshots, queries, subscriptions/control, and static
repository helpers are NOT operation-facade replacements and are excluded unless a finding explains
why. The compatibility methods listed below were identified by research and will be verified and
populated with caller evidence by Plan 01-02.

| Method | Visibility | Deprecation | Matching Operation | Prod Callers | Test Callers | Retention Reason | Deletion Req | Target Phase |
|--------|------------|-------------|--------------------|--------------|--------------|------------------|-------------|--------------|
| export_current_html | pub (deprecated) | #[deprecated] | ExportCurrentHtml | | | | DELETE-01 | Phase 4 |
| export_current | pub (deprecated) | #[deprecated] | ExportCurrent | | | | DELETE-01 | Phase 4 |
| set_default_agent_profile_id | pub (deprecated) | #[deprecated] | SetDefaultAgentProfile | | | | DELETE-01 | Phase 4 |
| approve_delegation_confirmation | pub (deprecated) | #[deprecated] | ApproveDelegation | | | | DELETE-01 | Phase 4 |
| reject_delegation_confirmation | pub (deprecated) | #[deprecated] | RejectDelegation | | | | DELETE-01 | Phase 4 |
| prompt | pub (deprecated) | #[deprecated] | Prompt | | | | DELETE-01 | Phase 4 |
| compact | pub (deprecated) | #[deprecated] | Compact | | | | DELETE-01 | Phase 4 |
| self_healing_edit_with_options | pub (deprecated) | #[deprecated] | SelfHealingEdit | | | | DELETE-01 | Phase 4 |
| invoke_agent | pub (deprecated) | #[deprecated] | InvokeAgent | | | | DELETE-01 | Phase 4 |
| invoke_team | pub (deprecated) | #[deprecated] | InvokeTeam | | | | DELETE-01 | Phase 4 |
| summarize_branch | pub (deprecated) | #[deprecated] | BranchSummary | | | | DELETE-01 | Phase 4 |
| fork_current_session | crate-private | none | ForkSession | | | | DELETE-01 | Phase 4 |
| reload_plugins | crate-private | none | PluginLoad | | | | DELETE-01 | Phase 4 |
| run_plugin_command | crate-private | none | PluginCommand | | | | DELETE-01 | Phase 4 |
| load_plugins | crate-private | none | PluginLoad | | | | DELETE-01 | Phase 4 |

---

## Authority Reconciliation

Per D-18, when authorities conflict, record an explicit finding rather than silently choosing one
source. Reconciliation entries are populated by Plan 01-02 when evidence collection surfaces
conflicts between current source, planning documents, design specs, and historical plan material.

| Conflict ID | Authority A | Authority B | Description | Resolution | Finding Ref |
|-------------|-------------|-------------|-------------|------------|-------------|
| _(populated by Plan 01-02 when conflicts are found)_ | - | - | - | - | - |

---

## Findings

Per D-04 and D-05, findings explain cross-operation gaps, contradictions, risks, and obsolete plan
material. Every finding records affected requirement IDs, target Phase 2-5, dependencies,
obligation, disposition, confidence, evidence gaps, and blockers. Phase 1 does not write
implementation task prose (D-05).

**Finding obligation taxonomy (D-16):** `blocking` \| `required` \| `hardening` \| `informational`
**Disposition taxonomy (D-13):** `active` \| `obsolete` \| `deferred_stage_10` \| `retained_compatibility`
**Confidence taxonomy (D-14):** `high` \| `medium` \| `low`

| ID | Obligation | Disposition | Description | Evidence | Requirements | Target Phase | Dependencies | Confidence | Gaps | Blockers |
|----|-----------|-------------|-------------|----------|--------------|--------------|--------------|-----------|------|---------|
| _(populated by Plan 01-02)_ | - | - | - | - | - | - | - | - | - | - |

---

## Requirement Traceability

Maps audit requirements AUDIT-01 through AUDIT-03 to their evidence and completion status.
In this draft, all requirements are pending. Plan 01-03 performs final traceability closure.

| Requirement | Description | Status | Evidence | Notes |
|-------------|-------------|--------|----------|-------|
| AUDIT-01 | Maintainers can determine the trustworthy Stage 9 completion state from current source, tests, boundary guards, and Git history | pending | | Populated by Plan 01-02, closed by Plan 01-03 |
| AUDIT-02 | The audit identifies each live-session product operation's public variant, internal mapping, dispatch mode, outcome projection, production callers, and test callers | pending | | Populated by Plan 01-02, closed by Plan 01-03 |
| AUDIT-03 | The audit clearly separates completed baseline behavior, actual gaps, obsolete plan content, and Stage 10 scope | pending | | Populated by Plan 01-02, closed by Plan 01-03 |

---

## Validation Summary

Records validation runs against this audit artifact. The validator (`validate-audit.sh`) supports
three modes: `--schema-only` (structural check), `--evidence-only` (evidence completeness check),
and default final mode (full closure check).

| Mode | Date | Result | Notes |
|------|------|--------|-------|
| schema-only | 2026-07-11 | pass | Scaffold created by Plan 01-01 Task 1 |
| evidence-only | - | not_run | Requires Plan 01-02 evidence collection |
| final | - | not_run | Requires Plan 01-03 final closure |
