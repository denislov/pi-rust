---
phase: 03-production-adapter-convergence
verified: 2026-07-12T10:14:50Z
status: human_needed
score: 3/5 must-haves verified
behavior_unverified: 2
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 3/5
  gaps_closed:
    - "CR-01 implementation gap: every owner-bearing interactive task now returns CodingAgentSession on operation failure, and finish_prompt restores it before error projection."
    - "WR-01 implementation gap: successful direct and navigation forks now update prompt_context.session_target from the mutated owner."
    - "WR-02 implementation gap: delegation fallback now depends on visible UiEvent projection and forwards each ProductEvent once."
    - "WR-03/WR-04 implementation gaps: the stale loop test was renamed and the magic subscription count was replaced by per-runner checks."
    - "INTER-02 documentation mismatch: checklist and traceability now record the implemented requirement as complete."
  gaps_remaining: []
  regressions: []
deferred:
  - truth: "Interactive structural guards automatically discover every future owner-returning runner and parse Rust bodies without comment/string brace ambiguity."
    addressed_in: "Phase 5"
    evidence: "Phase 5 goal and success criteria own regression-resistant boundary enforcement and parser/source-scan hardening; the current 13 production runners are all explicitly covered."
behavior_unverified_items:
  - truth: "Interactive prompt/background/profile/delegation task runners return the acquired live owner and exact CliError when a real canonical operation fails."
    test: "Induce real failures after owner acquisition in run_coding_set_default_agent_profile_task, run_coding_delegation_rejection_task, and one pre-existing prompt runner; await PromptTask.done, pass the returned completion through finish_prompt, and execute another canonical operation on the restored owner. Include a durable PartialCommit case."
    expected: "PromptTaskCompletion::Failed carries the same owner and unchanged error, finish_prompt restores it before AgentError projection, and the next operation remains usable."
    why_human: "The current test directly fabricates PromptTaskCompletion::Failed and therefore exercises only finish_prompt, not runner-to-channel owner transport, canonical error preservation, or PartialCommit behavior."
  - truth: "A real ForkSession task failure preserves the pre-fork owner, subscriber boundary, and session target."
    test: "Use deterministic session-store failure controls to make run_coding_fork_session_task fail after acquiring the owner; await task.done, finish the completion, then continue with the restored pre-fork session."
    expected: "The exact fork error is projected, coding_session remains the original usable owner, prompt_context.session_target remains unchanged, and no replacement owner is opened."
    why_human: "Successful direct/navigation forks are behavior-tested, but no test makes the fork runner or canonical ForkSession operation fail; the existing failure test constructs the completion envelope manually."
human_verification:
  - test: "Exercise real profile/rejection/prompt runner failures, including one PartialCommit, through PromptTask.done and finish_prompt."
    expected: "The same owner and exact error cross the task boundary, error projection occurs after restoration, and a subsequent canonical operation succeeds."
    why_human: "No current automated test triggers these runner/operation failure transitions."
  - test: "Exercise a real ForkSession runner failure with deterministic storage fault injection."
    expected: "The pre-fork owner, subscription continuity, and old session target survive and remain usable."
    why_human: "Current fork tests cover successful target continuation only."
---

# Phase 3: Production Adapter Convergence Verification Report

**Phase Goal:** Every first-party product adapter executes live-session product work through `CodingAgentSession::run` while preserving its existing external contract.
**Verified:** 2026-07-12T10:14:50Z
**Status:** human_needed
**Re-verification:** Yes - after Plan 03-07 gap closure

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | JSON and persistent/transient print flows preserve output, errors, and session effects while using `CodingAgentOperation::Prompt`. | VERIFIED | Current source uses `run(Prompt)` at `json_mode.rs:100` and `print_mode.rs:129,150` with exact Prompt outcome extraction. The JSON/print production guard passes. Previously passing adapter behavior suites were included in the successful final workspace gate. |
| 2 | RPC prompt, agent, team, delegation, self-healing, profile, and plugin commands preserve responses, errors, events, and control handling while using canonical operations. | VERIFIED | `rpc/prompt.rs:377,601,783,917` and `rpc/commands.rs:615,858,1047,1141,1159,1223` call `run(...)`; current mutation error branches restore `self.coding_session`. The named response-before-events test and RPC production guard pass. |
| 3 | Interactive background workflows and mutations use canonical operations without changing visible behavior. | PRESENT_BEHAVIOR_UNVERIFIED | All 13 owner runners subscribe before `run(...)`, return through `complete_owned_task`, and `finish_prompt` restores `PromptTaskFailure.session` before `AgentError`. Delegation success/continuation and visible fallback behavior pass. However, the claimed four-path failure test fabricates `PromptTaskCompletion::Failed`; no runner or canonical operation actually fails, and no `PartialCommit` crosses the task boundary in a test. |
| 4 | Interactive fork/navigation retain owner/subscriber continuity, event sequencing, snapshots, projections, and session targets. | PRESENT_BEHAVIOR_UNVERIFIED | Direct fork and both tree-navigation tests perform a subsequent prompt and prove it is persisted in the forked session; `finish_prompt` updates `session_target` only on successful fork results. The real fork failure transition remains untested because the unit test constructs the failure envelope manually. |
| 5 | Production adapters contain neither replaced broad workflow calls nor local deprecation suppressions. | VERIFIED | Precise current-source audit found canonical calls only; remaining `set_default_agent_profile_id` calls are `InteractiveRoot` projection methods. Five production guard tests passed, including JSON/print, RPC, interactive, and no-SwitchActiveLeaf checks. |

**Score:** 3/5 truths verified (2 present, behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/pi-coding-agent/src/protocol/json_mode.rs` | Canonical JSON prompt adapter | VERIFIED | Exists, substantive, wired to `run(Prompt)`, and guarded. |
| `crates/pi-coding-agent/src/print_mode.rs` | Canonical persistent/transient print adapter | VERIFIED | Both branches use `run(Prompt)` and retain existing projections. |
| `crates/pi-coding-agent/src/protocol/rpc/prompt.rs` | Select-driven canonical RPC operations | VERIFIED | Canonical futures remain inside existing `tokio::select!` and event/control topology. |
| `crates/pi-coding-agent/src/protocol/rpc/commands.rs` | Canonical RPC mutation/plugin operations | VERIFIED | Exact outcome extraction and owner restoration remain wired. |
| `crates/pi-coding-agent/src/interactive/prompt_task.rs` | Owner-preserving canonical interactive runners | VERIFIED IN CODE | `PromptTaskCompletion::{Completed,Failed,SetupFailed}`, `complete_owned_task`, fork target derivation, and visibility-aware fallback are substantive and wired across all current runners. Real failure behavior is not directly tested. |
| `crates/pi-coding-agent/src/interactive/loop.rs` | Restore-before-error and success-only fork-target projection | VERIFIED IN CODE | `finish_prompt` restores failed owners at lines 2315-2320 and updates fork targets at lines 2151-2154/2295-2313. |
| `crates/pi-coding-agent/tests/interactive_mode.rs` and `interactive_sessions.rs` | Visible behavior and continuation evidence | PARTIAL | Strong success-path delegation/fork/navigation continuation tests exist; no real runner failure or PartialCommit case exists. |
| `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` | Current-tree adapter convergence enforcement | VERIFIED WITH DEFERRED HARDENING | All current adapters are covered; automatic future-runner discovery/parser completeness remains Phase 5 work. |

GSD artifact queries reported all 23 plan-declared artifacts present and substantive. Automatic key-link queries could not resolve conceptual `from` labels, so links were verified manually against current code and focused tests.

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| JSON/print options | Canonical runtime | `run(Prompt)` plus exact outcome match | WIRED | Three production call sites confirmed. |
| RPC command acknowledgement | Background operation/events | Existing pinned future, bounded queue, replay/final drain | WIRED | Response-before-events focused test passed. |
| Interactive spawned owner | Main-loop completion | `complete_owned_task` -> `PromptTaskCompletion` -> `finish_prompt` | WIRED, BEHAVIOR UNVERIFIED ON REAL FAILURE | Source is ownership-correct; current test bypasses the runner/channel boundary. |
| Successful fork result | Next request target | `active_session_target` -> result payload -> `finish_prompt` | WIRED | Direct and navigation continuation tests prove subsequent prompts reach the forked log. |
| Delegation ProductEvent | Visible feedback/fallback | `CodingEventBridge` visibility classifier plus single original-event forward | WIRED | Unit projection classification and integration visible-feedback test pass. |

### Data-Flow Trace

The relevant typed flows are:

`adapter input -> CodingAgentOperation -> CodingAgentSession::run -> exact CodingAgentOperationOutcome -> existing adapter projection`

and for interactive ownership:

`CodingAgentSession -> runner-local mutable owner -> complete_owned_task -> PromptTaskCompletion -> finish_prompt -> restored owner / refreshed target`.

No adapter-local replacement facade, private operation/service import, new event cache, or replacement persistence path was found.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Failure-envelope finish behavior | `cargo test -p pi-coding-agent --lib interactive::r#loop::tests::prompt_task_failures_restore_the_live_owner_before_projecting_errors -- --exact --nocapture` | 1 passed | PASS, but narrow: fabricated envelope only |
| Tree navigation continuation | `cargo test -p pi-coding-agent --test interactive_sessions interactive_tree_navigation -- --nocapture` | 2 passed | PASS |
| Direct fork continuation | `cargo test -p pi-coding-agent --test interactive_mode scripted_interactive_fork_after_rust_native_prompt_creates_session -- --exact --nocapture` | 1 passed | PASS |
| Delegation rejection continuation/visible feedback | `cargo test -p pi-coding-agent --test interactive_mode scripted_interactive_delegation_rejection_preserves_owner_and_visible_fallback_semantics -- --exact --nocapture` | 1 passed | PASS |
| RPC response-before-events | `cargo test -p pi-coding-agent --test rpc_mode rpc_prompt_returns_response_then_agent_events -- --exact --nocapture` | 1 passed | PASS |
| Current production adapter guards | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards production_ -- --nocapture` | 5 passed | PASS |

The orchestrator's final-tree evidence also records successful `cargo fmt --check`, focused crate suites, `cargo build --workspace`, `cargo test --workspace`, `cargo check --workspace`, precise source audit, and `git diff --check`.

### Probe Execution

No Phase 3 plan or summary declares a probe, and no `scripts/**/probe-*.sh` file exists. Probe execution is not applicable.

### Requirements Coverage

| Requirement | Status | Evidence |
|---|---|---|
| ADAPT-01..04 | SATISFIED | JSON/print canonical calls, exact outcomes, behavior suites, and source guard. |
| RPC-01..04 | SATISFIED | All background/mutation/plugin calls are canonical; focused protocol behavior and guards pass. |
| INTER-01 | CODE SATISFIED / FAILURE BEHAVIOR UNVERIFIED | All background runners use `run`; no real pre-existing prompt runner failure crosses the completion channel in a test. |
| INTER-02 | CODE SATISFIED / FAILURE BEHAVIOR UNVERIFIED | Profile/rejection operations and fallback behavior are wired; real operation-failure owner continuity is not tested. |
| INTER-03 | SUCCESS BEHAVIOR VERIFIED / FAILURE BEHAVIOR UNVERIFIED | Direct/navigation continuation proves target refresh; fork failure survival lacks a real failing runner test. |
| INTER-04 | HUMAN DECISION REQUIRED | Current event/control/success continuity is tested; operation-error owner continuity is present in code but not behaviorally exercised. |
| INTER-05 | SATISFIED | Current production source and guards contain no compatibility calls/suppressions. |

No Phase 3 requirement is orphaned. All 13 IDs appear in plan frontmatter, `REQUIREMENTS.md`, and roadmap traceability.

### Review Findings And Anti-Patterns

| Finding | Classification | Disposition |
|---|---|---|
| 03-REVIEW WR-01: failure test fabricates completion instead of failing a runner/operation | Non-blocking implementation-wise; blocking a fully automated `passed` verdict | Routes SC3/SC4 to `PRESENT_BEHAVIOR_UNVERIFIED` and overall status `human_needed`. Do not claim real task-runner failure coverage exists. |
| 03-REVIEW WR-02: hard-coded 13-runner list and naive brace scan | Non-blocking current-tree verification debt | All 13 current runners are covered and source is clean. Automatic discovery and parser completeness are deferred to Phase 5 regression-hardening. |
| `interactive/loop.rs:93` pre-existing extension placeholder comment | INFO, out of Phase 3 scope | Introduced by commit `4133e055` before this phase; unrelated to operation adapter convergence. |
| Expected dead-code warnings for broad compatibility methods and `ensure_idle` | INFO | Phase 4 owns caller migration and compatibility deletion. |

No Phase 3-modified file contains an unreferenced `TBD`, `FIXME`, or `XXX` debt marker. No production failure hook or placeholder implementation was introduced by Plan 03-07.

### Human Verification Required

#### 1. Real Interactive Operation-Failure Owner Continuity

**Test:** At the `PromptTask` boundary, induce deterministic profile mutation, delegation rejection, and pre-existing prompt failures after owner acquisition; include one `PartialCommit`; await `task.done`, call `finish_prompt`, then run another canonical operation.

**Expected:** The same owner and exact error cross the channel, restoration precedes error projection, durable ambiguity is not rewritten, and the next operation succeeds.

**Why human:** Existing tests do not trigger these runner/operation failure paths.

#### 2. Real Fork Failure Continuity

**Test:** Induce a deterministic `ForkSession` failure after owner acquisition and complete it through the real task channel and `finish_prompt`.

**Expected:** The old owner, target, subscriber continuity, and usable session state remain intact; no replacement owner is opened.

**Why human:** Current tests prove successful fork continuation only; the failure unit test manually constructs the envelope.

### Gaps Summary

No observable implementation blocker remains in the current source. The previous CR-01 and WR-01..04 code gaps are closed, every production adapter uses the canonical operation path, and all current source audits and focused success-path behavior checks pass.

The phase cannot receive an automated `passed` verdict because two behavior-dependent truths rely on a runner/channel ownership transition that no real failure test exercises. This is verification debt, not an observed runtime failure. A developer may either add the deterministic runner-level failure tests described above or explicitly accept the current source-level evidence before Phase 3 is treated as fully verified.

---

_Verified: 2026-07-12T10:14:50Z_
_Verifier: the agent (gsd-verifier, generic-agent workaround)_
