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

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness via Cargo 1.96.0 plus a phase-local Bash artifact validator |
| **Config file** | Workspace `Cargo.toml` and `crates/pi-coding-agent/Cargo.toml` |
| **Quick run command** | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh` |
| **Full suite command** | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh && cargo test -p pi-coding-agent --test public_api canonical_operation_runtime_variants_are_public -- --nocapture && cargo test -p pi-coding-agent --test api_boundary_guards coding_session_run_is_the_canonical_operation_dispatcher -- --nocapture && cargo test -p pi-coding-agent --test product_runtime_boundary_guards -- --nocapture && git diff --check` |
| **Estimated runtime** | ~60 seconds |

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
| 01-W0-01 | TBD | 0 | AUDIT-01, AUDIT-02, AUDIT-03 | N/A | Validator does not read secrets or mutate production/runtime state | artifact/schema | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh` | No - W0 | pending |
| 01-AUDIT-01 | TBD | TBD | AUDIT-01 | N/A | Evidence collection remains read-only | artifact/schema + focused tests | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh` | No - W0 | pending |
| 01-AUDIT-02 | TBD | TBD | AUDIT-02 | N/A | Caller inventory records paths without executing external services | artifact/schema + live source comparison | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh` | No - W0 | pending |
| 01-AUDIT-03 | TBD | TBD | AUDIT-03 | N/A | Stage 10 and secret-bearing runtime state remain excluded | artifact/schema | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh` | No - W0 | pending |

*Status values: pending, green, red, flaky.*

---

## Wave 0 Requirements

- [ ] `.planning/phases/01-evidence-based-baseline/validate-audit.sh` - validate all 15 exact public variants appear once, required matrix fields and sections exist, taxonomy values are allowed, evidence/finding IDs are unique, AUDIT requirement IDs are present, findings route only to Phase 2-5, and gaps/blockers use explicit `none` when empty
- [ ] `01-AUDIT.md` command-ledger template - record evidence ID, exact command, date, exit status, executed-test count, and result for every focused test or source scan
- [ ] Ensure every Cargo test filter used by the audit executes at least one test; zero-test filters fail validation

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
