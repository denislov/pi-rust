---
phase: 1
slug: evidence-based-baseline
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-11
---

# Phase 1 - Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
>
> **Wave 0 owner:** Task `01-01-01` creates the audit validator and command-ledger contract.
> **Nyquist status:** Not compliant until Plan `01-03` Task `01-03-02` runs the final gate.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness via Cargo 1.96.0 plus a phase-local Bash artifact validator |
| **Config file** | Workspace `Cargo.toml` and `crates/pi-coding-agent/Cargo.toml` |
| **Quick run command** | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh` |
| **Full suite command** | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh && cargo test -p pi-coding-agent --test public_api canonical_operation_runtime_variants_are_public -- --nocapture && cargo test -p pi-coding-agent --test api_boundary_guards coding_session_run_is_the_canonical_operation_dispatcher -- --nocapture && cargo test -p pi-coding-agent --test product_runtime_boundary_guards -- --nocapture && git diff --check` |
| **Estimated runtime** | ~60 seconds |

### Non-Zero-Test Cargo Evidence Rule

Every recorded Cargo command must report at least one executed test. Zero-test output is a
validation failure, not a pass. The three focused Cargo commands below are the only approved
evidence filters for this phase; each must execute at least one test:

1. `cargo test -p pi-coding-agent --test public_api canonical_operation_runtime_variants_are_public -- --nocapture`
2. `cargo test -p pi-coding-agent --test api_boundary_guards coding_session_run_is_the_canonical_operation_dispatcher -- --nocapture`
3. `cargo test -p pi-coding-agent --test product_runtime_boundary_guards -- --nocapture`

The `validate-audit.sh` evidence mode rejects any Cargo ledger row whose executed-test count
is not a positive integer.

---

## Sampling Rate

- **After every task commit:** Run `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh` plus the exact focused Cargo command for evidence added or changed
- **After every plan wave:** Run the full suite command above and rerun all caller scans recorded in the audit command ledger
- **Before `$gsd-verify-work`:** The artifact validator, focused Cargo evidence, and `git diff --check` must be green; no audit blocker may remain
- **Max feedback latency:** 90 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01-01 | 0 | AUDIT-01, AUDIT-02, AUDIT-03 | T-01-01, T-01-02, T-01-05 | Validator does not read secrets, mutate production/runtime state, or execute ledger content; no external package dependency | artifact/schema | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh --schema-only` | Yes | pending |
| 01-01-02 | 01-01 | 1 | AUDIT-01, AUDIT-02, AUDIT-03 | N/A | Validation map names concrete plan/wave/task identifiers without premature compliance claims | artifact/schema | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh --schema-only` | Yes | pending |
| 01-02-01 | 01-02 | 2 | AUDIT-01, AUDIT-02 | T-01-02, T-01-04 | Evidence collection remains read-only; command ledger records exact command, date, exit status, and executed-test count | artifact/schema + focused tests | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh --evidence-only && cargo test -p pi-coding-agent --test public_api canonical_operation_runtime_variants_are_public -- --nocapture && cargo test -p pi-coding-agent --test api_boundary_guards coding_session_run_is_the_canonical_operation_dispatcher -- --nocapture` | No - Wave 2 | pending |
| 01-02-02 | 01-02 | 2 | AUDIT-02 | T-01-02 | Caller inventory records file/symbol paths without executing external services or printing secrets | artifact/schema + live source comparison | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh --evidence-only` | No - Wave 2 | pending |
| 01-02-03 | 01-02 | 2 | AUDIT-01 | T-01-04 | History reconciliation uses focused Stage 9 Git analysis only; records provenance for every cited commit | artifact/schema | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh --evidence-only` | No - Wave 2 | pending |
| 01-03-01 | 01-03 | 3 | AUDIT-03 | T-01-03 | Findings use locked taxonomy; Stage 10 scope excluded from Phase 2-5 routing; no implementation task prose | artifact/schema | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh --evidence-only` | No - Wave 3 | pending |
| 01-03-02 | 01-03 | 3 | AUDIT-01, AUDIT-02, AUDIT-03 | T-01-03 | Final gate enforces Audit Status: final, zero blockers, complete traceability, and all finding categories | artifact/schema + focused tests | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh && cargo test -p pi-coding-agent --test public_api canonical_operation_runtime_variants_are_public -- --nocapture && cargo test -p pi-coding-agent --test api_boundary_guards coding_session_run_is_the_canonical_operation_dispatcher -- --nocapture && cargo test -p pi-coding-agent --test product_runtime_boundary_guards -- --nocapture && git diff --check` | No - Wave 3 | pending |

*Status values: pending, green, red, flaky.*

---

## Wave 0 Requirements

Wave 0 is owned by task `01-01-01` (Plan 01-01, Task 1). It creates both the validator and the
command-ledger contract before any evidence collection begins.

- [ ] `.planning/phases/01-evidence-based-baseline/validate-audit.sh` - validate all 15 exact public variants appear once, required matrix fields and sections exist, taxonomy values are allowed, evidence/finding IDs are unique, AUDIT requirement IDs are present, findings route only to Phase 2-5, and gaps/blockers use explicit `none` when empty
- [ ] `01-AUDIT.md` command-ledger template - record evidence ID, exact command, date, exit status, executed-test count, and result for every focused test or source scan
- [ ] Ensure every Cargo test filter used by the audit executes at least one test; zero-test output is a validation failure

No framework installation or production test fixture is required.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Authority conflicts are explained rather than silently resolved | AUDIT-01, AUDIT-03 | The semantic quality of conflict findings cannot be fully inferred from Markdown structure | Compare each conflict finding against its cited live-tree evidence, current planning authority, design reference, and historical-plan claim; confirm the selected disposition follows `01-CONTEXT.md` D-17 through D-20 |

---

## Validation Sign-Off

- [ ] All tasks have automated verification or explicit Wave 0 dependencies
- [ ] Sampling continuity: no three consecutive tasks without automated verification
- [ ] Wave 0 creates the audit validator and command-ledger contract
- [ ] No watch-mode flags are used
- [ ] Feedback latency remains below 90 seconds
- [ ] Every Cargo filter records a non-zero executed-test count
- [ ] `nyquist_compliant: true` is set in frontmatter after the final plan map is populated

**Approval:** pending
