# Canonical Operation Runtime Convergence

## What This Is

This project completed the Stage 9 runtime convergence for the existing `pi-rust` coding-agent workspace. `CodingAgentSession::run(CodingAgentOperation)` is the single public live-session operation dispatcher used by every first-party adapter and test, and the replaced workflow-specific session methods are deleted. The authoritative closure evidence is [05-STAGE-9-CLOSURE.md](milestones/v1.0-phases/05-boundary-enforcement-and-stage-9-closure/05-STAGE-9-CLOSURE.md).

The existing implementation plan at `docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md` is design input and historical evidence, not the new execution structure. Current code, tests, source guards, and repository history determine what is actually complete and how the remaining work is phased.

## Core Value

Every first-party live-session product operation follows one typed, admitted, behavior-preserving runtime path through `CodingAgentSession::run`.

## Requirements

### Validated

- [x] The workspace has a typed product-operation architecture centered on public `CodingAgentOperation`, internal operation metadata/admission, and typed outcomes - existing
- [x] `CodingAgentSession` owns runtime services, capabilities, plugins, profiles, events, and session persistence behind a product-level facade - existing
- [x] JSON, print, JSONL RPC, and interactive entry points already operate as adapters around the shared `pi-coding-agent` runtime - existing
- [x] Rust-native sessions use typed durable events, replay-derived state, and explicit partial-commit semantics - existing
- [x] Boundary and integration test suites exist for public API, product adapters, session behavior, interactive behavior, RPC behavior, agents, teams, profiles, and delegation - existing
- [x] Audit the current Stage 9 implementation against live source, tests, source guards, and Git history instead of trusting prior checklist state - Validated in Phase 1: Evidence-Based Baseline
- [x] Establish the exact canonical public operation contract and dispatcher baseline that subsequent migration work can rely on - Validated in Phase 2: Canonical Facade Correctness
- [x] Route all JSON and print product work through `CodingAgentSession::run(CodingAgentOperation)` without changing observable output behavior - Validated in Phase 3: Production Adapter Convergence
- [x] Route all RPC prompt, agent, team, delegation, profile, self-healing, plugin, and related product work through canonical operations without changing wire behavior or control/event multiplexing - Validated in Phase 3: Production Adapter Convergence
- [x] Route all interactive background work, mutations, delegation decisions, plugin actions, branch summaries, and session navigation through canonical operations while preserving UI projection and event continuity - Validated in Phase 3: Production Adapter Convergence

### Validated In Phases 4-5

- [x] Migrate owner, public API, and integration tests from workflow-specific session methods to canonical operations without weakening assertions - Validated in Phase 4
- [x] Delete the replaced broad live-session workflow methods only after all production and test callers have migrated; do not retain equivalent compatibility methods under new names - Validated in Phase 4
- [x] Strengthen compiler-visible and source-level boundaries so first-party adapters cannot regress to broad workflow calls or local deprecation suppressions - Validated in Phase 5
- [x] Verify focused `pi-coding-agent` suites and the full workspace with formatting, tests, checks, source audits, and clean diffs - Evidence in the Stage 9 closure report
- [x] Update Stage 9 documentation and identify Stage 10 typed `ProductEvent` payload convergence and compatibility-subscription deletion as the next bounded runtime stage - Validated in Phase 5

### Validated In Phases 6-7

- [x] Publish a stable typed `CodingAgentProductEvent` contract with exhaustive payload, identity, terminal, and durability semantics - Validated in Phase 6
- [x] Migrate RPC, JSON/print, interactive, and first-party tests to typed product events while preserving observable behavior - Validated in Phase 7
- [x] Delete compatibility event storage, receivers, subscriptions, duplicate broadcasts, stable raw-event exports, and public raw adapter bypasses - Validated in Phase 7

### Out of Scope

- A new lifecycle-grade public operation control handle - control signals remain separate from ordinary operations in this milestone
- RPC wire-command or interactive rendering redesign - adapters must preserve their existing external behavior
- Exposure of raw plugin load options, plugin registries, session services, provider internals, capability internals, or Flow nodes through the stable API - these remain implementation details
- Unrelated session-log crash consistency, manifest atomicity, plugin resource limits, credential storage, provider-stream performance, placeholder crates, and CI modernization - important concerns, but not required for Stage 9 convergence
- Renaming or recreating deleted workflow-specific session methods - convergence requires one operation facade, not a relocated compatibility facade

## Context

The operational product is the Rust 2024 `pi-coding-agent` crate, not the workspace-root placeholder binary. Its architecture is layered: product adapters submit work to the product operation runtime; `CodingAgentSession` owns admission, capabilities, services, flows, events, and persistence; `pi-agent-core` owns generic agent and flow execution; `pi-ai` owns providers and transport; and `pi-tui` owns generic terminal mechanics.

The repository now has closed public operation and product-event boundaries: first-party adapters use typed contracts exported by `pi_coding_agent::api`, `CodingAgentSession::run` performs canonical admission and dispatch, and compiler-driven guards prevent broad workflow or raw compatibility-event facades from returning.

The most sensitive areas are the large session owner and interactive modules, event/control multiplexing, durable navigation transitions, and integration suites whose assertions encode behavioral guarantees. Changes should be sliced along existing ownership boundaries and verified before compatibility methods are removed.

## Current State

v1.0 shipped on 2026-07-13. Phases 6-7 of v1.1 are complete: the exhaustive typed product-event contract is public, all first-party adapters and tests consume typed events, and the raw compatibility storage/receiver/subscription path is deleted behind fail-closed guards. Phase 8 client connection, replay, and scoped-control planning is next.

## Current Milestone: v1.1 Typed Product Events and Client Lifecycle Contract

**Goal:** Turn the existing internal typed event, retained replay, snapshot, and control foundations into a stable public product-event and long-lived client contract without regressing operation, durability, or adapter behavior.

**Target features:**
- Publish a typed `CodingAgentProductEvent` contract with operation identity, terminal status, durability, and payload semantics derived from the existing internal event inventory.
- Migrate RPC, interactive, JSON/print, and tests away from compatibility event unwrapping, then delete obsolete compatibility receivers and subscriptions with behavioral evidence.
- Promote existing `connect`, snapshot cursor, retained replay, event-gap, submitted-operation, draft, and prompt-control foundations into reconnectable client lifecycle APIs.
- Close directly related adapter inventory and compile-fixture guard debt while preserving the private ownership of services, queues, and Flow nodes.

## Next Milestone Goals

v1.1 combines the bounded Stage 10 typed event convergence with the existing
client lifecycle foundations: reconnectable retained replay, snapshot recovery,
submitted operation state, client drafts, scoped control, detach/close, and
shutdown. Stage 11 remains a contract goal, not a requirement to rename the
session owner to `CodingAgentRuntime`.

## Active Requirements

- [ ] Public client connections support snapshot, replay, stale-cursor recovery, submitted operation state, drafts, and scoped control.
- [ ] Detach/close, shutdown, and operation/outcome/terminal-event association have explicit behavior.
- [ ] Directly related adapter inventory and compile-fail guard debt is closed.

## Constraints

- **Architecture**: Preserve the dependency direction `pi-coding-agent -> pi-agent-core -> pi-ai` and `pi-coding-agent -> pi-tui`; product semantics must not move into lower-level crates
- **Public API**: New and migrated callers use contracts exported by `pi_coding_agent::api`; internal `Operation`, dispatch metadata, plugin load options, services, and Flow nodes remain private
- **Behavior compatibility**: JSON output, print output, RPC responses, interactive projections, event ordering, control handling, session replay, and persistent navigation behavior must not regress
- **Durability**: Preserve typed session facts, replay authority, append/manifest ordering, operation identifiers, recovery markers, and explicit `PartialCommit` reporting
- **Testing**: Use deterministic offline fixtures and retain existing assertions; migration is not permission to reduce coverage or replace behavior checks with compile-only checks
- **Deletion order**: Production adapters and tests migrate before broad workflow methods are deleted; missed callers must be migrated rather than restoring deleted methods
- **Verification**: Completion requires `cargo fmt --check`, focused `pi-coding-agent` tests, `cargo test --workspace`, `cargo check --workspace`, source audits, and `git diff --check`
- **Scope**: Stage 10 event compatibility work and unrelated reliability/security/performance initiatives remain outside this milestone

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Preserve the Stage 9 architectural goal | The typed canonical operation facade is the verified runtime boundary | Complete |
| Re-plan from current evidence instead of inheriting Tasks 1-10 | The previous plan's checkboxes and phase boundaries did not determine live completion | Complete |
| Treat the previous plan as reference material | It remains intact as superseded historical evidence | Complete |
| Audit before assigning remaining implementation phases | Source, tests, guards, and Git history established the migration baseline | Complete |
| Keep Stage 10 out of this milestone | Event payload/subscription convergence remains bounded and deferred | Complete |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `$gsd-transition`):
1. Requirements invalidated? Move to Out of Scope with reason
2. Requirements validated? Move to Validated with phase reference
3. New requirements emerged? Add to Active
4. Decisions to log? Add to Key Decisions
5. "What This Is" still accurate? Update if drifted

**After each milestone** (via `$gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check - still the right priority?
3. Audit Out of Scope - reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-07-13 after Phase 7 verification*
