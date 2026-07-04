# Self-Healing Edit Capability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Add the self-healing edit workflow to public/RPC capability reporting and reserve an operation kind for the future session-owner entrypoint.

**Architecture:** Treat self-healing edit as a persistent-session workflow capability, alongside compact, branch summary, export, and plugin reload. Add the capability to `CodingAgentCapabilities`, serialize it as RPC `selfHealingEdit`, and reserve `OperationKind::SelfHealingEdit` with operation string `self_healing_edit` while keeping durable events and adapter commands out of this slice.

**Tech Stack:** Rust 2024, `pi-coding-agent` capability service, protocol serialization via serde, existing RPC/public API tests.

---

## File Structure

- Modify `crates/pi-coding-agent/src/coding_session/context.rs`: add `self_healing_edit: CapabilityStatus` and populate it from persistent workflow capability state.
- Modify `crates/pi-coding-agent/src/coding_session/operation_control.rs`: add `OperationKind::SelfHealingEdit` and `as_str()` mapping.
- Modify `crates/pi-coding-agent/src/coding_session/capability_service.rs`: add tests for idle/disabled/busy self-healing capability.
- Modify `crates/pi-coding-agent/src/protocol/types.rs`: serialize `selfHealingEdit` in `RpcCapabilities`.
- Modify `crates/pi-coding-agent/tests/public_api.rs`: assert the public API capability field.
- Modify `crates/pi-coding-agent/tests/rpc_mode.rs`: assert RPC idle and running capability serialization.
- Modify `docs/TODO.md`: record capability surface progress.

## Task 1: Add RED Tests

- [x] **Step 1: Update public API expected capabilities**

In `crates/pi-coding-agent/tests/public_api.rs`, add this field to the expected `CodingAgentCapabilities` literal after `plugin_reload`:

```rust
self_healing_edit: CapabilityStatus::Available,
```

- [x] **Step 2: Update RPC idle capability test**

In `crates/pi-coding-agent/tests/rpc_mode.rs`, add `"selfHealingEdit"` to the disabled persistent workflow capability list in `rpc_state_reports_capabilities_when_idle`:

```rust
for capability in [
    "compact",
    "fork",
    "cloneSession",
    "branchSummary",
    "export",
    "pluginReload",
    "selfHealingEdit",
] {
```

- [x] **Step 3: Update RPC busy capability tests**

In `rpc_state_reports_agent_invocation_busy_while_running`, after the delegation busy assertions add:

```rust
assert_eq!(capabilities["selfHealingEdit"]["status"], "disabled");
assert_eq!(
    capabilities["selfHealingEdit"]["reason"],
    "requires persistent Rust-native session"
);
```

In `rpc_state_reports_prompt_busy_while_running`, after the delegation busy assertions add:

```rust
assert_eq!(capabilities["selfHealingEdit"]["status"], "disabled");
assert_eq!(
    capabilities["selfHealingEdit"]["reason"],
    "requires persistent Rust-native session"
);
```

These RPC tests run without an active persistent session. Persistent busy behavior is covered by the capability service active-operation test.

- [x] **Step 4: Update capability service tests**

In `crates/pi-coding-agent/src/coding_session/capability_service.rs`, add `capabilities.self_healing_edit` to the arrays in:

```rust
capabilities_report_persistent_workflows_busy_for_active_operation
capabilities_disable_persistent_session_operations_without_persistence
```

Then add this test:

```rust
#[test]
fn self_healing_edit_operation_kind_reports_stable_name() {
    assert_eq!(OperationKind::SelfHealingEdit.as_str(), "self_healing_edit");
}
```

- [x] **Step 5: Run RED tests**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --no-run
```

Expected: compilation fails because `self_healing_edit` and `OperationKind::SelfHealingEdit` do not exist yet.

## Task 2: Implement Capability and Protocol Surface

- [x] **Step 1: Add public capability field**

In `crates/pi-coding-agent/src/coding_session/context.rs`, add this field after `plugin_reload`:

```rust
pub self_healing_edit: CapabilityStatus,
```

Then populate it in `CodingAgentCapabilities::from_runtime_state()` from `persistent_session_capability.clone()`:

```rust
self_healing_edit: persistent_session_capability.clone(),
```

- [x] **Step 2: Add operation kind**

In `crates/pi-coding-agent/src/coding_session/operation_control.rs`, add the enum variant:

```rust
#[allow(dead_code)]
SelfHealingEdit,
```

and map it in `as_str()`:

```rust
Self::SelfHealingEdit => "self_healing_edit",
```

The variant is reserved for the next session-owner entrypoint and is allowed as dead code in this slice.

- [x] **Step 3: Add RPC capability field**

In `crates/pi-coding-agent/src/protocol/types.rs`, add this field after `plugin_reload` in `RpcCapabilities`:

```rust
#[serde(rename = "selfHealingEdit")]
pub self_healing_edit: RpcCapabilityStatus,
```

Then populate it in `impl From<CodingAgentCapabilities> for RpcCapabilities`:

```rust
self_healing_edit: capabilities.self_healing_edit.into(),
```

- [x] **Step 4: Run focused tests**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --no-run
/home/whai/.cargo/bin/cargo test -p pi-coding-agent coding_session_public_api_symbols_are_importable -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent self_healing_edit_operation_kind_reports_stable_name -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent rpc_state_reports_capabilities_when_idle -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent rpc_state_reports_agent_invocation_busy_while_running -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent rpc_state_reports_prompt_busy_while_running -- --nocapture
```

Expected: tests pass.

## Task 3: Update TODO and Verify

- [x] **Step 1: Update TODO capability item**

In `docs/TODO.md`, update the workflow capability integration item so it says self-healing edit capability is now present and uses persistent-session workflow gating.

- [x] **Step 2: Add progress note**

Append under progress notes:

```markdown
- 2026-07-04: Self-healing edit capability surface slice added. `CodingAgentCapabilities` and RPC `get_state.capabilities.selfHealingEdit` now report persistent-session workflow availability, disabled non-persistent state, and busy operation status, with `OperationKind::SelfHealingEdit` reserved for the upcoming session-owned edit workflow entrypoint.
```

- [x] **Step 3: Run verification**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt --check
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet
/home/whai/.cargo/bin/cargo check --workspace --quiet
/home/whai/.cargo/bin/cargo test --workspace --quiet
git diff --check
```

Expected: all commands exit 0.
