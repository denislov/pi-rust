---
phase: 4
slug: test-convergence-and-compatibility-deletion
status: planned
nyquist_compliant: true
wave_0_complete: false
created: 2026-07-13
---

# Phase 4 - Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust libtest + Tokio async tests |
| **Config file** | Cargo manifests; no separate test config |
| **Quick run command** | `cargo test -p pi-coding-agent --test <suite> <filter> -- --exact` |
| **Full suite command** | `cargo test -p pi-coding-agent` |
| **Estimated runtime** | ~120 seconds |

---

## Sampling Rate

- **After every task commit:** Run the exact affected integration target or owner-test filter plus a receiver-aware zero-caller audit
- **After every plan wave:** Run all affected targets, both boundary targets, and `cargo check -p pi-coding-agent`
- **Before `/gsd-verify-work`:** `cargo fmt --check`, `cargo test -p pi-coding-agent`, `cargo check -p pi-coding-agent`, source audits, and `git diff --check` must pass
- **Max feedback latency:** 120 seconds

## Security Acceptance Rule

This phase enforces OWASP ASVS Level 1 with a high-severity threshold. All applicable ASVS L1 controls must be addressed in the four plans; every high-severity threat in the plan threat registers must name a concrete mitigation and have automated verification; execution cannot be signed off while any high-severity threat is open. The final closure task and this validation artifact are the acceptance authority for that rule.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 4-01-01 | 01 | 1 | TEST-01, TEST-02 | T-04-01, T-04-02, T-04-03 | Public tests enter through admitted typed operations while behavior and fault boundaries remain narrow | integration | `cargo test -p pi-coding-agent --test agent_invocation --test agent_team_flow --test delegation_execution -- --nocapture` | yes | green |
| 4-01-02 | 01 | 1 | DELETE-01, DELETE-02, DELETE-03, DELETE-04 | T-04-04, T-04-05 | G1 definitions/calls are absent and retained APIs are present | source/API guard | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --test api_boundary_guards -- --nocapture && cargo check -p pi-coding-agent` | yes | green |
| 4-02-01 | 02 | 2 | TEST-01, TEST-02, TEST-03, TEST-04 | T-04-06, T-04-07, T-04-08 | G2 tests use admitted operations, preserve assertions, keep helpers outcome-only, and limit private load_plugins to exactly four justified co-located owner-test calls | unit + integration | `cargo test -p pi-coding-agent --lib --test agent_profile_runtime --test agent_profile_session --test public_api -- --nocapture` | yes | pending |
| 4-02-02 | 02 | 2 | TEST-04, DELETE-01, DELETE-02, DELETE-03, DELETE-04 | T-04-09, T-04-10 | reload_plugins and other G2 broad methods/callers/synonyms are absent; load_plugins remains private with exactly four owner-test calls and no public/helper/wrapper/generic-fault/non-test exposure | source/API guard | `cargo test -p pi-coding-agent --lib --test agent_profile_runtime --test agent_profile_session --test public_api --test product_runtime_boundary_guards --test api_boundary_guards -- --nocapture && cargo check -p pi-coding-agent` | yes | pending |
| 4-03-01 | 03 | 3 | TEST-01, TEST-02 | T-04-11, T-04-12, T-04-13 | Durable delegation replay, IDs, exact errors, and restricted fault paths remain asserted | integration | `cargo test -p pi-coding-agent --test delegation_execution --test public_api --lib -- --nocapture` | yes | pending |
| 4-03-02 | 03 | 3 | DELETE-01, DELETE-02, DELETE-03, DELETE-04 | T-04-14, T-04-15 | Delegation methods have zero callers/post-deletion definitions and retained helpers are positive-guarded | source/API guard | `cargo test -p pi-coding-agent --test delegation_execution --test public_api --test product_runtime_boundary_guards --test api_boundary_guards -- --nocapture && cargo check -p pi-coding-agent` | yes | pending |
| 4-04-01 | 04 | 4 | TEST-01, TEST-02, TEST-03 | T-04-16, T-04-17 | Navigation and summary operations preserve replay-authoritative behavior and continuity | integration | `cargo test -p pi-coding-agent --lib --test public_api -- --nocapture` | yes | pending |
| 4-04-02 | 04 | 4 | TEST-03, TEST-04, DELETE-01, DELETE-02, DELETE-03, DELETE-04 | T-04-18, T-04-19, T-04-20, T-04-21 | Final receiver-aware audit enumerates all 16 deleted methods including summarize_branch_for_navigation; separately requires private load_plugins and exactly four justified owner-test calls while rejecting public/helper/wrapper/generic-fault/non-test exposure; retained lifecycle/query/subscription/control/static APIs remain present | source/API + full regression | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards final_receiver_aware_compatibility_absence_and_retained_api_guard -- --exact && cargo fmt --check && cargo test -p pi-coding-agent && cargo check -p pi-coding-agent && cargo test --workspace && cargo check --workspace && git diff --check` | yes | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- [ ] Wave 0 prerequisite checks are pending; no separate Wave 0 implementation plan exists.
- [ ] Plan 04-01 updates `canonical_operation_facade_has_no_new_workflow_wrappers` to assert deleted names are absent.
- [ ] Plan 04-02 updates `public_api.rs` compile contracts to require canonical operations and stop compiling removed methods.
- [ ] Plan 04-02 permits only narrowly reusable typed outcome extractors where repeated matches justify them.
- [ ] Plan 04-02 deletes reload_plugins and adds a positive guard requiring private load_plugins to have exactly four justified co-located owner-test callers and no broader exposure.
- [ ] Plan 04-04 adds summarize_branch_for_navigation to the absent-definition, receiver-call, and synonym ledgers for the complete 16-method deletion set.

No framework installation is required.

---

## Manual-Only Verifications

All phase behaviors have automated verification.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 is complete and covers all missing references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** planned; execution must turn all task statuses green.
