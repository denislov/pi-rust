# Canonical Operation Runtime Convergence

## What This Is

This project completes the Stage 9 runtime convergence for the existing `pi-rust` coding-agent workspace. It will audit the current implementation, then make `CodingAgentSession::run(CodingAgentOperation)` the single public live-session operation dispatcher used by every first-party adapter and test before deleting the replaced workflow-specific session methods.

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

### Active

- [ ] Establish the exact canonical public operation contract and dispatcher baseline that subsequent migration work can rely on
- [ ] Route all JSON and print product work through `CodingAgentSession::run(CodingAgentOperation)` without changing observable output behavior
- [ ] Route all RPC prompt, agent, team, delegation, profile, self-healing, plugin, and related product work through canonical operations without changing wire behavior or control/event multiplexing
- [ ] Route all interactive background work, mutations, delegation decisions, plugin actions, branch summaries, and session navigation through canonical operations while preserving UI projection and event continuity
- [ ] Migrate owner, public API, and integration tests from workflow-specific session methods to canonical operations without weakening assertions
- [ ] Delete the replaced broad live-session workflow methods only after all production and test callers have migrated; do not retain equivalent compatibility methods under new names
- [ ] Strengthen compiler-visible and source-level boundaries so first-party adapters cannot regress to broad workflow calls or local deprecation suppressions
- [ ] Verify focused `pi-coding-agent` suites and the full workspace with formatting, tests, checks, source audits, and clean diffs
- [ ] Update Stage 9 documentation to reflect the audited implementation and identify Stage 10 typed `ProductEvent` payload convergence as the next runtime simplification stage

### Out of Scope

- Typed `ProductEvent` payload convergence and compatibility event-subscription deletion - reserved for Stage 10
- A new lifecycle-grade public operation control handle - control signals remain separate from ordinary operations in this milestone
- RPC wire-command or interactive rendering redesign - adapters must preserve their existing external behavior
- Exposure of raw plugin load options, plugin registries, session services, provider internals, capability internals, or Flow nodes through the stable API - these remain implementation details
- Unrelated session-log crash consistency, manifest atomicity, plugin resource limits, credential storage, provider-stream performance, placeholder crates, and CI modernization - important concerns, but not required for Stage 9 convergence
- Renaming or recreating deleted workflow-specific session methods - convergence requires one operation facade, not a relocated compatibility facade

## Context

The operational product is the Rust 2024 `pi-coding-agent` crate, not the workspace-root placeholder binary. Its architecture is layered: product adapters submit work to the product operation runtime; `CodingAgentSession` owns admission, capabilities, services, flows, events, and persistence; `pi-agent-core` owns generic agent and flow execution; `pi-ai` owns providers and transport; and `pi-tui` owns generic terminal mechanics.

The repository already contains public and internal operation contracts, a canonical-looking `CodingAgentSession::run` path, service-owned product workflows, deterministic faux-provider tests, Rust-native session persistence, and extensive boundary guards. However, the codebase map and the prior Stage 9 plan both indicate that adapter migration and broad-method deletion are incomplete. The new roadmap must begin with an evidence-based audit because prior plan checkboxes are not treated as authoritative.

The most sensitive areas are the large session owner and interactive modules, event/control multiplexing, durable navigation transitions, and integration suites whose assertions encode behavioral guarantees. Changes should be sliced along existing ownership boundaries and verified before compatibility methods are removed.

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
| Preserve the Stage 9 architectural goal | The typed canonical operation facade remains the desired runtime boundary | Pending |
| Re-plan from current evidence instead of inheriting Tasks 1-10 | The previous plan's checkboxes and phase boundaries may not match the live implementation | Pending |
| Treat the previous plan as reference material | It contains useful contracts, tests, risks, and non-goals without constraining the new roadmap structure | Pending |
| Audit before assigning remaining implementation phases | Existing behavior must be classified from source, tests, guards, and Git history | Pending |
| Keep Stage 10 out of this milestone | Mixing event-payload convergence into operation-dispatch convergence would expand risk and obscure completion | Pending |

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
*Last updated: 2026-07-11 after Phase 1 completion*
