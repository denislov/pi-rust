---
phase: 02-canonical-facade-correctness
verified: 2026-07-11T08:35:28Z
status: passed
score: 5/5 must-haves verified
behavior_unverified: 0
overrides_applied: 0
deferred:
  - truth: "Stable API privacy guards reject future glob re-exports, not only current explicit identifiers."
    addressed_in: "Phase 5"
    evidence: "Phase 5 success criteria require regression-resistant compiler/API checks and stable API rejection of internal contracts."
  - truth: "Fault-control source scanning covers new production-scope controls declared inside session_log/store.rs."
    addressed_in: "Phase 5"
    evidence: "Phase 5 owns recursive boundary enforcement and source-scan hardening."
  - truth: "Rust source sanitization distinguishes lifetimes from character literals."
    addressed_in: "Phase 5"
    evidence: "Phase 5 owns reliable source-audit enforcement; the current weakness affects future guard sensitivity, not current runtime semantics."
  - truth: "Alternate trait-facade scanning parses complete trait bodies rather than only declaration lines."
    addressed_in: "Phase 5"
    evidence: "Phase 5 explicitly requires the canonical operation boundary to be regression-resistant."
---

# Phase 2: Canonical Facade Correctness Verification Report

**Phase Goal:** First-party callers can rely on one complete stable operation facade whose dispatch and outcome semantics preserve the existing runtime contract.
**Verified:** 2026-07-11T08:35:28Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | A first-party caller can import every required operation, outcome, and support type from `pi_coding_agent::api` without importing internal runtime contracts. | VERIFIED | `lib.rs::api` uses explicit curated re-exports; the downstream test constructs all 15 public operations and names the lifecycle, observation, subscription, control, profile, delegation, error, and outcome closure. `stable_api_signature_closure_is_importable` passed. |
| 2 | Every public operation submitted through `CodingAgentSession::run` reaches the metadata-selected async, sync-read-only, or sync-mutable dispatcher. | VERIFIED | `run` performs `into_internal`, reads `operation.metadata().dispatch_mode`, exhaustively selects `run_operation`, `run_sync_operation`, or `run_sync_mut_operation`, then projects once. The independent 15-row contract and representative behavioral dispatcher test both passed. |
| 3 | Every internal operation outcome is converted through one exhaustive public projection, including plugin, profile, delegation, fork, navigation, and both export results. | VERIFIED | `OperationOutcome` is crate-private and `CodingAgentOperationOutcome::from_internal` is a wildcard-free exhaustive match. The 15-family fixture includes both `Export` branches and `operation_outcome_projection_covers_all_families` passed. |
| 4 | Fork, active-leaf switch, branch-summary reuse, plugin, profile, and delegation operations retain durable state, event continuity, and explicit error or partial-commit semantics. | VERIFIED | Four focused state-transition tests passed. They cover immediate and reopened state, no duplicate branch-summary append, monotonic event sequences, plugin output/error guard release, profile reopen, delegation queue effects, pre-append no-change, post-append `PartialCommit`, matching operation IDs, and replay authority. |
| 5 | Stable API checks demonstrate that internal operations, dispatch metadata, plugin load options, services, and Flow nodes remain inaccessible to callers. | VERIFIED | `coding_session` is a private crate module; `Operation`, metadata/dispatch, plugin options, and runtime services are `pub(crate)` or private; the current `api` body has no glob re-export and does not export forbidden identifiers. `stable_api_excludes_internal_runtime_contracts` passed. |

**Score:** 5/5 truths verified (0 present-but-behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/pi-coding-agent/src/lib.rs` | Complete curated stable facade | VERIFIED | Explicit `api` re-export closure exists; no glob re-export; implementation registries/services are absent. |
| `crates/pi-coding-agent/tests/public_api.rs` | Facade-only signature and usability evidence | VERIFIED | One `pi_coding_agent::api` import surface names support types and constructs exactly 15 operations. |
| `crates/pi-coding-agent/tests/api_boundary_guards.rs` | Stable/private boundary evidence | VERIFIED | Current forbidden internal identifiers are rejected from the isolated `api` module body. |
| `crates/pi-coding-agent/src/coding_session/public_operation.rs` | Independent operation and outcome ledgers | VERIFIED | Test-owned internal/dispatch/outcome expectations cover 15 distinct public families; production conversion/projection matches remain exhaustive. |
| `crates/pi-coding-agent/src/coding_session/mod.rs` | Canonical dispatcher and high-risk behavior evidence | VERIFIED | `run` owns the public path; named tests exercise all dispatch families and required state/error/replay transitions. |
| `crates/pi-coding-agent/src/coding_session/session_service.rs` | Durable append/manifest and PartialCommit ownership | VERIFIED | Durable delegation events carry the operation ID into `PartialCommit`; switch uses the admitted operation ID; replay is authoritative after append. |
| `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` | Closed method/fault-control structural checks | VERIFIED WITH ADVISORIES | Both current-tree checks pass. Four parser/scanner hardening issues from `02-REVIEW.md` are deferred to Phase 5 because they concern future bypass resistance, not a current leak or semantic failure. |
| `.planning/phases/02-canonical-facade-correctness/02-VALIDATION.md` | Reconciled Nyquist evidence | VERIFIED | Seven task rows are green and the record is signed off. Verification independently reran the behavior-dependent named tests rather than trusting this claim. |

The GSD artifact query reported 8/8 artifacts present and substantive. Its automatic key-link query could not resolve conceptual symbol names or file-to-file links, so the links below were verified manually from current source and named behavior tests.

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `pi_coding_agent::api` imports | `CodingAgentSession` operation/lifecycle signatures | Explicit downstream import and type naming | WIRED | Public API test compiles using the stable facade only. |
| `CodingAgentOperation::into_internal` | `Operation::metadata` | Independent 15-row contract | WIRED | Each public case checks a fixed expected private family and dispatch mode. |
| `CodingAgentSession::run` | Three private dispatchers | Exhaustive `OperationDispatchMode` match | WIRED | Async, read-only sync, and mutable sync behavior all pass through public `run`. |
| `OperationOutcome` | `CodingAgentOperationOutcome::from_internal` | One exhaustive projection | WIRED | All outcome families and both export branches are directly constructed and checked. |
| Navigation operations | Session log, replay, and `EventService` | Public outcome plus immediate/reopened state and event assertions | WIRED | Switch/fork/reuse tests prove the current durable and event contract. |
| Delegation decisions | Durable append, manifest update, and replay | Existing test-only failure points | WIRED | Append failure leaves state unchanged; manifest failure returns matching `PartialCommit`; reopened replay resolves the queue. |

### Data-Flow Trace

Not applicable as a UI/data-rendering Level 4 check. The relevant runtime data flows were instead traced as typed Rust values:

`CodingAgentOperation -> Operation -> metadata dispatcher -> OperationOutcome -> CodingAgentOperationOutcome`, and for durable mutations, `operation -> SessionService append/manifest -> replay-derived owner state`.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Stable facade signature closure | `cargo test -p pi-coding-agent --test public_api stable_api_signature_closure_is_importable -- --exact` | 1 passed | PASS |
| All 15 public mapping/dispatch rows | `cargo test -p pi-coding-agent --lib coding_session::public_operation::tests::operation_contract_covers_all_public_variants -- --exact` | 1 passed | PASS |
| All three metadata dispatch families | `cargo test -p pi-coding-agent coding_session::tests::canonical_run_uses_each_metadata_dispatch_family -- --exact` | 1 passed | PASS |
| Exhaustive outcome projection | `cargo test -p pi-coding-agent coding_session::public_operation::tests::operation_outcome_projection_covers_all_families -- --exact` | 1 passed | PASS |
| Navigation and summary durability | `cargo test -p pi-coding-agent coding_session::tests::canonical_run_preserves_navigation_and_branch_summary_durability -- --exact` | 1 passed | PASS |
| Navigation no-commit/PartialCommit/replay | `cargo test -p pi-coding-agent coding_session::tests::canonical_durable_mutations_distinguish_no_commit_partial_commit_and_replay -- --exact` | 1 passed | PASS |
| Plugin/profile/delegation contract | `cargo test -p pi-coding-agent coding_session::tests::canonical_run_preserves_plugin_profile_and_delegation_contracts -- --exact` | 1 passed | PASS |
| Delegation no-commit/PartialCommit/replay | `cargo test -p pi-coding-agent coding_session::tests::canonical_delegation_decisions_distinguish_no_commit_partial_commit_and_replay -- --exact` | 1 passed | PASS |
| Current stable API privacy | `cargo test -p pi-coding-agent --test api_boundary_guards stable_api_excludes_internal_runtime_contracts -- --exact` | 1 passed | PASS |
| Test-only fault controls | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards session_store_failure_controls_remain_test_only -- --exact` | 1 passed | PASS |
| Closed canonical method ledger | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards canonical_operation_facade_has_no_new_workflow_wrappers -- --exact` | 1 passed | PASS |

### Probe Execution

No Phase 02 plan or summary declares a probe script, and this phase is not backed by a `probe-*.sh` contract. Probe execution was not applicable.

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|---|---|---|---|---|
| FACADE-01 | 02-01, 02-03 | Complete stable facade closure | SATISFIED | Explicit `api` exports and facade-only 15-operation/support-type compilation test. |
| FACADE-02 | 02-02, 02-03 | Public operation conversion and metadata-selected dispatch | SATISFIED | Exhaustive `run` dispatch source, independent 15-row ledger, and three-family behavior test. |
| FACADE-03 | 02-02, 02-03 | One exhaustive internal-to-public outcome projection | SATISFIED | Wildcard-free projection and passing 15-family fixture, including both export results. |
| FACADE-04 | 02-01, 02-03 | Internal runtime contracts excluded from stable API | SATISFIED | Private module/visibility, explicit re-export body, and passing current-tree privacy test. |
| FACADE-05 | 02-03 | High-risk operation compatibility | SATISFIED | Passing state, event, error, persistence, PartialCommit, operation-ID, and replay tests. |

No Phase 02 requirement is orphaned: all five IDs appear in plan frontmatter and map uniquely to Phase 02 in `REQUIREMENTS.md`.

### Anti-Patterns and Review Findings

No `TBD`, `FIXME`, or `XXX` debt markers were found in the phase-modified source/test files. No placeholder implementation or production test instrumentation was found.

| Finding | Classification Against Phase Goal | Disposition |
|---|---|---|
| WR-01: identifier-based privacy guard misses a hypothetical glob re-export | Warning, not blocker | Current `api` has only explicit re-exports and currently exposes none of the forbidden contracts. Parser-complete future regression enforcement belongs to Phase 5. |
| WR-02: generic fault vocabulary scan skips `store.rs` | Warning, not blocker | Exact known definitions and all seven current call sites are directly `#[cfg(test)]`; no current production fault control was found. Broader future-name detection belongs to Phase 5. |
| WR-03: sanitizer can confuse lifetimes and char literals | Warning, not blocker | Current closed-ledger test passes and manual source inspection confirms the expected method/fault boundaries. Replace the hand lexer during Phase 5 hardening. |
| WR-04: alternate-trait scan checks only the declaration line | Warning, not blocker | No alternate trait facade currently exists. Complete trait-body parsing is Phase 5 regression-hardening work. |

Disconfirmation pass:

- **Partially met enforcement claim:** the structural guards are not parser-complete against all hypothetical future Rust syntax; recorded above and deferred to Phase 5.
- **Potentially misleading green test:** `stable_api_excludes_internal_runtime_contracts` alone would not detect a future glob re-export. The current truth is still verified by the explicit on-disk `api` body, private module/type visibility, and downstream positive compilation.
- **Error path checked independently:** delegation append and manifest failures, navigation append/manifest failures, plugin command error/permit release, and persistent profile reopen were all exercised by named tests. No uncovered error path was found that invalidates a Phase 02 roadmap truth.

### Deferred Hardening

The four review warnings map specifically to Phase 5, whose goal is to make the operation boundary regression-resistant and whose success criteria own recursive source audits and stable API boundary enforcement. They do not represent a current internal export, alternate facade, production fault hook, wrong dispatch, wrong projection, or broken durable transition.

### Human Verification Required

None. All Phase 02 roadmap truths are programmatically observable and have passing behavioral or structural evidence.

### Gaps Summary

No Phase 02 goal gap was found. The stable facade is complete for the current signature closure, dispatch and projection are exhaustive, high-risk state transitions have direct behavioral evidence, and internal runtime contracts are inaccessible in the current tree.

---

_Verified: 2026-07-11T08:35:28Z_
_Verifier: the agent (gsd-verifier, generic-agent workaround)_
