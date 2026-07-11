---
phase: 02
slug: canonical-facade-correctness
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-11
---

# Phase 02 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness plus Tokio `#[tokio::test]` |
| **Config file** | `Cargo.toml` and `crates/pi-coding-agent/Cargo.toml` |
| **Quick run command** | `cargo test -p pi-coding-agent --test public_api --test api_boundary_guards` |
| **Owner quick run command** | `cargo test -p pi-coding-agent coding_session::tests::` |
| **Full crate command** | `cargo test -p pi-coding-agent` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | Measure during Wave 0 and update before phase sign-off |

---

## Sampling Rate

- **After every task commit:** Run the task-owned focused test target plus `cargo fmt --check`.
- **After every plan wave:** Run `cargo test -p pi-coding-agent` and `cargo check -p pi-coding-agent`.
- **Before `$gsd-verify-work`:** Run `cargo fmt --check`, `cargo test --workspace`, `cargo check --workspace`, focused source audits, and `git diff --check`.
- **Max feedback latency:** Focused commands should complete before the next task begins; record measured runtimes during Wave 0.

---

## Per-Task Verification Map

Task IDs are confirmed against `02-01-PLAN.md` through `02-03-PLAN.md`. Execution must preserve every requirement row and record actual runtimes/statuses before sign-off.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | FACADE-01 | T-02-01 | Stable callers import the complete operation/session contract closure only from `pi_coding_agent::api` | integration/API compile | `cargo test -p pi-coding-agent --test public_api stable_api_signature_closure_is_importable -- --nocapture` | ✅ extend | ⬜ pending |
| 02-01-02 | 01 | 1 | FACADE-04 | T-02-02 | Internal operation, metadata, raw plugin options, services, and Flow nodes remain inaccessible | compile/API plus narrow source guard | `cargo test -p pi-coding-agent --test api_boundary_guards stable_api_excludes_internal_runtime_contracts -- --nocapture` | ✅ extend | ⬜ pending |
| 02-02-01 | 02 | 1 | FACADE-02 | T-02-03 | All 15 public variants map to the expected private operation and metadata-selected dispatcher | owner unit plus dispatcher behavior | `cargo test -p pi-coding-agent coding_session::public_operation::tests::operation_contract_covers_all_public_variants -- --exact --nocapture` | ❌ W0 | ⬜ pending |
| 02-02-02 | 02 | 1 | FACADE-03 | T-02-04 | Every internal outcome projects exhaustively, including `Export` and `ExportHtml`, and run behavior covers all three dispatch families | owner unit plus dispatcher behavior | `cargo test -p pi-coding-agent coding_session::public_operation::tests::operation_outcome_projection_covers_all_families -- --exact --nocapture && cargo test -p pi-coding-agent coding_session::tests::canonical_run_uses_each_metadata_dispatch_family -- --exact --nocapture` | ❌ W0 | ⬜ pending |
| 02-03-01 | 03 | 2 | FACADE-05 | T-02-05, T-02-08 | Fork, switch, and branch-summary reuse preserve state, errors, events, replay, and applicable `PartialCommit` | owner integration-style unit tests | `cargo test -p pi-coding-agent coding_session::tests::canonical_run_preserves_navigation_and_branch_summary_durability -- --exact --nocapture && cargo test -p pi-coding-agent coding_session::tests::canonical_durable_mutations_distinguish_no_commit_partial_commit_and_replay -- --exact --nocapture && cargo fmt --check` | ⚠️ partial | ⬜ pending |
| 02-03-02 | 03 | 2 | FACADE-05 | T-02-06, T-02-07 | Plugin load/command, profile mutation, and delegation decisions preserve canonical outcomes plus applicable state/error/event/reopen and delegation pre/post-append semantics | owner integration-style unit tests | `cargo test -p pi-coding-agent coding_session::tests::canonical_run_preserves_plugin_profile_and_delegation_contracts -- --exact --nocapture && cargo test -p pi-coding-agent coding_session::tests::canonical_delegation_decisions_distinguish_no_commit_partial_commit_and_replay -- --exact --nocapture && cargo fmt --check` | ⚠️ partial | ⬜ pending |
| 02-03-03 | 03 | 2 | FACADE-01, FACADE-02, FACADE-03, FACADE-04, FACADE-05 | T-02-01 through T-02-08 | Final tree passes focused, crate, workspace, path-safe source audits, test-only failure-control guard, compatibility-facade guard, formatting, and diff gates | phase closure | `cargo fmt --check && cargo test -p pi-coding-agent && cargo check -p pi-coding-agent && cargo test --workspace && cargo check --workspace && cargo test -p pi-coding-agent --test public_api --test api_boundary_guards -- --nocapture && test "$(sed -n '/pub enum CodingAgentOperation {/,/^}/p' crates/pi-coding-agent/src/coding_session/public_operation.rs | rg -c '^    [A-Z]')" -eq 15 && sh -c 'for path in crates/pi-coding-agent/src/protocol crates/pi-coding-agent/src/print_mode.rs crates/pi-coding-agent/src/interactive crates/pi-coding-agent/src/lib.rs crates/pi-coding-agent/src/coding_session crates/pi-coding-agent/src/coding_session/session_log/store.rs; do test -e "$path" || exit 2; done; rg -n "CodingAgentOperation::" crates/pi-coding-agent/src/protocol crates/pi-coding-agent/src/print_mode.rs crates/pi-coding-agent/src/interactive; status=$?; test "$status" -eq 1' && rg -n '#\[cfg\(test\)\]' crates/pi-coding-agent/src/coding_session/session_log/store.rs && sh -c 'git diff -U0 -- crates/pi-coding-agent/src/coding_session/session_service.rs crates/pi-coding-agent/src/coding_session/delegation_confirmation_service.rs | rg -n "^\\+.*(fail_if_injected|StoreFailurePoint|inject.*failure)"; status=$?; test "$status" -eq 1' && sh -c 'git diff -U0 -- crates/pi-coding-agent/src/lib.rs crates/pi-coding-agent/src/coding_session | rg --pcre2 -n "^\\+\\s*pub\\s+mod\\s+(?!api\\b)|^\\+.*(compatibility facade|compat facade)"; status=$?; test "$status" -eq 1' && git diff --check` | ✅ existing commands | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Add a test-only 15-row operation contract matrix at the owner layer; do not add production instrumentation or public test hooks.
- [ ] Add direct exhaustive internal-outcome projection tests, explicitly covering the separate `Export` and `ExportHtml` public branches.
- [ ] Add an explicit stable-facade signature-closure ledger/test in `crates/pi-coding-agent/tests/public_api.rs`; extend exports only after the ledger identifies omissions.
- [ ] Strengthen `crates/pi-coding-agent/tests/api_boundary_guards.rs` with compiler-visible checks where feasible and narrowly scoped source guards otherwise.
- [ ] Add focused canonical success/state/reopen/event tests for plugin command, profile mutation, and delegation approval/rejection where current evidence is compatibility-path or error-path focused.
- [ ] Reuse existing test-only deterministic failure controls for no-append, partial-commit, and replay scenarios; do not create production failure hooks.
- [ ] Measure the focused and full-crate command runtimes and replace the estimated-runtime placeholder before validation sign-off.

---

## Manual-Only Verifications

All Phase 2 facade and durability requirements must have automated verification. No manual-only behavior is currently accepted.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verification or explicit Wave 0 dependencies.
- [ ] Sampling continuity: no three consecutive tasks without automated verification.
- [ ] Wave 0 covers every missing test reference in the map.
- [ ] No watch-mode flags are used.
- [ ] Focused feedback latency is measured and acceptable for per-task execution.
- [x] All FACADE-01 through FACADE-05 rows map to final PLAN.md task IDs.
- [ ] `nyquist_compliant: true` and `wave_0_complete: true` are set in frontmatter.

**Approval:** pending
