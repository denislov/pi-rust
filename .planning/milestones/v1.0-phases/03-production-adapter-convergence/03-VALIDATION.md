---
phase: 3
slug: production-adapter-convergence
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-07-11
---

# Phase 3 - Validation Strategy

> Per-phase validation contract for feedback sampling during production-adapter convergence.

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness plus Tokio 1.52.3 async tests |
| **Config file** | Workspace and crate Cargo manifests; no separate test config |
| **Quick run command** | `cargo test -p pi-coding-agent --test <adapter-suite> <test-name> -- --exact` |
| **Full phase command** | `cargo test -p pi-coding-agent` |
| **Final workspace commands** | `cargo test --workspace` and `cargo check --workspace` |
| **Estimated quick feedback** | Under 30 seconds for one exact integration test on a warm build |

## Sampling Rate

- **After every task commit:** Run the exact named adapter test(s), the scoped source audit, and `cargo check -p pi-coding-agent` when public types or adapter ownership change.
- **After every plan wave:** Run every integration suite touched by that wave plus `cargo check -p pi-coding-agent`.
- **Before `$gsd-verify-work`:** Run `cargo fmt --check`, `cargo test -p pi-coding-agent`, `cargo test --workspace`, `cargo check --workspace`, scoped source audits, and `git diff --check`.
- **Max feedback latency:** 30 seconds for task-level exact tests; broader crate/workspace gates run at plan, wave, and phase boundaries.

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 03-01-01 | 01 | 1 | ADAPT-01, ADAPT-03 | T-03-01 | Adapter preserves typed admission and error projection | integration | `cargo test -p pi-coding-agent --test json_mode -- --nocapture` | yes | pending |
| 03-01-02 | 01 | 1 | ADAPT-02, ADAPT-03 | T-03-01 | Persistent and transient print paths retain session effects | integration | `cargo test -p pi-coding-agent --test print_mode -- --nocapture && cargo test -p pi-coding-agent --test session_print_mode -- --nocapture` | yes | pending |
| 03-02-01 | 02 | 2 | RPC-01, RPC-03 | T-03-02 | Prompt/agent/team operations retain bounded event and control handling | integration | `cargo test -p pi-coding-agent --test rpc_mode -- --nocapture` | yes | pending |
| 03-03-01 | 03 | 3 | RPC-02, RPC-04 | T-03-01, T-03-03 | Mutation commands preserve response shapes and reject invalid outcomes | integration/source guard | `cargo test -p pi-coding-agent --test rpc_mode -- --nocapture` plus scoped source audit | partial W0 | pending |
| 03-04-01 | 04 | 4 | INTER-01, INTER-04 | T-03-02 | Interactive background tasks retain abort, steering, follow-up, and event projection | integration | named tests in `interactive_mode.rs`, `interactive_abort.rs`, and `interactive_sessions.rs` | yes | pending |
| 03-05-01 | 05 | 5 | INTER-02, INTER-04 | T-03-01, T-03-03 | Profile and delegation mutations execute asynchronously without detached state or blocking | integration | focused profile and delegation-rejection tests | partial W0 | pending |
| 03-06-01 | 06 | 6 | INTER-03, INTER-04 | T-03-02 | Branch summary/fork keeps subscriber continuity, sequence, hydration, and owner identity | integration | `cargo test -p pi-coding-agent --test interactive_mode scripted_interactive_fork_after_rust_native_prompt_creates_session -- --exact` plus navigation tests | yes | pending |
| 03-06-02 | 06 | 6 | ADAPT-04, RPC-04, INTER-05 | T-03-04 | Production adapters contain no replaced broad calls or local deprecation suppressions | source guard | focused boundary guard plus exact `rg` closure audit | partial W0 | pending |

## Wave 0 Requirements

- [ ] Add narrow executable source guards for JSON/print, RPC, and interactive canonical-call/deprecation constraints, or lock equivalent exact command audits into each plan.
- [ ] Add a focused direct interactive `/branch-summary` parity test covering visible and persisted summary behavior.
- [ ] Add a focused interactive default-profile mutation test covering selection, persistence, and refreshed projection.
- [ ] Adapt or add delegation-rejection behavior coverage when the synchronous handler becomes async.

Existing faux providers, tempfile sessions, support guards, and interactive harnesses are sufficient; no new test framework or external fixture package is required.

## Manual-Only Verifications

All phase behaviors have automated verification. The optional `scripts/tui-smoke.sh` capture may supplement, but cannot replace, deterministic integration assertions.

## Validation Sign-Off

- [ ] Every plan task has an exact automated command or an explicit Wave 0 dependency.
- [ ] No three consecutive implementation tasks omit automated verification.
- [ ] All four Wave 0 gaps are implemented or explicitly replaced by executable source-audit commands in PLAN.md.
- [ ] RPC tests prove response-before-events ordering, final drain behavior, idempotency, and control routing.
- [ ] Interactive navigation tests prove subscriber continuity, event sequencing, refreshed snapshots, and retained owner identity.
- [ ] No watch-mode flags or network-dependent providers are used.
- [x] `nyquist_compliant: true` is set in frontmatter.

**Approval:** pending
