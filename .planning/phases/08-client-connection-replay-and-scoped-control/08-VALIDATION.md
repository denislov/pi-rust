---
phase: 08
slug: client-connection-replay-and-scoped-control
status: draft
nyquist_compliant: true
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
| 08-01-T1 | 1 | CLIENT-01, CLIENT-02, CLIENT-03, CONTROL-01 | T-08-API-LEAK | Public projections are exhaustively constructible while private types remain unreachable. | compile contract + source guard | `cargo test -p pi-coding-agent --test public_api client_contract --quiet` | present in plan | ⬜ pending |
| 08-01-T2 | 1 | CLIENT-01, CLIENT-02, CLIENT-03, CONTROL-01 | T-08-API-LEAK | Public API boundary rejects private services, queues, and Flow nodes. | source guard | `cargo test -p pi-coding-agent --test api_boundary_guards --quiet` | present in plan | ⬜ pending |
| 08-01-T3 | 1 | CLIENT-01, CLIENT-02, CLIENT-03, CONTROL-01 | T-08-API-LEAK | Stable value contract serializes and round-trips. | serialization unit | `cargo test -p pi-coding-agent --lib public_projection --quiet` | present in plan | ⬜ pending |
| 08-02-T1 | 2 | CLIENT-03 | T-08-STALE-HANDLE, T-08-RESOURCE-BOUND | Generation and state-machine RED tests cover takeover and non-evicting caps. | deterministic unit | `cargo test -p pi-coding-agent --lib client_service --quiet` | present in plan | ⬜ pending |
| 08-02-T2 | 2 | CLIENT-03 | T-08-STALE-HANDLE, T-08-RESOURCE-BOUND | ClientService transitions and typed capacity rejections pass. | deterministic unit | `cargo test -p pi-coding-agent --lib client_service --quiet` | present in plan | ⬜ pending |
| 08-02-T3 | 2 | CLIENT-03 | T-08-STALE-HANDLE | Session constructors isolate registries. | owner unit | `cargo test -p pi-coding-agent --lib coding_session::tests::client --quiet` | present in plan | ⬜ pending |
| 08-03-T1 | 2 | CLIENT-01, CLIENT-02 | T-08-REPLAY-GAP | Replay/live boundary preserves sequence authority. | unit + async integration | `cargo test -p pi-coding-agent --lib event_service --quiet` | present in plan | ⬜ pending |
| 08-03-T2 | 2 | CLIENT-01, CLIENT-02 | T-08-REPLAY-GAP | Gap and lag recovery are distinct typed branches. | async integration | `cargo test -p pi-coding-agent --test public_api client_connection --quiet` | present in plan | ⬜ pending |
| 08-04-T1 | 3 | CLIENT-01, CLIENT-02, CLIENT-03 | T-08-REPLAY-GAP, T-08-STALE-HANDLE | Public connection RED scenarios cover races and stale handles. | deterministic race | `cargo test -p pi-coding-agent --test public_api client_connection --quiet` | present in plan | ⬜ pending |
| 08-04-T2 | 3 | CLIENT-01, CLIENT-02 | T-08-REPLAY-GAP | Fixed transaction lock order retries coherent snapshots. | async integration | `cargo test -p pi-coding-agent --test public_api client_connection --quiet` | present in plan | ⬜ pending |
| 08-04-T3 | 3 | CLIENT-03 | T-08-STALE-HANDLE | Provenance commit/abort/double-consume is exact once. | deterministic unit | `cargo test -p pi-coding-agent --test public_api client_state --quiet` | present in plan | ⬜ pending |
| 08-05-T1 | 4 | CONTROL-01 | T-08-CONTROL-AUTH, T-08-CONTROL-REPLAY | Scoped authorization and signature conflict tests pass. | async integration + unit | `cargo test -p pi-coding-agent --test public_api scoped_control --quiet` | present in plan | ⬜ pending |
| 08-05-T2 | 4 | CONTROL-01 | T-08-CONTROL-AUTH, T-08-CONTROL-REPLAY, T-08-RESOURCE-BOUND | Receipt-first retry, typed capacity rejection, draft preservation, and FIFO pass. | async integration + unit | `cargo test -p pi-coding-agent --lib operation_control --quiet` | present in plan | ⬜ pending |
| 08-06-T1 | 5 | CLIENT-01, CLIENT-02 | T-08-API-LEAK, T-08-06-INPUT | RPC consumes public connection contract. | RPC integration | `cargo test -p pi-coding-agent --test rpc_mode --quiet` | present in plan | ⬜ pending |
| 08-06-T2 | 5 | CLIENT-03, CONTROL-01 | T-08-API-LEAK | RPC mirrors removed without wire regressions. | protocol integration | `cargo test -p pi-coding-agent --test protocol_events --quiet` | present in plan | ⬜ pending |
| 08-06-T3 | 5 | CLIENT-01, CLIENT-02, CLIENT-03, CONTROL-01 | T-08-API-LEAK, T-08-06-INPUT | Final source guards and workspace checks pass. | workspace gate | `cargo test -p pi-coding-agent --test api_boundary_guards --quiet` | present in plan | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

The concrete plan/task IDs above are the RED-first dependencies for execution. Runtime status remains pending until implementation runs; no task claims Nyquist coverage solely through a workspace-wide command.

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

- [ ] 08-01-T1 creates the RED public contract and boundary assertions.
- [ ] Add deterministic test helpers for controlled product-event publication, test-sized retained capacity, same-id takeover generations, blocked Prompt control receivers, and response-loss retry.
- [ ] 08-03-T1 and 08-04-T1 add public recovery tests for both retained-history gaps and live receiver lag.
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

- [ ] All concrete plan tasks (08-01 through 08-06) have `<automated>` verification or explicit RED-first dependencies.
- [ ] Sampling continuity: no three consecutive tasks without automated focused verification.
- [ ] Wave 1 contains all RED-first contract tests before behavior wiring in later waves.
- [ ] No watch-mode flags appear in verification commands.
- [ ] Per-task focused feedback latency remains under 30 seconds, or the plan records a narrower deterministic filter.
- [ ] Every Phase 8 requirement appears in at least one plan frontmatter `requirements` field.
- [ ] Every locked D-01 through D-21 decision is cited by at least one plan `must_haves`/task acceptance criterion.
- [ ] All high-severity threat rows have automated fail-closed assertions.
- [x] `nyquist_compliant: true` reflects complete planning-time task/command/requirement/threat mapping; runtime pass/fail remains pending.
- [ ] `wave_0_complete: true` is set only after 08-01 RED-first tests are implemented and observed failing for the intended missing behavior before production wiring.

**Approval:** pending
