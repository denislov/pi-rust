# Product Event Adapter Flow-Node Independence Audit Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Confirm and lock down that product event adapters translate `CodingAgentEvent` semantics without depending on concrete Flow node IDs or exposing Flow node fields in protocol payloads.

**Architecture:** Keep Flow node IDs inside Flow graph implementation/tests. `CodingProtocolEventAdapter` should consume only semantic product events. Add a protocol regression test that serializes representative product-event-derived protocol events and recursively rejects Flow-node-shaped fields such as `nodeId`, `flowNode`, `flowNodeId`, `flowNodeName`, and `lastNode`.

**Tech Stack:** Rust 2024, existing `CodingProtocolEventAdapter`, `ProtocolEvent`, and deterministic protocol unit tests.

---

### Task 1: Audit Current Coupling

**Files:**
- Inspect: `crates/pi-coding-agent/src/protocol/`
- Inspect: `crates/pi-coding-agent/src/interactive/`
- Inspect: `crates/pi-coding-agent/src/coding_session/`

- [x] **Step 1: Search for Flow node coupling**

Search product adapters for `FlowOutcome`, `last_node`, `node_id`, `lastNode`, and concrete Flow node IDs.

- [x] **Step 2: Decide implementation scope**

If adapters already avoid Flow node IDs, add regression coverage and documentation updates only. If coupling is found, route adapters back through semantic `CodingAgentEvent` fields.

### Task 2: Add Protocol Regression Guard

**Files:**
- Modify: `crates/pi-coding-agent/tests/protocol_events.rs`

- [x] **Step 1: Add recursive JSON field assertion helper**

Add a test-local helper that recursively walks serialized protocol event JSON and rejects Flow-node-shaped field names.

- [x] **Step 2: Add representative adapter serialization test**

Serialize prompt, compaction, self-healing edit, default-profile, and delegation product events through `CodingProtocolEventAdapter`, then assert no protocol event contains Flow node fields.

- [x] **Step 3: Run focused test**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test protocol_events product_event_protocol_adapter_does_not_emit_flow_node_fields -- --nocapture
```

Expected: PASS if the current adapter boundary is already semantic.

### Task 3: Update Docs And Verify

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-04-product-event-adapter-flow-node-independence-plan.md`

- [x] **Step 1: Update TODO**

Mark the cross-cutting product-event-adapter Flow node independence item complete with the audit and regression guard.

- [x] **Step 2: Mark plan steps complete**

Update this plan's checkboxes as implementation proceeds.

- [x] **Step 3: Run verification**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt --check
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test protocol_events product_event_protocol_adapter_does_not_emit_flow_node_fields -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test protocol_events --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet
/home/whai/.cargo/bin/cargo check --workspace --quiet
git diff --check
```
