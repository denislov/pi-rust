---
gsd_state_version: 1.0
milestone: v1.1
milestone_name: Typed Product Events and Client Lifecycle Contract
current_phase: 09
current_phase_name: lifecycle-association-guards-and-closure
status: executing
stopped_at: Completed 09-03-PLAN.md
last_updated: "2026-07-14T05:38:48.487Z"
last_activity: 2026-07-14
last_activity_desc: Phase 09 execution started
progress:
  total_phases: 4
  completed_phases: 3
  total_plans: 23
  completed_plans: 18
  percent: 75
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-07-13)

**Core value:** Every first-party live-session product operation follows one typed, admitted, behavior-preserving runtime path through `CodingAgentSession::run`.
**Current focus:** Phase 09 — lifecycle-association-guards-and-closure

## Current Position

Phase: 09 (lifecycle-association-guards-and-closure) — EXECUTING
Plan: 4 of 8
Status: Ready to execute
Last activity: 2026-07-14 — Phase 09 execution started

## Performance Metrics

**Velocity:**

- Total plans completed: 34
- Average duration: -
- Total execution time: 0.0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01 | 3 | - | - |
| 02 | 3 | - | - |
| 03 | 9 | - | - |
| 04 | 4 | - | - |
| 5 | 3 | - | - |
| 07 | 5 | - | - |
| 8 | 7 | - | - |

**Recent Trend:**

- Last 5 plans: -
- Trend: -

*Updated after each plan completion*
| Phase 01 P01 | 7min | 2 tasks | 3 files |
| Phase 01 P02 | 38min | 3 tasks | 2 files |
| Phase 02 P01 | 8 min | 2 tasks | 3 files |
| Phase 02 P02 | 25 min | 2 tasks | 4 files |
| Phase 02 P03 | 1h 11m | 3 tasks | 4 files |
| Phase 03 P01 | 10 min | 3 tasks | 3 files |
| Phase 03 P02 | 6 min | 2 tasks | 1 files |
| Phase 03 P03 | 9 min | 2 tasks | 2 files |
| Phase 03 P04 | 13 min | 3 tasks | 3 files |
| Phase 03 P06 | 28 min | 3 tasks | 7 files |
| Phase 03 P07 | 1h 14m | 3 tasks | 5 files |
| Phase 03 P08 | 17min | 2 tasks | 4 files |
| Phase 03 P09 | 47min | 2 tasks | 1 files |
| Phase 04 P01 | 10 min | 2 tasks | 6 files |
| Phase 04 P02 | 17 min | 2 tasks | 8 files |
| Phase 04 P03 | 0 min | 2 tasks | 3 files |
| Phase 04 P04 | 15 min | 2 tasks | 5 files |
| Phase 05 P02 | 4min | 1 tasks | 14 files |
| Phase 06 P01 | 18min | 3 tasks | 5 files |
| Phase 06 P02 | 13min | 3 tasks | 4 files |
| Phase 06 P03 | 8min | 2 tasks | 3 files |
| Phase 08 P04 | 10min | 2 tasks | 7 files |
| Phase 08 P05 | 9min | 3 tasks | 7 files |
| Phase 09 P01 | 58 min | 2 tasks | 7 files |
| Phase 09 P02 | 10min | 1 tasks | 1 files |
| Phase 09 P03 | 17min | 2 tasks | 6 files |

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [Roadmap]: Use architecture- and migration-dependency phases because this milestone converges a shared runtime boundary.
- [Phase 1]: Treat live source, tests, boundary guards, and Git history as authoritative; prior plan checkboxes are reference evidence only.
- [Milestone v1.1]: Combine typed `ProductEvent` payload convergence with the existing client lifecycle foundations; keep owner renaming and broad daemon orchestration deferred.
- [Phase ?]: 01-01: Audit schema frozen with 15-row Operation Matrix seeded from live source; validator enforces locked taxonomies in three modes
- [Phase ?]: 01-01: Wave 0 ownership at task 01-01-01; Nyquist compliance pending until Plan 01-03 final gate
- [Phase 01]: 01-02: Populated 15-row Operation Matrix from live source with 46 evidence IDs, 26 production callers, 32 test callers, 16 compatibility methods, 4 authority conflicts, and 8 findings; corrected 3 non-deprecated methods and fixed validator SIGPIPE/taxonomy bugs
- [Phase 01]: 01-02: Corrected compatibility inventory - set_default_agent_profile_id, approve_delegation_confirmation, reject_delegation_confirmation are NOT deprecated; routed missing Stage 9 guards to Phase 5 hardening
- [Phase ?]: 01-03: Added F-BASE-01 informational finding for completed baseline; fixed validator blocking-finding bug per D-15/D-16; Phase 1 audit final with Nyquist validation approved
- [Phase 02]: 02-01: Existing stable facade already closed positive caller signature graph; evidence added without widening exports — The facade-only closure test compiled without production additions
- [Phase 02]: 02-01: ProfileRegistry and ProfileRegistryOptions remain implementation-private — Callers consume projected profile query results rather than registry ownership
- [Phase 02]: 02-02: Keep ExportCurrent and ExportCurrentHtml as distinct test-owned expectations even though both map to private Export options. — This detects collapse of the two public export inputs without changing the private production enum.
- [Phase 02]: 02-02: Prove dispatcher selection with fixed metadata assertions plus public run behavior, without production instrumentation. — Owner metadata and observable outcomes provide independent evidence without changing runtime semantics for tests.
- [Phase 02]: 02-02: Keep ProfileRegistry behavior coverage owner-scoped after registry types were removed from the stable api facade. — The tests require implementation ownership and should not force private registries back into the public API.
- [Phase 02]: 02-03: Preserve the durable delegation transaction ID in PartialCommit errors. — Replay and the public error must identify the same appended decision transaction.
- [Phase 02]: 02-03: Enforce a closed CodingAgentSession method ledger and test-only fault controls. — New workflow facades and production failure injection must fail structurally at the owner boundary.
- [Phase 03]: 03-01: JSON and print adapters route Prompt through CodingAgentSession::run with exhaustive outcome extraction; narrow source guard locks canonical operations and rejects production deprecation suppression. — Lowest-risk adapter tier migrated first per D-01/D-04/D-05/D-06; guard preserves test-only allowances and compatibility definitions per D-19.
- [Phase ?]: 03-02: All four select-driven RPC background operations (prompt, agent, team, delegation approval) route through CodingAgentSession::run(CodingAgentOperation) with exhaustive outcome extraction; #[allow(deprecated)] removed from three RPC handlers.
- [Phase 03]: 03-03: All five short-lived RPC mutation commands (self-healing edit, default-profile mutation, delegation rejection, plugin load, plugin command) route through CodingAgentSession::run(CodingAgentOperation) with exhaustive outcome extraction; narrow source guard locks canonical operations across src/protocol/rpc/. - Switched profile/rejection handlers to take()/restore ownership pattern; drain events and restore owner on every error path; guard covers 14 replaced workflow methods.
- [Phase ?]: 03-04: All nine ordinary interactive background workflows (prompt, agent, team, approval, compact, self-heal, plugin reload/command, direct branch summary) route through CodingAgentSession::run(CodingAgentOperation) with exhaustive outcome extraction; six #[allow(deprecated)] removed; PluginReloadTaskResult.outcome changed to public CodingAgentPluginLoadOutcome; direct branch summary uses AlwaysCreate with hydrate_transcript: false; navigation variant reserved for Plan 06.
- [Phase 03]: 03-06: Direct /fork and summary-before-fork tree navigation route through CodingAgentSession::run(CodingAgentOperation::ForkSession/BranchSummary) with one receiver spanning both operations; no-owner tree fallback uses the same canonical fork task; fork_rust_native_choice removed; narrow interactive source guard and SwitchActiveLeaf audit close Phase 3.
- [Phase 03]: 03-07: Interactive PromptTask failures return the live owner through one completion envelope; successful forks synchronize the next session target; delegation fallback follows visible UiEvent projection; named per-runner guards replace magic subscription counts.
- [Phase 03]: 03-08: Preserve PartialCommit as a structured CliError carrying exact operation ID and message. — Durable uncertainty must remain attributable across adapter task channels.
- [Phase 03]: 03-08: Keep persistence fault injection behind exactly two specialized cfg(test) owner methods and one durable pending-delegation fixture method. — Interactive tests need real fixtures without exposing selectors, services, queues, or production hooks.
- [Phase 03]: Preserve task-level Failed for profile/rejection and Completed(Coding(PromptTurnOutcome::Failed)) for prompt finalization uncertainty. — The production runner contracts intentionally distinguish canonical operation errors from prompt outcome-level finalization errors; tests must enforce rather than flatten that distinction.
- [Phase 03]: Verify failed fork continuity with the original pre-task ProductEvent receiver and no replacement SessionOpened transition. — Resubscribing after restoration would not prove EventService identity survived owner transfer; the original receiver is the continuity authority.
- [Phase 04]: 04-01 migrated agent/team/export tests and deleted four obsolete methods behind receiver-aware guards.
- [Phase 04]: 04-02 retained private `load_plugins` only for four D-03-justified co-located owner tests; public `PluginLoad` remains optionless.
- [Phase 04]: 04-03 preserved delegation durability and `PartialCommit` identity while deleting both public decision methods.
- [Phase 04]: 04-04 completed navigation/summary convergence and the receiver-aware 16-method absence ledger.
- [Phase 05]: Recursive fail-closed adapter guards and external consumer compile fixtures enforce the canonical boundary; authoritative evidence is in `05-STAGE-9-CLOSURE.md`.
- [Phase 06]: Keep event-level terminal status independent from the five existing root-operation terminal associations. — Tool, message, delegation, and session-write completion must not be mislabeled as root-operation completion.
- [Phase 06]: Retain transitional event strings through explicit legacy-name mapping while typed enums and snake_case Serde are authoritative. — Phase 7 owns consumer migration and existing receivers must remain source-compatible in Phase 6.
- [Phase 06]: Classify all 15 public operations and outcomes without expanding the five current root-terminal associations. — Phase 6 records current behavior; Phase 9 owns association closure.
- [Phase 06]: Parse public operation and outcome enums in a source guard so documentation drift fails closed. — Set equality catches additions, renames, omissions, and duplicate matrix rows.
- [Phase 08]: Keep client records, session/capability/active-operation projections, capability generation, event cursor/replay, and recovery metadata under one SnapshotState mutex. — Atomic snapshots and replay/live cuts must not combine independently timed authorities.
- [Phase 08]: Keep broadcast transport outside snapshot authority and send only after coordinator transaction release. — Receiver work cannot invert the coordinator lock or observe a pre-commit cursor.
- [Phase 08]: Keep CodingAgentClientConnection as a generation-scoped Arc coordinator handle with state/preparation methods but no dispatcher.
- [Phase 08]: Commit submission provenance only after canonical IntentRouter admission returns the operation id; precommit drop preserves the draft.
- [Phase 09]: Represent submitted terminal evidence as an exhaustive tagged anchor: ProductEvent, OutcomeOnly, or TerminalUncertain. — Separate acknowledgement domains cannot collapse into a guessed event sequence.
- [Phase 09]: Keep outcome acknowledgement identity opaque and free of generation/signature authority. — Runtime validation, not public construction, remains authoritative.
- [Phase 09]: Project submitted event durability as Durable or Uncertain. — Session identity and pending-write implementation details remain private.
- [Phase 09]: Represent Compact failure with dedicated CompactPromptFailed evidence so PromptCompleted remains excluded from Compact root association. — Exact branch-specific evidence prevents compatibility events from being promoted by generic terminal status.
- [Phase 09]: Derive TerminalAssociated versus OutcomeOnly from the closed permitted-evidence set while retaining NotApplicable as a zero-row guard. — The descriptor stays exhaustive while controls remain outside CodingAgentOperation.
- [Phase 09]: Keep reconnectable client contents in place and model detach as connection validity, not record deletion or operation cancellation. — Detach must preserve client-local facts and session-owned work for same-id reconnect.
- [Phase 09]: Use a coordinator-owned lifecycle epoch with watch notification so blocked receivers wake without moving transport authority into ClientService. — The epoch closes blocked receiver races while SnapshotCoordinator remains the sole lifecycle authority.
- [Phase 09]: Allow session-owned terminal finalization after detach while rejecting connection-owned mutation through the shared lifecycle gate. — Active Prompt work outlives a connection generation and must retain its authoritative terminal state.

### Pending Todos

None yet.

### Blockers/Concerns

- No active milestone blockers.
- Carry the v1.0 audit's three non-blocking hardening items into future planning where relevant.

## Deferred Items

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| v1.1 | Typed `ProductEvent` payload convergence and compatibility subscription deletion | Active | Milestone kickoff |
| v1.1 | Client lifecycle contract over connect, replay, recovery, control, detach, and shutdown | Active | Milestone kickoff |

## Session Continuity

Last session: 2026-07-14T05:37:53.720Z
Stopped at: Completed 09-03-PLAN.md
Resume file: None

## Operator Next Steps

- Start the next milestone with /gsd-new-milestone
