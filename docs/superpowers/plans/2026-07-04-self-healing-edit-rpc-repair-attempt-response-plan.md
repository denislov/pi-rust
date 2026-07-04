# Self-Healing Edit RPC Repair Attempt Response Plan

**Goal:** Include structured self-healing edit repair attempts in successful RPC `self_healing_edit` responses.

**Scope:** Serialize `SelfHealingEditOutcome::repair_attempts` into `data.repairAttempts` using RPC camelCase field names. Do not change command inputs, durable event schema, product events, or repair policy behavior.

---

### Task 1: Add RED RPC Coverage

**Files:**
- Modify: `crates/pi-coding-agent/tests/rpc_mode.rs`

- [x] **Step 1: Assert planned repair response contains repairAttempts**

Extend `rpc_self_healing_edit_uses_planned_repair_attempts` to assert `data.repairAttempts[0]` contains attempt number, replacement old/new text, diagnostics, and post-attempt check output.

- [x] **Step 2: Run RED test**

Run the focused RPC test and expect assertion failure because `repairAttempts` is not serialized yet.

### Task 2: Serialize Repair Attempts

**Files:**
- Modify: `crates/pi-coding-agent/src/protocol/rpc/commands.rs`

- [x] **Step 1: Add RPC repair attempt serializer**

Map attempt, edits, diagnostics, and check output from `SelfHealingEditRepairAttempt`.

- [x] **Step 2: Include repairAttempts in success response data**

Add `repairAttempts` to `rpc_self_healing_edit_data`.

### Task 3: Update Docs And Verify

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-04-self-healing-edit-rpc-repair-attempt-response-plan.md`

- [x] **Step 1: Run focused GREEN test**

Run the focused RPC repair-attempt test.

- [x] **Step 2: Update TODO and plan checkboxes**

Record that RPC success responses expose repair-attempt details.

- [x] **Step 3: Run verification**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt --check
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test rpc_mode rpc_self_healing_edit_uses_planned_repair_attempts -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test rpc_mode --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet
/home/whai/.cargo/bin/cargo check --workspace --quiet
/home/whai/.cargo/bin/cargo test --workspace --quiet
git diff --check
```
