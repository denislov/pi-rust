---
phase: 03-production-adapter-convergence
reviewed: 2026-07-12T03:58:44Z
depth: standard
files_reviewed: 11
files_reviewed_list:
  - crates/pi-coding-agent/src/protocol/json_mode.rs
  - crates/pi-coding-agent/src/print_mode.rs
  - crates/pi-coding-agent/src/protocol/rpc/prompt.rs
  - crates/pi-coding-agent/src/protocol/rpc/commands.rs
  - crates/pi-coding-agent/src/interactive/prompt_task.rs
  - crates/pi-coding-agent/src/interactive/loop.rs
  - crates/pi-coding-agent/src/interactive/commands.rs
  - crates/pi-coding-agent/src/interactive/session_actions.rs
  - crates/pi-coding-agent/src/interactive/event_bridge.rs
  - crates/pi-coding-agent/src/interactive/root.rs
  - crates/pi-coding-agent/src/interactive/app.rs
findings:
  critical: 1
  warning: 4
  info: 3
  total: 8
status: issues_found
---

# Phase 03: Code Review Report

**Reviewed:** 2026-07-12T03:58:44Z
**Depth:** standard
**Files Reviewed:** 11
**Status:** issues_found

## Summary

Phase 03 migrated all first-party product adapters (JSON, print, RPC, interactive) to route through `CodingAgentSession::run(CodingAgentOperation)` as the canonical operation dispatcher. The migration is mechanically sound: every replaced broad workflow call now goes through `run()` with exhaustive `CodingAgentOperationOutcome` extraction, and `#[allow(deprecated)]` attributes were removed from production source.

However, the interactive adapter migration introduces a **session data loss regression** for three operations that were previously synchronous (fork, default-profile mutation, delegation rejection). When these newly-async tasks fail, the session is permanently dropped because `finish_prompt`'s error handler does not restore `coding_session` — unlike the RPC counterparts which explicitly restore the session on every error path. Additionally, a tree-navigation fork path no longer updates `prompt_context.session_target`, and the delegation rejection fallback notice uses a different emptiness check than the old synchronous code.

## Critical Issues

### CR-01: Session data loss on task failure for newly-migrated async operations

**File:** `crates/pi-coding-agent/src/interactive/loop.rs:2302-2306` (error handler); `crates/pi-coding-agent/src/interactive/prompt_task.rs:1025-1068, 1070-1131, 1629-1697`

**Issue:**

Three operations that were **synchronous** before Phase 03 — `set_default_agent_profile`, `reject_delegation_confirmation`, and `fork` — have been migrated to async `PromptTask` functions that call `coding_session.take()` to move session ownership into a spawned task. When the task returns `Err`, the session is **consumed and dropped** inside the task function (via `?` propagation), and `finish_prompt`'s error handler does NOT restore `coding_session`:

```rust
// loop.rs:2302-2306 — finish_prompt error handler
Err(error) => {
    root.apply_events(vec![UiEvent::AgentError {
        error: error.to_string(),
    }]);
    // coding_session is NOT restored — remains None
}
root.set_status(InteractiveStatus::Idle);
```

For example, in `run_coding_set_default_agent_profile_task` (prompt_task.rs:1025-1068), if `session.run(SetDefaultAgentProfile)` fails, the `?` at line 1061 propagates the error, `session` is dropped, and `SetDefaultAgentProfileTaskResult { session }` is never constructed. The same pattern applies to `run_coding_delegation_rejection_task` (line 1114) and `run_coding_fork_session_task` (line 1686).

**This is a behavior regression.** The old synchronous code operated on `&mut session` (borrowed) or used static methods that didn't touch `coding_session`:

- **Old `set_default_agent_profile`**: `session.set_default_agent_profile_id(profile_id)` on `&mut session` — session stayed in `coding_session` on failure.
- **Old `reject_delegation_confirmation`**: `session.reject_delegation_confirmation(...)` on `&mut session` — session stayed in `coding_session` on failure.
- **Old `fork`**: `fork_rust_native_choice(choice, target_leaf_id)` was a **static method** that created a new session — original `coding_session` was never touched.

**Contrast with RPC adapter:** The RPC handlers (`handle_set_default_agent_profile`, `handle_reject_delegation`) correctly restore the session on every error path:

```rust
// commands.rs:882-884 — RPC error handler restores session
Err(error) => {
    let drained = drain_product_events_to_protocol_events(&mut receiver, &mut adapter);
    self.coding_session = Some(session);  // session restored!
    ...
}
```

The interactive adapter is inconsistent with the RPC adapter and with the old synchronous behavior.

**Impact:** If a fork, profile mutation, or delegation rejection operation fails (e.g., filesystem error during fork, canonical admission failure, or user abort), the user's entire session — including conversation history, compaction state, and durable facts — is permanently lost. `coding_session` becomes `None` and the user has no way to recover without manually reopening the session (and `prompt_context.session_target` may point to the wrong session, compounding the problem).

**Fix:**

The task result types should include the session even on error, so `finish_prompt` can restore it. One approach:

```rust
// Change task functions to return the session on error:
struct TaskError {
    error: CliError,
    session: CodingAgentSession,
}

async fn run_coding_set_default_agent_profile_task(
    mut session: CodingAgentSession,
    ...
) -> Result<SetDefaultAgentProfileTaskResult, TaskError> {
    ...
    {
        let mut mutation = Box::pin(session.run(...));
        loop {
            tokio::select! {
                _ = &mut abort_rx => {
                    break Err(CliError::UnsupportedMode(...));
                }
                ...
                outcome = &mut mutation => {
                    break outcome.map_err(CliError::from).and_then(...);
                }
            }
        }
    }.map_err(|error| TaskError { error, session })?;  // session included in error
    ...
}
```

Then in `finish_prompt`:

```rust
Err(task_error) => {
    *coding_session = Some(task_error.session);  // restore session
    root.apply_events(vec![UiEvent::AgentError {
        error: task_error.error.to_string(),
    }]);
}
```

Alternatively, a minimal fix for just the three new task types: have the spawned task always return the session (both on success and failure) via a custom result type, and have `finish_prompt` restore it in the error arm.

## Warnings

### WR-01: `prompt_context.session_target` not updated after tree navigation fork

**File:** `crates/pi-coding-agent/src/interactive/loop.rs:2283-2301`

**Issue:**

The old synchronous tree navigation fork code updated `prompt_context.session_target` after forking:

```rust
// Old code (removed in Phase 03):
if let Some((choice, target_id)) = tree_navigation_fork {
    match fork_rust_native_choice(&choice, Some(&target_id)) {
        Ok(hydrated) => {
            root.apply_hydrated_session(hydrated, Some("Navigated to selected point".into()));
            if let Some(active) = root.active_session.as_ref() {
                prompt_context.session_target =
                    Some(ResolvedSessionTarget::OpenTarget(active.id.clone()));
            }
        }
        ...
    }
}
```

The new code calls `start_tree_navigation_fork_task`, which spawns the fork task. After the task completes, `finish_prompt` handles the `ForkSession` result (lines 2283-2301), but it does **not** update `prompt_context.session_target`. Only `root.set_default_agent_profile_id` and `*coding_session = Some(result.session)` are called.

This means `prompt_context.session_target` retains the pre-navigation target. If `coding_session` later becomes `None` (e.g., due to CR-01's data loss, or a future operation that takes the session), subsequent session reopening would use the stale `session_target` and open the wrong (pre-navigation) session.

**Fix:**

Add `session_target` sync to the `ForkSession` arm in `finish_prompt`:

```rust
Ok(PromptTaskResult::ForkSession(result)) => {
    ...
    root.set_default_agent_profile_id(
        result.session.view().default_agent_profile_id.clone(),
    );
    *coding_session = Some(result.session);
    // Restore session_target from the active session choice
    if let Some(active) = root.active_session.as_ref() {
        prompt_context.session_target =
            Some(ResolvedSessionTarget::OpenTarget(active.id.clone()));
    }
}
```

Note: `finish_prompt` would need to accept `prompt_context` as a parameter (or the sync should be done at the call site after `finish_prompt` returns, alongside the existing `default_agent_profile_id` sync).

### WR-02: Delegation rejection fallback notice uses different emptiness check

**File:** `crates/pi-coding-agent/src/interactive/prompt_task.rs:1082, 1098, 1117, 1121-1125`

**Issue:**

The old synchronous delegation rejection code checked whether any **UiEvents** were produced (after conversion via `CodingEventBridge::handle_product_event`) to decide whether to show the fallback notice:

```rust
// Old code:
let mut ui_events = Vec::new();
while let Ok(Some(event)) = receiver.try_recv() {
    ui_events.extend(bridge.handle_product_event(&event));
}
if ui_events.is_empty() {
    ui_events.push(UiEvent::SystemNotice { ... });
}
```

The new async task checks whether any **ProductEvents** were received (`had_events`), before UiEvent conversion:

```rust
// New code:
let mut had_events = false;
...
event = receiver.recv() => {
    if let Ok(event) = event {
        had_events = true;  // set on ProductEvent receipt
        ...
    }
}
...
let fallback_notice = if had_events { None } else { Some(fallback_text) };
```

If a `ProductEvent` is received but produces no visible `UiEvent` when projected through `CodingEventBridge`/`UiProjection`, the old code would show the fallback notice (because `ui_events.is_empty()` is `true`), but the new code would **not** show it (because `had_events` is `true`). The user could see no feedback for the rejection.

**Fix:**

Either track UiEvent production instead of ProductEvent receipt, or document that this behavior difference is intentional and acceptable. If the latter, add a comment explaining that `ProductEvent` receipt implies visible projection in practice.

### WR-03: Weakened test with misleading name

**File:** `crates/pi-coding-agent/src/interactive/loop.rs:2576-2585`

**Issue:**

The test `interactive_loop_sync_delegation_rejection_uses_product_event_stream_boundary` was weakened: the assertion `assert!(source.contains(&product_subscription))` (checking for `.subscribe_product_events()` in loop.rs) was removed. The test now only checks that the compatibility `subscribe()` path is NOT used and that `UiProjection::new()` IS present. However, the test name still contains "uses_product_event_stream_boundary", which is misleading since the product event subscription assertion was removed.

**Fix:**

Either rename the test to reflect what it actually checks (e.g., `interactive_loop_does_not_use_compatibility_subscribe`), or re-add the product subscription assertion if `loop.rs` is expected to contain it.

### WR-04: Fragile magic number subscription count assertion

**File:** `crates/pi-coding-agent/src/interactive/prompt_task.rs:1804`

**Issue:**

The assertion `assert_eq!(source.matches(&product_subscription).count(), 13)` uses a hardcoded magic number that must be manually updated whenever an owner-returning task is added or removed. The comment says "Update this count when adding or removing owner-returning tasks," but this is fragile and error-prone. The 03-06 summary notes that this assertion was already broken during 03-05 (count was 10 but should have been 12), and was only fixed during 03-06 when `--lib` tests were finally run.

**Fix:**

Consider computing the expected count dynamically (e.g., by counting task spawn functions) or using a range assertion (`assert!(count >= 13)`) that doesn't break on additions. Alternatively, assert the presence of specific subscription calls by function name rather than counting all occurrences.

## Info

### IN-01: Unnecessary `Result` return on new spawn functions

**File:** `crates/pi-coding-agent/src/interactive/prompt_task.rs:187-205` (`spawn_set_default_agent_profile`), `prompt_task.rs:207-221` (`spawn_delegation_rejection`)

**Issue:**

Both `spawn_set_default_agent_profile` and `spawn_delegation_rejection` return `Result<Self, CliError>` but always return `Ok(...)`. The `Result` wrapper is unnecessary since these functions cannot fail. (Note: this matches the existing pattern of `spawn_plugin_reload`, `spawn_branch_summary`, etc., so it's a pre-existing convention rather than a new defect.)

**Fix:** Consider changing the return type to `Self` (or document that the `Result` is retained for API consistency with other spawn functions that may fail).

### IN-02: Duplicate `PromptRunOptions` construction in fork task starters

**File:** `crates/pi-coding-agent/src/interactive/loop.rs:1899-1920` (`start_fork_task`), `loop.rs:1944-1965` (`start_tree_navigation_fork_task`)

**Issue:**

Both `start_fork_task` and `start_tree_navigation_fork_task` construct an identical `PromptRunOptions` from `prompt_context` fields (with `prompt: String::new()` and `invocation: PromptInvocation::Text(String::new())`). This is ~20 lines of duplicated field assignments. The fork operation only uses `options.session` and `options.session_target`, making most fields unused.

**Fix:** Extract a helper function `fork_prompt_run_options(prompt_context: &PromptContext) -> PromptRunOptions` to eliminate duplication.

### IN-03: `handle_product_event` marked as dead code

**File:** `crates/pi-coding-agent/src/interactive/event_bridge.rs:153`

**Issue:**

`handle_product_event` is marked `#[allow(dead_code)]` because its only production caller (the old synchronous delegation rejection code) has migrated to canonical operations. The method is retained for bridge unit tests. This is acceptable for now, but the dead code should be cleaned up when the corresponding test coverage is migrated or the bridge is refactored.

**Fix:** No immediate action needed. Consider removing the method (and its tests) when the `CodingEventBridge` is no longer needed, or document why the bridge tests still require this method.

---

_Reviewed: 2026-07-12T03:58:44Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_
