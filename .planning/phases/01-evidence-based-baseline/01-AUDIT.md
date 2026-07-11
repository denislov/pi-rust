Audit Status: draft

<!-- 01-AUDIT.md - Phase 1 Evidence-Based Baseline audit artifact.
     This is a STRUCTURAL SCAFFOLD created by Plan 01-01.
     Evidence collection, assessment, and findings are populated by Plan 01-02.
     Final validation and traceability closure happen in Plan 01-03.
     The validate-audit.sh script enforces this contract mechanically. -->

# Phase 1 Audit: Canonical Operation Runtime Convergence

**Audit Status:** draft
**Created:** 2026-07-11
**Owner Plan:** 01-01 (scaffold), 01-02 (evidence), 01-03 (final)
**Authority:** `.planning/PROJECT.md`, `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`

---

## Audit Contract

This artifact is the single source of truth for the Stage 9 completion state. It is
machine-scannable and validated by `validate-audit.sh`. The taxonomies below are locked
by `01-CONTEXT.md` decisions D-10 and D-12 through D-16. No prose synonym may replace a
locked value.

### Locked Taxonomies

| Field | Allowed Values | Source |
|-------|---------------|--------|
| implementation | `complete` \| `partial` \| `missing` \| `not_applicable` | D-12 |
| verification | `passed` \| `failed` \| `blocked` \| `not_run` | D-10 |
| disposition | `active` \| `obsolete` \| `deferred_stage_10` \| `retained_compatibility` | D-13 |
| confidence | `high` \| `medium` \| `low` | D-14 |
| finding_obligation | `blocking` \| `required` \| `hardening` \| `informational` | D-16 |
| evidence_gaps | `none` \| comma-separated gap descriptions | D-15 |
| blockers | `none` \| comma-separated blocker descriptions | D-15 |

### Evidence Rules

- **D-06:** Every completed behavior claim requires current source evidence plus a focused
  behavior or public API test.
- **D-07:** Boundary claims additionally require Rust visibility/type evidence, a compile/API
  test, or a precise source guard when Rust cannot directly express the boundary.
- **D-14:** High confidence requires aligned source, focused tests, and applicable boundary
  evidence. Medium means source is clear but verification or history is incomplete. Low means
  the conclusion is indirect and needs downstream verification.
- **D-15:** Evidence gaps reduce confidence but permit an audit conclusion. Blockers prevent a
  reliable conclusion or downstream planning. Only blockers prevent Phase 1 verification passing.

### Scope Rules

- **D-05:** Every gap records exact evidence, affected requirement IDs, target Phase 2-5, and
  dependencies. Phase 1 does not write implementation tasks.
- **D-20:** Phase 1 does not modify the old implementation plan or `docs/TODO.md`.
- Findings route to Phase 2 (facade), Phase 3 (adapters), Phase 4 (tests/deletion), or
  Phase 5 (boundary/closure). Stage 10 work is recorded as `deferred_stage_10`.

### Known Requirement ID Prefixes

`AUDIT-*`, `FACADE-*`, `ADAPT-*`, `RPC-*`, `INTER-*`, `TEST-*`, `DELETE-*`, `GUARD-*`, `CLOSE-*`

---

## Authority Order

Per D-17, authorities are layered. When authorities conflict, record an explicit finding (D-18)
rather than silently choosing one source.

1. **Current source, tests, and guards** - define what exists in the live tree.
2. **Current `PROJECT.md`, `REQUIREMENTS.md`, `ROADMAP.md`** - define milestone requirements.
3. **Stage 9 design and reference architecture** - define target constraints.
4. **Historical implementation plan and `docs/TODO.md`** - historical execution clues only.

Per D-19, Git analysis focuses on Stage 9 commits, commits named by the old plan, changes around
2026-07-10, and current broad-method or adapter history. Per D-09, the current tree is
authoritative for what exists; Git corroborates timing, rationale, and scope.

---

## Evidence Index

Evidence IDs are stable references used by the Operation Matrix, Findings, and Command Ledger.

### Evidence ID Namespaces

| Prefix | Category | Description |
|--------|----------|-------------|
| SRC-* | Source | File and symbol references from the live tree |
| TEST-* | Test | Focused behavior, public API, and integration test references |
| GUARD-* | Boundary Guard | Source-scanning or compile/API boundary guard references |
| GIT-* | Git History | Commit, diff, or blame references for corroboration |
| SCAN-* | Source Scan | Recursive rg scan results for caller/compatibility inventory |

### Registered Evidence IDs

| Evidence ID | Category | Reference | Collected In |
|-------------|----------|-----------|--------------|
| SRC-OP-01 | Source | `public_operation.rs:42-83` - CodingAgentOperation enum (15 variants) | 01-02 Task 1 |
| SRC-OP-02 | Source | `public_operation.rs:107-157` - into_internal exhaustive mapping (15 arms) | 01-02 Task 1 |
| SRC-OP-03 | Source | `public_operation.rs:161-182` - from_internal exhaustive projection (14 arms + export split) | 01-02 Task 1 |
| SRC-OP-04 | Source | `operation.rs:72-159` - Operation::metadata() with static_kind, origin, class, dispatch_mode | 01-02 Task 1 |
| SRC-OP-05 | Source | `mod.rs:249-261` - CodingAgentSession::run canonical dispatcher selecting from dispatch_mode | 01-02 Task 1 |
| SRC-OP-06 | Source | `operation.rs:98-103` - ApproveDelegationConfirmation has static_kind=None (dynamic) | 01-02 Task 1 |
| SRC-OP-07 | Source | `mod.rs:638,659,713` - set_default_agent_profile_id, approve/reject_delegation_confirmation NOT deprecated | 01-02 Task 1 |
| SRC-OP-08 | Source | `mod.rs:367,415,766,810,871,922,973,1134` - 8 deprecated pub compatibility method signatures | 01-02 Task 1 |
| SRC-OP-09 | Source | `mod.rs:467,1019,1040,1089` - 4 crate-private compatibility method signatures (not deprecated) | 01-02 Task 1 |
| SRC-OP-10 | Source | `mod.rs:862-870` - self_healing_edit delegates through self_healing_edit_with_options with #[allow(deprecated)] | 01-02 Task 1 |
| TEST-API-01 | Test | `public_api.rs:127` - canonical_operation_runtime_variants_are_public (1 test) | 01-02 Task 1 |
| TEST-API-02 | Test | `public_api.rs:196` - coding_session_run_dispatches_public_runtime_operations (PluginLoad, SetDefaultAgentProfile, PluginCommand, ForkSession, SwitchActiveLeaf) | 01-02 Task 1 |
| TEST-API-03 | Test | `public_api.rs:178` - coding_session_run_public_operation_facade_is_importable (ExportCurrent run) | 01-02 Task 1 |
| TEST-API-04 | Test | `public_api.rs:584-1091` - self_healing_edit behavior tests (7 tests, compat path via .self_healing_edit/_with_options) | 01-02 Task 1 |
| TEST-API-05 | Test | `public_api.rs:545` - summarize_branch behavior test (compat path via .summarize_branch) | 01-02 Task 1 |
| TEST-OWNER-01 | Test | `mod.rs:3187` - canonical_run_switches_active_leaf (SwitchActiveLeaf via run) | 01-02 Task 1 |
| TEST-OWNER-02 | Test | `mod.rs:3248` - canonical_run_forks_current_session (ForkSession via run) | 01-02 Task 1 |
| TEST-OWNER-03 | Test | `mod.rs:3323` - canonical_fork_preserves_owner_runtime_and_event_stream (fork event continuity) | 01-02 Task 1 |
| TEST-OWNER-04 | Test | `mod.rs:3430` - canonical_switch_reports_partial_commit_after_durable_leaf_change (switch partial commit) | 01-02 Task 1 |
| TEST-OWNER-05 | Test | `mod.rs:3562` - delegation_approval_operation_kind_uses_pending_team_target (dynamic kind resolves to AgentTeam) | 01-02 Task 1 |
| TEST-OWNER-06 | Test | `mod.rs:3593` - resolve_operation_admission_returns_structured_dynamic_contract (admission static_kind=None, dispatch=Async) | 01-02 Task 1 |
| TEST-OWNER-07 | Test | `mod.rs:4200` - canonical_run_reuses_branch_summary_when_requested (BranchSummary via run) | 01-02 Task 1 |
| TEST-OWNER-08 | Test | `mod.rs:4325,4415` - compact behavior tests (compat path via .compact) | 01-02 Task 1 |
| TEST-INT-01 | Test | `agent_invocation.rs:63,110,138,166,199` - invoke_agent behavior (5 tests, compat path) | 01-02 Task 1 |
| TEST-INT-02 | Test | `agent_team_flow.rs:62,144,202,249,296` - invoke_team behavior (5 tests, compat path) | 01-02 Task 1 |
| TEST-INT-03 | Test | `delegation_execution.rs` - prompt/approve/reject delegation behavior (compat path via .prompt, .approve_delegation_confirmation, .reject_delegation_confirmation) | 01-02 Task 1 |
| TEST-INT-04 | Test | `agent_profile_runtime.rs:63,141` - prompt behavior (2 tests, compat path via .prompt) | 01-02 Task 1 |
| TEST-INT-05 | Test | `agent_profile_session.rs:46` - set_default_agent_profile_id behavior (compat path) | 01-02 Task 1 |
| GUARD-DISPATCH-01 | Boundary Guard | `api_boundary_guards.rs:79` - coding_session_run_is_the_canonical_operation_dispatcher (source-scan: run body has dispatch primitives, no compat calls) | 01-02 Task 1 |
| GUARD-VIS-01 | Boundary Guard | `api_boundary_guards.rs:5` - root_public_modules_are_marked_migration_private | 01-02 Task 1 |
| GUARD-VIS-02 | Boundary Guard | `api_boundary_guards.rs:44` - root_reexports_are_explicit_compatibility_surface | 01-02 Task 1 |
| GUARD-VIS-03 | Boundary Guard | `api_boundary_guards.rs:157` - stable_api_does_not_export_compatibility_event_receiver | 01-02 Task 1 |
| GUARD-PROD-01 | Boundary Guard | `product_runtime_boundary_guards.rs:5` - product_sources_do_not_register_global_provider_runtime_outside_compat_boundary | 01-02 Task 1 |
| GUARD-PROD-02 | Boundary Guard | `product_runtime_boundary_guards.rs:28` - adapters_do_not_construct_or_run_low_level_agents | 01-02 Task 1 |
| GUARD-PROD-03 | Boundary Guard | `product_runtime_boundary_guards.rs:56` - adapters_do_not_access_event_service_directly_for_projection | 01-02 Task 1 |
| GUARD-PROD-04 | Boundary Guard | `product_runtime_boundary_guards.rs:75` - runtime_service_production_paths_require_capability_snapshot | 01-02 Task 1 |
| GUARD-PROD-05 | Boundary Guard | `product_runtime_boundary_guards.rs:118` - plugin_command_paths_use_capability_aware_execution | 01-02 Task 1 |
| GUARD-PROD-06 | Boundary Guard | `product_runtime_boundary_guards.rs:249` - rpc_running_product_events_do_not_use_unbounded_channels | 01-02 Task 1 |
| GUARD-PROD-07 | Boundary Guard | `product_runtime_boundary_guards.rs:270` - event_receiver_lag_maps_to_snapshot_recovery_error | 01-02 Task 1 |
| GIT-STAGE9-01 | Git History | commit `0fff6bd` - feat: expand canonical coding operations (public_operation.rs +51, public_api.rs +46) | 01-02 Task 3 |
| GIT-STAGE9-02 | Git History | commit `ebe48df` - refactor: make session run canonical dispatcher (mod.rs +117, public_operation.rs +173, api_boundary_guards.rs +87) | 01-02 Task 3 |
| GIT-STAGE9-03 | Git History | commit `5c5382c` - feat: complete session mutation operations (mod.rs +458, session_service.rs +305, session_log/store.rs +229) | 01-02 Task 3 |
| SCAN-PROD-01 | Source Scan | `rg CodingAgentOperation:: crates/pi-coding-agent/src/` - 0 production adapter hits (all hits in owner tests) | 01-02 Task 2 |
| SCAN-PROD-02 | Source Scan | `rg deprecated method calls in protocol/ print_mode.rs interactive/` - 30+ hits across all adapters | 01-02 Task 2 |
| SCAN-PROD-03 | Source Scan | `rg #[allow(deprecated)] in protocol/ print_mode.rs interactive/` - 13 hits across print, json, rpc, interactive | 01-02 Task 2 |
| SCAN-TEST-01 | Source Scan | `rg CodingAgentOperation:: crates/pi-coding-agent/tests/` - 14 hits in public_api.rs (canonical run callers) | 01-02 Task 2 |
| SCAN-TEST-02 | Source Scan | `rg deprecated method calls in tests/` - 50+ hits across integration tests (compat path callers) | 01-02 Task 2 |

---

## Command Ledger

Every focused test or source scan recorded as audit evidence must appear here with exact command,
date, exit status, executed-test count, result, and evidence ID. The validator rejects Cargo
ledger rows whose executed-test count is not a positive integer. Zero-test output is a validation
failure.

| Evidence ID | Command | Date | Exit Status | Test Count | Result |
|-------------|---------|------|-------------|------------|--------|
| TEST-API-01 | `cargo test -p pi-coding-agent --test public_api canonical_operation_runtime_variants_are_public -- --nocapture` | 2026-07-10 | 0 | 1 | pass |
| GUARD-DISPATCH-01 | `cargo test -p pi-coding-agent --test api_boundary_guards coding_session_run_is_the_canonical_operation_dispatcher -- --nocapture` | 2026-07-10 | 0 | 1 | pass |
| GUARD-PROD-01 through GUARD-PROD-07 | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards -- --nocapture` | 2026-07-10 | 0 | 7 | pass |
| SCAN-PROD-01 | `rg -n 'CodingAgentOperation::' crates/pi-coding-agent/src/ --type rust` | 2026-07-10 | 0 | 0 | 0 production adapter hits; all matches in mod.rs owner tests and public_operation.rs test |
| SCAN-PROD-02 | `rg -n '\.(prompt\|compact\|self_healing_edit\|invoke_agent\|invoke_team\|summarize_branch\|export_current\|export_current_html\|set_default_agent_profile_id\|approve_delegation_confirmation\|reject_delegation_confirmation\|fork_current_session\|reload_plugins\|run_plugin_command\|load_plugins)\(' crates/pi-coding-agent/src/protocol/ crates/pi-coding-agent/src/print_mode.rs crates/pi-coding-agent/src/interactive/ --type rust` | 2026-07-10 | 0 | 0 | 30+ hits across all adapter roots |
| SCAN-PROD-03 | `rg -n '#\[allow\(deprecated\)\]' crates/pi-coding-agent/src/protocol/ crates/pi-coding-agent/src/print_mode.rs crates/pi-coding-agent/src/interactive/ --type rust` | 2026-07-10 | 0 | 0 | 13 hits: print_mode(2), json_mode(1), prompt_task(6), rpc/commands(1), rpc/prompt(3) |
| SCAN-TEST-01 | `rg -n 'CodingAgentOperation::' crates/pi-coding-agent/tests/ --type rust` | 2026-07-10 | 0 | 0 | 14 hits in public_api.rs: ExportCurrent, PluginLoad, SetDefaultAgentProfile, PluginCommand, ForkSession, SwitchActiveLeaf, BranchSummary |
| SCAN-TEST-02 | `rg -n '\.(prompt\|compact\|self_healing_edit\|invoke_agent\|invoke_team\|summarize_branch\|export_current\|set_default_agent_profile_id\|approve_delegation_confirmation\|reject_delegation_confirmation)\(' crates/pi-coding-agent/tests/ --type rust` | 2026-07-10 | 0 | 0 | 50+ hits across agent_invocation, agent_team_flow, agent_profile_runtime, delegation_execution, agent_profile_session, public_api |
| GIT-STAGE9-01 | `git show --stat --oneline 0fff6bd` | 2026-07-10 | 0 | 0 | 6 files, +132 -24: public_operation.rs, public_api.rs, mod.rs, lib.rs, TODO.md, plan.md |
| GIT-STAGE9-02 | `git show --stat --oneline ebe48df` | 2026-07-10 | 0 | 0 | 8 files, +441 -114: mod.rs, public_operation.rs, operation.rs, api_boundary_guards.rs, public_api.rs, operation_control.rs, TODO.md, plan.md |
| GIT-STAGE9-03 | `git show --stat --oneline 5c5382c` | 2026-07-10 | 0 | 0 | 10 files, +1021 -132: mod.rs, public_operation.rs, intent_router.rs, session_log/store.rs, session_service.rs, api_boundary_guards.rs, public_api.rs, TODO.md, plan.md, design.md |

---

## Operation Matrix

One row per public `CodingAgentOperation` variant. Structural columns are seeded from live source
(`public_operation.rs`, `operation.rs`). Assessment columns are populated by Plan 01-02.

**Column key:**
- `variant` - exact `CodingAgentOperation` spelling
- `internal` - exact `Operation` variant mapping
- `kind` - `static` if `static_kind` is `Some`, `dynamic` if `None`
- `origin` - `OperationOrigin` from metadata
- `class` - `OperationClass` from metadata
- `dispatch` - `OperationDispatchMode` from metadata
- `outcome` - exact `CodingAgentOperationOutcome` variant
- `prod_callers` - production caller files/symbols (populated by 01-02)
- `test_callers` - test caller files/symbols (populated by 01-02)
- `impl` - implementation status (D-12 taxonomy, populated by 01-02)
- `verify` - verification status (D-10 taxonomy, populated by 01-02)
- `disp` - disposition (D-13 taxonomy, populated by 01-02)
- `conf` - confidence level (D-14 taxonomy, populated by 01-02)
- `evidence` - comma-separated evidence IDs (populated by 01-02)
- `gaps` - evidence gaps; `none` when empty (D-15)
- `blockers` - blockers; `none` when empty (D-15)

| variant | internal | kind | origin | class | dispatch | outcome | prod_callers | test_callers | impl | verify | disp | conf | evidence | gaps | blockers |
|---------|----------|------|--------|-------|----------|---------|--------------|--------------|------|--------|------|------|----------|------|----------|
| Prompt | Operation::Prompt | static | ClientRoot | SessionWriteRoot | Async | Prompt(PromptTurnOutcome) | print_mode:128 .prompt(); json_mode:99 .prompt(); rpc/prompt:893 .prompt(); interactive/prompt_task:634 .prompt() | TEST-INT-03 delegation_execution .prompt(); TEST-INT-04 agent_profile_runtime .prompt(); TEST-API-05 public_api:540 .prompt() | complete | passed | active | high | SRC-OP-01, SRC-OP-02, SRC-OP-04, SRC-OP-05, GUARD-DISPATCH-01, TEST-API-01, TEST-INT-03, TEST-INT-04 | no canonical run() behavior test for Prompt; behavior proven through .prompt() compat path only | none |
| Compact | Operation::ManualCompaction | static | ClientRoot | SessionWriteRoot | Async | Compact(PromptTurnOutcome) | interactive/prompt_task:874 .compact() | TEST-OWNER-08 mod.rs:4325,4415 .compact() | complete | passed | active | high | SRC-OP-01, SRC-OP-02, SRC-OP-04, SRC-OP-05, GUARD-DISPATCH-01, TEST-API-01, TEST-OWNER-08 | no canonical run() behavior test for Compact; behavior proven through .compact() compat path only | none |
| BranchSummary | Operation::BranchSummary | static | ClientRoot | SessionWriteRoot | Async | BranchSummary(PromptTurnOutcome) | interactive/prompt_task:1186 .summarize_branch() | TEST-OWNER-07 mod.rs:4200 run(); TEST-API-05 public_api:545 .summarize_branch(); TEST-OWNER-08 mod.rs:4115 .summarize_branch() | complete | passed | active | high | SRC-OP-01, SRC-OP-02, SRC-OP-03, SRC-OP-04, SRC-OP-05, GUARD-DISPATCH-01, TEST-API-01, TEST-OWNER-07, TEST-OWNER-08 | none | none |
| SelfHealingEdit | Operation::SelfHealingEdit | static | ClientRoot | SessionWriteRoot | Async | SelfHealingEdit(SelfHealingEditOutcome) | rpc/commands:614 .self_healing_edit_with_options() | TEST-API-04 public_api:584-1091 .self_healing_edit/_with_options() (7 tests) | complete | passed | active | high | SRC-OP-01, SRC-OP-02, SRC-OP-04, SRC-OP-05, SRC-OP-10, GUARD-DISPATCH-01, TEST-API-01, TEST-API-04 | no canonical run() behavior test for SelfHealingEdit; behavior proven through .self_healing_edit_with_options() compat path only | none |
| InvokeAgent | Operation::AgentInvocation | static | ClientRoot | NonSessionRoot | Async | AgentInvocation(AgentInvocationOutcome) | interactive/prompt_task:712 .invoke_agent(); rpc/prompt:376 .invoke_agent() | TEST-INT-01 agent_invocation.rs .invoke_agent() (5 tests) | complete | passed | active | high | SRC-OP-01, SRC-OP-02, SRC-OP-04, SRC-OP-05, GUARD-DISPATCH-01, TEST-API-01, TEST-INT-01 | no canonical run() behavior test for InvokeAgent; behavior proven through .invoke_agent() compat path only | none |
| InvokeTeam | Operation::AgentTeam | static | ClientRoot | NonSessionRoot | Async | AgentTeam(AgentTeamOutcome) | interactive/prompt_task:784 .invoke_team(); rpc/prompt:593 .invoke_team() | TEST-INT-02 agent_team_flow.rs .invoke_team() (5 tests) | complete | passed | active | high | SRC-OP-01, SRC-OP-02, SRC-OP-04, SRC-OP-05, GUARD-DISPATCH-01, TEST-API-01, TEST-INT-02 | no canonical run() behavior test for InvokeTeam; behavior proven through .invoke_team() compat path only | none |
| PluginLoad | Operation::PluginLoad | static | ClientRoot | RuntimeWrite | Async | PluginLoad(CodingAgentPluginLoadOutcome) | interactive/prompt_task:979,1042 .reload_plugins(); rpc/commands:1125,1191 .reload_plugins() | TEST-API-02 public_api:208 run(); TEST-INT-05 agent_profile_session .set_default_agent_profile_id() (indirect) | complete | passed | active | high | SRC-OP-01, SRC-OP-02, SRC-OP-03, SRC-OP-04, SRC-OP-05, GUARD-DISPATCH-01, TEST-API-01, TEST-API-02 | none | none |
| PluginCommand | Operation::PluginCommand | static | ClientRoot | NonSessionRoot | SyncReadOnly | PluginCommand(String) | interactive/prompt_task:1068 .run_plugin_command(); rpc/commands:1135 .run_plugin_command() | TEST-API-02 public_api:232 run() (error path); TEST-OWNER-01 mod.rs:3387 run() | complete | passed | active | high | SRC-OP-01, SRC-OP-02, SRC-OP-03, SRC-OP-04, SRC-OP-05, GUARD-DISPATCH-01, TEST-API-01, TEST-API-02 | none | none |
| SetDefaultAgentProfile | Operation::SetDefaultAgentProfile | static | ClientRoot | RuntimeWrite | SyncMutable | DefaultAgentProfileChanged | interactive/loop:805,1998,2013,2027,2047,2390 .set_default_agent_profile_id(); interactive/root:1298 .set_default_agent_profile_id(); rpc/commands:845 .set_default_agent_profile_id() | TEST-API-02 public_api:217 run(); TEST-OWNER-01 mod.rs:3367,3398 run(); TEST-INT-05 agent_profile_session:46 .set_default_agent_profile_id() | complete | passed | active | high | SRC-OP-01, SRC-OP-02, SRC-OP-03, SRC-OP-04, SRC-OP-05, SRC-OP-07, GUARD-DISPATCH-01, TEST-API-01, TEST-API-02, TEST-INT-05 | none | none |
| ApproveDelegation | Operation::ApproveDelegationConfirmation | dynamic | ClientRoot | NonSessionRoot | Async | DelegationApproved | interactive/prompt_task:823 .approve_delegation_confirmation(); rpc/prompt:769 .approve_delegation_confirmation() | TEST-INT-03 delegation_execution .approve_delegation_confirmation(); TEST-OWNER-05 mod.rs:3562 dynamic kind; TEST-OWNER-06 mod.rs:3593 admission | complete | passed | active | high | SRC-OP-01, SRC-OP-02, SRC-OP-03, SRC-OP-04, SRC-OP-05, SRC-OP-06, GUARD-DISPATCH-01, TEST-API-01, TEST-OWNER-05, TEST-OWNER-06, TEST-INT-03 | no canonical run() behavior test for ApproveDelegation; behavior proven through .approve_delegation_confirmation() compat path; dynamic kind verified by TEST-OWNER-05/06 | none |
| RejectDelegation | Operation::RejectDelegationConfirmation | static | ClientRoot | Control | SyncMutable | DelegationRejected | interactive/loop:1331 .reject_delegation_confirmation(); rpc/commands:1041 .reject_delegation_confirmation() | TEST-INT-03 delegation_execution .reject_delegation_confirmation() | complete | passed | active | high | SRC-OP-01, SRC-OP-02, SRC-OP-03, SRC-OP-04, SRC-OP-05, SRC-OP-07, GUARD-DISPATCH-01, TEST-API-01, TEST-INT-03 | no canonical run() behavior test for RejectDelegation; behavior proven through .reject_delegation_confirmation() compat path only | none |
| ForkSession | Operation::ForkSession | static | ClientRoot | SessionWriteRoot | SyncMutable | SessionForked | interactive/prompt_task:1276 .fork_current_session() | TEST-API-02 public_api:241 run() (error path); TEST-OWNER-02 mod.rs:3283,3376 run() (success); TEST-OWNER-03 mod.rs:3323 event continuity | complete | passed | active | high | SRC-OP-01, SRC-OP-02, SRC-OP-03, SRC-OP-04, SRC-OP-05, GUARD-DISPATCH-01, TEST-API-01, TEST-API-02, TEST-OWNER-02, TEST-OWNER-03 | none | none |
| SwitchActiveLeaf | Operation::SwitchActiveLeaf | static | ClientRoot | SessionWriteRoot | SyncMutable | ActiveLeafSwitched | none (no production adapter calls run() for SwitchActiveLeaf) | TEST-API-02 public_api:254 run() (error path); TEST-OWNER-01 mod.rs:3221,3471 run() (success); TEST-OWNER-04 mod.rs:3430 partial commit | complete | passed | active | high | SRC-OP-01, SRC-OP-02, SRC-OP-03, SRC-OP-04, SRC-OP-05, GUARD-DISPATCH-01, TEST-API-01, TEST-API-02, TEST-OWNER-01, TEST-OWNER-04 | none | none |
| ExportCurrent | Operation::Export(ExportOptions::view()) | static | ClientRoot | ReadOnly | SyncReadOnly | Export(CodingAgentSessionExport) | none (no production adapter calls run() for ExportCurrent) | TEST-API-03 public_api:188 run(); TEST-INT-01 agent_invocation:147 .export_current(); TEST-INT-02 agent_team_flow:94 .export_current(); TEST-INT-03 delegation_execution:321 .export_current() | complete | passed | active | high | SRC-OP-01, SRC-OP-02, SRC-OP-03, SRC-OP-04, SRC-OP-05, GUARD-DISPATCH-01, TEST-API-01, TEST-API-03, TEST-INT-01, TEST-INT-02, TEST-INT-03 | none | none |
| ExportCurrentHtml | Operation::Export(ExportOptions::html(path)) | static | ClientRoot | ReadOnly | SyncReadOnly | ExportHtml(PathBuf) | interactive/session_actions:389 CodingAgentSession::export_session_html() (static helper, not deprecated instance method) | none (no test calls ExportCurrentHtml via run() or .export_current_html()) | complete | not_run | active | medium | SRC-OP-01, SRC-OP-02, SRC-OP-03, SRC-OP-04, SRC-OP-05, SRC-OP-08, GUARD-DISPATCH-01, TEST-API-01 | no focused behavior test for ExportCurrentHtml through any path; source and dispatch guard prove implementation but no test verifies outcome | none |

---

## Production Caller Inventory

Production caller inventory is populated by Plan 01-02 from live source scans. The expected
caller roots are:

- `crates/pi-coding-agent/src/protocol/json_mode.rs` - JSON one-shot prompt path
- `crates/pi-coding-agent/src/print_mode.rs` - Print persistent and transient prompt paths
- `crates/pi-coding-agent/src/protocol/rpc/` - Streaming RPC callers and control multiplexing
- `crates/pi-coding-agent/src/interactive/` - Interactive background operations and mutations

| Caller File | Caller Symbol | Operation | Canonical (run) | Compatibility Path | Evidence |
|-------------|---------------|-----------|-----------------|--------------------|---------|
| print_mode.rs:128 | run_persistent_print | Prompt | no | .prompt() with #[allow(deprecated)] at :108 | SCAN-PROD-02, SCAN-PROD-03 |
| print_mode.rs:144 | run_transient_print | Prompt | no | .prompt() with #[allow(deprecated)] at :132 | SCAN-PROD-02, SCAN-PROD-03 |
| protocol/json_mode.rs:99 | run_json_mode | Prompt | no | .prompt() with #[allow(deprecated)] at :87 | SCAN-PROD-02, SCAN-PROD-03 |
| protocol/rpc/prompt.rs:893 | run_prompt_background | Prompt | no | .prompt() with #[allow(deprecated)] at :825 | SCAN-PROD-02, SCAN-PROD-03 |
| protocol/rpc/prompt.rs:376 | run_agent_background | InvokeAgent | no | .invoke_agent() with #[allow(deprecated)] at :210 | SCAN-PROD-02, SCAN-PROD-03 |
| protocol/rpc/prompt.rs:593 | run_team_background | InvokeTeam | no | .invoke_team() with #[allow(deprecated)] at :432 | SCAN-PROD-02, SCAN-PROD-03 |
| protocol/rpc/prompt.rs:769 | run_delegation_approval | ApproveDelegation | no | .approve_delegation_confirmation() with #[allow(deprecated)] at :825 | SCAN-PROD-02, SCAN-PROD-03, SRC-OP-07 |
| protocol/rpc/commands.rs:614 | handle_self_healing_edit | SelfHealingEdit | no | .self_healing_edit_with_options() with #[allow(deprecated)] at :501 | SCAN-PROD-02, SCAN-PROD-03 |
| protocol/rpc/commands.rs:845 | handle_set_default_profile | SetDefaultAgentProfile | no | .set_default_agent_profile_id() (not deprecated, no allow) | SCAN-PROD-02, SRC-OP-07 |
| protocol/rpc/commands.rs:1041 | handle_reject_delegation | RejectDelegation | no | .reject_delegation_confirmation() (not deprecated, no allow) | SCAN-PROD-02, SRC-OP-07 |
| protocol/rpc/commands.rs:1125,1191 | handle_plugin_load | PluginLoad | no | .reload_plugins() (crate-private, not deprecated) | SCAN-PROD-02, SRC-OP-09 |
| protocol/rpc/commands.rs:1135 | handle_plugin_command | PluginCommand | no | .run_plugin_command() (crate-private, not deprecated) | SCAN-PROD-02, SRC-OP-09 |
| interactive/prompt_task.rs:634 | run_prompt_background | Prompt | no | .prompt() with #[allow(deprecated)] at :609 | SCAN-PROD-02, SCAN-PROD-03 |
| interactive/prompt_task.rs:712 | run_agent_background | InvokeAgent | no | .invoke_agent() with #[allow(deprecated)] at :681 | SCAN-PROD-02, SCAN-PROD-03 |
| interactive/prompt_task.rs:784 | run_team_background | InvokeTeam | no | .invoke_team() with #[allow(deprecated)] at :754 | SCAN-PROD-02, SCAN-PROD-03 |
| interactive/prompt_task.rs:823 | run_delegation_approval | ApproveDelegation | no | .approve_delegation_confirmation() with #[allow(deprecated)] at :850 | SCAN-PROD-02, SCAN-PROD-03, SRC-OP-07 |
| interactive/prompt_task.rs:874 | run_compact | Compact | no | .compact() with #[allow(deprecated)] at :850 | SCAN-PROD-02, SCAN-PROD-03 |
| interactive/prompt_task.rs:930 | run_self_healing_edit | SelfHealingEdit | no | .self_healing_edit_with_options() with #[allow(deprecated)] at :906 | SCAN-PROD-02, SCAN-PROD-03 |
| interactive/prompt_task.rs:979,1042 | run_plugin_reload | PluginLoad | no | .reload_plugins() (crate-private, not deprecated) | SCAN-PROD-02, SRC-OP-09 |
| interactive/prompt_task.rs:1068 | run_plugin_command | PluginCommand | no | .run_plugin_command() (crate-private, not deprecated) | SCAN-PROD-02, SRC-OP-09 |
| interactive/prompt_task.rs:1186 | run_branch_summary | BranchSummary | no | .summarize_branch() with #[allow(deprecated)] at :1159 | SCAN-PROD-02, SCAN-PROD-03 |
| interactive/prompt_task.rs:1276 | fork_session | ForkSession | no | .fork_current_session() (crate-private, not deprecated) | SCAN-PROD-02, SRC-OP-09 |
| interactive/loop.rs:805,1998-2047,2390 | set_default_agent_profile | SetDefaultAgentProfile | no | .set_default_agent_profile_id() (not deprecated, no allow) | SCAN-PROD-02, SRC-OP-07 |
| interactive/loop.rs:1331 | reject_delegation | RejectDelegation | no | .reject_delegation_confirmation() (not deprecated, no allow) | SCAN-PROD-02, SRC-OP-07 |
| interactive/root.rs:1298 | apply_profile | SetDefaultAgentProfile | no | .set_default_agent_profile_id() (not deprecated, no allow) | SCAN-PROD-02, SRC-OP-07 |
| interactive/session_actions.rs:389 | export_session_html | ExportCurrentHtml | no | CodingAgentSession::export_session_html() (static helper, not deprecated instance method) | SCAN-PROD-02 |

---

## Test Caller Inventory

Test caller inventory is populated by Plan 01-02 from live source scans. The expected test roots are:

- `crates/pi-coding-agent/tests/public_api.rs` - Stable facade and public operation behavior
- `crates/pi-coding-agent/tests/api_boundary_guards.rs` - Public/private API and dispatcher boundary
- `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` - Adapter call and deprecation guards
- `crates/pi-coding-agent/src/coding_session/mod.rs` (owner tests) - Owner unit tests
- Integration tests under `crates/pi-coding-agent/tests/`

| Test File | Test Name | Operation | Canonical (run) | Compatibility Path | Evidence |
|-----------|-----------|-----------|-----------------|--------------------|---------|
| public_api.rs:127 | canonical_operation_runtime_variants_are_public | all 15 variants | yes (constructs all variants, asserts public) | - | TEST-API-01 |
| public_api.rs:178 | coding_session_run_public_operation_facade_is_importable | ExportCurrent | yes (run) | - | TEST-API-03 |
| public_api.rs:196 | coding_session_run_dispatchs_public_runtime_operations | PluginLoad, SetDefaultAgentProfile, PluginCommand, ForkSession, SwitchActiveLeaf | yes (run) | - | TEST-API-02 |
| public_api.rs:540 | prompt error test | Prompt | no | .prompt() | TEST-API-05, SCAN-TEST-02 |
| public_api.rs:545 | summarize_branch test | BranchSummary | no | .summarize_branch() | TEST-API-05, SCAN-TEST-02 |
| public_api.rs:584-1091 | self_healing_edit tests (7 tests) | SelfHealingEdit | no | .self_healing_edit() / .self_healing_edit_with_options() | TEST-API-04, SCAN-TEST-02 |
| agent_invocation.rs:63,110,138,166,199 | invoke_agent tests (5 tests) | InvokeAgent | no | .invoke_agent() | TEST-INT-01, SCAN-TEST-02 |
| agent_invocation.rs:147 | export_current assertion | ExportCurrent | no | .export_current() | TEST-INT-01, SCAN-TEST-02 |
| agent_team_flow.rs:62,144,202,249,296 | invoke_team tests (5 tests) | InvokeTeam | no | .invoke_team() | TEST-INT-02, SCAN-TEST-02 |
| agent_team_flow.rs:94 | export_current assertion | ExportCurrent | no | .export_current() | TEST-INT-02, SCAN-TEST-02 |
| agent_profile_runtime.rs:63,141 | prompt tests (2 tests) | Prompt | no | .prompt() | TEST-INT-04, SCAN-TEST-02 |
| delegation_execution.rs | prompt/approve/reject delegation tests (17+ calls) | Prompt, ApproveDelegation, RejectDelegation, ExportCurrent | no | .prompt(), .approve_delegation_confirmation(), .reject_delegation_confirmation(), .export_current() | TEST-INT-03, SCAN-TEST-02 |
| agent_profile_session.rs:46 | set_default_agent_profile_id test | SetDefaultAgentProfile | no | .set_default_agent_profile_id() | TEST-INT-05, SCAN-TEST-02 |
| mod.rs:3187 | canonical_run_switches_active_leaf | SwitchActiveLeaf | yes (run) | - | TEST-OWNER-01 |
| mod.rs:3248 | canonical_run_forks_current_session | ForkSession | yes (run) | - | TEST-OWNER-02 |
| mod.rs:3323 | canonical_fork_preserves_owner_runtime_and_event_stream | ForkSession | yes (run, event continuity) | - | TEST-OWNER-03 |
| mod.rs:3430 | canonical_switch_reports_partial_commit_after_durable_leaf_change | SwitchActiveLeaf | yes (run, partial commit) | - | TEST-OWNER-04 |
| mod.rs:3562 | delegation_approval_operation_kind_uses_pending_team_target | ApproveDelegation | partial (tests dynamic kind resolution, not run()) | - | TEST-OWNER-05 |
| mod.rs:3593 | resolve_operation_admission_returns_structured_dynamic_contract | ApproveDelegation | partial (tests admission contract, not run()) | - | TEST-OWNER-06 |
| mod.rs:4200 | canonical_run_reuses_branch_summary_when_requested | BranchSummary | yes (run) | - | TEST-OWNER-07 |
| mod.rs:4115 | branch_summary_persistent_session_records_model_summary | BranchSummary | no | .summarize_branch() | TEST-OWNER-08 |
| mod.rs:4325,4415 | compact behavior tests (2 tests) | Compact | no | .compact() | TEST-OWNER-08 |
| api_boundary_guards.rs:79 | coding_session_run_is_the_canonical_operation_dispatcher | (source-scan guard for run body) | yes (guards dispatch primitives, rejects compat calls in run body) | - | GUARD-DISPATCH-01 |
| api_boundary_guards.rs:5 | root_public_modules_are_marked_migration_private | (visibility guard) | - | - | GUARD-VIS-01 |
| api_boundary_guards.rs:44 | root_reexports_are_explicit_compatibility_surface | (re-export guard) | - | - | GUARD-VIS-02 |
| api_boundary_guards.rs:157 | stable_api_does_not_export_compatibility_event_receiver | (API surface guard) | - | - | GUARD-VIS-03 |
| product_runtime_boundary_guards.rs:5 | product_sources_do_not_register_global_provider_runtime_outside_compat_boundary | (runtime boundary) | - | - | GUARD-PROD-01 |
| product_runtime_boundary_guards.rs:28 | adapters_do_not_construct_or_run_low_level_agents | (adapter boundary) | - | - | GUARD-PROD-02 |
| product_runtime_boundary_guards.rs:56 | adapters_do_not_access_event_service_directly_for_projection | (event boundary) | - | - | GUARD-PROD-03 |
| product_runtime_boundary_guards.rs:75 | runtime_service_production_paths_require_capability_snapshot | (capability boundary) | - | - | GUARD-PROD-04 |
| product_runtime_boundary_guards.rs:118 | plugin_command_paths_use_capability_aware_execution | (plugin boundary) | - | - | GUARD-PROD-05 |
| product_runtime_boundary_guards.rs:249 | rpc_running_product_events_do_not_use_unbounded_channels | (channel boundary) | - | - | GUARD-PROD-06 |
| product_runtime_boundary_guards.rs:270 | event_receiver_lag_maps_to_snapshot_recovery_error | (recovery boundary) | - | - | GUARD-PROD-07 |

**Boundary guard scope note:** The seven `product_runtime_boundary_guards.rs` tests guard
runtime architecture constraints (provider registration, low-level agent construction, event
service access, capability snapshots, plugin command execution, channel bounds, and lag
recovery). They do **not** reject replaced broad workflow calls or local `#[allow(deprecated)]`
attributes in production adapter files. The Stage 9 adapter-call and deprecation-suppression
guards described by the old implementation plan are **not present** in the current tree. This
absence is routed to Phase 5 hardening as finding F-GUARD-01 (populated in Task 3).

---

## Compatibility Inventory

Per D-03, broad live-session compatibility methods are tracked separately from the operation matrix.
Per DELETE-04, construction/open/resume, snapshots, queries, subscriptions/control, and static
repository helpers are NOT operation-facade replacements and are excluded unless a finding explains
why. The compatibility methods listed below were verified from current source by Plan 01-02.

**Excluded non-replacements (DELETE-04):** `create`, `open`, `open_or_create`, `non_persistent`,
`list`, `hydrate`, `tree_view`, `clone_session`, `fork_session` (static), `export_session_html`
(static helper using SessionService directly), `snapshot`, `connect`, `subscribe`,
`subscribe_product_events_public`, `product_event_replay_handle`, `hydrate_current`,
`fork_current_session` (static), and all `pub(crate)` query/view helpers are NOT operation-facade
replacements and remain available. The deprecated `subscribe` method is a compatibility event
subscription whose deletion is Stage 10 scope (deferred_stage_10).

| Method | Visibility | Deprecation | Matching Operation | Prod Callers | Test Callers | Retention Reason | Deletion Req | Target Phase |
|--------|------------|-------------|--------------------|--------------|--------------|------------------|-------------|--------------|
| export_current_html | pub | #[deprecated] | ExportCurrentHtml | none (production uses static export_session_html instead) | none | No production or test callers; retained for API compatibility | DELETE-01 | Phase 4 |
| export_current | pub | #[deprecated] | ExportCurrent | none | agent_invocation.rs:147, agent_team_flow.rs:94, delegation_execution.rs:321 | Test callers use .export_current() for export assertions | DELETE-01 | Phase 4 |
| set_default_agent_profile_id | pub | none (NOT deprecated) | SetDefaultAgentProfile | interactive/loop.rs:805,1998-2047,2390; interactive/root.rs:1298; rpc/commands.rs:845 | agent_profile_session.rs:46 | Not yet deprecated; all prod and test callers bypass run() | DELETE-01 | Phase 4 |
| approve_delegation_confirmation | pub | none (NOT deprecated) | ApproveDelegation | interactive/prompt_task.rs:823; rpc/prompt.rs:769 | delegation_execution.rs:749,1244,1474 | Not yet deprecated; all prod and test callers bypass run() | DELETE-01 | Phase 4 |
| reject_delegation_confirmation | pub | none (NOT deprecated) | RejectDelegation | interactive/loop.rs:1331; rpc/commands.rs:1041 | delegation_execution.rs:1332,1563 | Not yet deprecated; all prod and test callers bypass run() | DELETE-01 | Phase 4 |
| prompt | pub | #[deprecated] | Prompt | print_mode.rs:128,144; json_mode.rs:99; rpc/prompt.rs:893; interactive/prompt_task.rs:634 | public_api.rs:540; agent_profile_runtime.rs:63,141; delegation_execution.rs (17+ calls) | All prod and test callers bypass run(); #[allow(deprecated)] in all prod files | DELETE-01 | Phase 4 |
| compact | pub | #[deprecated] | Compact | interactive/prompt_task.rs:874 | mod.rs:4325,4415 | Prod and test callers bypass run(); #[allow(deprecated)] in prompt_task.rs | DELETE-01 | Phase 4 |
| self_healing_edit | pub | none (NOT deprecated, delegates through deprecated self_healing_edit_with_options with #[allow(deprecated)]) | SelfHealingEdit | none | public_api.rs:600,1058,1091 | Delegates to deprecated options method; test callers use both forms | DELETE-01 | Phase 4 |
| self_healing_edit_with_options | pub | #[deprecated] | SelfHealingEdit | rpc/commands.rs:614; interactive/prompt_task.rs:930 | public_api.rs:667,720,798,943,1015 (5 tests) | Prod and test callers bypass run(); #[allow(deprecated)] in prod files | DELETE-01 | Phase 4 |
| invoke_agent | pub | #[deprecated] | InvokeAgent | interactive/prompt_task.rs:712; rpc/prompt.rs:376 | agent_invocation.rs (5 tests) | Prod and test callers bypass run(); #[allow(deprecated)] in prod files | DELETE-01 | Phase 4 |
| invoke_team | pub | #[deprecated] | InvokeTeam | interactive/prompt_task.rs:784; rpc/prompt.rs:593 | agent_team_flow.rs (5 tests) | Prod and test callers bypass run(); #[allow(deprecated)] in prod files | DELETE-01 | Phase 4 |
| summarize_branch | pub | #[deprecated] | BranchSummary | interactive/prompt_task.rs:1186 | public_api.rs:545; mod.rs:4115,4241 | Prod and test callers bypass run(); #[allow(deprecated)] in prompt_task.rs | DELETE-01 | Phase 4 |
| fork_current_session | pub(crate) | none | ForkSession | interactive/prompt_task.rs:1276 | none | Crate-private; production caller bypasses run() | DELETE-01 | Phase 4 |
| reload_plugins | pub(crate) | none | PluginLoad | interactive/prompt_task.rs:979,1042; rpc/commands.rs:1125,1191 | none | Crate-private; production callers bypass run() | DELETE-01 | Phase 4 |
| run_plugin_command | pub(crate) | none | PluginCommand | interactive/prompt_task.rs:1068; rpc/commands.rs:1135 | none | Crate-private; production callers bypass run() | DELETE-01 | Phase 4 |
| load_plugins | pub(crate) | none | PluginLoad | none (internal only, called by reload_plugins) | none | Crate-private; no direct production or test callers | DELETE-01 | Phase 4 |

---

## Authority Reconciliation

Per D-18, when authorities conflict, record an explicit finding rather than silently choosing one
source. Reconciliation entries are populated by Plan 01-02 when evidence collection surfaces
conflicts between current source, planning documents, design specs, and historical plan material.

| Conflict ID | Authority A | Authority B | Description | Resolution | Finding Ref |
|-------------|-------------|-------------|-------------|------------|-------------|
| CONFLICT-01 | 01-AUDIT.md scaffold (from research) | Current source mod.rs:638,659,713 | Scaffold marked set_default_agent_profile_id, approve_delegation_confirmation, reject_delegation_confirmation as #[deprecated] but current source shows NO #[deprecated] attribute on these three methods | Current source wins (D-09). Corrected in Compatibility Inventory: these three methods are pub with no deprecation. | F-COMPAT-01 |
| CONFLICT-02 | Historical plan narrative (pre-ebe48df context) | Current source mod.rs:249-261 + GUARD-DISPATCH-01 | Old plan context suggested run() called deprecated wrappers and carried a deprecation allowance; current source proves run() uses metadata-selected internal dispatch with no compat calls | Current source wins (D-09). Historical plan Tasks 1-3 checked boxes are completed baseline, not assumptions. GIT-STAGE9-02 corroborates timing. | F-HIST-01 |
| CONFLICT-03 | Historical plan narrative (pre-5c5382c context) | Commit 5c5382c + owner tests TEST-OWNER-01..04 | Old plan suggested fork was admitted but rejected and switch had no owner operation; current source/tests show canonical fork/switch with event continuity and partial commit | Current source/tests win (D-09). Fork/switch are completed baseline. GIT-STAGE9-03 corroborates timing. | F-HIST-01 |
| CONFLICT-04 | Historical plan Tasks 4-10 unchecked boxes | Current source scans SCAN-PROD-01..03, SCAN-TEST-01..02 | Historical plan unchecked boxes for adapter migration, test migration, deletion, guards, and closure; current source scans confirm these are genuinely incomplete | Authorities agree - no conflict. Historical plan unchecked boxes are active gaps confirmed by current evidence. | F-ADAPT-01, F-TEST-01 |

---

## Findings

Per D-04 and D-05, findings explain cross-operation gaps, contradictions, risks, and obsolete plan
material. Every finding records affected requirement IDs, target Phase 2-5, dependencies,
obligation, disposition, confidence, evidence gaps, and blockers. Phase 1 does not write
implementation task prose (D-05).

**Finding obligation taxonomy (D-16):** `blocking` \| `required` \| `hardening` \| `informational`
**Disposition taxonomy (D-13):** `active` \| `obsolete` \| `deferred_stage_10` \| `retained_compatibility`
**Confidence taxonomy (D-14):** `high` \| `medium` \| `low`

| ID | Obligation | Disposition | Description | Evidence | Requirements | Target Phase | Dependencies | Confidence | Gaps | Blockers |
|----|-----------|-------------|-------------|----------|--------------|--------------|--------------|-----------|------|---------|
| F-ADAPT-01 | required | active | No production adapter uses CodingAgentSession::run(); all JSON, print, RPC, and interactive callers invoke deprecated compatibility methods with local #[allow(deprecated)] suppressions | SCAN-PROD-01, SCAN-PROD-02, SCAN-PROD-03, SRC-OP-05, GUARD-DISPATCH-01 | ADAPT-01, ADAPT-02, ADAPT-04, RPC-01, RPC-02, RPC-04, INTER-01, INTER-02, INTER-05 | Phase 3 | facade correctness complete (SRC-OP-05, GUARD-DISPATCH-01) | high | none | none |
| F-TEST-01 | required | active | Integration tests and some public API tests use compatibility methods instead of run(); behavior is proven through compat path but canonical-run coverage is incomplete for 8 of 15 variants | SCAN-TEST-01, SCAN-TEST-02, TEST-API-04, TEST-API-05, TEST-INT-01, TEST-INT-02, TEST-INT-03, TEST-INT-04, TEST-INT-05, TEST-OWNER-08 | TEST-01, TEST-02, TEST-03 | Phase 4 | adapter migration (F-ADAPT-01) | high | none | none |
| F-DELETE-01 | required | retained_compatibility | 16 broad live-session compatibility methods remain in mod.rs; 8 are #[deprecated], 3 are pub without deprecation, 1 delegates through deprecated, 4 are crate-private; all have matching canonical operations | SRC-OP-08, SRC-OP-09, SRC-OP-10, SRC-OP-07, SCAN-PROD-02, SCAN-TEST-02 | DELETE-01, DELETE-02, DELETE-03 | Phase 4 | adapter + test migration (F-ADAPT-01, F-TEST-01) | high | none | none |
| F-COMPAT-01 | hardening | active | Three overlapping pub methods (set_default_agent_profile_id, approve_delegation_confirmation, reject_delegation_confirmation) are NOT marked #[deprecated] despite having canonical operation replacements; all prod callers omit #[allow(deprecated)] because no deprecation warning is emitted | SRC-OP-07, SCAN-PROD-02, CONFLICT-01 | DELETE-01, GUARD-02 | Phase 5 | compatibility deletion (F-DELETE-01) | high | none | none |
| F-GUARD-01 | hardening | active | product_runtime_boundary_guards.rs tests 7 runtime architecture constraints but does NOT reject replaced broad workflow calls or local #[allow(deprecated)] in production adapter files; the Stage 9 adapter-call and deprecation-suppression guards described by the old plan are absent | GUARD-PROD-01, GUARD-PROD-02, GUARD-PROD-03, GUARD-PROD-04, GUARD-PROD-05, GUARD-PROD-06, GUARD-PROD-07, SCAN-PROD-03 | GUARD-01, GUARD-02 | Phase 5 | none | high | none | none |
| F-EVID-01 | required | active | ExportCurrentHtml variant has no focused behavior test through any path; source and dispatch guard prove implementation exists but no test verifies the HTML export outcome | SRC-OP-01, SRC-OP-02, SRC-OP-03, GUARD-DISPATCH-01, TEST-API-01 | TEST-02 | Phase 4 | none | high | none | none |
| F-HIST-01 | informational | obsolete | Historical plan (2026-07-10) narrative for Tasks 1-3 is obsolete: run() no longer calls deprecated wrappers (GIT-STAGE9-02), fork/switch are canonical with event continuity and partial commit (GIT-STAGE9-03); historical plan checked boxes for Tasks 1-3 are confirmed completed baseline | GIT-STAGE9-01, GIT-STAGE9-02, GIT-STAGE9-03, SRC-OP-05, GUARD-DISPATCH-01, TEST-OWNER-01, TEST-OWNER-02, TEST-OWNER-03, TEST-OWNER-04, CONFLICT-02, CONFLICT-03 | CLOSE-04 | Phase 5 | none | high | none | none |
| F-STAGE10-01 | informational | deferred_stage_10 | Compatibility event subscription deletion (deprecated subscribe method) is Stage 10 scope, not Stage 9; typed ProductEvent payload convergence remains the next runtime simplification milestone | SRC-OP-08, REQUIREMENTS.md v2 | CLOSE-04 | Phase 5 | Stage 10 milestone | high | none | none |

**D-20 update location note:** Phase 5 should update `docs/TODO.md` and
`docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md` to reflect
that Tasks 1-3 are completed baseline and Tasks 4-10 remain active gaps, after final verification
passes. No historical document is modified in Phase 1.

---

## Requirement Traceability

Maps audit requirements AUDIT-01 through AUDIT-03 to their evidence and completion status.
In this draft, all requirements are pending. Plan 01-03 performs final traceability closure.

| Requirement | Description | Status | Evidence | Notes |
|-------------|-------------|--------|----------|-------|
| AUDIT-01 | Maintainers can determine the trustworthy Stage 9 completion state from current source, tests, boundary guards, and Git history | pending | | Populated by Plan 01-02, closed by Plan 01-03 |
| AUDIT-02 | The audit identifies each live-session product operation's public variant, internal mapping, dispatch mode, outcome projection, production callers, and test callers | pending | | Populated by Plan 01-02, closed by Plan 01-03 |
| AUDIT-03 | The audit clearly separates completed baseline behavior, actual gaps, obsolete plan content, and Stage 10 scope | pending | | Populated by Plan 01-02, closed by Plan 01-03 |

---

## Validation Summary

Records validation runs against this audit artifact. The validator (`validate-audit.sh`) supports
three modes: `--schema-only` (structural check), `--evidence-only` (evidence completeness check),
and default final mode (full closure check).

| Mode | Date | Result | Notes |
|------|------|--------|-------|
| schema-only | 2026-07-11 | pass | Scaffold created by Plan 01-01 Task 1 |
| evidence-only | - | not_run | Requires Plan 01-02 evidence collection |
| final | - | not_run | Requires Plan 01-03 final closure |
