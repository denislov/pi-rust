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

Task IDs are provisional until PLAN.md files are generated. The planner must replace or confirm them while preserving every requirement row.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | FACADE-01 | T-02-01 | Stable callers import the complete operation/session contract closure only from `pi_coding_agent::api` | integration/API compile | `cargo test -p pi-coding-agent --test public_api` | ✅ extend | ⬜ pending |
| 02-01-02 | 01 | 1 | FACADE-04 | T-02-02 | Internal operation, metadata, raw plugin options, services, and Flow nodes remain inaccessible | compile/API plus narrow source guard | `cargo test -p pi-coding-agent --test api_boundary_guards` | ✅ extend | ⬜ pending |
| 02-02-01 | 02 | 2 | FACADE-02 | T-02-03 | All 15 public variants map to the expected private operation and metadata-selected dispatcher | owner unit plus dispatcher behavior | `cargo test -p pi-coding-agent coding_session::tests::operation_contract` | ❌ W0 | ⬜ pending |
| 02-02-02 | 02 | 2 | FACADE-03 | T-02-04 | Every internal outcome projects exhaustively, including `Export` and `ExportHtml` | owner unit | `cargo test -p pi-coding-agent coding_session::tests::operation_outcome_projection` | ❌ W0 | ⬜ pending |
| 02-03-01 | 03 | 3 | FACADE-05 | T-02-05 | Fork, switch, branch-summary reuse, plugin, profile, and delegation preserve state, errors, events, replay, and applicable `PartialCommit` | owner integration-style unit tests | `cargo test -p pi-coding-agent canonical_` | ⚠️ partial | ⬜ pending |

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
- [ ] All FACADE-01 through FACADE-05 rows map to final PLAN.md task IDs.
- [ ] `nyquist_compliant: true` and `wave_0_complete: true` are set in frontmatter.

**Approval:** pending
