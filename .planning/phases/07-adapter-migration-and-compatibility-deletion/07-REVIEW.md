---
phase: 07-adapter-migration-and-compatibility-deletion
reviewed: 2026-07-13T10:27:22Z
depth: standard
files_reviewed: 19
files_reviewed_list:
  - crates/pi-coding-agent/src/coding_session/event.rs
  - crates/pi-coding-agent/src/coding_session/event_service.rs
  - crates/pi-coding-agent/src/coding_session/mod.rs
  - crates/pi-coding-agent/src/coding_session/public_event.rs
  - crates/pi-coding-agent/src/interactive/event_bridge.rs
  - crates/pi-coding-agent/src/interactive/loop.rs
  - crates/pi-coding-agent/src/interactive/prompt_task.rs
  - crates/pi-coding-agent/src/protocol/events.rs
  - crates/pi-coding-agent/src/protocol/json_mode.rs
  - crates/pi-coding-agent/src/protocol/rpc/event_queue.rs
  - crates/pi-coding-agent/src/protocol/rpc/events.rs
  - crates/pi-coding-agent/src/protocol/rpc/prompt.rs
  - crates/pi-coding-agent/src/protocol/rpc/state.rs
  - crates/pi-coding-agent/tests/event_boundary_guards.rs
  - crates/pi-coding-agent/tests/product_event_contract.rs
  - crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs
  - crates/pi-coding-agent/tests/protocol_events.rs
  - crates/pi-coding-agent/tests/public_api.rs
  - docs/product-event-contract.md
findings:
  critical: 2
  warning: 1
  info: 0
  total: 3
status: issues_found
---

# Phase 7: Code Review Report

**Reviewed:** 2026-07-13T10:27:22Z
**Depth:** standard
**Files Reviewed:** 19
**Status:** issues_found

## Summary

The Phase 7 diff was reviewed against `b2a2fe8..HEAD`, the actual task commits, the 45-event contract, and the focused adapter/API/source-guard suites. Sequence assignment, retention-before-broadcast ordering, typed durability metadata, replay/lag plumbing, `PartialCommit` attribution, and the five existing root-terminal associations were traced through the changed runtime and tests.

The focused gate currently passes (21 event-boundary tests, 11 interactive bridge tests, 1 product-event contract test, 12 protocol tests, and 23 public-API tests), but it does not establish the claimed compatibility closure. The stable facade and first-party tests still expose and consume the raw `CodingAgentEvent` path, and the live typed interactive projection has a concrete usage/context-token regression that the raw-handler tests cannot see.

## Narrative Findings (AI reviewer)

## Critical Issues

### CR-01: [BLOCKER] The raw compatibility event path remains public and first-party tests still depend on it

**Files:** `crates/pi-coding-agent/src/lib.rs:72`; `crates/pi-coding-agent/src/protocol/events.rs:46`; `crates/pi-coding-agent/src/interactive/event_bridge.rs:162`; `crates/pi-coding-agent/tests/public_api.rs:14`; `crates/pi-coding-agent/tests/protocol_events.rs:108`

**Issue:** Phase 7 claims that `CodingAgentEvent` is only the private `EventService::emit` admission value and that first-party adapters/tests consume typed product events. In the shipped surface, however, `pi_coding_agent::api` still re-exports `CodingAgentEvent`; `CodingProtocolEventAdapter::push` remains a public raw-event entry point; `CodingEventBridge::handle` remains a public raw-event entry point; `public_api.rs` explicitly asserts that the raw enum is importable; and essentially every protocol integration assertion calls `adapter.push(&CodingAgentEvent)` rather than exercising the sequenced product-event boundary. This is not merely a stale symbol: callers can still bypass `ProductEvent` sequence, terminal-operation, and durability metadata, while the first-party tests certify that bypass as stable API. It contradicts COMPAT-01/02, the plan threat model, and the project rule that new/migrated callers use the typed `pi_coding_agent::api` contract.

The final guard does not detect this leak. Its forbidden list at `event_boundary_guards.rs:596-603` rejects receiver/storage spellings but not a `CodingAgentEvent` facade export or a public raw adapter signature, so the guard passes while the compatibility path remains supported.

**Fix:** Remove `CodingAgentEvent` from `pi_coding_agent::api` and make raw projection methods private to the one internal admission/conversion boundary (or delete them). Migrate protocol and interactive behavior tests to events observed from `CodingAgentSession::subscribe_product_events_public()` or to a narrowly `cfg(test)` typed fixture that takes `CodingAgentProductEventKind`; do not preserve a production raw-event adapter just to serve tests. Extend the source guard to reject `CodingAgentEvent` in the stable facade and public adapter signatures while explicitly allowing only the private enum definition, the exhaustive conversion, and `EventService::emit`.

### CR-02: [BLOCKER] The live typed interactive path drops the established context-token fallback

**File:** `crates/pi-coding-agent/src/interactive/event_bridge.rs:500-507`

**Issue:** Before the migration, an `AssistantMessageCompleted` event computed context size with `calculate_context_tokens`, which returns `usage.total_tokens` when present and otherwise saturating-sums `input`, `output`, `cache_read`, and `cache_write` (`event_bridge.rs:130-139`, used by the retained raw handler at lines 171-175). The new live `ProductEvent` path instead treats `usage.total_tokens == 0` as `None` without considering non-zero component counters. Providers or fixtures that report component usage but omit the aggregate therefore lose the footer's current-context value after the typed migration, even though the same event projected through the old handler still reports it. This is a direct behavior regression in the human-facing adapter.

The current tests miss it because `interactive_event_bridge.rs` exercises `bridge.handle(&CodingAgentEvent)` for usage events, while the only product-event bridge test covers an assistant text delta rather than usage completion.

**Fix:** Preserve the fallback in the typed branch, for example by adding a helper over `CodingAgentProductEventUsage` that returns `total_tokens` when non-zero and otherwise uses saturating addition of the four component counts. Add a regression through `ProductEvent -> push_product_event` with `total_tokens = 0` and non-zero component counts, asserting the same `UiEvent::UsageUpdate.context_tokens` value as the pre-migration behavior.

## Warnings

### WR-01: The live typed UI matcher is duplicated from, but barely tested compared with, the retained raw matcher

**Files:** `crates/pi-coding-agent/src/interactive/event_bridge.rs:162-479`; `crates/pi-coding-agent/src/interactive/event_bridge.rs:482-760`; `crates/pi-coding-agent/tests/interactive_event_bridge.rs:49-537`

**Issue:** `CodingEventBridge` now contains two independently maintained, hundreds-of-lines projections for the same semantic events: the old raw `handle` matcher and the new `handle_typed` matcher used by live `ProductEvent` delivery. The integration suite has more than twenty calls to the raw matcher and no calls to the live product-event entry point; only one co-located product-event test exists, and it checks a simple assistant delta. The two implementations have already drifted (CR-02). Future payload, formatting, no-op, or delegation changes can continue passing the large bridge suite while breaking the actual live-session path.

**Fix:** Make the typed matcher the single projection implementation. Delete the production raw handler, or gate a test-only raw helper that immediately performs the exhaustive raw-to-typed conversion and delegates to the typed matcher. Migrate the existing usage, tool, delegation, compaction, failure, recovery, and self-healing cases so their assertions enter through a `ProductEvent`/typed fixture. This will turn the current behavior suite into coverage of the runtime path instead of coverage of a parallel compatibility implementation.

---

_Reviewed: 2026-07-13T10:27:22Z_
_Reviewer: the agent (gsd-code-reviewer; GENERIC-AGENT WORKAROUND)_
_Depth: standard_
