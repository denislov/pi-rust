---
phase: 07-adapter-migration-and-compatibility-deletion
audited: 2026-07-13
status: secured
threats_total: 16
threats_closed: 16
threats_open: 0
asvs_level: 1
block_on: high
threat_register_origin: plan_time
threat_register_sources:
  - 07-01-PLAN.md
  - 07-02-PLAN.md
  - 07-03-PLAN.md
  - 07-04-PLAN.md
  - 07-05-PLAN.md
audit_mode: GENERIC-AGENT WORKAROUND
---

# Phase 7 Security Audit

## Scope and Method

This State B audit verifies the 16 threats registered at plan time in the five Phase 7
`<threat_model>` blocks. It does not add unrelated vulnerability-scanning scope. Verification used
OWASP ASVS Level 1 depth: each declared mitigation had to be present in the cited runtime,
adapter, facade, or test boundary. The blocking threshold is `high`.

The audit treated all mitigations as absent until current implementation and test evidence was
found. It also rechecked the two blockers and one warning reported in `07-REVIEW.md` against the
gap-fix commits `376b600` and `6a7eac1`; commit messages alone were not accepted as evidence.

## Threat Verification

| Threat ID | Category | Severity | Disposition | Status | Evidence |
|---|---|---:|---|---|---|
| T-07-01 | Tampering | high | mitigate | CLOSED | The raw-to-typed conversion is an exhaustive `match` beginning at `crates/pi-coding-agent/src/coding_session/public_event.rs:828`; text, tool arguments, summaries, errors, IDs, and usage fields are cloned field-by-field (representative payload copies at `public_event.rs:1209-1305`). `crates/pi-coding-agent/tests/event_boundary_guards.rs:31-62` requires the 45-row inventory and rejects a wildcard conversion, while typed payload suites pass. |
| T-07-02 | Information Disclosure | high | mitigate | CLOSED | `pi_coding_agent::api` exports the typed event envelope and payload types but not `CodingAgentEvent` (`crates/pi-coding-agent/src/lib.rs:64-93`). The fail-closed stable-facade and adapter guard is `crates/pi-coding-agent/tests/event_boundary_guards.rs:508-530`; the raw value remains only at the private `EventService::emit` admission boundary (`event_service.rs:171-188`). |
| T-07-03 | Spoofing | medium | mitigate | CLOSED | `EventService::emit` derives `operation_id`, terminal status, and durability from the admitted event before constructing the envelope (`event_service.rs:171-185`); adapter callers cannot supply those metadata fields. Typed terminal association is derived from the payload in `coding_session/event.rs`, and `product_event_contract.rs:65-112` binds operation identity, durability, terminal status, terminal association, and serialized identity. |
| T-07-04 | Tampering | high | mitigate | CLOSED | `CodingProtocolEventAdapter::push_product_event` accepts `CodingAgentProductEvent` and directly matches its typed kind (`crates/pi-coding-agent/src/protocol/events.rs:49-54`); representative tool payloads are copied to protocol values, with malformed JSON becoming the established `null` data value rather than control flow. `tests/protocol_events.rs:71-1005` exercises the typed path and exact payload/order behavior. |
| T-07-05 | Denial of Service | high | mitigate | CLOSED | The RPC queue remains an `mpsc` channel with fixed capacity 128 (`protocol/rpc/event_queue.rs:4,18-24`), preserves FIFO event delivery, and carries explicit `Overflow { skipped }` (`event_queue.rs:32-45`). Overflow is projected as `event_stream_lag` with `recovery: fresh_snapshot` (`protocol/rpc/prompt.rs:1043-1056`); bounded-order/overflow and RPC regression suites pass. |
| T-07-06 | Information Disclosure | medium | mitigate | CLOSED | Machine projection consumes only the established public typed payload hierarchy (`protocol/events.rs:1-15,49-54`). The stable facade excludes raw `CodingAgentEvent` (`lib.rs:64-93`), and `event_boundary_guards.rs:508-530` rejects raw facade exports, public raw adapter signatures, and raw-event first-party adapter fixtures. |
| T-07-07 | Tampering | high | mitigate | CLOSED | `CodingEventBridge::handle_product_event` accepts the typed envelope and dispatches on its typed event (`interactive/event_bridge.rs:152-160`). Message/tool/error text is projected as data; malformed tool JSON is preserved as a string rather than executed or treated as a command (`tests/interactive_event_bridge.rs:271-292`). The broad UI suite now enters through typed fixtures and checks exact usage, tool, delegation, compaction, self-healing, failure, recovery, and no-op projections. |
| T-07-08 | Repudiation | high | mitigate | CLOSED | Recovery publication carries both exact `operation_id` and `recovery_id` (`event_service.rs:708-715`), and typed recovery assertions exist in `coding_session/mod.rs:1893-1905`. Exact `PartialCommit` operation attribution is asserted in `coding_session/mod.rs:3160-3175`, `4310-4321`, and `interactive/loop.rs:2800-2880`; the focused session and interactive-loop suites pass. |
| T-07-09 | Spoofing | medium | mitigate | CLOSED | Session/profile transitions remain EventService/session-owned; the pre-fork receiver continuity and monotonic sequence assertions are retained at `coding_session/mod.rs:3052-3108`. Typed session/navigation and interactive session suites pass, with no adapter API for synthesizing envelope ownership metadata. |
| T-07-10 | Denial of Service | high | mitigate | CLOSED | The single product broadcast is bounded (`event_service.rs:83-100`), replay retention evicts at the configured bound (`event_service.rs:153-168`), and publication retains before broadcasting under the sequence lock (`event_service.rs:171-188`). Tests cover retained resume/gap, zero retention, concurrent order, bounded window, and lag-to-snapshot recovery (`event_service.rs:1210-1299,2395-2433`). |
| T-07-11 | Information Disclosure | high | mitigate | CLOSED | The legacy raw receiver/subscription and duplicate raw sender are absent and guarded at `event_boundary_guards.rs:576-615`; recursive guards reject raw compatibility storage/transport and require exactly one conversion inside private `EventService::emit` (`event_boundary_guards.rs:618-674`). The curated `api` facade contains typed receiver/types only (`lib.rs:64-93`). |
| T-07-12 | Repudiation | high | mitigate | CLOSED | Startup recovery, session durability, navigation, receiver continuity, and exact operation/recovery facts remain asserted in typed session/EventService/public tests. `product_event_contract.rs:35-112` binds pending/committed/skipped durability to operation/session identity; `coding_session/mod.rs:1893-1946,3052-3108,4061-4230` retains recovery/navigation/durable-log evidence. |
| T-07-13 | Tampering | high | mitigate | CLOSED | `EventService::emit` performs the sole production raw-to-typed conversion exactly once before retention/broadcast (`event_service.rs:171-188`). `event_boundary_guards.rs:618-674` rejects raw retained clones, accessors, transports, and obsolete conversion names and counts the conversion call; `event_boundary_guards.rs:31-116` locks the exhaustive 45-event inventory. |
| T-07-14 | Information Disclosure | high | mitigate | CLOSED | Raw events are absent from the stable facade and public adapter signatures (`lib.rs:64-93`, `protocol/events.rs:49-54`, `interactive/event_bridge.rs:156-160`). Source guards at `event_boundary_guards.rs:508-530,576-674` and passing `public_api`, protocol, JSON, RPC, and typed serialization suites cover the external boundary. |
| T-07-15 | Denial of Service | high | mitigate | CLOSED | Retained replay and live delivery remain independently bounded (`event_service.rs:83-100,127-168`; `event_queue.rs:4-45`). Replay rejects stale cursors with typed `EventStreamGap` (`event_service.rs:127-150`), and RPC lag/overflow requires a fresh snapshot (`protocol/rpc/prompt.rs:1043-1056`). Focused replay, queue, RPC, and session tests pass. |
| T-07-16 | Repudiation | high | mitigate | CLOSED | Envelope construction preserves sequence, operation ID, terminal status, and durability together (`event_service.rs:171-188`). `product_event_contract.rs:65-112,160-168` asserts exact durability/operation association and contiguous sequence; typed session and UI tests preserve recovery IDs and `PartialCommit` operation IDs (`coding_session/mod.rs:1893-1905,3160-3175,4310-4321`; `interactive/loop.rs:2800-2880`). |

## Cross-Boundary Conclusions

- Raw internal `CodingAgentEvent` cannot cross the stable facade or public protocol/UI adapter
  entry points. It remains intentionally private as the input to `EventService::emit` and in
  explicitly `cfg(test)` internal constructors.
- Untrusted model/tool text is copied into owned typed payload fields and projected as output data.
  The adapters do not derive operation identity, durability, terminal ownership, or control
  decisions from that text. Malformed tool arguments retain the established data-only fallback.
- Live broadcast, retained replay, and RPC forwarding remain bounded. Lag and stale replay produce
  explicit recovery semantics instead of unbounded buffering or silent continuation.
- Sequence allocation, retention-before-broadcast ordering, operation/recovery attribution,
  durability state, and `PartialCommit` operation identity remain intact and covered by typed tests.
- Source guards reject reintroduction of raw facade exports, public raw adapter signatures,
  compatibility storage/receivers/broadcasts, and non-exhaustive inventory drift.

## Accepted Risks

None. All plan-time threats use the `mitigate` disposition, and all declared mitigations are
present. No `accept` or `transfer` entries require a risk-owner or transfer record.

The transitional serialized `family` and `kind` fields are intentionally retained as documented
wire compatibility fields generated from the typed payload. They are not raw-event storage and do
not constitute an accepted open threat in this register.

## Threat Flags and Unregistered Surface

The five Phase 7 summaries contain no `## Threat Flags` entries. The implementation review's two
blockers and one coverage warning map to existing registered threats T-07-02/T-07-11/T-07-14
(raw facade/adapter exposure) and T-07-07/T-07-13 (typed UI projection drift). Current code and the
gap-fix tests close those mapped findings. No unregistered flag was found.

## Audit Trail

| Date | Action | Result |
|---|---|---|
| 2026-07-13 | Loaded all five plan threat registers, all five summaries, `07-REVIEW.md`, `07-VALIDATION.md`, `.planning/REQUIREMENTS.md`, `docs/product-event-contract.md`, ASVS-level guidance, and gap-fix commits `376b600`/`6a7eac1`. | 16 plan-time threats classified; no summary threat flags. |
| 2026-07-13 | Used CodeGraph before direct source inspection to trace raw admission, typed conversion, public facade/adapters, replay/overflow, sequence/durability, recovery, and guards. | Required controls located in current source and verified at their declared boundaries. |
| 2026-07-13 | Ran `cargo test -p pi-coding-agent --test event_boundary_guards --test product_event_contract --test protocol_events --test interactive_event_bridge --test public_api --test json_mode --test rpc_mode --test interactive_sessions --quiet`. | PASS: 143 tests. |
| 2026-07-13 | Ran focused library suites for `public_event`, `coding_session::event_service::tests`, `coding_session::tests`, `interactive::r#loop::tests`, and `protocol::rpc::event_queue`. | PASS: 3 + 25 + 56 + 9 + 2 tests. |
| 2026-07-13 | Ran `cargo fmt --check` and `git diff --check`. | PASS; only pre-existing `load_plugins`/`ensure_idle` dead-code warnings were emitted. |

## Gate Decision

`threats_open: 0`. All 16 registered threats are CLOSED at ASVS Level 1, including every high
severity threat at or above the `block_on: high` threshold. Phase 7 is **SECURED**.
