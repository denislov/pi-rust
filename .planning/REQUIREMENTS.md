# Requirements: Canonical Operation Runtime Convergence

**Defined:** 2026-07-11
**Core Value:** Every first-party live-session product operation follows one typed, admitted, behavior-preserving runtime path through `CodingAgentSession::run`.

## v1 Requirements

### Baseline Audit

- [x] **AUDIT-01**: Maintainers can determine the trustworthy Stage 9 completion state from current source, tests, boundary guards, and Git history rather than prior plan checkboxes
- [x] **AUDIT-02**: The audit identifies each live-session product operation's public variant, internal mapping, dispatch mode, outcome projection, production callers, and test callers
- [x] **AUDIT-03**: The audit clearly separates completed baseline behavior, actual gaps, obsolete plan content, and Stage 10 scope

### Canonical Facade

- [x] **FACADE-01**: First-party callers can obtain the complete stable `CodingAgentOperation`, outcome, and required support types through `pi_coding_agent::api`
- [x] **FACADE-02**: `CodingAgentSession::run` converts every public operation to an internal operation and selects the async, sync-read-only, or sync-mutable dispatcher from operation metadata
- [x] **FACADE-03**: Every internal operation outcome is projected through one exhaustive mapping into a public operation outcome
- [x] **FACADE-04**: Internal operations, dispatch metadata, plugin load options, services, and Flow nodes are not exposed through the stable API
- [x] **FACADE-05**: Fork, active-leaf switch, branch-summary reuse, plugin, profile, and delegation operations preserve their persistence, event-continuity, and error semantics

### JSON And Print

- [x] **ADAPT-01**: The JSON adapter executes prompt work through `CodingAgentSession::run(CodingAgentOperation::Prompt)`
- [x] **ADAPT-02**: The print adapter executes persistent and non-persistent prompt paths through the canonical operation facade
- [x] **ADAPT-03**: JSON and print output, error, and session behavior remain unchanged by the migration
- [x] **ADAPT-04**: JSON and print production code contains no replaced broad workflow calls or local `#[allow(deprecated)]` attributes

### RPC

- [x] **RPC-01**: RPC prompt, agent, team, and delegation-approval background tasks execute through canonical operations
- [x] **RPC-02**: RPC self-healing edit, profile mutation, delegation rejection, plugin load, and plugin command work executes through canonical operations
- [x] **RPC-03**: The RPC migration preserves existing `tokio::select!` control handling, event forwarding, response shapes, and error protocol
- [x] **RPC-04**: RPC production code contains no replaced broad workflow calls or local deprecation suppressions

### Interactive

- [x] **INTER-01**: Interactive prompt, agent, team, compaction, self-healing, plugin, and branch-summary background work executes through canonical operations
- [x] **INTER-02**: Interactive profile mutation and delegation rejection execute through canonical operations
- [x] **INTER-03**: Session fork and navigation use canonical operations and refresh snapshots and projections after transitions
- [x] **INTER-04**: The interactive migration preserves event/control multiplexing, subscriber continuity, product-event sequencing, and UI behavior
- [x] **INTER-05**: Interactive production code contains no replaced broad workflow calls or local deprecation suppressions

### Test Migration

- [x] **TEST-01**: Owner unit tests, public API tests, and integration tests use `run()` to verify public workflows
- [x] **TEST-02**: Existing behavior assertions for agents, teams, profiles, delegation, export, branch summaries, and self-healing edits are preserved
- [ ] **TEST-03**: Test helpers only extract typed outcomes and do not create a new production compatibility facade
- [ ] **TEST-04**: Owner tests that genuinely require custom internal options may use crate-private operation paths without expanding the public API

### Compatibility Deletion

- [x] **DELETE-01**: Every public or crate-private broad live-session workflow method replaced by canonical operations is deleted
- [x] **DELETE-02**: Broad workflow deletion occurs only after all production and test callers have migrated
- [x] **DELETE-03**: Missed callers are migrated instead of restoring deleted methods or recreating compatibility entry points under new names
- [x] **DELETE-04**: Construction, open/resume, snapshot, query, event subscription, control, and static repository helpers that are not operation-facade replacements remain available

### Boundary Enforcement

- [ ] **GUARD-01**: Boundary tests recursively scan first-party adapters and reject replaced broad workflow calls
- [ ] **GUARD-02**: Boundary tests reject local deprecation suppressions in JSON, print, RPC, and interactive production files
- [ ] **GUARD-03**: Stable API completeness tests require canonical facade types while rejecting internal runtime types
- [ ] **GUARD-04**: Rust visibility, sealed contracts, and compile/API tests are preferred when they can express a boundary; source scanning remains only where necessary

### Closure

- [ ] **CLOSE-01**: Source audits find no unexpected old methods, old calls, or production deprecation suppressions
- [ ] **CLOSE-02**: `cargo fmt --check`, `cargo test -p pi-coding-agent`, `cargo check -p pi-coding-agent`, and `git diff --check` pass
- [ ] **CLOSE-03**: `cargo test --workspace` and `cargo check --workspace` pass
- [ ] **CLOSE-04**: Stage 9 documentation matches the final implementation and identifies Stage 10 typed `ProductEvent` payload convergence as the next runtime simplification stage

## v2 Requirements

### Event Contract Convergence

- **EVENT-01**: Compatibility event subscriptions are deleted
- **EVENT-02**: `ProductEvent` payloads converge on the Stage 10 typed contract
- **EVENT-03**: Callers that still depend on compatibility event payloads or subscriptions are migrated

## Out of Scope

| Feature | Reason |
|---------|--------|
| RPC wire protocol redesign | Stage 9 must preserve existing external adapter behavior |
| Interactive UI or rendering redesign | Operation convergence should not alter presentation semantics |
| New public lifecycle control handle | Control signals remain separate from ordinary operations in this milestone |
| Session-log crash consistency and atomic manifest replacement | Important durability work, but independent from canonical operation convergence |
| Lua plugin CPU and memory isolation | Important security work, but outside the Stage 9 runtime boundary |
| Credential storage, provider-stream performance, placeholder crates, and CI modernization | Separate concerns with different ownership and verification criteria |
| Exposure of internal services, Flow nodes, plugin registries, or provider internals | These must remain implementation details behind the stable facade |
| Renamed replacements for deleted broad workflow methods | Convergence requires one public operation facade rather than relocated compatibility methods |

## Traceability

Traceability will be populated during roadmap creation. Each v1 requirement must map to exactly one phase.

| Requirement | Phase | Status |
|-------------|-------|--------|
| AUDIT-01 | Phase 1 | Complete |
| AUDIT-02 | Phase 1 | Complete |
| AUDIT-03 | Phase 1 | Complete |
| FACADE-01 | Phase 2 | Complete |
| FACADE-02 | Phase 2 | Complete |
| FACADE-03 | Phase 2 | Complete |
| FACADE-04 | Phase 2 | Complete |
| FACADE-05 | Phase 2 | Complete |
| ADAPT-01 | Phase 3 | Complete |
| ADAPT-02 | Phase 3 | Complete |
| ADAPT-03 | Phase 3 | Complete |
| ADAPT-04 | Phase 3 | Complete |
| RPC-01 | Phase 3 | Complete |
| RPC-02 | Phase 3 | Complete |
| RPC-03 | Phase 3 | Complete |
| RPC-04 | Phase 3 | Complete |
| INTER-01 | Phase 3 | Complete |
| INTER-02 | Phase 3 | Complete |
| INTER-03 | Phase 3 | Complete |
| INTER-04 | Phase 3 | Complete |
| INTER-05 | Phase 3 | Complete |
| TEST-01 | Phase 4 | Complete |
| TEST-02 | Phase 4 | Complete |
| TEST-03 | Phase 4 | Pending |
| TEST-04 | Phase 4 | Pending |
| DELETE-01 | Phase 4 | Complete |
| DELETE-02 | Phase 4 | Complete |
| DELETE-03 | Phase 4 | Complete |
| DELETE-04 | Phase 4 | Complete |
| GUARD-01 | Phase 5 | Pending |
| GUARD-02 | Phase 5 | Pending |
| GUARD-03 | Phase 5 | Pending |
| GUARD-04 | Phase 5 | Pending |
| CLOSE-01 | Phase 5 | Pending |
| CLOSE-02 | Phase 5 | Pending |
| CLOSE-03 | Phase 5 | Pending |
| CLOSE-04 | Phase 5 | Pending |

**Coverage:**

- v1 requirements: 37 total
- Mapped to phases: 37
- Unmapped: 0

---
*Requirements defined: 2026-07-11*
*Last updated: 2026-07-11 after roadmap mapping*
