# Roadmap: Canonical Operation Runtime Convergence

## Overview

Stage 9 proceeds in dependency order from evidence to enforcement. The first phase establishes the trustworthy live baseline; the second makes the typed operation facade complete and behavior-preserving; the third moves every first-party production adapter onto that facade; the fourth migrates tests and removes the compatibility methods only after callers are gone; and the final phase hardens the boundary, runs closure audits, and records Stage 10 typed `ProductEvent` convergence as the next milestone rather than mixing it into this one.

## Phases

- [ ] **Phase 1: Evidence-Based Baseline** - Establish the authoritative Stage 9 completion state and exact remaining gap set from live evidence.
- [ ] **Phase 2: Canonical Facade Correctness** - Make the stable public operation contract and its internal dispatch path complete, exhaustive, and behavior-preserving.
- [ ] **Phase 3: Production Adapter Convergence** - Route JSON, print, RPC, and interactive product work through canonical operations without changing adapter behavior.
- [ ] **Phase 4: Test Convergence and Compatibility Deletion** - Move behavior coverage to `run()` and remove the replaced broad session methods after every caller has migrated.
- [ ] **Phase 5: Boundary Enforcement and Stage 9 Closure** - Prevent regression to compatibility paths, verify the workspace, and close Stage 9 with accurate documentation.

## Phase Details

### Phase 1: Evidence-Based Baseline

**Goal**: Maintainers have a trustworthy, source-backed statement of what Stage 9 already delivers and what remains to be implemented.
**Depends on**: Nothing (first phase)
**Requirements**: AUDIT-01, AUDIT-02, AUDIT-03
**Success Criteria** (what must be TRUE):

  1. Maintainers can inspect one current-state audit that reconciles source, tests, boundary guards, and Git history without treating old checklist marks as completion evidence.
  2. Every live-session product operation is listed with its public variant, internal mapping, dispatch mode, public outcome, production callers, and test callers.
  3. The audit classifies each finding as completed baseline, actual Stage 9 gap, obsolete plan content, or deferred Stage 10 work.

**Plans**: 1/3 plans executed

- [x] 01-01-PLAN.md
- [ ] 01-02-PLAN.md
- [ ] 01-03-PLAN.md

### Phase 2: Canonical Facade Correctness

**Goal**: First-party callers can rely on one complete stable operation facade whose dispatch and outcome semantics preserve the existing runtime contract.
**Depends on**: Phase 1
**Requirements**: FACADE-01, FACADE-02, FACADE-03, FACADE-04, FACADE-05
**Success Criteria** (what must be TRUE):

  1. A first-party caller can import every required operation, outcome, and support type from `pi_coding_agent::api` without importing internal runtime contracts.
  2. Every public operation submitted through `CodingAgentSession::run` reaches the metadata-selected async, sync-read-only, or sync-mutable dispatcher.
  3. Every internal operation outcome is converted through one exhaustive public projection, including plugin, profile, delegation, fork, navigation, and export results.
  4. Fork, active-leaf switch, branch-summary reuse, plugin, profile, and delegation operations retain their durable state, event continuity, and explicit error or partial-commit semantics.
  5. Stable API checks demonstrate that internal operations, dispatch metadata, plugin load options, services, and Flow nodes remain inaccessible to callers.

**Plans**: TBD

### Phase 3: Production Adapter Convergence

**Goal**: Every first-party product adapter executes live-session product work through `CodingAgentSession::run` while preserving its existing external contract.
**Depends on**: Phase 2
**Requirements**: ADAPT-01, ADAPT-02, ADAPT-03, ADAPT-04, RPC-01, RPC-02, RPC-03, RPC-04, INTER-01, INTER-02, INTER-03, INTER-04, INTER-05
**Success Criteria** (what must be TRUE):

  1. JSON and both persistent and transient print flows produce the same outputs, errors, and session effects while executing prompts through `CodingAgentOperation::Prompt`.
  2. RPC prompt, agent, team, delegation, self-healing, profile, and plugin commands preserve response shapes, errors, event forwarding, and `tokio::select!` control handling while using canonical operations.
  3. Interactive prompt and background workflows, mutations, delegation decisions, plugin actions, compaction, and branch summaries use canonical operations without changing visible behavior.
  4. Interactive fork and navigation retain subscriber continuity, product-event sequencing, and refreshed snapshots and projections after transitions.
  5. JSON, print, RPC, and interactive production sources contain neither replaced broad workflow calls nor local deprecation suppressions for those calls.

**Plans**: TBD

### Phase 4: Test Convergence and Compatibility Deletion

**Goal**: The test suite proves public workflows through the canonical facade, and the obsolete broad live-session facade no longer exists.
**Depends on**: Phase 3
**Requirements**: TEST-01, TEST-02, TEST-03, TEST-04, DELETE-01, DELETE-02, DELETE-03, DELETE-04
**Success Criteria** (what must be TRUE):

  1. Owner, public API, and integration tests exercise public workflows through `run()` while preserving existing assertions for agents, teams, profiles, delegation, export, branch summaries, and self-healing edits.
  2. Test-only helpers extract typed outcomes without becoming a production compatibility facade, and owner tests use crate-private operation paths only when custom internal options are genuinely required.
  3. All replaced public and crate-private broad live-session workflow methods are absent after production and test callers have migrated.
  4. A missed caller fails the migration checks and must move to canonical operations; no deleted workflow is restored or recreated under another name.
  5. Construction, open/resume, snapshots, queries, event subscriptions, control paths, and static repository helpers remain available because they are not operation-facade replacements.

**Plans**: TBD

### Phase 5: Boundary Enforcement and Stage 9 Closure

**Goal**: The canonical operation boundary is regression-resistant, the complete workspace is verified, and Stage 9 is accurately closed.
**Depends on**: Phase 4
**Requirements**: GUARD-01, GUARD-02, GUARD-03, GUARD-04, CLOSE-01, CLOSE-02, CLOSE-03, CLOSE-04
**Success Criteria** (what must be TRUE):

  1. Recursive adapter guards reject replaced workflow calls and local production deprecation suppressions across JSON, print, RPC, and interactive sources.
  2. Compiler-visible visibility, sealed-contract, and API checks enforce the stable facade wherever possible, with source scanning retained only for boundaries Rust cannot directly express.
  3. Stable API completeness checks require the canonical public types and reject internal operation, dispatch, service, plugin-option, and Flow contracts.
  4. Source audits, formatting, focused `pi-coding-agent` tests and checks, diff checks, and full workspace tests and checks all pass from the final tree.
  5. Stage 9 documentation matches the verified implementation and names typed `ProductEvent` payload convergence and compatibility subscription deletion as Stage 10 work.

**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Evidence-Based Baseline | 1/3 | In Progress|  |
| 2. Canonical Facade Correctness | 0/TBD | Not started | - |
| 3. Production Adapter Convergence | 0/TBD | Not started | - |
| 4. Test Convergence and Compatibility Deletion | 0/TBD | Not started | - |
| 5. Boundary Enforcement and Stage 9 Closure | 0/TBD | Not started | - |
