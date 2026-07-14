---
phase: 09-lifecycle-association-guards-and-closure
reviewed: 2026-07-14T00:00:00Z
depth: standard
files_reviewed: 34
files_reviewed_list:
  - crates/pi-coding-agent/src/coding_session/client_service.rs
  - crates/pi-coding-agent/src/coding_session/error.rs
  - crates/pi-coding-agent/src/coding_session/event_service.rs
  - crates/pi-coding-agent/src/coding_session/intent_router.rs
  - crates/pi-coding-agent/src/coding_session/manual_compaction_flow.rs
  - crates/pi-coding-agent/src/coding_session/mod.rs
  - crates/pi-coding-agent/src/coding_session/operation_control.rs
  - crates/pi-coding-agent/src/coding_session/public_event.rs
  - crates/pi-coding-agent/src/coding_session/public_operation.rs
  - crates/pi-coding-agent/src/coding_session/public_projection.rs
  - crates/pi-coding-agent/src/coding_session/session_log/transaction.rs
  - crates/pi-coding-agent/src/coding_session/session_service.rs
  - crates/pi-coding-agent/src/coding_session/snapshot_coordinator.rs
  - crates/pi-coding-agent/src/interactive/app.rs
  - crates/pi-coding-agent/src/interactive/event_bridge.rs
  - crates/pi-coding-agent/src/interactive/loop.rs
  - crates/pi-coding-agent/src/lib.rs
  - crates/pi-coding-agent/src/protocol/events.rs
  - crates/pi-coding-agent/src/protocol/rpc.rs
  - crates/pi-coding-agent/src/protocol/rpc/commands.rs
  - crates/pi-coding-agent/src/protocol/rpc/prompt.rs
  - crates/pi-coding-agent/src/protocol/rpc/state.rs
  - crates/pi-coding-agent/src/protocol/rpc/wire.rs
  - crates/pi-coding-agent/src/protocol/types.rs
  - crates/pi-coding-agent/src/tools/edit.rs
  - crates/pi-coding-agent/tests/api_boundary_guards.rs
  - crates/pi-coding-agent/tests/event_boundary_guards.rs
  - crates/pi-coding-agent/tests/interactive_mode.rs
  - crates/pi-coding-agent/tests/operation_association.rs
  - crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs
  - crates/pi-coding-agent/tests/protocol_events.rs
  - crates/pi-coding-agent/tests/public_api.rs
  - crates/pi-coding-agent/tests/rpc_mode.rs
  - docs/product-event-contract.md
findings:
  critical: 2
  warning: 0
  info: 0
  total: 2
status: issues_found
---

# Phase 09: Code Review Report

**Reviewed:** 2026-07-14
**Depth:** standard
**Files Reviewed:** 34
**Status:** issues_found

## Summary

The lifecycle and association implementation is broadly covered by deterministic tests, but two correctness defects remain in paths that are not exercised by the happy-path shutdown and partial-commit fixtures. Both can violate explicit Phase 9 durability/compatibility guarantees in production.

## Critical Issues

### CR-01: Shutdown can reject queued admitted terminal events

**File:** `crates/pi-coding-agent/src/coding_session/public_projection.rs:622-645`

**Issue:** `CodingAgentReconnectReceiver::ensure_delivery_live` only permits a delivery after `RuntimeShutDown` when that delivery is the shutdown lifecycle event itself. A receiver can legally be behind while Phase B drains: the admitted operation publishes its terminal `ProductEvent`, `shutdown()` observes the operation become idle, publishes `Runtime.ShutDown`, and transitions the coordinator to `ShutDown` before a slow receiver is scheduled. When that receiver then reads the already-queued terminal event, `validate_receiver` returns `RuntimeShutDown` and `ensure_delivery_live` rejects it. The terminal evidence is therefore lost even though it was published before the shutdown event, violating the documented “drain admitted work, then publish shutdown, then close receivers” ordering.

**Fix:** Give the coordinator/event service an authoritative shutdown-event sequence (or equivalent drain boundary) and allow reconnect receivers to deliver all queued events at or before that boundary, including terminal events, before accepting only the shutdown event and closing. Add a deterministic slow-consumer test that publishes an admitted terminal event, completes Phase B, then consumes the receiver and asserts terminal event followed by `Runtime.ShutDown`.

### CR-02: Failed non-leaf transactions lose `PartialCommit` identity

**File:** `crates/pi-coding-agent/src/coding_session/session_service.rs:707-742`

**Issue:** `transaction.fail(...)` flushes the operation's pending session events at line 728. If the subsequent manifest update at lines 729-732 fails, those events are already durable, but the raw store error is returned. Unlike `commit_non_leaf_transaction` (lines 681-700), this path does not wrap the error as `CodingSessionError::PartialCommit { operation_id, ... }`. Failed Prompt/Compact/self-healing operations can therefore report a generic session error without the admitted operation id or `TerminalUncertain` recovery marker, making recovery and association ambiguous after a real partial write.

**Fix:** Map the manifest-update error to `CodingSessionError::PartialCommit { operation_id: operation_id.clone(), message: error.to_string() }` after `transaction.fail(...)`, and add a deterministic manifest-failure fixture for the failed-transaction path that asserts the original admitted id and uncertainty anchor are preserved.

## Warnings

No additional warning-level findings.

---

_Reviewed: 2026-07-14_
_Reviewer: generic-agent workaround (gsd-code-reviewer)_
_Depth: standard_
