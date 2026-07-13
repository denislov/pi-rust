---
phase: 08
slug: client-connection-replay-and-scoped-control
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-13
---

# Phase 08 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Cargo test, Rust built-in test harness, Tokio `#[tokio::test]` |
| **Config file** | Workspace and crate `Cargo.toml`; no separate test-runner config |
| **Quick run command** | `cargo test -p pi-coding-agent --lib client_projection` |
| **Focused contract command** | `cargo test -p pi-coding-agent --test public_api --test protocol_events` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | Quick target under 30 seconds; full workspace runtime measured during execution |

---

## Sampling Rate

- **After every task commit:** Run the narrowest relevant command from the verification map, always including `cargo test -p pi-coding-agent --lib client_projection` for client-state changes.
- **After every plan wave:** Run `cargo test -p pi-coding-agent`.
- **Before `$gsd-verify-work`:** `cargo fmt --check`, `cargo test --workspace`, `cargo check --workspace`, source audits, and `git diff --check` must be green.
- **Max feedback latency:** 30 seconds for per-task focused sampling; split filters further if a focused command exceeds the target.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 08-W0-01 | Wave 0 | 0 | CLIENT-01 | T-08-REPLAY-GAP | Snapshot cursor and atomic replay/live handoff do not omit or handoff-duplicate product events. | async integration | `cargo test -p pi-coding-agent --test public_api client_connection` | ❌ W0 | ⬜ pending |
| 08-W0-02 | Wave 0 | 0 | CLIENT-02 | T-08-STALE-RECOVERY | Retention gaps and live lag return typed fresh-snapshot recovery with bounded metadata and no private error-string parsing. | unit + async integration | `cargo test -p pi-coding-agent --lib event_service` | Partial; public recovery tests ❌ W0 | ⬜ pending |
| 08-W0-03 | Wave 0 | 0 | CLIENT-03 | T-08-STALE-HANDLE | Same-id takeover restores client state and invalidates old-generation mutation, acknowledgement, and control authority. | async integration | `cargo test -p pi-coding-agent --test public_api client_state` | ❌ W0 | ⬜ pending |
| 08-W0-04 | Wave 0 | 0 | CONTROL-01 | T-08-CONTROL-AUTH | Only the submitting client controls the immutable target Prompt; stable control ids deduplicate retries and preserve order. | async integration | `cargo test -p pi-coding-agent --test public_api scoped_control` | ❌ W0 | ⬜ pending |
| 08-W0-05 | Wave 0 | 0 | CLIENT-01, CLIENT-02, CLIENT-03, CONTROL-01 | T-08-API-LEAK | Stable API exports public projections only; private `ProductEvent`, raw Prompt control, services, queues, and Flow nodes remain unreachable. | source guard + compile contract | `cargo test -p pi-coding-agent --test api_boundary_guards --test product_runtime_boundary_guards` | Existing guards; Phase 8 assertions ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

The planner replaces `08-W0-*` placeholders with concrete plan/task IDs once PLAN.md files establish their waves. No implementation task may claim Nyquist coverage solely through a workspace-wide command; each must name a focused automated assertion.

---

## Required Scenario Matrix

- Connect at sequence zero, publish during recovery, and assert every sequence appears exactly once across the replay/live handoff boundary.
- Deliver without acknowledgement, disconnect, reconnect, and assert at-least-once replay; acknowledge and assert the next reconnect starts after the applied sequence.
- Exhaust retained history and assert `FreshSnapshotRequired` carries requested sequence, oldest available sequence, fresh cursor, and a typed retained-history-gap reason.
- Force live receiver lag and assert fresh-snapshot recovery uses a distinct typed lag reason.
- Reconnect the same client id and assert the new generation sees the prior drafts/submitted state while the old handle rejects draft mutation, acknowledgement, snapshot mutation, and control.
- Assert a different client id cannot acquire control for a Prompt even when it knows the operation id.
- Retry the same `(client_id, operation_id, control_id)` and assert the original receipt is returned without duplicate enqueue; assert identical text under a different id is distinct and ordered.
- Assert rejected Prompt/Steer/FollowUp submissions retain their drafts and accepted submissions clear only the accepted entries.
- Assert submitted operation advances `Accepted → Running → Terminal`, remains visible until terminal acknowledgement, and covers non-Prompt canonical operations without granting them Prompt control.

---

## Wave 0 Requirements

- [ ] Extend `crates/pi-coding-agent/tests/public_api.rs` or add a focused client-connection integration suite covering public snapshot, recovery, state, and control types.
- [ ] Add deterministic test helpers for controlled product-event publication, test-sized retained capacity, same-id takeover generations, blocked Prompt control receivers, and response-loss retry.
- [ ] Add public recovery tests for both retained-history gaps and live receiver lag.
- [ ] Add boundary/source assertions preventing public exposure of private `ProductEvent`, `ProductEventReplayHandle`, raw `PromptControlHandle`, `OperationControl`, services, queues, and Flow nodes.
- [ ] Preserve existing RPC and interactive assertions while client-local state and replay behavior move behind the public connection contract.

---

## Manual-Only Verifications

All Phase 8 behaviors must have deterministic automated verification. No behavior is approved as manual-only.

---

## Security Verification

| Threat Ref | STRIDE | Behavior to Prove | Blocking Severity |
|------------|--------|-------------------|-------------------|
| T-08-STALE-HANDLE | Elevation of Privilege | Takeover increments generation and every old-generation mutation/control path fails closed. | High |
| T-08-CONTROL-AUTH | Spoofing / Elevation of Privilege | Control checks submitting client, generation, and exact Prompt operation id before enqueue. | High |
| T-08-CONTROL-REPLAY | Tampering | Stable control id deduplicates response-loss retry within `(client, target operation)` scope. | High |
| T-08-REPLAY-GAP | Tampering / Repudiation | Replay/live handoff preserves sequence authority and reports typed recovery instead of hiding loss. | High |
| T-08-RESOURCE-BOUND | Denial of Service | Retained events, idempotency receipts, and client-local queues remain bounded or have an explicit bounded-lifecycle policy. | High |
| T-08-API-LEAK | Information Disclosure / Elevation of Privilege | Stable API exposes projections and typed outcomes, not internal senders, services, registries, queues, or Flow nodes. | High |

---

## Validation Sign-Off

- [ ] All concrete plan tasks have `<automated>` verification or explicit Wave 0 dependencies.
- [ ] Sampling continuity: no three consecutive tasks without automated focused verification.
- [ ] Wave 0 covers every `❌ W0` reference above.
- [ ] No watch-mode flags appear in verification commands.
- [ ] Per-task focused feedback latency remains under 30 seconds, or the plan records a narrower deterministic filter.
- [ ] Every Phase 8 requirement appears in at least one plan frontmatter `requirements` field.
- [ ] Every locked D-01 through D-21 decision is cited by at least one plan `must_haves`/task acceptance criterion.
- [ ] All high-severity threat rows have automated fail-closed assertions.
- [ ] `nyquist_compliant: true` and `wave_0_complete: true` are set only after the concrete plan/task map and Wave 0 tests are present.

**Approval:** pending
