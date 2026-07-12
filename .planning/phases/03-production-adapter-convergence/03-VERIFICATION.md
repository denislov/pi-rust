---
phase: 03-production-adapter-convergence
verified: 2026-07-12T16:02:38Z
status: passed
score: 5/5 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification:
  previous_status: human_needed
  previous_score: 3/5
  gaps_closed:
    - "Real profile, delegation-rejection, and prompt-finalization failures now traverse production PromptTask spawning, the actual done channel, exact durable errors, finish_prompt, and a subsequent canonical operation."
    - "Real ForkSession AppendEvents failure now proves source-owner, subscriber, old-target, cleanup, exact-error, and post-restoration event continuity."
  gaps_remaining: []
  regressions: []
deferred:
  - truth: "Interactive structural guards automatically discover every future owner-returning runner and parse Rust bodies without comment/string brace ambiguity."
    addressed_in: "Phase 5"
    evidence: "Phase 5 owns regression-resistant boundary enforcement and source-scan hardening; all 13 current production runners are explicitly covered and production source is clean."
---

# Phase 3: Production Adapter Convergence Verification Report

**Phase Goal:** Every first-party product adapter executes live-session product work through `CodingAgentSession::run` while preserving its existing external contract.
**Verified:** 2026-07-12T16:02:38Z
**Status:** passed
**Re-verification:** Yes - after gap plans 03-08 and 03-09
**Dispatch:** generic-agent workaround for unavailable typed `gsd-verifier` dispatch

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | JSON and persistent/transient print flows preserve output, errors, and session effects while executing prompts through `CodingAgentOperation::Prompt`. | VERIFIED | Current production calls are `json_mode.rs:100` and `print_mode.rs:129,150`. Exact Prompt outcome extraction remains wired; JSON/print integration coverage and the production source guard passed in the workspace gate. |
| 2 | RPC prompt, agent, team, delegation, self-healing, profile, and plugin commands preserve responses, errors, event forwarding, and `tokio::select!` controls while using canonical operations. | VERIFIED | Current calls are in `rpc/prompt.rs:377,601,783,917` and `rpc/commands.rs:615,858,1047,1141,1159,1223`. Existing pinned-future, response-first, bounded-event, control, drain, idempotency, and owner-restoration tests passed in `cargo test --workspace`. |
| 3 | Interactive prompt/background workflows and mutations use canonical operations without visible behavior or owner-continuity regressions. | VERIFIED | All current runners call `session.run(...)` and complete through `complete_owned_task`. The three real failure tests spawn production tasks, await actual `task.done`, preserve exact profile/rejection/prompt contracts, run `finish_prompt`, retain the old target, and successfully execute `PluginLoad` afterward. |
| 4 | Interactive fork/navigation retain source or forked owner identity, subscriber continuity, event sequencing, snapshots/projections, and correct session targets across success and failure. | VERIFIED | Success continuation remains covered by direct/tree-navigation integration tests. The real fork failure test uses production `spawn_fork_session`, retains the pre-task receiver, proves no replacement `SessionOpened`, preserves the source owner/old target/session count, then observes `DefaultAgentProfileChanged` on the original receiver. |
| 5 | JSON, print, RPC, and interactive production sources contain neither replaced broad workflow calls nor local deprecation suppressions, and internal/test-only seams do not leak through the stable API. | VERIFIED | `product_runtime_boundary_guards` passed all 13 tests, including the three adapter guards, closed method ledger, test-only store controls, private-runtime boundaries, and no production `SwitchActiveLeaf` caller. |

**Score:** 5/5 truths verified (0 behavior-unverified)

### Re-verification Delta

The previous report left truths 3 and 4 at `PRESENT_BEHAVIOR_UNVERIFIED` because its only failure evidence fabricated `PromptTaskCompletion::Failed`. Plans 03-08 and 03-09 closed that evidence gap without changing the adapter orchestration:

- `CodingSessionError::PartialCommit` now converts losslessly to structured `CliError::PartialCommit { operation_id, message }`, with identical canonical display text.
- Three specialized `#[cfg(test)] pub(crate)` owner methods expose only the existing AppendEvents/UpdateManifest fault points and a real durable pending-delegation fixture; boundary guards prove they are absent from production and `pi_coding_agent::api`.
- Four real-runner tests exercise profile, rejection, prompt, and fork failures through production spawn, event channel, oneshot completion, and `finish_prompt` paths.
- Prompt finalization factually remains `PromptTaskCompletion::Completed(PromptTaskResult::Coding(...))` containing `PromptTurnOutcome::Failed(CodingSessionError::PartialCommit)`; it is not rewritten into task-level `CliError`.
- Delegation rejection factually remains the task-level structured `CliError::PartialCommit` path and preserves exact operation identity.

## Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/pi-coding-agent/src/protocol/json_mode.rs` | Canonical JSON prompt adapter | VERIFIED | Exists, substantive, wired to `run(Prompt)`, behavior-tested, and source-guarded. |
| `crates/pi-coding-agent/src/print_mode.rs` | Canonical persistent/transient print adapter | VERIFIED | Both branches use `run(Prompt)` and preserve existing projection/session paths. |
| `crates/pi-coding-agent/src/protocol/rpc/prompt.rs` | Canonical select-driven RPC operations | VERIFIED | Four background operation families remain inside the existing pinned/select/event/control topology. |
| `crates/pi-coding-agent/src/protocol/rpc/commands.rs` | Canonical RPC mutation/plugin operations | VERIFIED | Six call sites use typed operations with exact outcomes and owner restoration. |
| `crates/pi-coding-agent/src/interactive/prompt_task.rs` | Canonical owner-preserving interactive runners | VERIFIED | Fifteen canonical calls cover prompt, agent/team, delegation, compaction, self-heal, plugin, summary, and fork paths; all 13 owner runners use the shared completion boundary. |
| `crates/pi-coding-agent/src/interactive/loop.rs` | Completion projection and real failure-path tests | VERIFIED | Failure arm restores the owner before `AgentError`; four real-runner tests exercise actual completion contracts and post-restoration use. |
| `crates/pi-coding-agent/src/error.rs` and `coding_session/error.rs` | Lossless adapter error boundary | VERIFIED | Structured PartialCommit identity and unchanged non-partial conversion are directly tested. |
| `crates/pi-coding-agent/src/coding_session/mod.rs` | Narrow test-only persistence fixture bridge | VERIFIED | Directly `cfg(test)`, crate-visible only, specialized, substantive, and excluded from stable API. |
| `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` | Current production adapter/API enforcement | VERIFIED | All 13 guards passed; all plan-declared artifacts across 03-01..03-09 passed `verify.artifacts` (28/28). |

## Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| JSON/print inputs | Canonical runtime | `run(Prompt)` plus exact Prompt outcome extraction | WIRED | Three production call sites and parity suites are green. |
| RPC acknowledgement/control state | Canonical background future and ProductEvents | Existing `Box::pin`, `tokio::select!`, bounded queue, replay cursors, and final drains | WIRED | Production source retains the adapter-owned topology; full workspace tests pass. |
| Interactive acquired owner | Main-loop completion | `PromptTask::spawn_*` -> real `task.done` -> untouched completion -> `finish_prompt` | WIRED AND BEHAVIOR-VERIFIED | Profile/rejection/prompt/fork tests exercise the real channel rather than a fabricated envelope. |
| Durable mutation failure | Typed adapter error/outcome | CodingSessionError conversion or PromptTurnOutcome retention plus JSONL operation ID comparison | WIRED AND BEHAVIOR-VERIFIED | Rejection uses structured `CliError::PartialCommit`; prompt remains completed/failed outcome; both match durable IDs. |
| Failed fork source EventService | Post-restoration operation | Pre-task receiver observes later canonical profile event | WIRED AND BEHAVIOR-VERIFIED | The exact original receiver sees `DefaultAgentProfileChanged`; no replacement `SessionOpened` is emitted. |
| Successful fork owner | Next request target/projection | SessionForked result -> resolved target -> hydration/owner restoration | WIRED AND BEHAVIOR-VERIFIED | Direct and both tree-navigation continuation tests remain green. |

## Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Real profile/rejection/prompt/fork failure continuity | `cargo test -p pi-coding-agent --lib 'interactive::r#loop::tests::real_' -- --nocapture` | 4 passed, 0 failed | PASS |
| Production adapter/API/test-seam guards | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards -- --nocapture` | 13 passed, 0 failed | PASS |
| Complete workspace behavior | `cargo test --workspace` | All unit, integration, and doc-test targets passed | PASS |
| Workspace type/build consistency | `cargo check --workspace` | Passed; only expected Phase 4 dead-code warnings | PASS |
| Formatting and patch hygiene | `cargo fmt --check`; `git diff --check` | Both passed | PASS |

## Probe Execution

No Phase 3 plan or summary declares a probe, and no `scripts/**/probe-*.sh` path is part of this phase. Probe execution is not applicable.

## Requirements Coverage

| Requirement | Status | Evidence |
|---|---|---|
| ADAPT-01 | SATISFIED | JSON prompt calls `run(CodingAgentOperation::Prompt)`. |
| ADAPT-02 | SATISFIED | Persistent and transient print prompt paths use the canonical facade. |
| ADAPT-03 | SATISFIED | Existing JSON/print output, error, ordering, persistence, continue/fork, and no-session behavior tests pass. |
| ADAPT-04 | SATISFIED | JSON/print production guard passes with no replaced calls or local deprecation suppression. |
| RPC-01 | SATISFIED | Prompt, agent, team, and approval background tasks call canonical operations. |
| RPC-02 | SATISFIED | Self-heal, profile, rejection, plugin load, and plugin command handlers call canonical operations. |
| RPC-03 | SATISFIED | Existing response, error, event, select/control, drain, idempotency, and persistence suites pass. |
| RPC-04 | SATISFIED | RPC production guard passes. |
| INTER-01 | SATISFIED | All background workflows use canonical operations; real prompt failure retains its completed failed-outcome contract and owner usability. |
| INTER-02 | SATISFIED | Profile and rejection use canonical operations; real task-level failure and PartialCommit owner/error continuity pass. |
| INTER-03 | SATISFIED | Direct/navigation success and real fork failure preserve required target, owner, cleanup, and projection behavior. |
| INTER-04 | SATISFIED | Event/control multiplexing, owner return, original-subscriber continuity, sequencing/projection, and post-failure operation behavior are exercised. |
| INTER-05 | SATISFIED | Interactive production guard passes with no broad calls, suppressions, private imports, or invented SwitchActiveLeaf path. |

All 13 Phase 3 requirement IDs occur in plan frontmatter, `REQUIREMENTS.md`, and roadmap traceability. No requirement is orphaned.

## Review Findings And Deferred Items

| Finding | Classification | Verification disposition |
|---|---|---|
| Manual 13-runner discovery and raw brace parser | DEFERRED, NON-BLOCKING | Current production runners are all explicitly checked and all production guards pass. Automatic open-world discovery/parser hardening is specifically owned by Phase 5 and is not a Phase 3 goal failure. |
| Real task tests await `task.done` without a timeout | WARNING, NON-BLOCKING ROBUSTNESS DEBT | The four real tasks completed immediately and passed. A timeout would make future deadlocks fail faster, but its absence does not make the currently exercised transition uncertain or false. |
| JSONL and session-count helpers ignore malformed records/partial directories | WARNING, NON-BLOCKING ASSERTION HARDENING | Current tests observe typed PartialCommit IDs, unchanged source identity/count, no replacement SessionOpened, and successful post-restoration use. Stricter parsing/filesystem snapshots would improve corruption diagnostics but no current corruption or cleanup failure was observed. |
| Restore-before-projection is not regression-locked by an observable callback | WARNING, NON-BLOCKING STRUCTURAL DEBT | Current production source performs `*coding_session = Some(session)` immediately before `root.apply_events(AgentError)`, and real tests prove the resulting owner/error state and subsequent use. Reversing those statements would weaken the intended internal ordering without changing today's external postcondition; a helper/AST guard is useful future hardening, not evidence of a current Phase 3 contract failure. |
| Compatibility methods and `ensure_idle` emit dead-code warnings | EXPECTED DEFERRED WORK | Phase 4 explicitly owns remaining test migration and compatibility deletion. Workspace test/check still pass. |
| `interactive/loop.rs:93` extension placeholder comment | INFO, OUT OF SCOPE | Pre-existing unrelated UI extension placeholder; not introduced or used by Phase 3 adapter convergence. |

## Anti-Patterns Found

No Phase 3 gap-closure file contains an unreferenced `TBD`, `FIXME`, or `XXX` marker. No production fault hook, generic store selector, replacement operation facade, private service export, blocking bridge, detached session mutation, or Stage 10 event-contract change was introduced.

## Human Verification Required

None. The two former manual UAT items now have deterministic offline behavioral coverage through the actual private production task boundary.

## Gaps Summary

No Phase 3 goal gap remains. All five roadmap truths and all 13 requirements are verified. The remaining review findings concern future regression detection and diagnostic strictness, not an observed missing behavior or unwired artifact.

---

_Verified: 2026-07-12T16:02:38Z_
_Verifier: the agent (gsd-verifier, generic-agent workaround)_
