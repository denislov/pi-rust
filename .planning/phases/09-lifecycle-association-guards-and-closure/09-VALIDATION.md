---
phase: 09
slug: lifecycle-association-guards-and-closure
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-07-14
---

# Phase 09 - Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness with Tokio async tests and deterministic offline fixtures |
| **Config file** | Workspace and crate `Cargo.toml`; existing support in `crates/pi-coding-agent/tests/support/mod.rs` |
| **Quick run command** | `cargo test -p pi-coding-agent --test public_api --test api_boundary_guards --test product_runtime_boundary_guards --quiet` |
| **Full suite command** | `cargo test --workspace --quiet` |
| **Estimated runtime** | Quick focused filters under 60 seconds; full workspace several minutes |

---

## Sampling Rate

- **After every task commit:** Run the narrow test target/filter named by that task plus `cargo fmt --all --check`.
- **After every plan wave:** Run `cargo test -p pi-coding-agent --quiet` and `git diff --check`.
- **Before `$gsd-verify-work`:** Run every blocking command in the final-gate row, including full workspace tests/checks and security/source audits.
- **Max feedback latency:** 60 seconds for ordinary task-level focused tests; broad workspace commands are wave/final gates.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 09-01-01 | 01 | 1 | CLIENT-04, CONTROL-02 | T-09-01, T-09-04 | Public lifecycle/association types expose no raw authority | external API + source guard | `cargo test -p pi-coding-agent --test public_api --test api_boundary_guards --quiet` | Yes | pending |
| 09-01-02 | 01 | 1 | CONTROL-02 | T-09-03 | Closed 15-row association taxonomy rejects omissions and duplicates | unit + source guard | `cargo test -p pi-coding-agent association_matrix --quiet` | Wave 0 | pending |
| 09-02-01 | 02 | 2 | CLIENT-04 | T-09-01, T-09-02 | Detach is generation-scoped, idempotent, state-preserving, and wakes receivers | unit + public integration | `cargo test -p pi-coding-agent detach --quiet` | Wave 0 | pending |
| 09-02-02 | 02 | 2 | CLIENT-04 | T-09-01 | Detached/stale/shutdown handles reject state, ack, draft, replay, submission, and control mutation | public integration | `cargo test -p pi-coding-agent lifecycle_rejection --quiet` | Wave 0 | pending |
| 09-03-01 | 03 | 3 | CONTROL-02 | T-09-03 | Submission provenance supports all 15 operations while Prompt-only draft rules remain intact | public integration | `cargo test -p pi-coding-agent submission_association --quiet` | Wave 0 | pending |
| 09-03-02 | 03 | 3 | CONTROL-02, COMPAT-03 | T-09-03 | TerminalAssociated operations anchor exactly one root event sequence; OutcomeOnly anchors explicit outcome ack | unit + integration | `cargo test -p pi-coding-agent terminal_association --quiet` | Wave 0 | pending |
| 09-03-03 | 03 | 3 | CONTROL-02, COMPAT-03 | T-09-03 | PartialCommit retains operation id and distinguishes exact event anchor from TerminalUncertain | durable integration | `cargo test -p pi-coding-agent partial_commit_association --quiet` | Wave 0 | pending |
| 09-04-01 | 04 | 4 | CLIENT-04, COMPAT-03 | T-09-02 | Shutdown closes admission/control, drains admitted work, publishes terminal then lifecycle event, and closes receivers | deterministic async integration | `cargo test -p pi-coding-agent shutdown --quiet` | Wave 0 | pending |
| 09-05-01 | 05 | 5 | CLIENT-04, COMPAT-03 | T-09-01, T-09-02 | RPC explicit detach and transport cleanup share one path without changing existing wire shapes | RPC integration | `cargo test -p pi-coding-agent --test rpc_mode lifecycle --quiet` | Wave 0 | pending |
| 09-05-02 | 05 | 5 | CLIENT-04, COMPAT-03 | T-09-02 | Interactive UI detach versus top-level owner shutdown preserves owner restoration/event ordering | interactive integration | `cargo test -p pi-coding-agent interactive_lifecycle --quiet` | Wave 0 | pending |
| 09-06-01 | 06 | 6 | GUARD-01 | T-09-05 | Recursive adapter discovery and classification set equality fail on an unlisted root | source guard | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards adapter --quiet` | Yes | pending |
| 09-06-02 | 06 | 6 | GUARD-02 | T-09-06 | Negative fixtures bind Cargo JSON code/symbol/primary span and adjacent positive fixture passes | compile fixture | `cargo test -p pi-coding-agent --test api_boundary_guards external --quiet` | Yes | pending |
| 09-06-03 | 06 | 6 | GUARD-01, GUARD-02, COMPAT-03 | T-09-01 through T-09-06 | Full public authority, security, source, compatibility, and workspace gates pass | full verification | `cargo fmt --all --check && cargo test -p pi-coding-agent --quiet && cargo test --workspace --quiet && cargo check --workspace && git diff --check` | Yes | pending |

*Status: pending -> green/red/flaky. Planner may split rows into additional tasks, but every row must retain an automated owner.*

---

## Wave 0 Requirements

- [ ] Add deterministic public lifecycle tests for detach, receiver wake-up, stale/detached/shutdown rejection, and state preservation.
- [ ] Add a closed 15-operation association matrix test before changing finalization behavior.
- [ ] Add exact root-terminal evidence, OutcomeOnly acknowledgement, and PartialCommit/TerminalUncertain fixtures.
- [ ] Add deterministic shutdown gates using channels/oneshots; do not use wall-clock sleeps.
- [ ] Add RPC and interactive lifecycle contract fixtures before adapter migration.
- [ ] Extend external consumer fixtures with adjacent positive cases and Cargo JSON diagnostic assertions.

Existing Rust/Tokio test infrastructure is sufficient; no new framework or dependency is required.

---

## Manual-Only Verifications

All phase behaviors have automated verification. TUI lifecycle behavior is exercised through deterministic adapter/owner tests rather than a manual terminal session.

---

## Validation Sign-Off

- [x] All planned behavior families have an automated test owner or explicit Wave 0 dependency.
- [x] Sampling continuity prevents three consecutive implementation tasks without automated verification.
- [x] Wave 0 covers all currently missing lifecycle/association/diagnostic fixtures.
- [x] No watch-mode flags are used.
- [x] Focused feedback latency target is under 60 seconds.
- [x] `nyquist_compliant: true` is set in frontmatter.

**Approval:** approved 2026-07-14 for planning; execution status remains pending.
