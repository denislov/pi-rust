---
phase: 05
slug: boundary-enforcement-and-stage-9-closure
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-07-13
---

# Phase 05 - Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness plus integration fixtures |
| **Config file** | Workspace `Cargo.toml` and `crates/pi-coding-agent/Cargo.toml` |
| **Quick run command** | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --test api_boundary_guards --test public_api -- --nocapture` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | Measure and record in the Stage 9 closure report |

---

## Sampling Rate

- **After every task commit:** Run the narrowest affected integration target and `git diff --check`.
- **After every plan wave:** Run the focused three-target boundary/API command.
- **Before `$gsd-verify-work`:** Run formatting, focused crate tests/checks, source audits, full workspace tests/checks, and diff checks from the final tree.
- **Max feedback latency:** Keep task-level checks focused; defer the full workspace suite to wave boundaries and final closure.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 05-01-01 | 01 | 1 | GUARD-01, GUARD-02 | T-05-01, T-05-03 | Adapter ownership and structural call scans fail closed without false positives | integration | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards -- --nocapture` | yes | pending |
| 05-02-01 | 02 | 1 | GUARD-03, GUARD-04 | T-05-01, T-05-02 | External crates can use only the explicit stable facade | integration/compile fixture | `cargo test -p pi-coding-agent --test api_boundary_guards --test public_api -- --nocapture` | yes | pending |
| 05-03-01 | 03 | 2 | CLOSE-01, CLOSE-02, CLOSE-03 | T-05-02, T-05-04 | Deleted paths stay absent while behavior and durable facts remain intact | crate/workspace regression | `cargo test -p pi-coding-agent && cargo check -p pi-coding-agent` | yes | pending |
| 05-03-02 | 03 | 2 | CLOSE-04 | T-05-05 | Closure evidence identifies the exact verified tree and bounded Stage 10 handoff | workspace/evidence | `cargo fmt --check && cargo test --workspace && cargo check --workspace && git diff --check` | yes | pending |

Threat register:

- `T-05-01`: A private or broad path bypasses canonical operation admission.
- `T-05-02`: A deleted facade is recreated under another name or visibility path.
- `T-05-03`: Scanner formatting/test-code handling causes false positives or false negatives.
- `T-05-04`: Guard refactoring weakens durable behavior or compatibility evidence.
- `T-05-05`: Closure documentation claims a tree or command result that was not actually verified.

---

## Wave 0 Requirements

Existing Rust integration infrastructure covers all phase requirements. The planner may add deterministic external-consumer fixture crates, but no new test framework or external package is required.

---

## Manual-Only Verifications

None. Source audits and closure-report inspection must be expressed as deterministic commands or structured evidence checks.

---

## Validation Sign-Off

- [ ] Every plan task has an automated command or an explicit dependency on the final closure verification task.
- [ ] No three consecutive implementation tasks lack focused automated verification.
- [x] Wave 0 requires no new framework or missing fixture bootstrap.
- [x] No watch-mode flags are used.
- [ ] Final command durations and results are recorded in the Stage 9 closure report.
- [x] `nyquist_compliant: true` is set in frontmatter.

**Approval:** pending execution
