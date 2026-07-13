# Requirements: Typed Product Events and Client Lifecycle Contract

**Defined:** 2026-07-13
**Milestone:** v1.1
**Core Value:** Every first-party live-session product operation follows one typed, admitted, behavior-preserving runtime path through `CodingAgentSession::run`.

## v1 Requirements

### Product Event Contract

- [x] **EVENT-01**: Public consumers can inspect product-event kind through a stable typed enum rather than string-only family/kind fields.
- [x] **EVENT-02**: Public product events expose stable sequence, operation identity, terminal status, durability, and documented payload semantics where the underlying event provides them.
- [x] **EVENT-03**: The event inventory covers all emitted product-event families and explicitly documents which operations have terminal events and how they associate with outcomes.

### Compatibility Boundary

- [x] **COMPAT-01**: RPC, interactive, JSON/print, and first-party tests consume typed product events without production calls to `compatibility_event()`.
- [x] **COMPAT-02**: Compatibility event receivers, legacy subscriptions, and compatibility storage are deleted or narrowed to test-only migration fixtures after behavior-preserving coverage passes.
- [ ] **COMPAT-03**: Event ordering, sequence identity, replay, durability, control multiplexing, `PartialCommit`, and external adapter responses remain behavior-compatible during migration.

### Client Lifecycle

- [ ] **CLIENT-01**: A public client connection can obtain a snapshot cursor and resume retained product events from that cursor.
- [ ] **CLIENT-02**: A stale cursor produces a typed fresh-snapshot-required/event-gap result, and reconnect behavior distinguishes resumable history from required snapshot recovery.
- [ ] **CLIENT-03**: Public client state exposes submitted-operation identity/status and client-local draft semantics without exposing internal services or queues.
- [ ] **CLIENT-04**: Client detach/close and runtime shutdown have explicit idempotent behavior and do not corrupt session ownership or event publication.

### Control And Association

- [ ] **CONTROL-01**: A scoped public control contract supports existing abort, steer, and follow-up semantics without placing control signals into the ordinary operation queue.
- [ ] **CONTROL-02**: Operation id, submitted client operation, terminal product event, and terminal outcome association is explicit and tested for every operation that exposes terminal semantics.

### Boundary Hardening

- [ ] **GUARD-01**: First-party adapter root ownership is independently discoverable or otherwise fail-closed against new unlisted adapter roots.
- [ ] **GUARD-02**: External compile-fail fixtures bind failures to the expected forbidden symbol or span, not only broad diagnostic categories.

## Future Requirements

- **RUNTIME-01**: Introduce a separately named `CodingAgentRuntime` owner type if future client lifecycle evidence requires it; naming alone is not a v1.1 goal.
- **RUNTIME-02**: Multi-session daemon orchestration and cross-session client routing beyond one `CodingAgentSession`.
- **UI-01**: New `pi-web-ui` product surface or broad GUI rendering redesign.

## Out of Scope

| Feature | Reason |
|---------|--------|
| New product workflows, Lua Flow expansion, or broad `pi-web-ui` construction | v1.1 stabilizes existing runtime/event/client contracts first. |
| Arbitrary generic control bus for every operation | Existing control semantics are prompt-scoped; broaden only with evidence. |
| Replacing `CodingAgentSession` solely to introduce `CodingAgentRuntime` | Ownership/name changes are deferred until the public contract requires them. |
| Unrelated session-log crash consistency, provider performance, credential storage, and CI modernization | These remain outside the bounded event/client lifecycle milestone. |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| EVENT-01 | Phase 6 | Complete |
| EVENT-02 | Phase 6 | Complete |
| EVENT-03 | Phase 6 | Complete |
| COMPAT-01 | Phase 7 | Complete |
| COMPAT-02 | Phase 7 | Complete |
| COMPAT-03 | Phase 9 | Pending |
| CLIENT-01 | Phase 8 | Pending |
| CLIENT-02 | Phase 8 | Pending |
| CLIENT-03 | Phase 8 | Pending |
| CLIENT-04 | Phase 9 | Pending |
| CONTROL-01 | Phase 8 | Pending |
| CONTROL-02 | Phase 9 | Pending |
| GUARD-01 | Phase 9 | Pending |
| GUARD-02 | Phase 9 | Pending |

**Coverage:** 14 v1 requirements, all mapped to a phase.

---
*Requirements defined: 2026-07-13*
*Last updated: 2026-07-13 after v1.1 kickoff*
