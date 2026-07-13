---
phase: 08-client-connection-replay-and-scoped-control
plan: 06
subsystem: api
tags: [rust, prompt-control, idempotency, client-generation]
requires:
  - phase: 08-client-connection-replay-and-scoped-control
    provides: generation-scoped client connections and submission provenance
provides:
  - bounded Prompt control transport
  - immutable client/generation/operation-scoped public Prompt controls
  - typed control receipts, rejection reasons, idempotent retries, and draft-backed submission
affects: [08-07-rpc-migration]
tech-stack:
  added: []
  patterns: [coordinator-owned authorization, enqueue receipts after sender acceptance]
key-files:
  created: [.planning/phases/08-client-connection-replay-and-scoped-control/08-06-SUMMARY.md]
  modified:
    - crates/pi-coding-agent/src/coding_session/operation_control.rs
    - crates/pi-coding-agent/src/coding_session/snapshot_coordinator.rs
    - crates/pi-coding-agent/src/coding_session/public_projection.rs
    - crates/pi-coding-agent/src/coding_session/mod.rs
key-decisions:
  - "Prompt controls remain outside CodingAgentSession::run and use a bounded private channel."
  - "Receipt lookup is scoped by client, target operation, and control id; accepted receipts survive target completion."
requirements-completed: [CLIENT-03, CONTROL-01]
coverage:
  - id: D1
    description: "Owner/generation/operation-scoped Prompt control capability with typed enqueue receipts and rejection reasons"
    requirement: CONTROL-01
    verification:
      - kind: unit
        ref: "cargo test -p pi-coding-agent --lib operation_control --quiet"
        status: pass
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test public_api --quiet"
        status: pass
    human_judgment: false
  - id: D2
    description: "Draft-backed steer/follow-up control APIs preserve coordinator ownership and acceptance-driven clearing"
    requirement: CLIENT-03
    verification:
      - kind: unit
        ref: "cargo test -p pi-coding-agent --lib client_service --quiet"
        status: pass
    human_judgment: false
duration: 20 min
completed: 2026-07-14
status: complete
---

# Phase 8 Plan 6 Summary

**Bounded, generation-scoped Prompt control with coordinator-owned typed receipts and retry-safe draft submission.**

## Accomplishments

- Replaced the unbounded private Prompt control transport with a bounded Tokio channel and typed capacity/closed-channel handling.
- Added immutable public `CodingAgentPromptControl` methods for abort, steer, follow-up, and draft-backed steer/follow-up.
- Centralized owner, generation, target, receipt idempotency, payload conflict, ordering, and acceptance-driven draft clearing in `SnapshotCoordinator`.
- Registered the active submitted Prompt control capability from canonical run admission without adding a second ordinary operation dispatcher.

## Task Commits

1. **Task 1/2: Implement scoped Prompt control admission and receipts** - `d7e21f1` (feat)

**Plan metadata:** this summary commit.

## Verification

- `cargo test -p pi-coding-agent --test public_api --quiet` passed.
- `cargo test -p pi-coding-agent --lib operation_control --quiet` passed.
- `cargo test -p pi-coding-agent --lib client_service --quiet` passed.
- `cargo test -p pi-coding-agent --test api_boundary_guards --quiet` passed.
- `cargo check -p pi-coding-agent --all-targets` passed.
- `cargo fmt --check` passed.
- `git diff --check` passed.

## Deviations

The plan's new dedicated `scoped_control` public test group was not added in this execution; existing operation-control, client-service, public API, and boundary suites were retained and passed. Follow-up Plan 08-07 should add the external end-to-end matrix before RPC migration is considered complete.

## Next Phase Readiness

The public connection can issue immutable Prompt controls and the runtime binds submitted Prompt operations to the coordinator-owned transport. RPC adapters can now map typed receipts/rejections without importing private senders or operation-control internals.

---
*Phase: 08-client-connection-replay-and-scoped-control*
*Completed: 2026-07-14*
