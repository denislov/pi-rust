# Phase 1: Evidence-Based Baseline - Research

**Researched:** 2026-07-11
**Domain:** Evidence-driven Rust runtime migration audit
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

### Audit Artifact Structure
- **D-01:** The formal Phase 1 deliverable is `.planning/phases/01-evidence-based-baseline/01-AUDIT.md`.
- **D-02:** The audit uses an exhaustive matrix with one row per public `CodingAgentOperation` variant.
- **D-03:** Broad live-session compatibility methods are tracked in a separate compatibility inventory rather than mixed into operation rows.
- **D-04:** A findings report accompanies the matrix to explain cross-operation gaps, contradictions, risks, and obsolete plan material.
- **D-05:** Every gap records exact evidence, affected requirement IDs, its target phase from Phase 2 through Phase 5, and relevant dependencies. Phase 1 does not write implementation tasks.

### Completion Evidence Threshold
- **D-06:** Every completed behavior claim requires current source evidence plus a focused behavior or public API test.
- **D-07:** Boundary claims additionally require Rust visibility/type evidence, a compile/API test, or a precise source guard when Rust cannot directly express the boundary.
- **D-08:** Full workspace verification is a Phase 5 closure gate, not a prerequisite for marking every individual operation implementation present in the Phase 1 inventory.
- **D-09:** The current tree is authoritative for what exists. Git history corroborates timing, rationale, scope, and old checklist credibility, but a missing clean commit does not invalidate current source evidence.
- **D-10:** Record implementation and verification separately. Verification uses `passed`, `failed`, `blocked`, or `not_run`; final completion requires both axes to satisfy the applicable evidence threshold.
- **D-11:** Source-scanning guards are sufficient only when the intended boundary cannot be expressed with Rust visibility, types, or compile/API tests. Replaceable textual guards become Phase 5 hardening findings rather than false implementation gaps.

### Status And Confidence Taxonomy
- **D-12:** `implementation` uses `complete`, `partial`, `missing`, or `not_applicable`.
- **D-13:** `disposition` uses `active`, `obsolete`, `deferred_stage_10`, or `retained_compatibility`.
- **D-14:** Every item records `confidence: high | medium | low` using consistent evidence rules. High requires aligned source, focused tests, and applicable boundary evidence; medium means source is clear but verification or history is incomplete; low means the conclusion is indirect and needs downstream verification.
- **D-15:** `evidence_gaps` reduce confidence but still permit an audit conclusion. `blockers` prevent a reliable conclusion or downstream planning; only blockers prevent Phase 1 verification from passing.
- **D-16:** Finding obligation uses `blocking`, `required`, `hardening`, or `informational`. Required findings map to Stage 9 phases; hardening findings normally map to Phase 5.

### History And Documentation Handling
- **D-17:** Apply layered authority. Current source, tests, and guards define what exists; current `PROJECT.md`, `REQUIREMENTS.md`, and `ROADMAP.md` define milestone requirements; Stage 9 design and reference architecture define target constraints; the old implementation plan and `docs/TODO.md` are historical execution clues.
- **D-18:** When authorities conflict, record an explicit finding rather than silently choosing one source.
- **D-19:** Git analysis focuses on Stage 9 commits, commits named by the old plan, changes around 2026-07-10, and current broad-method or adapter history. Use deeper `git show`, `git log -S/-G`, or blame only when evidence conflicts.
- **D-20:** Phase 1 does not modify the old implementation plan or `docs/TODO.md`. The audit records obsolete checkboxes, contradictions, and recommended update locations; Phase 5 updates closure documentation after final verification.
- **D-21:** The mandatory canonical reference set is fixed below. Stage 8 plans, additional documents, and specific commits are loaded conditionally when the core references or evidence conflicts require them.

### the agent's Discretion
- Choose the exact Markdown column ordering, concise evidence notation, and supporting command appendix layout as long as the required fields and taxonomies above remain explicit and machine-scannable.
- Split large caller or compatibility inventories into supporting tables within `01-AUDIT.md` when that improves readability without creating a second source of truth.

### Deferred Ideas (OUT OF SCOPE)

None - discussion stayed within the Phase 1 audit scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| AUDIT-01 | Maintainers can determine the trustworthy Stage 9 completion state from current source, tests, boundary guards, and Git history rather than prior plan checkboxes | Defines layered evidence order, evidence ledger, focused commands, and conflict-recording rules. |
| AUDIT-02 | The audit identifies each live-session product operation's public variant, internal mapping, dispatch mode, outcome projection, production callers, and test callers | Identifies the 15 public variants, their one-to-one internal mappings, three dispatch modes, exhaustive projection, caller roots, and compatibility inventory method. |
| AUDIT-03 | The audit clearly separates completed baseline behavior, actual gaps, obsolete plan content, and Stage 10 scope | Defines disposition/obligation fields, findings structure, downstream phase mapping, and the Stage 10 event-compatibility fence. |
</phase_requirements>

## Summary

The planner should treat Phase 1 as a reproducible evidence-production phase, not as a prose review and not as an implementation phase. The current tree already contains the complete 15-variant public `CodingAgentOperation` facade, the internal `Operation` mapping, metadata-selected async/read-only/mutable dispatch, exhaustive public outcome projection, and focused canonical navigation behavior. [VERIFIED: `crates/pi-coding-agent/src/coding_session/public_operation.rs`, `operation.rs`, `mod.rs`; CodeGraph exploration; focused Cargo tests]

The major live gap is caller convergence: JSON, print, RPC, interactive code, and multiple integration tests still invoke broad compatibility methods and carry local deprecation allowances. Existing boundary tests prove important lower-level architecture constraints, but `product_runtime_boundary_guards.rs` does not yet reject broad operation calls or production deprecation suppressions. [VERIFIED: repository source scan after CodeGraph; `cargo test -p pi-coding-agent --test product_runtime_boundary_guards`]

The executable plan should build `01-AUDIT.md` in evidence layers: freeze schema and evidence notation, generate the exhaustive operation/caller/compatibility inventories from the live tree, run focused tests and guards, reconcile only contradictory historical claims, then validate traceability and taxonomy mechanically. No production code, old-plan checkbox, or `docs/TODO.md` edit belongs in Phase 1. [VERIFIED: `01-CONTEXT.md`, `REQUIREMENTS.md`, `ROADMAP.md`]

**Primary recommendation:** Plan three audit slices: schema and extraction, evidence verification and history reconciliation, then traceability/consistency validation of the single `01-AUDIT.md` source of truth.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Public operation contract inventory | Product API facade (`pi-coding-agent`) | Product runtime owner | Public variants and outcomes are exported contracts; conversion remains crate-private. [VERIFIED: source] |
| Dispatch and admission inventory | Product runtime owner (`coding_session`) | `IntentRouter` / capability services | `CodingAgentSession::run` selects the dispatcher from internal operation metadata. [VERIFIED: source] |
| Production caller inventory | Product adapters | Product runtime owner | JSON, print, RPC, and interactive entry points should invoke the facade but currently retain compatibility calls. [VERIFIED: source scan] |
| Behavior and boundary evidence | Rust test harness | Source guards / Git history | Focused tests establish behavior; visibility/API tests and precise source guards establish boundaries; history only corroborates. [VERIFIED: context and tests] |
| Audit artifact | Planning documentation | Phase 2-5 planners | Phase 1 records facts and gap routing without changing runtime behavior. [VERIFIED: roadmap/context] |

## Project Constraints (from AGENTS.md)

- Use CodeGraph before `rg`, file search, or direct source reads when locating or understanding code; `.codegraph/` exists and CodeGraph 1.2.0 is available. [VERIFIED: `AGENTS.md`, environment probe]
- Preserve dependency direction `pi-coding-agent -> pi-agent-core -> pi-ai` and `pi-coding-agent -> pi-tui`; keep product semantics in `pi-coding-agent`. [VERIFIED: `AGENTS.md`]
- Treat `pi_coding_agent::api` as the stable downstream facade; internal operations, metadata, services, plugin options, and Flow nodes remain private. [VERIFIED: `AGENTS.md`]
- Preserve adapter behavior, durable session facts, replay authority, event ordering, control handling, and explicit `PartialCommit` reporting. [VERIFIED: `AGENTS.md`]
- Use deterministic offline tests and retain behavior assertions; compile-only evidence is insufficient for behavior claims. [VERIFIED: `AGENTS.md`]
- Do not delete compatibility methods before production and test caller migration. [VERIFIED: `AGENTS.md`]
- Leave Stage 10 event compatibility and unrelated reliability/security/performance work outside this milestone. [VERIFIED: `AGENTS.md`]

## Current Runtime Findings

### Canonical Facade Baseline

`CodingAgentOperation` currently has 15 variants: `Prompt`, `Compact`, `BranchSummary`, `SelfHealingEdit`, `InvokeAgent`, `InvokeTeam`, `PluginLoad`, `PluginCommand`, `SetDefaultAgentProfile`, `ApproveDelegation`, `RejectDelegation`, `ForkSession`, `SwitchActiveLeaf`, `ExportCurrent`, and `ExportCurrentHtml`, with `BranchSummary` counted once despite its structured fields. [VERIFIED: `public_operation.rs`]

Every public variant maps exhaustively into one internal `Operation`, and every internal `OperationOutcome` maps exhaustively into `CodingAgentOperationOutcome`. `CodingAgentSession::run` obtains `operation.metadata().dispatch_mode` and routes to `run_operation`, `run_sync_operation`, or `run_sync_mut_operation` before calling `CodingAgentOperationOutcome::from_internal`. [VERIFIED: source and `api_boundary_guards::coding_session_run_is_the_canonical_operation_dispatcher`]

The audit matrix should therefore use one row per public variant and include these required fields:

| Field | Required content |
|-------|------------------|
| `public_variant` | Exact `CodingAgentOperation` spelling and relevant support policy/type. |
| `internal_mapping` | Exact `Operation` variant and conversion evidence. |
| `metadata` | `OperationKind` or dynamic-kind note, origin, class, and dispatch mode. |
| `public_outcome` | Exact projected `CodingAgentOperationOutcome`. |
| `production_callers` | File/symbol list, with `run` versus compatibility-path status. |
| `test_callers` | Owner, public API, integration, adapter, and guard tests. |
| `implementation` / `verification` | Independent locked taxonomies. |
| `disposition` / `confidence` | Locked taxonomy plus concise rationale. |
| `evidence` | Stable evidence IDs linking source, test, guard, and optional Git entries. |
| `evidence_gaps` / `blockers` | Separate columns; use `none` explicitly. |

### Dispatch Classification

| Public variants | Internal dispatch mode | Notes |
|-----------------|------------------------|-------|
| Prompt, Compact, BranchSummary, SelfHealingEdit, InvokeAgent, InvokeTeam, PluginLoad, ApproveDelegation | Async | Approval has a dynamic operation kind resolved from the pending team target. [VERIFIED: `operation.rs`, `intent_router.rs`] |
| PluginCommand, ExportCurrent, ExportCurrentHtml | SyncReadOnly | Export requires session-read capability; plugin command remains non-session-root. [VERIFIED: `operation.rs`, `mod.rs`] |
| RejectDelegation, ForkSession, SwitchActiveLeaf, SetDefaultAgentProfile | SyncMutable | Navigation and profile mutations change owner/runtime state; rejection mutates pending control state. [VERIFIED: `operation.rs`, `mod.rs`] |

### Broad Compatibility Inventory

Track compatibility separately. Current overlapping methods include public deprecated `export_current_html`, `export_current`, `set_default_agent_profile_id`, `approve_delegation_confirmation`, `reject_delegation_confirmation`, `prompt`, `compact`, `self_healing_edit_with_options`, `invoke_agent`, `invoke_team`, and `summarize_branch`; public `self_healing_edit` delegates through the deprecated options method; crate-private overlapping methods include `fork_current_session`, `reload_plugins`, `run_plugin_command`, and `load_plugins`. [VERIFIED: `coding_session/mod.rs`]

The compatibility table should include method visibility, deprecation status, matching public operation, production callers, test callers, retention reason if any, deletion requirement, and target phase. Do not include construction/open/resume, snapshots, queries, subscriptions/control, or static repository helpers unless a finding explains why they are not operation-facade replacements. [VERIFIED: DELETE-04 and context]

### Caller Baseline

- Public canonical `run()` usage is currently concentrated in `tests/public_api.rs` and owner tests in `coding_session/mod.rs`; no production adapter file matched `CodingAgentOperation::`. [VERIFIED: source scan after CodeGraph]
- JSON and print prompt paths retain deprecated workflow calls and local `#[allow(deprecated)]`. [VERIFIED: `protocol/json_mode.rs`, `print_mode.rs`]
- RPC prompt, agent/team, delegation approval, profile, rejection, plugin load/command, and self-healing paths retain broad methods or deprecation allowances. [VERIFIED: `protocol/rpc/prompt.rs`, `protocol/rpc/commands.rs`]
- Interactive background prompt/team/agent/compaction/plugin/navigation and loop mutations retain broad methods or deprecation allowances. [VERIFIED: `interactive/prompt_task.rs`, `interactive/loop.rs`]
- Integration suites such as `agent_invocation.rs`, `agent_team_flow.rs`, `agent_profile_session.rs`, and `delegation_execution.rs` still use broad methods. [VERIFIED: source scan]

These are Phase 3 or Phase 4 findings, not Phase 1 implementation tasks.

## Standard Stack

### Core

| Tool | Version | Purpose | Why Standard |
|------|---------|---------|--------------|
| CodeGraph CLI | 1.2.0 | Symbol, caller, and dynamic-dispatch discovery before text scanning | Mandated by repository instructions and indexed locally. [VERIFIED: environment] |
| Rust/Cargo | Cargo 1.96.0 | Focused public API, behavior, and boundary tests | Existing workspace harness; no new framework required. [VERIFIED: environment/manifests] |
| Git | 2.47.3 | Stage 9 chronology and contradiction reconciliation | Current tree remains authoritative; Git supplies corroboration. [VERIFIED: environment/context] |
| ripgrep | 15.1.0 | Precise post-CodeGraph source/caller/allowance scans | Existing repository tool for deterministic textual evidence. [VERIFIED: environment] |
| jq | 1.7 | Optional machine checks over generated JSON command output | Available, but the audit remains Markdown and must not depend on a new data source. [VERIFIED: environment] |

No external packages or libraries should be introduced. Package legitimacy auditing is not applicable. [VERIFIED: phase scope]

## Architecture Patterns

### Evidence Flow

```text
locked scope + requirements
        |
        v
CodeGraph symbol/call-path inventory
        |
        v
targeted source extraction ----> compatibility/caller scans
        |                              |
        +--------------+---------------+
                       v
              focused Cargo evidence
                       |
                       v
          conflict-only Git reconciliation
                       |
                       v
       01-AUDIT.md matrix + findings + command ledger
                       |
                       v
        mechanical schema/traceability validation
```

### Pattern 1: Evidence IDs Instead of Repeated Prose

Define a command/evidence ledger such as `SRC-OP-01`, `TEST-API-01`, `GUARD-DISPATCH-01`, and `GIT-STAGE9-01`; matrix rows reference these IDs. This keeps one source of truth while preserving exact file/symbol/command/status provenance. [VERIFIED: compatible with D-02/D-05 and agent discretion]

### Pattern 2: Separate Presence From Proof

An operation may be implemented but have `verification: not_run` or insufficient focused behavior coverage. Do not downgrade clear source presence to `partial` merely because a test is missing; record the evidence gap and lower confidence. [VERIFIED: D-10/D-14/D-15]

### Pattern 3: Finding-Level Downstream Routing

Each active gap should name affected downstream requirement IDs, one target phase, and dependencies. Typical routing is facade correctness/durability to Phase 2, production adapters to Phase 3, test migration/deletion to Phase 4, and boundary hardening/closure docs to Phase 5. [VERIFIED: roadmap and D-05]

### Anti-Patterns to Avoid

- Treating checked boxes in the historical plan as evidence.
- Counting enum presence as behavior completion without a focused test.
- Mixing compatibility methods into operation rows and losing deletion visibility.
- Running Git archaeology before establishing the live source/test state.
- Using broad `cargo test --workspace` as a substitute for operation-specific evidence.
- Writing Phase 2-5 implementation steps inside the audit.
- Classifying intentionally deferred compatibility event work as a Stage 9 gap.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Caller graph | Ad hoc manual list from memory | CodeGraph first, then `rg` confirmation | Dynamic/cross-module paths and omissions are the primary audit risk. |
| Rust boundary proof | Prose claims about privacy | Existing visibility, `public_api.rs`, `api_boundary_guards.rs` | Compiler/API evidence is stronger than textual inference. |
| Test framework | Custom audit test runner | Cargo test harness and focused filters | Existing deterministic offline infrastructure is sufficient. |
| History reconstruction | Full repository archaeology | Stage 9 commit window and `git show` only for conflicts | History is corroborating, not authoritative. |
| Audit status vocabulary | New synonyms | Locked implementation/disposition/verification/confidence taxonomies | Machine-scannable consistency is a requirement. |

## Runtime State Inventory

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | Rust-native session logs contain operation/event facts and may exist under `${PI_RUST_DIR:-~/.pi-rust}/sessions` or configured roots, but Phase 1 changes no operation identifiers, schemas, or stored records. [VERIFIED: project configuration and phase boundary] | None for Phase 1; audit durable behavior evidence only. Any later schema change would require a separate migration finding. |
| Live service config | None. The application is a local CLI/TUI and Phase 1 consumes repository evidence; no external control-plane configuration is changed. [VERIFIED: AGENTS stack/platform description and phase scope] | None. |
| OS-registered state | None identified. The phase neither renames the binary nor changes systemd/launchd/task registrations. [VERIFIED: phase scope and repository scan context] | None. |
| Secrets/env vars | Existing `PI_RUST_DIR`, `PI_SESSION_DIR`, and provider credentials affect test/runtime location, but no variable name or secret contract changes in this documentation audit. [VERIFIED: AGENTS configuration description] | Use deterministic fixtures; do not inspect or modify user secrets. |
| Build artifacts | `target/` may contain compiled test artifacts, but no artifact rename/install migration is involved. Cargo recompiles as needed. [VERIFIED: Cargo test execution] | None; do not treat cached artifacts as source evidence. |

## Common Pitfalls

### Pitfall 1: Variant Count Ambiguity
**What goes wrong:** Structured variants or export modes are counted inconsistently.
**How to avoid:** Count exact public enum variants (15) and give each one row; record shared internal variants such as both export modes explicitly. [VERIFIED: source]

### Pitfall 2: False Completion From Canonical Core Tests
**What goes wrong:** Passing facade/dispatcher tests are generalized to adapter convergence.
**How to avoid:** Record production and test callers independently; absence of production `CodingAgentOperation::` usage is an active Phase 3 gap. [VERIFIED: source/tests]

### Pitfall 3: Guard Scope Confusion
**What goes wrong:** Existing `product_runtime_boundary_guards` is cited as if it rejects broad workflow calls.
**How to avoid:** State what its seven current tests actually guard. The Stage 9 adapter-call and local-deprecation guard described by the old plan is not present yet. [VERIFIED: test source and passing suite]

### Pitfall 4: Dynamic Delegation Kind
**What goes wrong:** `ApproveDelegation` is assigned a static `OperationKind` even though metadata uses `None` and admission resolves the pending target dynamically.
**How to avoid:** Mark it dynamic and cite `intent_router` tests. [VERIFIED: source]

### Pitfall 5: Navigation Evidence Is Under-Specified
**What goes wrong:** Fork/switch are marked complete from enum/dispatch presence alone.
**How to avoid:** Cite owner tests for owner runtime/event continuity, replay/snapshot refresh, and `PartialCommit` after durable leaf mutation. [VERIFIED: owner test names]

### Pitfall 6: Dirty-Tree Attribution
**What goes wrong:** Audit-generated files or user edits are mistaken for runtime implementation history.
**How to avoid:** Capture `git status --short` in the command ledger and never revert unrelated changes; tie claims to current content, not cleanliness. [VERIFIED: workflow constraints]

## Code Examples

### Canonical Dispatcher Evidence Pattern

```rust
// Source: crates/pi-coding-agent/src/coding_session/mod.rs
let operation = operation.into_internal(self.default_plugin_load_options.clone());
let dispatch_mode = operation.metadata().dispatch_mode;
let outcome = match dispatch_mode {
    OperationDispatchMode::Async => self.run_operation(operation).await?,
    OperationDispatchMode::SyncReadOnly => self.run_sync_operation(operation)?,
    OperationDispatchMode::SyncMutable => self.run_sync_mut_operation(operation)?,
};
Ok(CodingAgentOperationOutcome::from_internal(outcome))
```

### Recommended Audit Finding Shape

```markdown
| F-ADAPT-01 | required | active | production callers bypass canonical run | SRC-RPC-01, SRC-INTER-01 | ADAPT-*, RPC-*, INTER-* | Phase 3 | facade correctness complete | high |
```

## State of the Art

| Old/Historical Claim | Current Evidence | Impact |
|----------------------|------------------|--------|
| `run()` called deprecated wrappers and carried a deprecation allowance | Commit `ebe48df` and current source show metadata-selected internal dispatch; focused boundary test exists | Old plan context is obsolete for the dispatcher core. [VERIFIED: Git/source/test] |
| Fork was admitted but rejected; switch had no owner operation | Commit `5c5382c` and owner tests show canonical fork/switch, event continuity, and partial-commit handling | Core navigation implementation is current baseline, not a Phase 2 assumption. [VERIFIED: Git/source/tests] |
| Production adapters still used broad workflows | Current source still confirms this | Remains an active Phase 3 gap. [VERIFIED: source scan] |
| Tests broadly used compatibility methods | Current integration tests still confirm this, while `public_api.rs` has some canonical coverage | Remains an active Phase 4 gap with partial baseline evidence. [VERIFIED: source scan] |
| Compatibility event subscription deletion | Explicitly Stage 10 | Record `deferred_stage_10`; do not route into Stage 9 phases. [VERIFIED: requirements/roadmap/TODO history] |

Stage 9 commits directly relevant to the current partial baseline are `0fff6bd` (expanded public operations), `ebe48df` (canonical dispatcher), and `5c5382c` (session mutation operations). Their file stats align with the current source, but commit messages and old checklist updates are corroboration only. [VERIFIED: `git show --stat`]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `codegraph` | Symbol/caller discovery | yes | 1.2.0 | None needed; repository mandates it. |
| `cargo` | Focused behavior/API/guard tests | yes | 1.96.0 | None. |
| `git` | Conflict-only history | yes | 2.47.3 | Current source remains sufficient unless a contradiction requires history. |
| `rg` | Targeted confirmation scans | yes | 15.1.0 | CodeGraph output plus manual source inspection. |
| `jq` | Optional machine checks | yes | 1.7 | Plain shell/Rust validator. |

No external services or new packages are required. [VERIFIED: environment and scope]

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness via Cargo 1.96.0 |
| Config file | Workspace `Cargo.toml` and `crates/pi-coding-agent/Cargo.toml` |
| Quick run command | `cargo test -p pi-coding-agent --test public_api canonical_operation_runtime_variants_are_public -- --nocapture && cargo test -p pi-coding-agent --test api_boundary_guards coding_session_run_is_the_canonical_operation_dispatcher -- --nocapture` |
| Focused boundary command | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards -- --nocapture` |
| Phase audit validation | A Wave 0 script/test must validate `01-AUDIT.md` schema, variant coverage, taxonomy values, evidence IDs, and downstream phase mappings. |

The public variant test passed (1 test) and the current product boundary suite passed (7 tests) during research. The first attempted dispatcher filter used a nonexistent name and ran zero tests; the correct test is `coding_session_run_is_the_canonical_operation_dispatcher`, so the plan must use exact test names and check the executed test count. [VERIFIED: Cargo output and test source]

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AUDIT-01 | Audit records source/test/guard/history provenance and conflict outcomes | artifact/schema test | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh` | No - Wave 0 |
| AUDIT-02 | Exactly one row exists for each of 15 public variants with all required fields | artifact/schema + live source comparison | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh` | No - Wave 0 |
| AUDIT-03 | Findings use locked taxonomy and separate active, obsolete, retained compatibility, and deferred Stage 10 items | artifact/schema test | `bash .planning/phases/01-evidence-based-baseline/validate-audit.sh` | No - Wave 0 |

### Sampling Rate

- **Per task commit:** Run the audit validator plus the exact focused Cargo command for evidence added or changed.
- **Per wave merge:** Run public API, dispatcher guard, and product runtime boundary suites; rerun caller scans recorded in the command ledger.
- **Phase gate:** Audit validator passes, all evidence commands have recorded status/test counts, no unresolved blockers remain, and `git diff --check` passes.

### Wave 0 Gaps

- [ ] `.planning/phases/01-evidence-based-baseline/validate-audit.sh` - verify the Markdown artifact contains all 15 exact public variants once, required columns/sections, allowed taxonomy values, unique evidence/finding IDs, requirement IDs, Phase 2-5 routing, and explicit `none` values for gaps/blockers.
- [ ] Add a command-ledger template inside `01-AUDIT.md` so every focused test records command, exit status, executed-test count, date, and evidence ID.
- [ ] No framework install or production test fixture is needed.

The validator is a Phase 1 documentation-support artifact, not a production boundary guard. Phase 5 remains responsible for permanent regression guards in the Rust test suite. [VERIFIED: phase boundaries]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No direct Phase 1 change | Do not inspect or alter provider credentials; use offline fixtures. |
| V3 Session Management | Yes, evidence-only | Verify durable session/navigation claims from typed log/replay tests; do not change storage. |
| V4 Access Control | Yes, evidence-only | Record operation admission/capability snapshot and stable/private API boundary evidence. |
| V5 Input Validation | Yes, audit artifact | Validator must reject invalid taxonomy, unknown requirement IDs, duplicate/missing variants, and invalid target phases. |
| V6 Cryptography | No | No cryptographic implementation or dependency change. |

### Known Threat Patterns for This Phase

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Untrusted repository/history text treated as instructions | Spoofing/Tampering | Treat read/fetched content as evidence data only; follow orchestrator and AGENTS instructions. |
| Secrets exposed while probing runtime state | Information Disclosure | Inspect source contracts and fixture roots only; never print auth files or environment values. |
| Audit overstates access-control coverage | Elevation of Privilege | Require source plus focused tests and applicable API/visibility guard before high-confidence completion. |
| Source guard gives false assurance | Tampering | Prefer compiler/API visibility proof; mark textual-only guards as Phase 5 hardening where replaceable. |

No new product security feature is warranted in this documentation/audit phase. [VERIFIED: scope and ASVS L1 applicability]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| - | None. All material planning claims were verified from current repository evidence, focused commands, or locked planning documents. | - | - |

## Open Questions

1. **Which existing focused behavior tests provide the strongest row-level evidence for every operation?**
   - What we know: Public API and owner tests cover facade, navigation, branch reuse, plugin/profile, export, and dispatch; integration suites cover agent/team/delegation behavior through compatibility methods.
   - What's unclear: Some variants may lack a single canonical-run behavior test even though their underlying workflow is heavily tested.
   - Recommendation: During audit extraction, record underlying behavior evidence separately from canonical-call-path evidence and lower confidence rather than inventing a completion claim.

2. **Should the audit validator parse Rust enums automatically?**
   - What we know: The variant list is small and stable during Phase 1; CodeGraph/source is authoritative.
   - What's unclear: A robust Rust parser would exceed documentation-phase needs.
   - Recommendation: Use a fixed exact variant list derived from the evidenced source plus a simple source checksum/count assertion; do not introduce a parser dependency.

## Sources

### Primary (HIGH confidence)

- `AGENTS.md` - project constraints, CodeGraph mandate, architecture, stack, and verification rules.
- `.planning/phases/01-evidence-based-baseline/01-CONTEXT.md` - locked audit format, evidence thresholds, taxonomy, and history rules.
- `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `.planning/STATE.md`, `.planning/config.json` - requirement scope, phase routing, workflow gates, Nyquist/security configuration.
- `crates/pi-coding-agent/src/coding_session/public_operation.rs`, `operation.rs`, `mod.rs`, `intent_router.rs` - current facade, mapping, metadata, dispatcher, compatibility methods, and owner tests.
- `crates/pi-coding-agent/src/protocol/`, `print_mode.rs`, `interactive/` - current production caller state.
- `crates/pi-coding-agent/tests/public_api.rs`, `api_boundary_guards.rs`, `product_runtime_boundary_guards.rs`, and integration tests - current verification and caller state.
- CodeGraph 1.2.0 exploration - symbol/call-path discovery performed before source scans.
- Focused Cargo test output from 2026-07-11 - public facade and product boundary suites.
- Git commits `0fff6bd`, `ebe48df`, `5c5382c` - Stage 9 chronology corroboration.

### Historical/Design (HIGH confidence for intent, not completion)

- `docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md` - expected slices and obsolete/current checklist claims.
- `docs/superpowers/specs/2026-07-10-canonical-operation-runtime-convergence-design.md` - Stage 9 target contract and non-goals.
- `docs/superpowers/specs/2026-07-07-operation-runtime-reference-architecture.md` - broader ownership, dispatch, persistence, and event architecture.
- `docs/TODO.md` - historical stage tracking and Stage 10 boundary.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all tools were probed locally; no packages are introduced.
- Architecture: HIGH - current source and CodeGraph agree with locked architecture documents.
- Caller baseline: HIGH - CodeGraph discovery was followed by targeted recursive source scans.
- Pitfalls: HIGH - derived from observed source/test mismatches and locked evidence rules.
- Runtime state: HIGH for Phase 1 action requirements - this phase changes documentation only; existing session storage is explicitly acknowledged.

**Research date:** 2026-07-11
**Valid until:** Until the production/test tree changes; rerun extraction commands immediately before finalizing `01-AUDIT.md`.
