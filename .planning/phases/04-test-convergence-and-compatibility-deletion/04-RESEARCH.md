# Phase 4: Test Convergence and Compatibility Deletion - Research

**Researched:** 2026-07-13
**Domain:** Rust test migration to a canonical typed dispatcher and compatibility API deletion
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

### Test Migration Boundary
- **D-01:** Use a public-first rule: every test that verifies a product workflow must import stable contracts through `pi_coding_agent::api` where applicable and execute the workflow through `CodingAgentSession::run`.
- **D-02:** Public operation outcomes become the primary assertion entry point, but migration must retain all existing behavior, state, error, event, replay, and persistence assertions. Only demonstrably duplicate implementation-detail assertions already covered through the public contract may be removed.
- **D-03:** Co-located owner tests may use crate-private operation paths only when the public operation cannot express required custom internal options, metadata or dispatcher state, or a deterministic fault boundary. Each exception must state why the public path is insufficient; convenience alone is not sufficient.
- **D-04:** Do not widen the public operation or option surface merely to support an owner white-box test.

### Migration And Deletion Order
- **D-05:** Organize work by risk and caller layer rather than deleting all methods at once. Migrate lower-risk owner/public/integration callers first, then persistence-, navigation-, and delegation-sensitive methods.
- **D-06:** Before deleting any method definition, prove that both production and test callers are zero, then run the method group's focused behavior tests, relevant boundary guards, and `cargo check -p pi-coding-agent`.
- **D-07:** Delete the definition only after the zero-caller and focused gates pass. A missed caller must be migrated; do not restore the method or introduce an equivalent wrapper under a new name.
- **D-08:** After deletion, convert existing closed-method ledgers into explicit absence checks covering old definitions, old calls, and synonymous compatibility wrappers. Phase 5 remains responsible for recursive/parser-complete hardening.

### Public Workflow Method Removal
- **D-09:** Treat `CodingAgentSession::set_default_agent_profile_id`, `approve_delegation_confirmation`, and `reject_delegation_confirmation` as replaced operation entry points. Migrate all first-party public API and integration callers to `CodingAgentOperation` plus `run`, then delete these public methods.
- **D-10:** This milestone intentionally converges the stable API and accepts the resulting breaking change for external callers of those old methods. Do not retain deprecated shims or a transition facade.
- **D-11:** Preserve public construction, create/open/resume, snapshot/query, event subscription, control, and static repository helpers. Their survival is an explicit contract boundary, not an exception to operation convergence.

### Test Helper Ownership
- **D-12:** Keep private conversion, metadata, dispatcher, custom-internal-option, and fault-boundary fixtures in co-located `coding_session` tests. Put reusable public-behavior fixtures in `crates/pi-coding-agent/tests/support`.
- **D-13:** Shared integration helpers may perform exhaustive typed outcome extraction only. They must not select operations, create or own sessions, hide errors, or hold runtime services.
- **D-14:** Fault injection remains directly `cfg(test)`, crate-private, and action-specific. Do not expose generic failure selectors, internal services, queues, registries, or production hooks.
- **D-15:** No helper may recreate the deleted broad workflow facade, even if its immediate callers are tests.

### the agent's Discretion
- Choose the exact low-to-high risk method groups and focused Cargo commands, provided every group satisfies D-05 through D-08.
- Decide when a pure typed outcome extractor removes meaningful repetition; local exhaustive matches remain acceptable when sharing would obscure the contract.
- Choose test names and file-local fixture organization while preserving existing behavioral assertions and deterministic offline execution.

### Deferred Ideas (OUT OF SCOPE)
- Recursive and parser-complete adapter/source boundary hardening and final workspace closure audits - Phase 5.
- Typed `ProductEvent` payload convergence and compatibility event-subscription deletion - Stage 10.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TEST-01 | Owner, public API, and integration workflow tests use `run()` | Exact remaining test files and migration groups are inventoried below. [VERIFIED: current CodeGraph and `rg` source audit] |
| TEST-02 | Preserve agents, teams, profiles, delegation, export, branch-summary, and self-healing assertions | Each behavior family is mapped to its current suite and focused gate. [VERIFIED: current test source and Phase 3 verification] |
| TEST-03 | Helpers only extract typed outcomes | A strict helper contract and guard update are prescribed below. [VERIFIED: D-12 through D-15] |
| TEST-04 | Genuine custom internal options remain owner-private | `load_plugins(PluginLoadOptions)` and owner fault/metadata paths are identified as explicit exceptions; broad wrappers are not. [VERIFIED: current owner source] |
| DELETE-01 | Delete every replaced public and crate-private broad method | The 16 definite deletion targets are listed below; `load_plugins(PluginLoadOptions)` is explicitly retained as a narrow owner-private custom-option path, not a compatibility method. [VERIFIED: `coding_session/mod.rs` method ledger and current callers] |
| DELETE-02 | Delete only after production and test callers migrate | Phase 3 proves production zero; every deletion group has a zero-caller gate before definition removal. [VERIFIED: Phase 3 verification; D-06] |
| DELETE-03 | Migrate missed callers; add no renamed wrapper | Post-deletion absence and alternate-facade checks are specified. [VERIFIED: D-07, D-08, existing boundary guard] |
| DELETE-04 | Retain non-operation lifecycle/query/control helpers | The retained method ledger is explicitly preserved and compile-tested. [VERIFIED: current guard and D-11] |
</phase_requirements>

## Summary

Phase 3 has already removed all production adapter calls to broad workflow methods and passed the full workspace gate; Phase 4 therefore changes test call sites, owner test internals, the compatibility definitions in `CodingAgentSession`, and their boundary ledgers, without changing runtime behavior. [VERIFIED: `03-VERIFICATION.md:34-40,83-87`] Current source contains 16 definite deletion targets: 12 public methods and four crate-private methods. `load_plugins(PluginLoadOptions)` is not in that deletion set: four current owner-test callers require custom plugin candidates/registries that the public `PluginLoad` operation intentionally cannot express, while its one production caller is only the `reload_plugins` compatibility wrapper and disappears with that wrapper. [VERIFIED: current `mod.rs` callers at lines 1020, 2970, 3026, 3595, 3747 and public operation shape] Remaining real calls are concentrated in co-located owner tests and six integration/public API files; matches in `interactive/root.rs`, `session_service.rs`, `prompt_flow.rs`, and guard string literals are receiver-distinct retained internals or enforcement data, not compatibility callers. [VERIFIED: CodeGraph and receiver-aware source audit]

The planner should treat each method family as a migration proof: migrate tests through public operations, retain every state/event/replay/error assertion, prove zero compatibility callers, run focused behavior and guard targets plus `cargo check -p pi-coding-agent`, then delete only that family's definitions. [VERIFIED: D-02, D-05 through D-08] Do not wait to delete all definitions in one final task: grouped deletion makes compiler failures and behavioral regressions attributable. [VERIFIED: locked migration order]

**Primary recommendation:** Use four sequential risk groups, update the method ledger from “present compatibility wrappers” to “explicitly absent names” as each group closes, and finish with the complete crate gate while leaving recursive/parser-complete hardening to Phase 5.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Public workflow execution | Product runtime (`CodingAgentSession::run`) | private operation/Flow services | Tests must enter through the same stable typed facade as production. [VERIFIED: `coding_session/mod.rs:248-260`] |
| Public behavioral evidence | Integration tests | `tests/support` outcome extractors | Integration tests see only `pi_coding_agent::api`; helpers may extract but not execute. [VERIFIED: D-01, D-13] |
| Dispatcher, conversion, custom option, fault evidence | Co-located owner tests | private services | These contracts are intentionally invisible to external tests and must not widen the API. [VERIFIED: D-03, D-04, D-14] |
| Durable session truth | Session log/services | public outcomes and replay assertions | Migration must preserve typed facts, operation IDs, `PartialCommit`, and replay authority. [VERIFIED: project contract and Phase 3 verification] |
| Compatibility absence enforcement | Boundary integration tests | Rust compiler | Compiler visibility and exhaustive types come first; textual guards ban old names/wrappers. [VERIFIED: existing `product_runtime_boundary_guards.rs`] |

## Project Constraints (from AGENTS.md)

- Use CodeGraph before grep/find/read for indexed source; this research did so. [VERIFIED: repository `AGENTS.md` and successful CodeGraph exploration]
- Preserve `pi-coding-agent -> pi-agent-core -> pi-ai` and `pi-coding-agent -> pi-tui`; product semantics remain in `pi-coding-agent`. [VERIFIED: project instructions]
- Migrated callers use `pi_coding_agent::api`; private operations, metadata, services, plugin options, and Flow nodes remain private. [VERIFIED: project instructions]
- Preserve adapter output, event ordering, control, replay, persistent navigation, typed facts, append/manifest ordering, operation IDs, recovery markers, and `PartialCommit`. [VERIFIED: project instructions]
- Keep deterministic offline fixtures and all substantive assertions; do not replace behavior tests with compile-only checks. [VERIFIED: project instructions]
- Migrate callers before deletion; a missed caller is migrated, never answered with a restored or renamed wrapper. [VERIFIED: project instructions]
- Use default `rustfmt`, narrow lint allowances, typed errors, `snake_case`, behavior-oriented test names, and curated `api` exports. [VERIFIED: repository conventions]
- Required milestone verification includes formatting, focused crate tests, workspace test/check, source audits, and `git diff --check`. [VERIFIED: project instructions]

## Standard Stack

### Core

| Component | Version | Purpose | Why Standard |
|-----------|---------|---------|--------------|
| Rust | 1.96.0 installed; edition 2024 | Compile-time API deletion and exhaustive enum matching | Existing workspace language; deleting methods makes missed typed callers compile-fail. [VERIFIED: installed toolchain and manifests] |
| Cargo/libtest | 1.96.0 | Unit, integration, filtered, package, and workspace gates | Existing test runner supports `-p`, `--lib`, `--test`, and name filters. [CITED: doc.rust-lang.org/cargo/commands/cargo-test.html] |
| Tokio | 1.52.3 locked | Async owner/integration tests | Existing `#[tokio::test]` provides a per-test current-thread runtime by default. [CITED: docs.rs/tokio/1.52.0/tokio/attr.test.html] |
| `pi_coding_agent::api` | workspace 0.1.0 | Stable operations, outcomes, options, events, lifecycle/query/control types | It is the required external test boundary. [VERIFIED: current `src/lib.rs` and Phase 2 verification] |

### Supporting

| Component | Version | Purpose | When to Use |
|-----------|---------|---------|-------------|
| Faux providers and tempfile fixtures | existing workspace fixtures | Deterministic offline provider and persistence behavior | All workflow behavior tests. [VERIFIED: current tests and Phase 3 verification] |
| `product_runtime_boundary_guards` | existing integration target | Closed owner method ledger, old-call absence, private API enforcement | Every deletion group and final gate. [VERIFIED: current guard source] |
| `api_boundary_guards` | existing integration target | Stable facade call restrictions | Final public/API deletion proof. [VERIFIED: current guard source] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Direct/local exhaustive outcome match | Shared typed extractor in `tests/support` | Use only for repeated pure projection; it must accept an outcome and return a typed payload, never own a session or operation. [VERIFIED: D-13] |
| Grouped caller migration and deletion | Delete all methods at once | Rejected because failures lose method-family attribution and violate D-05/D-06 gates. [VERIFIED: locked decisions] |
| Existing source sanitizer/ledger | New Rust parser dependency | Out of scope; Phase 5 owns parser-complete hardening and this phase adds no dependency. [VERIFIED: deferred boundary] |

**Installation:** None. Do not change Cargo manifests or install packages. [VERIFIED: scope and existing stack]

## Package Legitimacy Audit

Not applicable: this phase installs no external package. [VERIFIED: phase scope]

## Exact Deletion And Caller Inventory

### Definitions To Delete

| Risk group | Definitions | Visibility | Current callers to migrate |
|------------|-------------|------------|----------------------------|
| G1: agent/team/export | `invoke_agent`, `invoke_team`, `export_current`, `export_current_html` | public, deprecated | `agent_invocation.rs`, `agent_team_flow.rs`, `delegation_execution.rs`; owner export HTML tests. [VERIFIED: source audit] |
| G2: prompt/compact/self-heal/public profile | `prompt`, `compact`, `self_healing_edit`, `self_healing_edit_with_options`, `set_default_agent_profile_id` | public; profile method currently not deprecated | `agent_profile_runtime.rs`, `agent_profile_session.rs`, `delegation_execution.rs`, `public_api.rs`, owner tests. [VERIFIED: source audit] |
| G2: plugin compatibility | `reload_plugins`, `run_plugin_command` | crate-private | owner tests only for `run_plugin_command`; `reload_plugins` has no remaining callers after Phase 3. [VERIFIED: source audit] |
| G3: delegation | `approve_delegation_confirmation`, `reject_delegation_confirmation` | public, currently not deprecated | `delegation_execution.rs`, `public_api.rs`, owner tests. [VERIFIED: source audit] |
| G4: navigation/summary | `fork_current_session`, `summarize_branch`, `summarize_branch_for_navigation` | public summary plus crate-private navigation methods | `summarize_branch_for_navigation` has zero current callers; Phase 3 replaced its only production role with canonical `BranchSummary { reuse: ReuseExisting }` plus `ForkSession`. [VERIFIED: source audit and Phase 3 verification] |

`load_plugins(PluginLoadOptions)` is **RESOLVED: retain as a narrow owner-private custom-option path**. Current callers are: the `reload_plugins` wrapper at `mod.rs:1020`, plus owner tests at `mod.rs:2970`, `3026`, `3595`, and `3747`. The four tests construct explicit plugin candidates and registries to cover diagnostics, capability events, durable plugin-load facts, plugin capability continuity across fork, and a private plugin-command error boundary. [VERIFIED: current source] Public `CodingAgentOperation::PluginLoad` carries no candidate/registry payload, so these options are genuinely unrepresentable; D-04 forbids widening the public operation solely for white-box coverage. [VERIFIED: `public_operation.rs`, `plugin_load_flow.rs`, D-03/D-04] Plan implication: 04-02 deletes `reload_plugins` and documents the four D-03 exceptions, while the final ledger positively retains `load_plugins` as owner-private and rejects any public/helper wrapper or non-test caller. [VERIFIED: 04-02 plan and TEST-04]

`summarize_branch_for_navigation` is **RESOLVED: delete in 04-04**. The current source audit finds its definition at `mod.rs:1192` and zero callers outside the definition itself. [VERIFIED: CodeGraph and receiver-aware `rg` audit] Phase 3's production navigation path directly uses `CodingAgentOperation::BranchSummary` with `reuse: ReuseExisting`, then `ForkSession`, preserving the same reuse semantics without this method. [VERIFIED: Phase 3 verification and 04-04 plan] Plan implication: include it in the 04-04 absent-definition ledger and final receiver-aware absence guard; retain only canonical operations and non-operation lifecycle/query/control/static helpers. It must not be recreated under another name. [VERIFIED: DELETE-01, DELETE-03, DELETE-04]

### Files Requiring Real Test Migration

| File | Current broad workflows | Required canonical outcomes / preserved evidence |
|------|-------------------------|--------------------------------------------------|
| `tests/agent_invocation.rs` | `invoke_agent`, `export_current` | `AgentInvocation`, `ExportCurrent`; preserve profile validation, output, events, replay/export. [VERIFIED: current source] |
| `tests/agent_team_flow.rs` | `invoke_team`, `export_current` | `AgentTeam`, `ExportCurrent`; preserve team ordering/state/export assertions. [VERIFIED: current source] |
| `tests/agent_profile_runtime.rs` | `prompt` | `Prompt`; preserve profile/provider/tool/delegation runtime behavior. [VERIFIED: current source] |
| `tests/agent_profile_session.rs` | profile mutation | `SetDefaultAgentProfile`; preserve persisted/reopened profile state. [VERIFIED: current source] |
| `tests/delegation_execution.rs` | prompt, export, approve/reject | `Prompt`, `ExportCurrent`, `ApproveDelegation`, `RejectDelegation`; preserve pending queue, execution, durable event, replay, error assertions. [VERIFIED: current source] |
| `tests/public_api.rs` | prompt, summary, self-heal, three public profile/delegation calls | Public operations and exact public outcomes; change old-method compile assertions into canonical positive and old-name absence evidence. [VERIFIED: current source] |
| `coding_session/mod.rs` tests | all families plus plugin/navigation/custom options | Prefer `run`; retain private mapping/metadata/fault tests only with written D-03 justification. [VERIFIED: current source] |

Do not classify `InteractiveRoot::set_default_agent_profile_id`, `SessionService::set_default_agent_profile_id`, `SessionService::switch_active_leaf`, `Agent::prompt`, static `fork_session`, or guard string literals as compatibility callers. They have distinct receivers/architectural responsibilities. [VERIFIED: CodeGraph source]

### Retained Contract Ledger

Preserve `run`, `create`, `open`, `open_or_create`, `non_persistent`, `list`, `export_session_html`, `subscribe`, `subscribe_product_events_public`, `snapshot`, `connect`, `capabilities`, `view`, profile/team queries, diagnostics, pending-delegation query, crate-private hydration/tree/clone/static fork helpers, product-event replay/snapshot/client queries, `prompt_control_handle`, and plugin UI/query helpers. [VERIFIED: current closed method ledger and D-11]

## Architecture Patterns

### System Architecture Diagram

```text
test intent / fixture
        |
        v
construct public CodingAgentOperation from pi_coding_agent::api
        |
        v
CodingAgentSession::run -> admission -> metadata-selected dispatcher
        |
        v
private Flow/services -> ProductEvents + durable session transaction
        |
        v
exact CodingAgentOperationOutcome extraction
        |
        +--> existing state/event/error/replay/persistence assertions
        |
        +--> zero-caller guard -> focused gate -> delete old definition
```

### Recommended Project Structure

```text
crates/pi-coding-agent/
├── src/coding_session/mod.rs                 # canonical run, owner-private tests, deletion targets
└── tests/
    ├── support/mod.rs                        # fixtures and optional pure outcome extraction only
    ├── public_api.rs                         # external facade compile/behavior contract
    ├── agent_invocation.rs                   # canonical agent operation
    ├── agent_team_flow.rs                    # canonical team operation
    ├── agent_profile_{runtime,session}.rs     # prompt/profile operations
    ├── delegation_execution.rs               # durable prompt/decision operations
    ├── product_runtime_boundary_guards.rs    # exact owner ledger and absence checks
    └── api_boundary_guards.rs                # public API call restrictions
```

### Pattern 1: Exact Public Outcome Extraction

```rust
let outcome = session
    .run(CodingAgentOperation::InvokeAgent(options))
    .await
    .expect("invoke agent through canonical facade");
let invocation = match outcome {
    CodingAgentOperationOutcome::AgentInvocation(outcome) => outcome,
    _ => unreachable!("invoke-agent operation returned another outcome"),
};
```

Source: current canonical facade and Phase 3 adapter pattern. [VERIFIED: `coding_session/mod.rs:248-260`; Phase 3 source]

### Pattern 2: Per-Group Deletion Proof

1. Migrate every listed test caller and retain assertions. [VERIFIED: D-02]
2. Run receiver-aware `rg`/guard proof that production and tests have zero old calls. [VERIFIED: D-06]
3. Run focused suites and `product_runtime_boundary_guards`, then `cargo check -p pi-coding-agent`. [VERIFIED: D-06]
4. Delete only that group's methods and update the closed ledger to require absence. [VERIFIED: D-07, D-08]
5. Re-run the same gate; migrate any compiler/source failure rather than restoring a wrapper. [VERIFIED: D-07]

### Anti-Patterns to Avoid

- **Operation-running test helper:** It recreates the deleted facade and hides which operation a test verifies. [VERIFIED: D-13, D-15]
- **Assertion thinning:** Replacing durable/event/replay checks with only an outcome match violates TEST-02. [VERIFIED: D-02]
- **Public API widening for tests:** Keep custom plugin options, metadata, and fault controls owner-private. [VERIFIED: D-03, D-04, D-14]
- **Global bare-name ban:** It would reject legitimate UI/service methods with the same name. Use owner method definitions and receiver-aware calls. [VERIFIED: current false-positive inventory]
- **Deleting static repository helpers:** `export_session_html`, static fork/clone/hydration, and lifecycle/query/control methods are not live-owner workflow replacements. [VERIFIED: D-11]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Workflow dispatch | Test-specific workflow functions | `CodingAgentSession::run` | Admission, capabilities, dispatcher mode, persistence, and errors already converge there. [VERIFIED: Phase 2] |
| Outcome abstraction | Dynamic/downcast or generic erased result | Exhaustive `CodingAgentOperationOutcome` match | Compile-visible and preserves the exact contract. [VERIFIED: public enum] |
| Failure injection | Public/generic fault selector | Existing action-specific `cfg(test)` owner bridges | Prevents production privilege/fault leakage. [VERIFIED: Phase 3 security] |
| Source parser | New ad-hoc parser or dependency | Existing sanitized ledger and narrow absence checks | Parser-complete work belongs to Phase 5. [VERIFIED: deferred scope] |
| Durable oracle | File substring-only replacement | Existing typed outcomes plus structured JSON/replay assertions | Operation identity and partial commits must remain attributable. [VERIFIED: Phase 3 tests] |

## Runtime State Inventory

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | `session.json` and `events.jsonl` contain semantic facts and operation IDs, not Rust compatibility method names. [VERIFIED: Phase 3 durability evidence] | No data migration. Preserve existing replay, append/manifest, and `PartialCommit` assertions. |
| Live service config | None; the product is a local CLI/TUI and no external control plane stores these Rust methods. [VERIFIED: project architecture] | None. |
| OS-registered state | None identified; no system service registration participates in this API refactor. [VERIFIED: repository architecture] | None. |
| Secrets/env vars | Existing provider variables, `PI_RUST_DIR`, and `PI_SESSION_DIR` are unchanged. [VERIFIED: project configuration] | No secret migration; keep deterministic faux providers. |
| Build artifacts / installed packages | `target/` may contain old binaries/test executables until Cargo rebuilds. [VERIFIED: Cargo workspace behavior] | Rebuild via focused tests/checks; no installed-package migration. |

## Common Pitfalls

### Pitfall 1: Compatibility Calls Hidden In Owner Tests
**What goes wrong:** Integration tests migrate, but hundreds of co-located owner assertions still call old methods, blocking deletion. [VERIFIED: current `mod.rs` inventory]
**How to avoid:** Inventory owner calls by method family and migrate them in the same deletion group; justify every private-operation exception inline.
**Warning signs:** `cargo check` passes but `cargo test --lib --no-run` fails after deletion.

### Pitfall 2: Sync Export/Profile Outcome Confusion
**What goes wrong:** Tests assume `run` is sync because old export/profile methods were sync. [VERIFIED: old and canonical signatures]
**How to avoid:** Convert affected tests to async `#[tokio::test]` and match `ExportCurrent`, `ExportCurrentHtml`, or `SetDefaultAgentProfile` exactly.
**Warning signs:** blocking executor use, duplicate static export path, or lost path/view distinction.

### Pitfall 3: Delegation Durability Regression
**What goes wrong:** Approval/rejection returns success but pending queue, transaction ID, replay, or `PartialCommit` assertions disappear. [VERIFIED: Phase 2/3 durable tests]
**How to avoid:** Migrate only the invocation; retain all durable-state and structured-error assertions in `delegation_execution.rs` and owner tests.
**Warning signs:** assertions reduced to `is_ok()`, string-only error checks, or reopened state no longer checked.

### Pitfall 4: Ledger Still Requires Deleted Methods
**What goes wrong:** `canonical_operation_facade_has_no_new_workflow_wrappers` currently expects compatibility methods exactly once, so correct deletion fails the guard. [VERIFIED: guard source lines 121-160]
**How to avoid:** Split present retained ledger from an explicit absent-old-method ledger; keep alternate-facade detection active.
**Warning signs:** weakening/removing the closed ledger merely to make deletion pass.

### Pitfall 5: Deleting Genuine Internal Option Paths Prematurely
**What goes wrong:** `load_plugins(PluginLoadOptions)` custom-option tests are forced through a public operation that only uses defaults, reducing coverage or widening API. [VERIFIED: current conversion and owner calls]
**How to avoid:** Keep a narrowly documented private operation/dispatcher test where options cannot be represented publicly; delete convenience-only calls.

## Recommended Plan Boundaries

| Plan | Scope | Gate before advancing |
|------|-------|-----------------------|
| 04-01 Boundary scaffold + G1 | Convert ledgers to retained/absent structure; migrate agent/team/export suites and owner export tests; delete four G1 methods. [VERIFIED: risk inventory] | Focused `agent_invocation`, `agent_team_flow`, relevant delegation export tests, boundary guards, crate check. |
| 04-02 G2 public behavior | Migrate prompt/profile/self-heal/public API and plugin owner tests; delete G2 compatibility methods; retain `load_plugins` only for the four documented custom-option owner-test calls. [VERIFIED: resolved caller inventory] | `public_api`, profile suites, self-heal owner/lib filters, guards, crate check. |
| 04-03 G3 delegation/durability | Migrate approval/rejection callers and owner durable fault/replay tests; delete both public methods without shims. [VERIFIED: D-09/D-10] | Full `delegation_execution`, exact durable owner tests, guards, crate check. |
| 04-04 G4 owner/navigation + closure | Migrate summary/fork/navigation/compaction remainder; delete remaining broad methods including zero-caller `summarize_branch_for_navigation`; prove retained ledger and retained private `load_plugins` seam. [VERIFIED: resolved caller inventory] | `cargo fmt --check`, `cargo test -p pi-coding-agent --lib`, all affected integration targets, full crate test/check, source audits, `git diff --check`. |

Plans 04-02 and 04-03 must remain sequential because delegation tests also contain prompt calls; migrate their prompt setup in 04-02 but leave delegation decisions/assertions for 04-03. [VERIFIED: current `delegation_execution.rs` mixed caller inventory]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust compiler | compile/API deletion | yes | 1.96.0 | none |
| Cargo | focused/full tests | yes | 1.96.0 | none |
| rustfmt | format gate | yes | 1.9.0-stable | none |
| CodeGraph | caller/receiver analysis | yes | indexed MCP | `rg` for exact textual closure after graph exploration |

No external service or network is required. [VERIFIED: deterministic fixture architecture]

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust libtest + Tokio 1.52.3 async tests |
| Config file | Cargo manifests; no separate test config |
| Quick run command | `cargo test -p pi-coding-agent --test <suite> <filter> -- --exact` |
| Full suite command | `cargo test -p pi-coding-agent` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TEST-01, TEST-02 | owner workflows preserve all behavior | unit | `cargo test -p pi-coding-agent --lib -- --nocapture` | yes |
| TEST-01, TEST-02 | agent/team/export | integration | `cargo test -p pi-coding-agent --test agent_invocation --test agent_team_flow -- --nocapture` | yes |
| TEST-01, TEST-02 | profiles | integration | `cargo test -p pi-coding-agent --test agent_profile_runtime --test agent_profile_session -- --nocapture` | yes |
| TEST-01, TEST-02 | delegation/durability | integration | `cargo test -p pi-coding-agent --test delegation_execution -- --nocapture` | yes |
| TEST-01, TEST-02 | public facade/self-heal/summary | integration | `cargo test -p pi-coding-agent --test public_api -- --nocapture` | yes |
| TEST-03, DELETE-01..04 | no helper facade; old methods absent; retained APIs present | source/API guard | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --test api_boundary_guards -- --nocapture` | yes; assertions require update |
| TEST-04 | justified owner-private custom/fault paths | unit + guard | exact owner tests plus `session_store_failure_controls_remain_test_only` | yes |

### Sampling Rate

- **Per task commit:** Exact affected integration target or owner test filter plus receiver-aware zero-caller audit.
- **Per deletion group:** Full affected targets, both boundary targets, and `cargo check -p pi-coding-agent`.
- **Phase gate:** `cargo fmt --check`; `cargo test -p pi-coding-agent`; `cargo check -p pi-coding-agent`; old definition/call/wrapper audits; `git diff --check`. [VERIFIED: project contract]

### Wave 0 Gaps

- [ ] Update `canonical_operation_facade_has_no_new_workflow_wrappers` so deleted names are asserted absent rather than expected once. [VERIFIED: current guard]
- [ ] Update `public_api.rs` compile contracts to require canonical operations and stop compiling the three removed public methods. [VERIFIED: current callers]
- [ ] Add/retain pure typed outcome extractors only where repeated matches are meaningful; no framework installation is needed. [VERIFIED: D-13]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no behavior change | Preserve faux-provider/auth fixtures and existing credential resolution; do not add diagnostics. [VERIFIED: scope] |
| V3 Session Management | yes | Preserve typed transactions, replay authority, event sequence continuity, and reopened-state assertions. [VERIFIED: Phase 2/3] |
| V4 Access Control | yes | Public tests enter through admitted operations; private services/options/fault controls remain inaccessible. [VERIFIED: boundary guards] |
| V5 Input Validation | yes | Preserve typed operation construction, `ProfileId`, pending-delegation lookup, and exact outcome matching. [VERIFIED: current contracts] |
| V6 Cryptography | no | No cryptography or credential-storage change. [VERIFIED: scope] |

### Known Threat Patterns For This Refactor

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Test bypasses operation admission through private/broad path | Elevation of Privilege / Tampering | Public-first `run`; explicit owner-private exception only. [VERIFIED: D-01/D-03] |
| Durable assertions removed during migration | Repudiation / Tampering | Retain operation-ID, replay, pending state, event, and `PartialCommit` checks. [VERIFIED: TEST-02] |
| Generic fault helper leaks to production/public API | Elevation of Privilege | Direct `cfg(test)`, crate-private, action-specific guards. [VERIFIED: Phase 3 security] |
| Old facade recreated under synonym | Tampering | Closed owner method ledger plus alternate-facade violations. [VERIFIED: existing guard] |
| Over-broad absence scan deletes legitimate service/UI methods | Denial of Service | Receiver-aware definition/call checks and retained ledger. [VERIFIED: source audit] |

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| Tests call workflow-specific `CodingAgentSession` methods | Tests submit typed operations and match public outcomes | Production and tests prove one admitted runtime path. [VERIFIED: milestone goal] |
| Guard expects compatibility methods to exist exactly once | Guard requires old definitions/calls/synonyms to be absent | Deleted facade cannot silently return. [VERIFIED: D-08] |
| Public profile/delegation mutation methods coexist with operations | Only operation variants remain | Intentional breaking API convergence with no shim. [VERIFIED: D-09/D-10] |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| — | None. Recommendations are grounded in current source, executable tests/guards, planning decisions, installed tools, or cited official docs. | — | — |

## Open Questions (RESOLVED)

1. **RESOLVED: retain `load_plugins(PluginLoadOptions)` only as a narrow owner-private custom-option path.**
   - Current callers: the compatibility call from `reload_plugins` at `mod.rs:1020`, plus four owner-test calls at `mod.rs:2970`, `3026`, `3595`, and `3747`.
   - Decision: delete `reload_plugins` in 04-02; retain `load_plugins` for the four tests because each supplies explicit plugin candidates/registries not representable by public `CodingAgentOperation::PluginLoad`. [VERIFIED: current source, public operation shape, D-03/D-04]
   - Constraints: no public export, integration helper, generic fault selector, or replacement wrapper; the final guard must require the four owner-only exceptions and reject any non-test caller. [VERIFIED: D-12 through D-15 and 04-02 plan]

2. **RESOLVED: delete `summarize_branch_for_navigation` in 04-04.**
   - Current callers: none; CodeGraph and receiver-aware `rg` find only the definition. [VERIFIED: current source audit]
   - Decision: it is obsolete compatibility workflow code. Phase 3 replaced its behavior with canonical `BranchSummary { reuse: ReuseExisting }` followed by `ForkSession`, so no test migration or private retention is needed. [VERIFIED: Phase 3 verification and 04-04 plan]
   - Plan implication: include its definition in the final absent-method ledger and retain only canonical public operations and non-operation lifecycle/query/control/static helpers. [VERIFIED: DELETE-01, DELETE-03, DELETE-04]

## Sources

### Primary (HIGH confidence)

- Current CodeGraph source/call paths for `CodingAgentSession`, broad methods, tests, and guards.
- Current `rg` definition/caller audit under `crates/pi-coding-agent/src` and `tests`.
- `.planning/phases/03-production-adapter-convergence/03-VERIFICATION.md` and `03-SECURITY.md`.
- Current `product_runtime_boundary_guards.rs`, `api_boundary_guards.rs`, and affected test targets.
- Repository `AGENTS.md`, project constraints, requirements, roadmap, state, and Phase 4 context.

### Secondary (MEDIUM confidence)

- [Cargo test official documentation](https://doc.rust-lang.org/cargo/commands/cargo-test.html) - package/target/filter and deterministic execution flags.
- [Tokio 1.52 test macro documentation](https://docs.rs/tokio/1.52.0/tokio/attr.test.html) - async test runtime behavior.

### Tertiary (LOW confidence)

- None used for implementation decisions.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - current manifests, lockfile, installed toolchain, and official docs agree.
- Architecture: HIGH - current definitions, receiver-aware callers, and Phase 3 production closure were inspected directly.
- Caller/deletion inventory: HIGH - CodeGraph and exact source audits were reconciled.
- Pitfalls: HIGH - each maps to an existing caller, guard, durability assertion, or locked decision.

**Research date:** 2026-07-13
**Valid until:** 2026-08-12, or until `coding_session/mod.rs` or affected tests materially change
