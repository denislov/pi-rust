---
phase: 08-client-connection-replay-and-scoped-control
plan: 05
subsystem: client-runtime
tags: [rust, client-connection, replay, raii, submission-provenance]
requires:
  - phase: 08-client-connection-replay-and-scoped-control
    provides: atomic SnapshotCoordinator authority and retained event recovery
provides:
  - Generation-scoped Arc-backed public client connection
  - Atomic snapshot, reconnect, acknowledgement, and draft facade
  - Exclusive RAII submission lease consumed by canonical CodingAgentSession::run
  - Accepted/running/terminal submission provenance with exact draft clearing
affects: [08-06-scoped-control, 08-07-rpc-migration]
tech-stack:
  added: []
  patterns: [generation-scoped handles, RAII admission guard, owned public projections]
key-files:
  created: []
  modified:
    - crates/pi-coding-agent/src/coding_session/public_projection.rs
    - crates/pi-coding-agent/src/coding_session/snapshot_coordinator.rs
    - crates/pi-coding-agent/src/coding_session/mod.rs
    - crates/pi-coding-agent/tests/public_api.rs
    - crates/pi-coding-agent/tests/api_boundary_guards.rs
key-decisions:
  - "Keep CodingAgentClientConnection as an Arc<SnapshotCoordinator> handle with client id/generation; it exposes state and preparation but no dispatcher."
  - "Consume a matching prepared lease only at CodingAgentSession::run and commit it immediately after IntentRouter admission returns the operation id."
  - "Use owned submission fingerprints so the closed runtime source guard remains stable and no borrowed operation data escapes preparation."
patterns-established:
  - "Precommit lease drop preserves the prompt draft; postcommit guard completion records terminal status without restoring it."
  - "Takeover increments generation and every connection route validates the immutable handle."
requirements-completed: [CLIENT-01, CLIENT-02, CLIENT-03]
coverage:
  - id: D1
    description: Stateful generation-scoped connection recovery and acknowledgement
    requirement: CLIENT-01
    verification:
      - kind: integration
        ref: cargo test -p pi-coding-agent --test public_api client_connection --quiet
        status: pass
    human_judgment: false
  - id: D2
    description: Canonical RAII submission lease lifecycle and draft boundary
    requirement: CLIENT-02
    verification:
      - kind: integration
        ref: cargo test -p pi-coding-agent --test public_api submission_lease --quiet
        status: pass
    human_judgment: false
  - id: D3
    description: No second dispatcher or private service escape hatch
    requirement: CLIENT-03
    verification:
      - kind: integration
        ref: cargo test -p pi-coding-agent --test api_boundary_guards --quiet
        status: pass
    human_judgment: false
duration: 9min
completed: 2026-07-14
status: complete
---

# Phase 08 Plan 05 Summary

**Clients now recover and prepare tracked Prompt submissions through one coordinator authority while unchanged no-lease calls continue through the sole canonical dispatcher.**

## Performance

- **Duration:** 9 min
- **Started:** 2026-07-13T17:03:19Z
- **Completed:** 2026-07-13T17:11:54Z
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments

- Added a cloneable, stateful public connection whose generation-scoped methods return owned atomic snapshots, retained replay, explicit acknowledgements, and typed draft mutations.
- Added a non-Clone RAII submission lease with session-wide exclusivity, takeover validation, precommit draft preservation, admission-time identity binding, and terminal completion/failure/cancellation handling.
- Preserved `CodingAgentSession::run(CodingAgentOperation)` as the only operation dispatcher and retained the no-lease path without client tracking.

## Task Commits

1. **Task 1: Prove public connection recovery and legacy no-lease compatibility** - `3562020`
2. **Task 2: Wire Arc-backed connection and typed error ownership** - `538e02b`
3. **Task 3: Implement exclusive lease and canonical SubmissionCommitGuard** - `3773c67`
4. **Guard compatibility fix** - `73a63b7`

## Verification

- `cargo fmt --check`
- `cargo test -p pi-coding-agent --test public_api client_connection --quiet`
- `cargo test -p pi-coding-agent --test public_api legacy_run --quiet`
- `cargo test -p pi-coding-agent --test public_api client_errors --quiet`
- `cargo test -p pi-coding-agent --test public_api submission_lease --quiet`
- `cargo test -p pi-coding-agent --test api_boundary_guards --quiet`
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --quiet` (16 passed)
- `cargo test -p pi-coding-agent --quiet` (focused failure repaired; rerun suites pass)
- `cargo check -p pi-coding-agent --all-targets`
- `git diff --check`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Source guard compatibility] Owned fingerprint values**
- **Found during:** Full `pi-coding-agent` verification
- **Issue:** The existing source sanitizer interpreted an early `&'static str` lifetime as a character literal and erased the owner method ledger.
- **Fix:** Store the lease kind as an owned `String`, which also removes borrowed operation data from the prepared lease.
- **Committed in:** `73a63b7`

**2. [Rule 1 - Closed owner API ledger] Private lease installation**
- **Found during:** Product runtime boundary verification
- **Issue:** A new `pub(crate)` owner helper widened the closed `CodingAgentSession` method ledger.
- **Fix:** Kept `install_submission_lease` private; the child public-projection module can invoke it without exposing a new crate-level facade.
- **Committed in:** `73a63b7`

**Total deviations:** 2 auto-fixed Rule 1 issues. No architecture or scope expansion.

## Issues Encountered

None remaining. Existing dead-code and deprecated compatibility warnings are unchanged and non-failing.

## Next Phase Readiness

Plan 08-06 can attach scoped control authorization and idempotent receipts to the same generation-bound connection without changing operation dispatch.

## Self-Check: PASSED

- All four implementation/test commits are present.
- Public and product-runtime boundary guards pass.
- The unrelated untracked `docs/next stage.md` remains untouched.
