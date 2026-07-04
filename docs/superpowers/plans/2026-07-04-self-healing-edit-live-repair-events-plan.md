# Self-Healing Edit Live Repair Events Plan

**Goal:** Emit `SelfHealingEditRepairAttempted` product events at the point each repair attempt completes inside `SelfHealingEditFlow`, while keeping durable session-log recording owned by `CodingAgentSession`.

**Scope:** Add a minimal Flow-context observer for repair attempts, wire the session owner to emit live product events through `EventService`, stop post-flow duplicate repair-attempt event emission, and keep existing RPC/interactive protocol mappings unchanged.

---

### Task 1: Add RED Flow Observer Coverage

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/flow_service.rs`

- [x] **Step 1: Add a test observer**

Add a test-only observer that sends observed repair attempts over a channel.

- [x] **Step 2: Assert repair event arrives before the Flow future completes**

Add a focused async test that runs a repaired self-healing edit and uses `tokio::select!` to require the observer message before `run_self_healing_edit()` returns.

- [x] **Step 3: Run RED test**

Run the focused test and expect failure because `SelfHealingEditObserver`/`with_repair_observer` do not exist yet.

### Task 2: Implement Flow Repair Observer

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/flow_service.rs`

- [x] **Step 1: Add `SelfHealingEditObserver`**

Define a crate-private asynchronous observer trait with `repair_attempted(&self, path, repair) -> BoxFuture<()>`.

- [x] **Step 2: Add observer wiring to options/context**

Store an optional observer in `SelfHealingEditOptions`, expose `with_repair_observer`, and notify it immediately when a repair-attempt record is created.

- [x] **Step 3: Run focused GREEN test**

Run the new Flow observer test and existing self-healing Flow tests.

### Task 3: Wire Session Owner Live Events

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/tests/public_api.rs`

- [x] **Step 1: Add session observer implementation**

Create a small owner-side observer that clones `EventService` and `operation_id` and emits `CodingAgentEvent::SelfHealingEditRepairAttempted`.

- [x] **Step 2: Use the observer when building self-healing edit options**

Pass the observer into `SelfHealingEditOptions` before running the Flow and remove post-flow repair-attempt event emission while retaining durable event-log recording.

- [x] **Step 3: Add duplicate guard coverage**

Assert session-level repair-attempt product events are emitted exactly once and still carry replacement/check data.

### Task 4: Update Docs And Verify

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-04-self-healing-edit-live-repair-events-plan.md`

- [x] **Step 1: Update TODO**

Record live Flow-time repair-attempt product events under Phase 6 self-healing edit progress/tests.

- [x] **Step 2: Mark plan steps complete**

Update this plan as each step completes.

- [x] **Step 3: Run verification**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt --check
/home/whai/.cargo/bin/cargo test -p pi-coding-agent self_healing_edit --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test public_api coding_session_self_healing_edit_uses_model_repair_strategy -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet
/home/whai/.cargo/bin/cargo check --workspace --quiet
/home/whai/.cargo/bin/cargo test --workspace --quiet
git diff --check
```
