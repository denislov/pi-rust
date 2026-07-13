---
phase: 07-adapter-migration-and-compatibility-deletion
reviewed: 2026-07-13T11:57:11Z
depth: standard
files_reviewed: 22
files_reviewed_list:
  - crates/pi-coding-agent/src/coding_session/event.rs
  - crates/pi-coding-agent/src/coding_session/event_service.rs
  - crates/pi-coding-agent/src/coding_session/mod.rs
  - crates/pi-coding-agent/src/coding_session/public_event.rs
  - crates/pi-coding-agent/src/interactive/event_bridge.rs
  - crates/pi-coding-agent/src/interactive/loop.rs
  - crates/pi-coding-agent/src/interactive/prompt_task.rs
  - crates/pi-coding-agent/src/lib.rs
  - crates/pi-coding-agent/src/protocol/events.rs
  - crates/pi-coding-agent/src/protocol/json_mode.rs
  - crates/pi-coding-agent/src/protocol/rpc/event_queue.rs
  - crates/pi-coding-agent/src/protocol/rpc/events.rs
  - crates/pi-coding-agent/src/protocol/rpc/prompt.rs
  - crates/pi-coding-agent/src/protocol/rpc/state.rs
  - crates/pi-coding-agent/tests/event_boundary_guards.rs
  - crates/pi-coding-agent/tests/interactive_event_bridge.rs
  - crates/pi-coding-agent/tests/product_event_contract.rs
  - crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs
  - crates/pi-coding-agent/tests/protocol_events.rs
  - crates/pi-coding-agent/tests/public_api.rs
  - crates/pi-coding-agent/tests/support/mod.rs
  - docs/product-event-contract.md
findings:
  critical: 0
  warning: 0
  info: 0
  total: 0
status: clean
---

# Phase 7: Code Review Report

**Reviewed:** 2026-07-13T11:57:11Z
**Depth:** standard
**Files Reviewed:** 22
**Status:** clean

## Summary

This is the focused post-fix re-review of the complete Phase 7 source scope and the corrective commits `376b600` (`fix(07): close raw event adapter bypasses`) and `6a7eac1` (`test(07): exercise typed product event adapters`). The review independently traced the stable facade, protocol/JSON/RPC projection, interactive projection, private raw admission boundary, typed test fixtures, and compatibility deletion guards. CodeGraph was used before direct source inspection.

All three findings from the original 2026-07-13 review are resolved. The stable facade and public adapters no longer expose `CodingAgentEvent`; the typed interactive usage path preserves the aggregate-token fallback; and protocol/interactive behavior suites now exercise the same typed matcher used by live product-event delivery. No new correctness defect, security issue, API leak, raw adapter bypass, duplicate projection implementation, or test-reliability regression was found in the fixes.

All reviewed files meet the Phase 7 quality and compatibility requirements. No active issues remain.

## Narrative Findings (AI reviewer)

No active Critical, Warning, or Info findings.

## Resolved Findings Audit Trail

### CR-01: [BLOCKER — RESOLVED] Raw stable-facade and adapter bypasses

**Originally reported at:** `crates/pi-coding-agent/src/lib.rs:72`, `crates/pi-coding-agent/src/protocol/events.rs:46`, `crates/pi-coding-agent/src/interactive/event_bridge.rs:162`, `crates/pi-coding-agent/tests/public_api.rs:14`, and `crates/pi-coding-agent/tests/protocol_events.rs:108` in the pre-fix tree.

**Resolution:** Commit `376b600` removes `CodingAgentEvent` from `pi_coding_agent::api`, deletes the production raw `CodingProtocolEventAdapter::push` and `CodingEventBridge::handle` paths, and leaves one typed matcher in each adapter. Current public entry points accept `CodingAgentProductEvent`; internal live paths accept private `ProductEvent` and delegate to the same typed matcher. Commit `6a7eac1` removes raw-event use from the external protocol, interactive, and public-API behavior suites and adds boundary checks for the stable facade, known adapter signatures, and first-party integration fixtures.

**Independent closure evidence:**

- `crates/pi-coding-agent/src/lib.rs:64-93` exports the typed event hierarchy and receiver but not `CodingAgentEvent`.
- `crates/pi-coding-agent/src/protocol/events.rs:41-57` provides internal/public typed entry points that both delegate to `push_typed`; no production raw projection method remains.
- `crates/pi-coding-agent/src/interactive/event_bridge.rs:152-160` provides internal/public typed entry points that both delegate to `handle_typed`; the previous parallel raw matcher is deleted.
- `crates/pi-coding-agent/src/protocol/json_mode.rs:82-117,189-223` and `crates/pi-coding-agent/src/protocol/rpc/events.rs:5-19` consume private `ProductEvent` values from the typed subscription/runtime path.
- `crates/pi-coding-agent/tests/protocol_events.rs` and `crates/pi-coding-agent/tests/interactive_event_bridge.rs` contain no `CodingAgentEvent` reference and invoke `push_product_event` / `handle_product_event` with typed envelopes.
- `crates/pi-coding-agent/tests/event_boundary_guards.rs:508-530` rejects the stable raw export, the deleted raw adapter signatures, and raw first-party adapter integration fixtures. The broader storage/receiver/transport guard at lines 618-678 continues to constrain raw state to the private admission/test-fixture boundary.

### CR-02: [BLOCKER — RESOLVED] Typed interactive context-token fallback

**Originally reported at:** `crates/pi-coding-agent/src/interactive/event_bridge.rs:500-507` in the pre-fix tree.

**Resolution:** Commit `376b600` changes the single typed UI matcher to call `calculate_context_tokens(&CodingAgentProductEventUsage)`. The helper prefers a non-zero `total_tokens` value and otherwise uses saturating addition across input, output, cache-read, and cache-write counters.

**Independent closure evidence:**

- `crates/pi-coding-agent/src/interactive/event_bridge.rs:126-139,178-199` implements and uses the behavior-compatible fallback on the live typed path.
- `crates/pi-coding-agent/tests/interactive_event_bridge.rs:128-162` supplies `total_tokens = 0` with component counts `30 + 20 + 5 + 0` and asserts `context_tokens: Some(55)` through `CodingAgentProductEvent -> handle_product_event`.
- The same suite also asserts that an all-zero usage snapshot maps to `context_tokens: None`, preserving the unknown-context case.

### WR-01: [WARNING — RESOLVED] Duplicate raw/typed UI matchers and live-path coverage gap

**Originally reported at:** the two matchers formerly occupying `crates/pi-coding-agent/src/interactive/event_bridge.rs:162-760` and the raw-driven assertions formerly in `crates/pi-coding-agent/tests/interactive_event_bridge.rs:49-537`.

**Resolution:** Commit `376b600` deletes the approximately 300-line raw matcher instead of maintaining a second projection. Both internal `ProductEvent` delivery and the public typed envelope now converge on `handle_typed`. Commit `6a7eac1` migrates the broad integration suite to typed product-event fixtures.

**Independent closure evidence:**

- The bridge has one semantic matcher, `handle_typed`, with no raw compatibility projection implementation.
- The typed integration suite covers assistant text/thinking/usage, aggregate and component-token fallback, zero usage, tool start/update/completion/failure, malformed tool arguments, prompt failure/abort, compaction, delegation confirmation/lifecycle folding, self-healing lifecycle, recovery, and established no-op event families.
- Protocol integration tests likewise enter through typed envelopes and preserve message/thinking, provider/model, tool, delegation, compaction, self-healing, capability, failure, `TurnEnd`, and `AgentEnd` assertions.

## Verification

- `cargo test -p pi-coding-agent --test event_boundary_guards --test product_event_contract --test protocol_events --test interactive_event_bridge --test public_api --test json_mode --test rpc_mode --test interactive_sessions --quiet` — PASS, 143 tests.
- `cargo test -p pi-coding-agent --lib public_event --quiet` — PASS, 3 tests.
- `cargo test -p pi-coding-agent --lib coding_session::event_service::tests --quiet` — PASS, 25 tests.
- `cargo test -p pi-coding-agent --lib coding_session::tests --quiet` — PASS, 56 tests.
- `cargo test -p pi-coding-agent --lib interactive::r#loop::tests --quiet` — PASS, 9 tests.
- `cargo test -p pi-coding-agent --lib protocol::rpc::event_queue --quiet` — PASS, 2 tests.
- Focused total — PASS, 238 distinct tests.
- `cargo fmt --check` — PASS.
- `git diff --check` — PASS.

The only emitted diagnostics were the pre-existing `load_plugins` and `ensure_idle` dead-code warnings; neither is part of the Phase 7 fixes or an active review finding.

---

_Reviewed: 2026-07-13T11:57:11Z_
_Reviewer: the agent (gsd-code-reviewer; GENERIC-AGENT WORKAROUND)_
_Depth: standard_
