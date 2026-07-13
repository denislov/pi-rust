# Roadmap: Typed Product Events and Client Lifecycle Contract

## Milestones

- [x] **v1.0 Canonical Operation Runtime Convergence** — Phases 1-5, shipped 2026-07-13. See [milestone archive](milestones/v1.0-ROADMAP.md).
- [ ] **v1.1 Typed Product Events and Client Lifecycle Contract** — Phases 6-9, in progress.

## Phases

### Phase 6: Product Event Inventory and Typed Contract

**Goal:** Freeze the emitted event inventory and implement the stable typed public product-event model, including identity, durability, terminal semantics, and payload boundaries.

**Requirements:** EVENT-01, EVENT-02, EVENT-03

**Success criteria:**

- Every current event emitter maps to a documented typed public event kind and payload contract.
- Public consumers no longer need string parsing to identify event kind.
- Operation identity, durability, terminal status, and unsupported/missing fields have explicit semantics.
- Focused event contract and serialization/projection tests pass offline.

### Phase 7: Adapter Migration and Compatibility Deletion

**Goal:** Migrate all first-party event consumers to typed product events and remove the compatibility receiver/subscription path without changing observable behavior.

**Requirements:** COMPAT-01, COMPAT-02

**Success criteria:**

- RPC, interactive, JSON/print, and test consumers match typed product events directly.
- Production code has no `compatibility_event()` consumer and no local compatibility deprecation suppression.
- Legacy receiver/subscription/storage is deleted or test-gated only where migration evidence requires it.
- Existing event ordering, adapter output, replay, and control assertions remain green.

### Phase 8: Client Connection, Replay, and Scoped Control

**Goal:** Promote snapshot, retained replay, cursor recovery, submitted-operation, draft, and prompt-control foundations into a public reconnectable client contract.

**Requirements:** CLIENT-01, CLIENT-02, CLIENT-03, CONTROL-01

**Success criteria:**

- A client can connect, receive a snapshot cursor, resume retained events, and handle stale cursors with a typed recovery result.
- Reconnect semantics distinguish replayable history from fresh-snapshot-required recovery.
- Submitted operation and client-local draft state are queryable/mutable through stable APIs without exposing internals.
- Abort, steer, and follow-up remain scoped control signals outside the ordinary operation queue.

### Phase 9: Lifecycle Association, Guards, and Closure

**Goal:** Close client lifecycle ownership and operation/event association, harden boundary guards, and verify the complete v1.1 contract.

**Requirements:** COMPAT-03, CLIENT-04, CONTROL-02, GUARD-01, GUARD-02

**Success criteria:**

- Detach/close and shutdown are explicit, idempotent, and preserve session/event ownership invariants.
- Operation id, submitted state, terminal outcome, and terminal event associations are tested for applicable operations.
- Adapter-root and compile-fixture guard debt is closed with fail-closed tests.
- Required formatting, focused tests, full workspace checks, security checks, source audits, and diff checks pass.

## Progress

- [x] Phase 6: Product Event Inventory and Typed Contract (completed 2026-07-13)
- [ ] Phase 7: Adapter Migration and Compatibility Deletion
- [ ] Phase 8: Client Connection, Replay, and Scoped Control
- [ ] Phase 9: Lifecycle Association, Guards, and Closure

---
*Roadmap created: 2026-07-13*
*Phase numbering continues from v1.0 (Phase 5).*
