# Self-Healing Edit Product Events Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add semantic `CodingAgentEvent` coverage for self-healing edit workflow lifecycle and map it to RPC protocol events plus interactive notices.

**Architecture:** `CodingAgentSession` remains the product runtime owner. `SelfHealingEditFlow` continues to return typed outcome/repair-attempt metadata without owning event emission. The owner emits product events around the session-owned workflow and adapter layers map those product events. Durable session events remain unchanged.

**Scope:** Add owner-level `SelfHealingEditStarted`, `SelfHealingEditRepairAttempted`, `SelfHealingEditCompleted`, and `SelfHealingEditFailed` events. Map them to `ProtocolEvent` semantic variants and interactive `SystemNotice` messages. Do not inject `EventService` into the low-level Flow and do not expose prompts/runtime/provider internals.

---

### Task 1: Add RED Coverage

**Files:**
- Modify: `crates/pi-coding-agent/tests/public_api.rs`
- Modify: `crates/pi-coding-agent/tests/protocol_events.rs`
- Modify: `crates/pi-coding-agent/tests/interactive_event_bridge.rs`

- [x] **Step 1: Assert session emits self-healing edit semantic events**

In the model-repair public API test, subscribe before the edit, collect events after completion, and assert start, repair attempt, and completion events include path/attempt metadata.

- [x] **Step 2: Assert protocol adapter maps self-healing edit events**

Add a `protocol_events` test that feeds self-healing edit lifecycle events through `CodingProtocolEventAdapter` and checks serialized event names/fields.

- [x] **Step 3: Assert interactive bridge maps self-healing edit notices**

Add an `interactive_event_bridge` test that maps self-healing edit start/repair/completed/failed events into `UiEvent::SystemNotice` text.

- [x] **Step 4: Run RED tests**

Run focused tests and expect compile failures because the event/protocol variants do not exist yet.

### Task 2: Add Product Event Types

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/event.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`

- [x] **Step 1: Add `CodingAgentEvent` variants**

Add self-healing edit start, repair attempted, completed, and failed variants carrying product-level metadata only.

- [x] **Step 2: Emit events in `CodingAgentSession::self_healing_edit_inner`**

Emit started after operation id allocation, repair-attempted from outcome/context repair attempts, completed on success, and failed before failing the transaction on error.

- [x] **Step 3: Update event-kind test helpers**

Update exhaustive helper mappings in tests.

### Task 3: Map RPC Protocol Events

**Files:**
- Modify: `crates/pi-coding-agent/src/protocol/types.rs`
- Modify: `crates/pi-coding-agent/src/protocol/events.rs`

- [x] **Step 1: Add serializable protocol payload structs**

Add self-healing edit replacement/check-output protocol payload structs.

- [x] **Step 2: Add protocol event variants**

Add `self_healing_edit_start`, `self_healing_edit_repair_attempt`, `self_healing_edit_end`, and `self_healing_edit_error` variants.

- [x] **Step 3: Map product events to protocol events**

Update `CodingProtocolEventAdapter::push`.

### Task 4: Map Interactive Notices

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/event_bridge.rs`

- [x] **Step 1: Map start/repair/completed/failed to system notices**

Use concise, transcript-visible notices with path and attempt/error details.

### Task 5: Update Docs And Verify

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-04-self-healing-edit-product-events-plan.md`

- [x] **Step 1: Run focused GREEN tests**

Run public API, protocol event, and interactive bridge focused tests.

- [x] **Step 2: Update TODO and plan checkboxes**

Add this plan to TODO source documents and mark self-healing edit product event mapping as in place.

- [x] **Step 3: Run verification**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt --check
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test protocol_events --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test interactive_event_bridge --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test public_api --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet
/home/whai/.cargo/bin/cargo check --workspace --quiet
/home/whai/.cargo/bin/cargo test --workspace --quiet
git diff --check
```
