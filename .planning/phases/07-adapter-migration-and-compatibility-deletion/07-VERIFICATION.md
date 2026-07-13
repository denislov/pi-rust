---
phase: 07-adapter-migration-and-compatibility-deletion
verified: 2026-07-13T12:05:24Z
status: passed
score: 12/12 must-haves verified
behavior_unverified: 0
overrides_applied: 0
gaps: []
requirement_coverage:
  - id: COMPAT-01
    status: satisfied
    evidence: "Typed protocol, JSON/RPC, interactive, public-facade, session, and first-party adapter tests pass through product-event inputs; print remains outcome-driven and has no compatibility event dependency."
  - id: COMPAT-02
    status: satisfied
    evidence: "Legacy receiver/subscription, duplicate raw broadcast, retained raw storage, stable raw export, and production raw adapter methods are absent and fail-closed by source guards."
artifacts:
  - path: crates/pi-coding-agent/src/coding_session/event.rs
    status: verified
    provides: "Typed-only ProductEvent envelope, independent metadata, and exactly five root-terminal associations"
  - path: crates/pi-coding-agent/src/coding_session/event_service.rs
    status: verified
    provides: "Single raw admission/conversion boundary and one retained typed broadcast"
  - path: crates/pi-coding-agent/src/protocol/events.rs
    status: verified
    provides: "Shared typed protocol matcher used by JSON and RPC"
  - path: crates/pi-coding-agent/src/interactive/event_bridge.rs
    status: verified
    provides: "Single typed UI matcher with behavior-compatible usage fallback"
  - path: crates/pi-coding-agent/src/lib.rs
    status: verified
    provides: "Stable typed facade without CodingAgentEvent or a legacy receiver"
  - path: crates/pi-coding-agent/tests/event_boundary_guards.rs
    status: verified
    provides: "Recursive deletion, facade, adapter, inventory, and exactly-once conversion guards"
  - path: crates/pi-coding-agent/tests/protocol_events.rs
    status: verified
    provides: "Typed protocol payload and ordering regressions"
  - path: crates/pi-coding-agent/tests/interactive_event_bridge.rs
    status: verified
    provides: "Typed UI projection regressions, including total_tokens=0 component fallback"
  - path: docs/product-event-contract.md
    status: verified
    provides: "45-event typed inventory, transitional wire fields, and five root-terminal mappings"
commands:
  - command: "cargo test -p pi-coding-agent --test event_boundary_guards --test product_event_contract --test protocol_events --test interactive_event_bridge --test public_api --test json_mode --test rpc_mode --test interactive_sessions --quiet"
    result: "passed: 143 tests"
  - command: "cargo test -p pi-coding-agent --lib public_event --quiet; cargo test -p pi-coding-agent --lib coding_session::event_service::tests --quiet; cargo test -p pi-coding-agent --lib coding_session::tests --quiet; cargo test -p pi-coding-agent --lib interactive::r#loop::tests --quiet; cargo test -p pi-coding-agent --lib protocol::rpc::event_queue --quiet"
    result: "passed: 3 + 25 + 56 + 9 + 2 tests"
  - command: "cargo test -p pi-coding-agent --lib coding_session::event --quiet"
    result: "passed: 37 tests"
  - command: "cargo test -p pi-coding-agent --test interactive_event_bridge coding_event_bridge_maps_assistant_events --quiet"
    result: "passed: typed total_tokens=0 fallback assertion"
  - command: "cargo test -p pi-coding-agent --test event_boundary_guards stable_facade_and_adapters_reject_raw_event_projection --quiet"
    result: "passed: stable facade and public raw adapter rejection"
  - command: "cargo fmt --check"
    result: passed
  - command: "cargo test --workspace --quiet"
    result: "passed: all workspace targets; pi-coding-agent library 657 passed, 1 ignored"
  - command: "cargo check --workspace"
    result: "passed with pre-existing load_plugins/ensure_idle dead-code warnings"
  - command: "git diff --check"
    result: passed
assessment: "Phase goal achieved deterministically; no human verification or closure gap remains."
---

# Phase 7: Adapter Migration and Compatibility Deletion Verification Report

**Phase Goal:** Migrate all first-party event consumers to typed product events and remove the compatibility receiver/subscription path without changing observable behavior.

**Verified:** 2026-07-13T12:05:24Z
**Status:** passed
**Re-verification:** No previous Phase 7 verification report existed. This initial verification includes independent review of the post-execution fixes `376b600` and `6a7eac1`.
**Verifier mode:** GENERIC-AGENT WORKAROUND for the typed `gsd-verifier` role.

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---:|---|---|---|
| 1 | The private live envelope owns one typed payload and independent sequence, operation, terminal, and durability metadata. | VERIFIED | `ProductEvent` contains only `CodingAgentProductEventKind` plus metadata; the raw constructor is `#[cfg(test)]` and the 37 event tests pass. |
| 2 | `EventService::emit` is the sole private raw admission boundary, converts exactly once, assigns sequence, retains, then broadcasts once. | VERIFIED | `event_service.rs:171-188` performs one `CodingAgentProductEventKind::from(&event)`, one `ProductEvent::new`, retain-before-send under the publication lock, and one `Sender<ProductEvent>` send. The recursive guard counts the conversion and rejects raw storage/transports. |
| 3 | RPC, JSON, and their shared protocol projection consume typed product events and preserve output/order. | VERIFIED | Internal `ProductEvent` and public `CodingAgentProductEvent` entry points converge on `push_typed`; JSON and RPC live paths call the typed internal entry. `protocol_events`, `json_mode`, and `rpc_mode` pass with typed integration fixtures. |
| 4 | Print behavior remains compatible without a legacy event dependency. | VERIFIED | `print_mode.rs` remains outcome-driven through `CodingAgentSession::run(CodingAgentOperation::Prompt)` and contains no raw event, subscription, or compatibility reference; workspace regressions pass. |
| 5 | Interactive projection consumes typed payloads through one matcher and preserves transcript/UI behavior. | VERIFIED | Internal/public typed entry points converge on `handle_typed`; the duplicate raw matcher is absent. The typed integration suite covers message/thinking, tools, delegation, compaction, self-healing, failures, recovery, and no-op families. |
| 6 | Interactive usage correctly falls back when `total_tokens == 0`. | VERIFIED | `calculate_context_tokens` saturating-sums input/output/cache-read/cache-write after a zero aggregate; the typed-envelope test asserts `30 + 20 + 5 + 0 = Some(55)` and the all-zero case remains `None`. |
| 7 | The stable `pi_coding_agent::api` facade exposes typed events/receiver but no raw `CodingAgentEvent` or legacy receiver/service. | VERIFIED | The `api` export region contains `CodingAgentProductEvent`, all typed families, and `CodingAgentProductEventReceiver`, but no `CodingAgentEvent`, raw receiver, EventService, or internal ProductEvent. Public API and boundary tests pass. |
| 8 | Compatibility storage, accessor, legacy subscription/receiver, duplicate broadcast, and path-specific migration suppression are absent from production. | VERIFIED | Recursive guards reject the deleted names/transports/conversions; direct source audit found only guard literals and `.product_sender.subscribe()`. Guarded migration files contain no `allow(deprecated)`. |
| 9 | Sequence, retain-before-broadcast, retained replay, lag/overflow, and fresh-snapshot recovery remain behavior-compatible. | VERIFIED | EventService tests cover cursor resume, stale gap, zero retention, bounded windows, concurrent monotonic order, and receiver lag; RPC queue/prompt tests preserve explicit `event_stream_lag` plus `recovery: fresh_snapshot`. |
| 10 | Durability, recovery, control/navigation continuity, and exact `PartialCommit` attribution remain intact. | VERIFIED | Real receiver contract covers pending/committed/skipped durability and serialization; typed session/interactive suites cover startup recovery IDs, original-receiver failed-fork continuity, navigation durability, abort/follow-up controls, and exact partial-commit operation IDs. |
| 11 | Transitional wire identity, the exhaustive 45-event inventory, and exactly five root-terminal associations remain unchanged. | VERIFIED | The four-layer inventory guard proves 45 source/fixture/executable/document rows; Serde tests retain legacy `family`/`kind` alongside typed snake_case payloads; event tests prove Prompt, Compact, SelfHealingEdit, AgentInvocation, and AgentTeam mappings while rejecting tool/session-write/message promotion. |
| 12 | Phase 7 did not expand into Phase 8/9, change lower crates, add dependencies, or alter schemas. | VERIFIED | `b2a2fe8..HEAD` changes only `pi-coding-agent`, tests, planning/debug evidence, and the product-event contract; no manifest, lockfile, `pi-ai`, `pi-agent-core`, or `pi-tui` file changed, and no new lifecycle/association API was introduced. |

**Score:** 12/12 truths verified; 0 present-but-behavior-unverified.

## Required Artifacts And Wiring

| Artifact | Existence | Substance | Wiring | Verification |
|---|---|---|---|---|
| `coding_session/event.rs` | Present | Typed envelope and exhaustive raw enum/conversion tests | Constructed only by EventService or the test-only fixture; projected by typed receivers/adapters | VERIFIED |
| `coding_session/event_service.rs` | Present | Sequence, retention, replay, lag, emit helpers, bounded receiver | Owned by `CodingAgentSession`; all live adapters subscribe to its ProductEvent stream | VERIFIED |
| `protocol/events.rs` | Present | One exhaustive typed stateful matcher | JSON, RPC, and public typed adapter entry points converge on it | VERIFIED |
| `interactive/event_bridge.rs` | Present | One typed UI matcher and usage fallback | Interactive loop applies ProductEvent through `UiProjection`; public typed tests call the same matcher | VERIFIED |
| `tests/event_boundary_guards.rs` | Present | Recursive and scoped fail-closed audits | Compiled and executed as a normal integration target in focused/workspace gates | VERIFIED |
| `tests/protocol_events.rs` | Present | Exact typed payload/order assertions | Calls public `push_product_event(CodingAgentProductEvent)`; no raw enum reference | VERIFIED |
| `tests/interactive_event_bridge.rs` | Present | Exact typed UI/transcript assertions | Calls public `handle_product_event(CodingAgentProductEvent)`; no raw enum reference | VERIFIED |
| `docs/product-event-contract.md` | Present | 45-row inventory and terminal/wire semantics | Parsed by source guards and checked against executable inventory | VERIFIED |

## Key Link Verification

| From | To | Link | Status |
|---|---|---|---|
| Emitters | `EventService::emit` | Private `CodingAgentEvent` admission only | WIRED |
| `EventService::emit` | `ProductEvent` | Exactly one exhaustive typed conversion, metadata derivation, retain, one broadcast | WIRED |
| Product receiver | JSON/RPC protocol adapter | `ProductEvent -> push_internal_product_event/push_product_event -> push_typed` | WIRED |
| Product receiver | Interactive projection | `ProductEvent -> UiProjection::apply_product_event -> push_product_event -> handle_typed` | WIRED |
| Internal ProductEvent | Stable receiver | `CodingAgentProductEvent::from_internal` pure typed projection | WIRED |
| Contract document | Boundary suite | Source/fixture/executable/document inventory set equality | WIRED |

## Requirements Coverage

| Requirement | Source Plans | Status | Evidence |
|---|---|---|---|
| COMPAT-01 | 07-01 through 07-05 | SATISFIED | Every applicable first-party live event projection and behavior suite uses typed product-event inputs; JSON/RPC/UI output and print outcome behavior remain green. |
| COMPAT-02 | 07-01, 07-04, 07-05 | SATISFIED | Receiver/subscription, duplicate broadcast, raw retained storage, raw facade export, and public raw adapter bypasses are deleted and guarded; only private admission and explicitly test-gated construction remain. |

No orphaned Phase 7 requirements were found. `COMPAT-03` is explicitly mapped to Phase 9; Phase 7 nevertheless preserves its existing regression contract without claiming Phase 9 association closure.

## Behavioral Evidence

| Behavior | Evidence | Result |
|---|---|---|
| Typed machine adapters and live mode outputs | 143-test focused integration command | PASS |
| Typed session/EventService/UI loop behavior | 95 focused library tests | PASS |
| Typed storage and five terminal associations | 37 `coding_session::event` tests | PASS |
| Zero aggregate usage fallback | Named typed integration test asserting `Some(55)` | PASS |
| Stable facade/raw adapter closure | Named boundary test | PASS |
| Full regression | `cargo test --workspace --quiet` | PASS |
| Build/format/diff | `cargo check --workspace`, `cargo fmt --check`, `git diff --check` | PASS |

No phase-declared or conventional migration probe script was referenced, so probe execution was not applicable.

## Adversarial / Disconfirmation Pass

Three plausible failure modes were checked explicitly:

1. **Passing tests could still exercise a raw bypass.** This was true before fixes `376b600`/`6a7eac1`; current external protocol and interactive suites contain no `CodingAgentEvent`, public raw adapter methods are gone, and guards reject their return.
2. **The typed UI matcher could silently differ from the deleted raw matcher.** The prior aggregate-token regression is closed by the component-sum fallback and a fail-capable typed-envelope `Some(55)` assertion; the broad typed suite covers each established projection family.
3. **Deleting the duplicate broadcast could reorder or lose replay state.** Publication remains serialized by one lock, retain occurs before send, and tests exercise concurrent monotonic sequences, replay windows, stale gaps, lag, and fresh-snapshot recovery.

The least-directly-proven path is print mode because it is intentionally outcome-driven rather than an event consumer. Direct inspection shows no compatibility dependency, and its unchanged canonical `run(Prompt)` path is covered by the workspace suite. The RPC unit module still uses the explicitly `#[cfg(test)]` raw constructor to synthesize private ProductEvent values; this is construction evidence, not a raw consumer or production bypass, and the external typed protocol suite independently exercises the public typed entry.

The only scanned placeholder (`interactive/loop.rs:93`) predates Phase 7 (`4133e055`) and concerns an unrelated extensions banner; it is not in the Phase 7 diff and does not affect the event migration goal. No new `TBD`, `FIXME`, `XXX`, `HACK`, empty implementation, dependency, or schema artifact was introduced.

## Human Verification

None required. All Phase 7 claims are deterministic, offline, and covered by executable behavior/source-contract tests; no visual quality, external service, or untested real-time invariant is part of this phase goal.

## Final Assessment

Phase 7 achieves its goal. Every first-party live event consumer now follows the typed product-event path; the compatibility receiver/subscription/storage and raw adapter bypasses are removed; `EventService::emit` is the single private raw admission boundary; and ordering, replay, overflow recovery, durability, control/navigation, recovery, PartialCommit attribution, transitional wire identity, the 45-event inventory, and the five existing root-terminal associations remain behavior-compatible. No Phase 8/9 scope expansion, lower-crate change, dependency change, or actionable gap was found.

The pre-existing user-owned changes to `.planning/STATE.md` and `docs/next stage.md` were not modified by verification.

---

_Verified: 2026-07-13T12:05:24Z_
_Verifier: the agent (gsd-verifier; GENERIC-AGENT WORKAROUND)_
