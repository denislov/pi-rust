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
| **Quick run command** | `cargo test -p pi-coding-agent compact_cancellation --lib --quiet && cargo test -p pi-coding-agent --test public_api --test operation_association --test api_boundary_guards --test product_runtime_boundary_guards --quiet` |
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
| 09-01-01 | 01 | 1 | CLIENT-04, CONTROL-02 | T-09-01, T-09-04 | Compile-ready lifecycle/rejection/terminal-anchor values are typed and deterministic | external value contract | `cargo test -p pi-coding-agent --test public_api lifecycle_values --quiet` | Yes | pending |
| 09-01-02 | 01 | 1 | CLIENT-04, CONTROL-02 | T-09-01, T-09-04 | Facade value exports expose no raw lifecycle authority and serialization omits internals | source guard + serialization | `cargo test -p pi-coding-agent --test api_boundary_guards public_lifecycle_values --quiet && cargo test -p pi-coding-agent --test public_api lifecycle_serialization --quiet` | Yes | pending |
| 09-02-01 | 02 | 1 | CONTROL-02 | T-09-03 | Closed 15-row descriptor rejects omissions/duplicates and encodes Compact branch roots | unit | `cargo test -p pi-coding-agent association_matrix --quiet` | Yes | pending |
| 09-03-01 | 03 | 2 | CLIENT-04, CONTROL-02 | T-09-01, T-09-02 | Detach is generation-scoped, idempotent, state-preserving, and fail closed | unit + public integration | `cargo test -p pi-coding-agent detach --quiet && cargo test -p pi-coding-agent lifecycle_rejection --quiet` | Yes | pending |
| 09-03-02 | 03 | 2 | CLIENT-04, CONTROL-02 | T-09-01, T-09-02 | Public detach wakes blocked receivers and active Prompt survives/rebinds | external API + source guard | `cargo test -p pi-coding-agent --test public_api detach --quiet && cargo test -p pi-coding-agent --test api_boundary_guards public_lifecycle --quiet` | Yes | pending |
| 09-04-01 | 04 | 3 | CONTROL-02, COMPAT-03 | T-09-03 | All 15 submissions use typed event/outcome/uncertain anchors with separate ack domains | unit + integration | `cargo test -p pi-coding-agent submission_association --quiet && cargo test -p pi-coding-agent outcome_acknowledgement --quiet` | Yes | pending |
| 09-04-02 | 04 | 3 | CONTROL-02, COMPAT-03 | T-09-03 | A pre-transfer crate-private `CompactCancellationHandle` cancels only the matching active admitted Compact id/generation, stale/mismatched/no-active/non-Compact requests fail closed, canonical Flow maps cancellation to typed Failed/PromptFailed evidence, external tests verify exact roots and PartialCommit without invoking private authority | colocated lib + durable integration | `cargo test -p pi-coding-agent compact_cancellation --lib --quiet && cargo test -p pi-coding-agent --test operation_association terminal_association --quiet && cargo test -p pi-coding-agent --test operation_association partial_commit_association --quiet` | Yes | pending |
| 09-05-01 | 05 | 4 | CLIENT-04, COMPAT-03 | T-09-02, T-09-04 | Final lifecycle event is additive and non-operation; all exhaustive public naming, cfg(test), protocol, and interactive bridge consumers plus internal/public/documented 45->46 inventories update atomically, with protocol/UI arms producing no synthetic legacy content | event contract + lib bridge + boundary guard | `cargo test -p pi-coding-agent event_contract --quiet && cargo test -p pi-coding-agent --lib interactive::event_bridge --quiet && cargo test -p pi-coding-agent --test event_boundary_guards --quiet` | Yes | pending |
| 09-05-02 | 05 | 4 | CLIENT-04, COMPAT-03 | T-09-02, T-09-03 | Opaque handle closes Phase A during moved-owner run; restored owner drains, publishes lifecycle, then closes receivers | unit + external integration | `cargo test -p pi-coding-agent shutdown --quiet && cargo test -p pi-coding-agent --test public_api shutdown --quiet` | Yes | pending |
| 09-06-01 | 06 | 5 | CLIENT-04, COMPAT-03 | T-09-01, T-09-04 | RPC lifecycle wire values are additive and old response snapshots remain exact | protocol serialization | `cargo test -p pi-coding-agent --test protocol_events lifecycle_wire --quiet` | Yes | pending |
| 09-06-02 | 06 | 5 | CLIENT-04, COMPAT-03 | T-09-01, T-09-02, T-09-04 | RPC shutdown invokes Phase A while owner is moved, then Phase B after restoration | RPC/protocol integration | `cargo test -p pi-coding-agent --test rpc_mode --quiet && cargo test -p pi-coding-agent --test protocol_events --quiet && cargo test -p pi-coding-agent --lib protocol::rpc --quiet` | Yes | pending |
| 09-07-01 | 07 | 5 | CLIENT-04, COMPAT-03 | T-09-01, T-09-02 | Embedded UI exit is detach-only and never silently recovers or terminates runtime work | interactive integration | `cargo test -p pi-coding-agent --test interactive_mode embedded_interactive_lifecycle --quiet` | Yes | pending |
| 09-07-02 | 07 | 5 | CLIENT-04, COMPAT-03 | T-09-01, T-09-02 | Interactive process exit requests Phase A immediately and performs Phase B after owner return | interactive integration + lib | `cargo test -p pi-coding-agent --test interactive_mode --quiet && cargo test -p pi-coding-agent --lib interactive --quiet` | Yes | pending |
| 09-08-01 | 08 | 6 | GUARD-01 | T-09-05 | Recursive adapter discovery/classification exact sets fail on unlisted/stale roots | source guard | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards adapter --quiet` | Yes | pending |
| 09-08-02 | 08 | 6 | GUARD-02 | T-09-06 | Negative fixtures bind Cargo JSON code/symbol/primary span and positive neighbor passes | compile fixture | `cargo test -p pi-coding-agent --test api_boundary_guards external --quiet` | Yes | pending |
| 09-08-03 | 08 | 6 | COMPAT-03, CLIENT-04, CONTROL-02, GUARD-01, GUARD-02 | T-09-01 through T-09-06 | Full authority, private Compact cancellation reachability/privacy, security, source, compatibility, formatting, crate/workspace, and diff gates pass | full verification | `cargo test -p pi-coding-agent compact_cancellation --lib --quiet && cargo test -p pi-coding-agent --test public_api --test operation_association --test api_boundary_guards --test event_boundary_guards --test product_runtime_boundary_guards --test rpc_mode --test protocol_events --test interactive_mode --quiet && cargo fmt --all --check && cargo test -p pi-coding-agent --quiet && cargo test --workspace --quiet && cargo check --workspace && git diff --check` | Yes | pending |

*Status: pending -> green/red/flaky. Planner may split rows into additional tasks, but every row must retain an automated owner.*

---

## Wave 0 Requirements

- [ ] Install compile-ready public lifecycle/anchor values in Wave 1; add detach, receiver wake-up, stale/detached rejection, and preservation fixtures in the same tasks as their Wave 2 implementation.
- [ ] Add a closed 15-operation association descriptor and unit test in Wave 1; introduce exact root-terminal, OutcomeOnly acknowledgement, and PartialCommit/TerminalUncertain integration fixtures only beside their Wave 3 implementation.
- [ ] For Compact, create a cloneable crate-private `CompactCancellationHandle` before session-owner transfer; bind `OperationControl::begin` to the admission id plus monotonic generation and Compact-only token; make `cancel(operation_id)` fail closed for no-active, non-Compact, stale, or mismatched identities; carry the permit token through `ManualCompactionOptions` into `FlowRunOptions.cancel`; and map `FlowError::Cancelled` to `CodingSessionError::Cancelled`. Prove the real canonical cancellation path in `coding_session/mod.rs`'s colocated lib test by reading the Running id from a retained submission connection. Keep external `operation_association` tests limited to public success/provider-failure/cardinality/PartialCommit evidence.
- [ ] Add the runtime-shutdown variant inside the existing Runtime family and atomically update every exhaustive inner-variant consumer: `public_event.rs` naming matches, `coding_session/mod.rs::typed_event_kind`, `CodingProtocolEventAdapter::push_typed`, and `CodingEventBridge::handle_typed`; the protocol and interactive compile-ready arms emit no synthetic legacy protocol/UI content. In the same task update the public fixture/executable ledger, every `event_boundary_guards.rs` count, and `docs/product-event-contract.md`, then run the event-contract, colocated interactive bridge, and inventory guards. `tests/public_api.rs::typed_event_family` is family-level read-only evidence, fixture-only integration consumers remain read-only, RPC wire routing remains Plan 06, and ownership-specific Interactive lifecycle behavior remains Plan 07.
- [ ] Add deterministic shutdown gates in the same task as Phase A/Phase B implementation using channels/oneshots; do not use wall-clock sleeps.
- [ ] Prove the opaque shutdown-request handle closes Phase A admission/control while `run(&mut self)` owns the unique session, then prove the restored owner completes Phase B.
- [ ] Add compile-ready RPC wire snapshots first; add RPC and Interactive behavior fixtures in the same executable tasks as adapter migration.
- [ ] Extend external consumer fixtures with adjacent positive cases and Cargo JSON diagnostic assertions.

Existing Rust/Tokio test infrastructure is sufficient; no new framework or dependency is required.

---

## Manual-Only Verifications

All phase behaviors have automated verification. TUI lifecycle behavior is exercised through deterministic adapter/owner tests rather than a manual terminal session.

---

## Validation Sign-Off

- [x] All planned behavior families have an automated test owner or explicit Wave 0 dependency.
- [x] Sampling continuity prevents three consecutive implementation tasks without automated verification.
- [x] Every missing lifecycle/association/adapter fixture is owned by the same task as the production behavior that makes its automated command green; compile-ready value/descriptor contracts land first.
- [x] No watch-mode flags are used.
- [x] Focused feedback latency target is under 60 seconds.
- [x] `nyquist_compliant: true` is set in frontmatter.

**Approval:** approved 2026-07-14 for planning; execution status remains pending.
