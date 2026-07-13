---
phase: 01-evidence-based-baseline
verified: 2026-07-11T01:45:35Z
status: passed
score: 3/3 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 1: Evidence-Based Baseline Verification Report

**Phase Goal:** Maintainers have a trustworthy, source-backed statement of what Stage 9 already delivers and what remains to be implemented.
**Verified:** 2026-07-11T01:45:35Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

This is a documentation-only phase. The deliverables are three artifacts: `01-AUDIT.md`
(the audit document), `validate-audit.sh` (a POSIX shell validator), and `01-VALIDATION.md`
(a Nyquist-compliant validation map). No production Rust code was modified. Goal-backward
verification confirms the audit accurately reflects the current codebase, evidence references
point to real files/symbols, the validator passes in all three modes, and all requirement IDs
are traced.

### Observable Truths

Roadmap Success Criteria are the non-negotiable contract. Plan frontmatter truths (12 across
3 plans) are verified as supporting evidence under each SC.

| # | Truth (Roadmap SC) | Status | Evidence |
| --- | --- | --- | --- |
| 1 | Maintainers can inspect one current-state audit that reconciles source, tests, boundary guards, and Git history without treating old checklist marks as completion evidence. | ✓ VERIFIED | `01-AUDIT.md` is the single audit with: Evidence Index (46 IDs across SRC/TEST/GUARD/GIT/SCAN namespaces), Command Ledger (11 entries with dates/exit-statuses/positive test counts), Authority Reconciliation (4 CONFLICT entries), and explicitly marks historical plan checkboxes as obsolete (CONFLICT-02/03, F-HIST-01). Validator passes final mode. Evidence refs verified against live tree (SRC-OP-01..10, GIT-STAGE9-01..03, GUARD-* all confirmed). |
| 2 | Every live-session product operation is listed with its public variant, internal mapping, dispatch mode, public outcome, production callers, and test callers. | ✓ VERIFIED | Operation Matrix has exactly 15 rows matching the 15 `CodingAgentOperation` enum variants (verified via codegraph). Each row carries variant, internal mapping, kind, origin, class, dispatch, outcome, prod_callers, test_callers, impl/verify/disp/conf/evidence/gaps/blockers. Mappings verified: `into_internal` (15 arms), `from_internal` (14 arms + Export split), `run()` metadata-selected dispatch (mod.rs:249-261). Production caller line numbers verified (print_mode.rs:128/144, json_mode.rs:99, rpc/commands.rs:614). |
| 3 | The audit classifies each finding as completed baseline, actual Stage 9 gap, obsolete plan content, or deferred Stage 10 work. | ✓ VERIFIED | 9 findings with locked dispositions: F-BASE-01 (completed baseline, informational), F-ADAPT-01/F-TEST-01/F-EVID-01 (active Stage 9 gaps, required), F-HIST-01 (obsolete plan content), F-STAGE10-01 (deferred_stage_10), F-DELETE-01 (retained_compatibility), F-COMPAT-01/F-GUARD-01 (hardening). All four SC#3 categories represented. Validator enforces locked taxonomies (D-12/D-13/D-14/D-16). |

**Score:** 3/3 truths verified (0 present, behavior-unverified)

**Plan frontmatter truths (12 across 3 plans) - all VERIFIED:**

| Plan | Truth | Status | Evidence |
| --- | --- | --- | --- |
| 01-01 | Single source of truth, one exact row per public operation, separate compatibility/findings sections | ✓ VERIFIED | 15 matrix rows; separate "Compatibility Inventory" and "Findings" sections confirmed |
| 01-01 | Schema records downstream phase/requirement routing without implementation tasks | ✓ VERIFIED | Findings have Target Phase 2-5 and Requirements columns; no implementation task prose found (D-05 compliant) |
| 01-01 | Locked taxonomies from D-10/D-12-D-16 | ✓ VERIFIED | Validator enforces allowed values and passes all 3 modes |
| 01-01 | Command ledger + validator reject structurally incomplete evidence | ✓ VERIFIED | Evidence mode rejects placeholders; final mode requires positive Cargo test counts |
| 01-02 | Behavior claims backed by source + focused test per D-06 | ✓ VERIFIED | 3 focused Cargo tests pass with counts 1/1/7 matching ledger |
| 01-02 | Impl/verify separate, focused not replaced by workspace-wide testing | ✓ VERIFIED | Matrix has separate impl/verify columns; workspace tests NOT run per D-08 |
| 01-02 | Textual guards at actual scope, replaceable guards -> Phase 5 | ✓ VERIFIED | F-GUARD-01 routes missing guards to Phase 5; boundary guard scope note describes 7 real tests accurately |
| 01-02 | History is corroborating, never primary authority | ✓ VERIFIED | Authority Order: current source = layer 1; 4 CONFLICT entries resolve "current source wins" (D-09) |
| 01-03 | Findings explain gaps/contradictions/obsolete, every gap has evidence/reqs/phase/deps | ✓ VERIFIED | 9 findings each carry evidence, requirements, target phase, dependencies, obligation, disposition, confidence |
| 01-03 | Final conclusions preserve impl/verify split, consistent confidence | ✓ VERIFIED | High-confidence rows have aligned source + focused test + boundary evidence per D-14 |
| 01-03 | Evidence gaps and blockers distinct, only blockers fail verification | ✓ VERIFIED | Validator blocking-finding check conditional on actual blockers (D-15/D-16); final passes with 0 blockers |
| 01-03 | Authority conflicts explicit, Git corroborating, historical docs unchanged | ✓ VERIFIED | 4 CONFLICT entries; 3 Git commits corroborate timing; no historical plan/TODO modified |

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| `.planning/phases/01-evidence-based-baseline/01-AUDIT.md` | Machine-scannable audit with 15-row matrix, evidence ledger, compatibility inventory, findings, traceability | ✓ VERIFIED | 427 lines; Audit Status: final; 15 operation rows; 46 evidence IDs; 11 command ledger entries; 9 findings; 4 authority conflicts; AUDIT-01/02/03 traced. Validator passes final mode. |
| `.planning/phases/01-evidence-based-baseline/validate-audit.sh` | Three-mode validator (schema/evidence/final), no external deps, parses Markdown as data | ✓ VERIFIED | 28KB; `bash -n` syntax OK; passes in all 3 modes (exit 0 each); `--schema-only`, `--evidence-only`, default final all PASS. No eval/source/exec of ledger content. |
| `.planning/phases/01-evidence-based-baseline/01-VALIDATION.md` | Nyquist-compliant validation map with concrete task IDs | ✓ VERIFIED | 100 lines; `nyquist_compliant: true`, `wave_0_complete: true`, status: complete; 7 task IDs (01-01-01 through 01-03-02) all green; 3 Cargo commands listed; zero-test rejection rule stated; Approval: approved. |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | --- | --- | ------ | ------- |
| `validate-audit.sh` | `01-AUDIT.md` | Parses fixed headings and table rows as data; never executes ledger text | ✓ WIRED | Validator references `01-AUDIT.md` and `public_operation.rs` as inputs; confirmed no eval/source of command cells |
| `01-VALIDATION.md` | `validate-audit.sh` | Wave 0 and per-task commands name the phase-local validator | ✓ WIRED | All 7 task rows reference `validate-audit.sh` commands; full suite command chains validator + 3 Cargo tests + git diff --check |
| `public_operation.rs` | `01-AUDIT.md` | Each public variant, conversion arm, and outcome represented once in matrix | ✓ WIRED | 15 enum variants -> 15 matrix rows; into_internal 15 arms + from_internal 14+split verified against source |
| `operation.rs` | `01-AUDIT.md` | Static/dynamic kind, origin, class, dispatch mode populate each row | ✓ WIRED | Operation::metadata() at operation.rs:72-159 confirmed; ApproveDelegation dynamic kind (static_kind=None) at :98 confirmed |
| `protocol/interactive/print_mode.rs` | `01-AUDIT.md` | Production caller discovery populates caller inventory | ✓ WIRED | print_mode.rs:128/144 .prompt(), json_mode.rs:99 .prompt(), rpc/commands.rs:614 .self_healing_edit_with_options() all confirmed in source |

### Data-Flow Trace (Level 4)

This phase produces documentation artifacts, not runtime data flows. The "data" is evidence
references. Traced upstream from audit claims to actual source:

| Audit Claim | Data Source | Produces Real Evidence | Status |
| ----------- | ----------- | --------------------- | ------ |
| 15 operation variants | `public_operation.rs:42-83` enum | Yes - 15 variants confirmed verbatim | ✓ FLOWING |
| into_internal mapping | `public_operation.rs:107-157` | Yes - 15 arms confirmed verbatim | ✓ FLOWING |
| run() canonical dispatch | `mod.rs:249-261` | Yes - metadata-selected Async/SyncReadOnly/SyncMutable confirmed | ✓ FLOWING |
| ApproveDelegation dynamic kind | `operation.rs:98-103` | Yes - static_kind=None confirmed | ✓ FLOWING |
| 3 NOT-deprecated methods | `mod.rs:638,659,713` | Yes - no #[deprecated] preceding confirmed | ✓ FLOWING |
| 8 deprecated compat methods | `mod.rs:367,415,766,810,871,922,973,1134` | Yes - 8 #[deprecated] confirmed (excluding :498 subscribe = Stage 10) | ✓ FLOWING |
| 0 production adapter run() calls | `rg CodingAgentOperation:: src/protocol|print_mode|interactive` | Yes - 0 hits confirmed | ✓ FLOWING |
| 3 Stage 9 Git commits | `0fff6bd, ebe48df, 5c5382c` | Yes - all 3 commits exist with matching messages | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| Validator syntax | `bash -n validate-audit.sh` | SYNTAX OK | ✓ PASS |
| Validator schema mode | `bash validate-audit.sh --schema-only` | RESULT: PASS (exit 0) | ✓ PASS |
| Validator evidence mode | `bash validate-audit.sh --evidence-only` | RESULT: PASS (exit 0) | ✓ PASS |
| Validator final mode | `bash validate-audit.sh` | RESULT: PASS (exit 0) | ✓ PASS |
| Public variants test | `cargo test -p pi-coding-agent --test public_api canonical_operation_runtime_variants_are_public -- --nocapture` | 1 passed; 0 failed | ✓ PASS |
| Canonical dispatcher guard | `cargo test -p pi-coding-agent --test api_boundary_guards coding_session_run_is_the_canonical_operation_dispatcher -- --nocapture` | 1 passed; 0 failed | ✓ PASS |
| Product runtime boundary guards | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards -- --nocapture` | 7 passed; 0 failed | ✓ PASS |
| Whitespace check | `git diff --check` | clean (exit 0) | ✓ PASS |

### Probe Execution

| Probe | Command | Result | Status |
| ----- | ------- | ------ | ------ |
| `validate-audit.sh` (final) | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh` | exit 0, RESULT: PASS (final mode) | PASS |
| `validate-audit.sh` (schema) | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh --schema-only` | exit 0, RESULT: PASS (schema mode) | PASS |
| `validate-audit.sh` (evidence) | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh --evidence-only` | exit 0, RESULT: PASS (evidence mode) | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| ----------- | ----------- | ----------- | ------ | -------- |
| AUDIT-01 | 01-01, 01-02, 01-03 | Trustworthy Stage 9 completion state from source/tests/guards/history | ✓ SATISFIED | Requirement Traceability row in 01-AUDIT.md marked complete; Evidence Index (46 IDs), Command Ledger (11 entries with positive test counts), Authority Reconciliation (4 conflicts), GIT-STAGE9-01..03 all verified against live tree |
| AUDIT-02 | 01-01, 01-02, 01-03 | Each operation's variant/mapping/dispatch/outcome/callers | ✓ SATISFIED | Requirement Traceability row marked complete; 15-row Operation Matrix + 26-row Production Caller Inventory + 32-row Test Caller Inventory + 16-row Compatibility Inventory; all verified against source |
| AUDIT-03 | 01-01, 01-02, 01-03 | Separates completed baseline, gaps, obsolete, Stage 10 | ✓ SATISFIED | Requirement Traceability row marked complete; 9 findings with locked dispositions covering all 4 categories (F-BASE-01 baseline, F-ADAPT-01/F-TEST-01/F-EVID-01 active gaps, F-HIST-01 obsolete, F-STAGE10-01 deferred Stage 10) |

No orphaned requirements. All 3 AUDIT-* IDs declared in plans are traced in the audit. REQUIREMENTS.md traceability table maps all 3 to Phase 1 with status Complete.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| `validate-audit.sh` | 88, 224 | `TBD` in PLACEHOLDER_PATTERNS regex | ℹ️ Info | Not a debt marker - this is the validator's placeholder-detection logic that REJECTS TBD values. Correct behavior, not incomplete work. |

No `TBD`, `FIXME`, or `XXX` debt markers found in any deliverable. No stub implementations
(documentation phase - no runtime code). No hardcoded empty data. No restricted files modified
(no production Rust under `crates/`, no `docs/superpowers/`, no `docs/TODO.md` changed across
phase commits `620bac9^..HEAD`).

### Human Verification Required

None. The phase's own `01-VALIDATION.md` flagged one manual-only item (authority conflict
semantic quality, AUDIT-01/AUDIT-03). The verifier performed this check by comparing all 4
CONFLICT entries against their cited live-tree evidence:

- **CONFLICT-01:** Scaffold marked 3 methods as `#[deprecated]`; current source at
  `mod.rs:638,659,713` shows NO `#[deprecated]`. Resolution "current source wins" (D-09) is
  correct. F-COMPAT-01 routing to Phase 5 is appropriate.
- **CONFLICT-02:** Historical plan suggested `run()` called deprecated wrappers; current source
  at `mod.rs:249-261` proves `run()` uses metadata-selected dispatch with no compat calls
  (confirmed by GUARD-DISPATCH-01 test). Resolution correct.
- **CONFLICT-03:** Historical plan suggested fork rejected/switch had no owner op; current
  owner tests (TEST-OWNER-01..04) prove canonical fork/switch with event continuity and partial
  commit. Resolution correct.
- **CONFLICT-04:** Historical unchecked boxes vs current scans - authorities agree (no real
  conflict; gaps genuinely incomplete). SCAN-PROD-01 confirms 0 production adapter run() calls.

All dispositions follow D-17 (layered authority) and D-18 (explicit conflict entries). No
uncertainty remains.

### Gaps Summary

No gaps found. All three ROADMAP Success Criteria are verified, all 12 plan frontmatter truths
are verified, all artifacts exist/are substantive/are wired with real data flowing, all key
links are wired, all requirement IDs are satisfied and traced, no anti-patterns or debt markers,
and all behavioral spot-checks and probes pass.

The phase goal - "Maintainers have a trustworthy, source-backed statement of what Stage 9
already delivers and what remains to be implemented" - is achieved. The audit is machine-
scannable, its evidence references are verified against the actual codebase, its findings
accurately reflect the current state (completed facade baseline + active adapter/test/deletion
gaps routed to Phases 2-5 + Stage 10 deferral), and the validator mechanically enforces the
audit contract in all three modes.

---

_Verified: 2026-07-11T01:45:35Z_
_Verifier: the agent (gsd-verifier)_
