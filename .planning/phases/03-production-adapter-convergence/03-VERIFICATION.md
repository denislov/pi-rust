---
phase: 03-production-adapter-convergence
verified: 2026-07-12T04:06:44Z
status: gaps_found
score: 3/5 must-haves verified
behavior_unverified: 1 # Pre-existing async task error-path session restoration (present + wired, no test exercises the invariant)
overrides_applied: 0
gaps:
  - truth: "Interactive prompt and background workflows, mutations, delegation decisions, plugin actions, compaction, and branch summaries use canonical operations without changing visible behavior (SC3)"
    status: failed
    reason: "CR-01 (code review): Session data loss on task failure for three newly-async operations (set_default_agent_profile, reject_delegation_confirmation, fork). The old synchronous code operated on &mut session (session survived failure); the new async code take()s the session and drops it on error via ? propagation. finish_prompt's error handler (loop.rs:2302-2306) does NOT restore coding_session, unlike the RPC adapter which restores self.coding_session = Some(session) on every error path. No test exercises the error path."
    artifacts:
      - path: "crates/pi-coding-agent/src/interactive/loop.rs"
        issue: "finish_prompt error handler (lines 2302-2306) does not restore coding_session - session permanently lost on any task failure"
      - path: "crates/pi-coding-agent/src/interactive/prompt_task.rs"
        issue: "run_coding_set_default_agent_profile_task (line 1061 ?), run_coding_delegation_rejection_task (line 1114 ?), run_coding_fork_session_task (line 1686 ?) drop session on error via ? propagation before constructing the result struct"
    missing:
      - "Restore coding_session in finish_prompt's Err(error) branch, or change task error result types to carry the session back on failure (matching the RPC adapter pattern)"
      - "Add a test that exercises the error path for profile mutation, delegation rejection, and fork, verifying the session survives"
  - truth: "Interactive fork and navigation retain subscriber continuity, product-event sequencing, and refreshed snapshots and projections after transitions (SC4)"
    status: failed
    reason: "CR-01 (code review): On fork failure, the session is permanently lost - no subscriber continuity, no snapshot refresh, no projection. Additionally, WR-01: prompt_context.session_target is not updated after tree navigation fork (loop.rs ForkSession branch lines 2283-2301), meaning subsequent session reopening would use the stale pre-navigation target."
    artifacts:
      - path: "crates/pi-coding-agent/src/interactive/loop.rs"
        issue: "ForkSession success branch (lines 2283-2301) does not update prompt_context.session_target after fork; error branch (lines 2302-2306) does not restore coding_session"
      - path: "crates/pi-coding-agent/src/interactive/prompt_task.rs"
        issue: "run_coding_fork_session_task (line 1686) drops session on error via ? propagation"
    missing:
      - "Restore coding_session on fork failure (same fix as SC3 CR-01)"
      - "Sync prompt_context.session_target from the forked session's active session choice after successful fork"
      - "Add a test that verifies session survival and session_target correctness after fork/navigation failure"
behavior_unverified_items:
  - truth: "Pre-existing async interactive tasks (prompt, agent, team, approval, compact, self-heal, plugin) preserve the session on error"
    test: "Trigger an operation failure (e.g., provider error, admission failure) during a pre-existing async interactive task and verify coding_session is restored afterward"
    expected: "coding_session should be Some(session) after the error is projected to the transcript; the user should be able to continue working"
    why_human: "The finish_prompt error handler (loop.rs:2302-2306) does not restore coding_session for ANY task type. For pre-existing async tasks this was the behavior before Phase 03 (not a regression), but no test exercises the error path, so the invariant is present-but-unverified. Grep cannot confirm the session is or is not lost at runtime without a behavioral test."
---

# Phase 3: Production Adapter Convergence Verification Report

**Phase Goal:** Every first-party product adapter executes live-session product work through `CodingAgentSession::run` while preserving its existing external contract.
**Verified:** 2026-07-12T04:06:44Z
**Status:** gaps_found
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | JSON and both persistent and transient print flows produce the same outputs, errors, and session effects while executing prompts through `CodingAgentOperation::Prompt` (SC1) | ✓ VERIFIED | `json_mode.rs:100` uses `.run(CodingAgentOperation::Prompt(...))` with exhaustive `CodingAgentOperationOutcome::Prompt` extraction; `print_mode.rs:129,150` use the same pattern for persistent and transient branches. No replaced broad workflow calls or `#[allow(deprecated)]` in production source. `production_json_and_print_use_canonical_operations` guard passes (13/13 boundary tests green). |
| 2 | RPC prompt, agent, team, delegation, self-healing, profile, and plugin commands preserve response shapes, errors, event forwarding, and `tokio::select!` control handling while using canonical operations (SC2) | ✓ VERIFIED | `rpc/prompt.rs:377,601,783,917` use `session.run(CodingAgentOperation::...)` for agent/team/approval/prompt inside existing `tokio::select!` shells. `rpc/commands.rs:1223` uses `session.run(CodingAgentOperation::PluginLoad)` plus 4 more canonical calls for self-healing/profile/rejection/plugin-command. RPC error handlers restore `self.coding_session = Some(session)` on every error path (~12 occurrences). `production_rpc_uses_canonical_operations` guard passes. |
| 3 | Interactive prompt and background workflows, mutations, delegation decisions, plugin actions, compaction, and branch summaries use canonical operations without changing visible behavior (SC3) | ✗ FAILED | Canonical operations ARE used (`prompt_task.rs` has 15 `session.run(CodingAgentOperation::...)` calls across all task types). However, CR-01 causes a **session data loss regression** for three newly-async operations (profile mutation, delegation rejection, fork). `finish_prompt` error handler (`loop.rs:2302-2306`) does NOT restore `coding_session`; task functions drop the session on error via `?` propagation. Old synchronous code operated on `&mut session` (session survived failure). No test exercises the error path. RPC adapter correctly restores session on every error path, proving the interactive omission is a defect. |
| 4 | Interactive fork and navigation retain subscriber continuity, product-event sequencing, and refreshed snapshots and projections after transitions (SC4) | ✗ FAILED | Fork/navigation use canonical operations (`prompt_task.rs:1494,1562,1598,1661` - BranchSummary + ForkSession). Success-path tests pass (`interactive_tree_navigation_*`, `scripted_interactive_fork_*`). However, CR-01 causes session loss on fork failure (no subscriber continuity after transition failure). WR-01: `prompt_context.session_target` not updated after tree navigation fork (`loop.rs:2283-2301`), compounding CR-01. |
| 5 | JSON, print, RPC, and interactive production sources contain neither replaced broad workflow calls nor local deprecation suppressions for those calls (SC5) | ✓ VERIFIED | Grep across all production adapter files (`json_mode.rs`, `print_mode.rs`, `rpc/prompt.rs`, `rpc/commands.rs`, `interactive/prompt_task.rs`, `interactive/loop.rs`, `interactive/commands.rs`, `interactive/session_actions.rs`) found ZERO replaced broad workflow calls (`.prompt(`, `.invoke_agent(`, `.invoke_team(`, `.compact(`, `.summarize_branch(`, `.fork_current_session(`, etc.) and ZERO `#[allow(deprecated)]` attributes. The `root.set_default_agent_profile_id()` calls in `loop.rs` are the legitimate local UI projection setter, explicitly allowed. All 4 boundary guard tests pass: `production_json_and_print_use_canonical_operations`, `production_rpc_uses_canonical_operations`, `production_interactive_uses_canonical_operations`, `production_adapters_do_not_introduce_switch_active_leaf`. |

**Score:** 3/5 truths verified (1 present, behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/pi-coding-agent/src/protocol/json_mode.rs` | Canonical Prompt submission inside existing JSON event select/drain shell | ✓ VERIFIED | Lines 99-107: `session.run(CodingAgentOperation::Prompt(...))` with exhaustive outcome extraction inside spawned task; receiver-before-run ordering preserved; imports via `crate::api` |
| `crates/pi-coding-agent/src/print_mode.rs` | Canonical Prompt submission in both persistent and transient print branches | ✓ VERIFIED | Lines 128-134 (persistent), 149-155 (transient): both use `.run(CodingAgentOperation::Prompt(...))` with exhaustive extraction; imports via `crate::api` |
| `crates/pi-coding-agent/src/protocol/rpc/prompt.rs` | Canonical background operations inside existing RPC concurrency topology | ✓ VERIFIED | Lines 377, 601, 783, 917: agent/team/approval/prompt use `session.run(CodingAgentOperation::...)` inside `Box::pin` + `tokio::select!` shells; all `#[allow(deprecated)]` removed |
| `crates/pi-coding-agent/src/protocol/rpc/commands.rs` | Canonical mutation operations with unchanged RPC wire projection | ✓ VERIFIED | Lines 1223+: self-healing/profile/rejection/plugin-load/plugin-command use `session.run(CodingAgentOperation::...)`; session restored on every error path; `#[allow(deprecated)]` removed; `ensure_mutable_coding_session` helper removed |
| `crates/pi-coding-agent/src/interactive/prompt_task.rs` | Canonical background operation futures within existing TUI task ownership | ⚠️ WIRED-WITH-DEFECT | 15 `session.run(CodingAgentOperation::...)` calls across all task types; all `#[allow(deprecated)]` removed. But three newly-async task functions (`run_coding_set_default_agent_profile_task`, `run_coding_delegation_rejection_task`, `run_coding_fork_session_task`) drop the session on error via `?` propagation - CR-01 defect. |
| `crates/pi-coding-agent/src/interactive/loop.rs` | Async scheduling and owner restoration for formerly synchronous mutations | ⚠️ WIRED-WITH-DEFECT | `finish_prompt` success branches restore `*coding_session = Some(result.session)` (10 variants), but the error branch (lines 2302-2306) does NOT restore `coding_session` - CR-01 defect. |
| `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` | Narrow complete production-adapter canonical-call/deprecation guards | ✓ VERIFIED | 4 guard tests exist and pass: `production_json_and_print_use_canonical_operations`, `production_rpc_uses_canonical_operations`, `production_interactive_uses_canonical_operations`, `production_adapters_do_not_introduce_switch_active_leaf` (13/13 total boundary tests green) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| JSON/print prompt options | `CodingAgentSession::run` | `CodingAgentOperation::Prompt` followed by exact Prompt outcome extraction | ✓ WIRED | `json_mode.rs:100`, `print_mode.rs:129,150` |
| RPC command acknowledgement | background operation and ProductEvent forwarding | unchanged response-first setup and bounded RpcProductEventQueue | ✓ WIRED | `rpc/prompt.rs` select-driven shell preserved; `rpc/commands.rs` take/restore pattern |
| Interactive pending request | background task owning CodingAgentSession | existing spawn/result channel and finish_prompt owner restoration | ⚠️ PARTIAL | Success path wired (10 finish_prompt variants restore session). Error path NOT wired (CR-01: `finish_prompt` Err branch does not restore session). |
| Pre-navigation ProductEvent receiver | summary and fork operations | one receiver and one mutable owner across both operations | ✓ WIRED | `prompt_task.rs:1494-1634`: BranchSummary(ReuseExisting) then ForkSession with one receiver spanning both |
| `PromptControlHandle` | pinned session.run future | existing tokio::select! control branches outside CodingAgentOperation | ✓ WIRED | `rpc/prompt.rs` and `interactive/prompt_task.rs` select! branches preserve abort/steer/follow-up routing |

### Data-Flow Trace (Level 4)

Not applicable - this phase migrates adapter call sites; the data sources (CodingAgentSession::run dispatch) were verified in Phase 2. No new data sources or rendering paths were introduced.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Boundary guards (SC5 enforcement) | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards -- --nocapture` | 13 passed; 0 failed | ✓ PASS |
| Tree navigation fork (SC4 success path) | `cargo test -p pi-coding-agent --test interactive_sessions interactive_tree_navigation -- --nocapture` | 2 passed; 0 failed | ✓ PASS |
| Direct fork (SC4 success path) | `cargo test -p pi-coding-agent --test interactive_mode scripted_interactive_fork_after_rust_native_prompt_creates_session -- --nocapture` | 1 passed; 0 failed | ✓ PASS |
| Session restoration on task error (SC3/SC4 error path) | No test exists | N/A | ✗ FAIL (no coverage) |
| `cargo fmt --check` | `cargo fmt --check` | Clean (no output) | ✓ PASS |
| `git diff --check` | `git diff --check` | Clean (no output) | ✓ PASS |

### Probe Execution

Not applicable - this phase does not declare or imply probe-based verification. No `scripts/*/tests/probe-*.sh` probes were found.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| ADAPT-01 | 03-01 | JSON adapter executes prompt through `run(Prompt)` | ✓ SATISFIED | `json_mode.rs:100` confirmed |
| ADAPT-02 | 03-01 | Print adapter executes persistent and non-persistent prompt through canonical facade | ✓ SATISFIED | `print_mode.rs:129,150` confirmed |
| ADAPT-03 | 03-01 | JSON and print output, error, and session behavior remain unchanged | ✓ SATISFIED | No broad calls/deprecation; behavior tests pass; no regression identified |
| ADAPT-04 | 03-01 | JSON and print production code contains no replaced broad calls or `#[allow(deprecated)]` | ✓ SATISFIED | Grep + `production_json_and_print_use_canonical_operations` guard pass |
| RPC-01 | 03-02 | RPC prompt, agent, team, delegation-approval background tasks execute through canonical operations | ✓ SATISFIED | `rpc/prompt.rs:377,601,783,917` confirmed |
| RPC-02 | 03-03 | RPC self-healing, profile, rejection, plugin load, plugin command execute through canonical operations | ✓ SATISFIED | `rpc/commands.rs` confirmed; session restored on every error path |
| RPC-03 | 03-02, 03-03 | RPC migration preserves select! control handling, event forwarding, response shapes, error protocol | ✓ SATISFIED | Select! branches preserved; tests pass; RPC error handlers restore session |
| RPC-04 | 03-03 | RPC production code contains no replaced broad calls or deprecation suppressions | ✓ SATISFIED | `production_rpc_uses_canonical_operations` guard pass |
| INTER-01 | 03-04 | Interactive prompt, agent, team, compaction, self-healing, plugin, branch-summary background work executes through canonical operations | ✓ SATISFIED | `prompt_task.rs` has 15 `session.run()` calls; all migrated |
| INTER-02 | 03-05 | Interactive profile mutation and delegation rejection execute through canonical operations | ✓ SATISFIED (code) / ⚠️ DOC MISMATCH | Code implements it (`prompt_task.rs:1036,1084`). REQUIREMENTS.md marks INTER-02 as `[ ]` unchecked and "Pending" in traceability - documentation was not updated. |
| INTER-03 | 03-06 | Session fork and navigation use canonical operations and refresh snapshots and projections after transitions | ⚠️ PARTIALLY SATISFIED | Canonical operations used (success path works). But CR-01: session lost on fork failure; WR-01: session_target not updated after navigation fork. |
| INTER-04 | 03-04, 03-05, 03-06 | Interactive migration preserves event/control multiplexing, subscriber continuity, product-event sequencing, and UI behavior | ⚠️ PARTIALLY SATISFIED | Success paths preserve behavior. Error paths do NOT preserve session continuity (CR-01). |
| INTER-05 | 03-06 | Interactive production code contains no replaced broad calls or deprecation suppressions | ✓ SATISFIED | `production_interactive_uses_canonical_operations` guard pass |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `interactive/loop.rs` | 2302-2306 | Error handler does not restore `coding_session` | 🛑 Blocker (CR-01) | Session permanently lost on any interactive task failure; regression for newly-async operations |
| `interactive/loop.rs` | 2283-2301 | `prompt_context.session_target` not updated after ForkSession | ⚠️ Warning (WR-01) | Stale session target after navigation; compounds CR-01 |
| `interactive/prompt_task.rs` | 1082,1098 | Delegation rejection fallback uses ProductEvent receipt check instead of UiEvent emptiness check | ⚠️ Warning (WR-02) | Possible missing fallback notice when ProductEvent produces no visible UiEvent |
| `interactive/loop.rs` | 2576-2585 | Test `interactive_loop_sync_delegation_rejection_uses_product_event_stream_boundary` weakened; name misleading | ⚠️ Warning (WR-03) | Test no longer checks for `.subscribe_product_events()` in loop.rs but name implies it does |
| `interactive/prompt_task.rs` | 1804 | Magic number subscription count assertion (13) | ⚠️ Warning (WR-04) | Fragile; already broken once during 03-05 (count was 10, should have been 12) |
| `interactive/event_bridge.rs` | 92, 153 | `#[allow(dead_code)]` on `handle_product_event` | ℹ️ Info | Expected - retained for bridge unit tests; cleanup in Phase 4 |
| `coding_session/operation_control.rs` | 145 | `ensure_idle` dead code warning | ℹ️ Info | Expected - all callers migrated; deletion in Phase 4 per D-19 |

### Human Verification Required

No additional human verification items beyond the behavior_unverified_items in frontmatter. The gaps (CR-01, WR-01) are code-level defects identified by source analysis, not UI/UX behaviors requiring human testing.

### Gaps Summary

**The phase goal is partially achieved but not fully achieved.**

**Achieved (3/5 success criteria):**
- SC1 (JSON/print): All three prompt paths use `run(Prompt)` with exhaustive outcome extraction. No broad calls or deprecation suppression.
- SC2 (RPC): All RPC operations use canonical operations. `tokio::select!` control handling preserved. RPC adapter correctly restores session on every error path.
- SC5 (Source guards): No replaced broad workflow calls or `#[allow(deprecated)]` in any production adapter. All 4 boundary guard tests pass.

**Not achieved (2/5 success criteria):**
- SC3 (Interactive behavior): CR-01 causes **session data loss on task failure** for three newly-async operations (profile mutation, delegation rejection, fork). The `finish_prompt` error handler (`loop.rs:2302-2306`) does NOT restore `coding_session`, while every success branch does. The task functions drop the session on error via `?` propagation. This is a regression: the old synchronous code operated on `&mut session` (session survived failure). The RPC adapter demonstrates the correct pattern (`self.coding_session = Some(session)` on every error path). No test exercises the error path.
- SC4 (Fork/navigation continuity): CR-01 causes session loss on fork failure. WR-01: `prompt_context.session_target` not updated after tree navigation fork, meaning subsequent session reopening would use the stale pre-navigation target.

**Root cause:** The interactive `finish_prompt` function's error handler was written for pre-existing async tasks (prompt, agent, team, etc.) that already used `take()`. Phase 03 migrated three synchronous operations to the same async `take()` pattern, but the error handler was not updated to restore the session on failure. The result is that any failure (operation error, abort, admission failure) during profile mutation, delegation rejection, or fork permanently drops the session - including conversation history, compaction state, and durable facts.

**Documentation discrepancy:** INTER-02 is marked as `[ ]` (unchecked) and "Pending" in REQUIREMENTS.md, but the code implements it. The requirements file was not updated after 03-05 completed.

---

_Verified: 2026-07-12T04:06:44Z_
_Verifier: the agent (gsd-verifier)_
