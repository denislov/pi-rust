---
phase: 09-lifecycle-association-guards-and-closure
plan: 04
subsystem: operation-association
tags: [rust, submitted-provenance, terminal-evidence, partial-commit, cancellation]

requires:
  - phase: 09-lifecycle-association-guards-and-closure
    provides: exhaustive 15-operation descriptor and generation-scoped client lifecycle from Plans 09-02 and 09-03
provides:
  - Generic submitted provenance for all 15 public operations with Prompt-only draft matching
  - Exact ProductEvent, OutcomeOnly, and TerminalUncertain finalization anchors
  - Admitted-id durable transactions and one-root cardinality enforcement
  - Exact-id Compact cancellation through FlowRunOptions with typed Cancelled outcomes
affects: [09-05-shutdown, 09-06-rpc-lifecycle, operation-recovery, client-snapshots]

tech-stack:
  added: [tokio-util in pi-coding-agent]
  patterns:
    - Coordinator transaction records exact root evidence at EventService publication time
    - Active operation identity binds kind, admitted id, monotonic generation, and optional Compact token

key-files:
  created:
    - crates/pi-coding-agent/tests/operation_association.rs
  modified:
    - crates/pi-coding-agent/src/coding_session/mod.rs
    - crates/pi-coding-agent/src/coding_session/snapshot_coordinator.rs
    - crates/pi-coding-agent/src/coding_session/public_projection.rs
    - crates/pi-coding-agent/src/coding_session/operation_control.rs
    - crates/pi-coding-agent/src/coding_session/intent_router.rs
    - crates/pi-coding-agent/src/coding_session/manual_compaction_flow.rs
    - crates/pi-coding-agent/src/coding_session/event_service.rs
    - crates/pi-coding-agent/src/coding_session/session_log/transaction.rs
    - crates/pi-coding-agent/src/coding_session/session_service.rs
    - crates/pi-coding-agent/tests/public_api.rs

key-decisions:
  - "Use the existing SnapshotCoordinator submitted record as the exact root-evidence authority updated inside EventService publication, avoiding retained-history scans or a second index."
  - "Bind persistent and transient workflow operation ids to the admitted capability snapshot id so durable facts, outcomes, events, and submitted state share one identity."
  - "Keep Compact cancellation crate-private and exact-id scoped; preserve PromptFailed compatibility while CodingSessionError::Cancelled distinguishes the typed failure."

patterns-established:
  - "Terminal finalization: descriptor-permitted event + exact admitted id + exactly-one cardinality, otherwise explicit TerminalUncertain."
  - "Acknowledgement domains: cumulative event cursor clears only ProductEvent anchors; exact opaque token clears only OutcomeOnly anchors."

requirements-completed: [CONTROL-02, COMPAT-03]

coverage:
  - id: D1
    description: "All 15 public operations can enter submitted provenance while only Prompt validates and clears its matching draft."
    requirement: CONTROL-02
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent submission_association --quiet"
        status: pass
    human_judgment: false
  - id: D2
    description: "OutcomeOnly finalization carries no event cursor and can only be cleared by its exact opaque acknowledgement."
    requirement: CONTROL-02
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent outcome_acknowledgement --quiet"
        status: pass
    human_judgment: false
  - id: D3
    description: "TerminalAssociated operations finalize from exact descriptor-permitted root events, including Compact compatibility-event disambiguation."
    requirement: COMPAT-03
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test operation_association terminal_association --quiet"
        status: pass
    human_judgment: false
  - id: D4
    description: "Compact cancellation is exact-id scoped, fail-closed, and reaches canonical Flow cancellation as typed Cancelled with same-id PromptFailed compatibility."
    requirement: CONTROL-02
    verification:
      - kind: unit
        ref: "cargo test -p pi-coding-agent compact_cancellation --lib --quiet"
        status: pass
    human_judgment: false
  - id: D5
    description: "PartialCommit preserves the original admitted id and records TerminalUncertain when no root event was established."
    requirement: COMPAT-03
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test operation_association partial_commit_association --quiet"
        status: pass
    human_judgment: false

duration: 1h 32m
completed: 2026-07-14
status: complete
---

# Phase 09 Plan 04: Exact Operation Association and Compact Cancellation Summary

**All 15 canonical operations now retain admitted-id submission provenance and finalize through exact root events, opaque outcome acknowledgements, or explicit durability uncertainty, with reachable typed Compact cancellation.**

## Performance

- **Duration:** 1h 32m
- **Started:** 2026-07-14T05:44:08Z
- **Completed:** 2026-07-14T07:16:41Z
- **Tasks:** 2
- **Files modified:** 18

## Accomplishments

- Generalized preparation and submitted state from Prompt-only provenance to the exhaustive 15-operation descriptor while retaining Prompt-specific draft fingerprint validation and clearing.
- Replaced latest-global sequence guessing with exact same-transaction root-event observation, descriptor filtering, admitted-id matching, and one-root cardinality validation.
- Added distinct ProductEvent, OutcomeOnly, and TerminalUncertain anchors with non-interchangeable acknowledgement contracts and explicit uncertain durability.
- Bound persistent/manual-compaction and transient Prompt transactions to the admitted capability snapshot operation id.
- Added a crate-private exact-id Compact cancellation handle, monotonic guard generation, permit token propagation, `FlowRunOptions.cancel`, and exhaustive `FlowError::Cancelled` mapping.

## Task Commits

1. **Task 1 RED: failing submission and acknowledgement contracts** - `b8aed5a` (test)
2. **Task 1 GREEN: generic submission terminal anchors** - `33559b3` (feat)
3. **Task 2 RED: failing exact terminal/PartialCommit association** - `8dec0c1` (test)
4. **Task 2 GREEN: exact root evidence and Compact cancellation** - `6d88399` (feat)
5. **Verification fixes: dispatcher and owner boundary guards** - `d6e8156`, `3d23b5f`, `4f2b660` (fix)
6. **Verification fix: transient admitted-id binding** - `490fbb7` (fix)

## Files Created/Modified

- `crates/pi-coding-agent/tests/operation_association.rs` - Public Compact root-cardinality and deterministic PartialCommit uncertainty fixtures.
- `crates/pi-coding-agent/src/coding_session/snapshot_coordinator.rs` - Exhaustive submitted descriptors, exact anchors, root observation, cardinality, and acknowledgement separation.
- `crates/pi-coding-agent/src/coding_session/mod.rs` - Generic lease consumption, admitted-id finalization, Compact token handoff, and canonical cancellation coverage.
- `crates/pi-coding-agent/src/coding_session/operation_control.rs` - Generation-scoped active identity and crate-private exact Compact cancellation handle.
- `crates/pi-coding-agent/src/coding_session/manual_compaction_flow.rs` - Cancellation-carrying options and typed Flow cancellation mapping.
- `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs` - Admitted operation-id transaction constructor.
- `crates/pi-coding-agent/src/coding_session/session_service.rs` - Snapshot-bound Prompt/Compact transactions and PartialCommit wrapping after non-leaf append.
- `crates/pi-coding-agent/tests/public_api.rs` - Exhaustive preparation, outcome acknowledgement, terminal projection, and lock-order contracts.

## Decisions Made

- Exact root evidence is recorded into the existing submitted-operation record during the same coordinator transaction that allocates and retains the ProductEvent; bounded replay history is never scanned as association authority.
- Compact success accepts only `Session::CompactionCompleted`; same-id compatibility `Workflow::PromptCompleted` remains visible but cannot satisfy the Compact descriptor.
- Compact failure/cancellation normalizes only same-id `PromptFailed` to `CompactPromptFailed` evidence, preserving compatibility payloads without adding an aborted event variant.
- Outcome-only acknowledgement ids are opaque deterministic identities derived from the admitted operation id and remain free of public generation/signature authority.

## TDD Gate Compliance

- **Task 1 RED:** `b8aed5a` failed because terminal status exposed no anchor and preparation remained Prompt-only.
- **Task 1 GREEN:** `33559b3` made both `submission_association` and `outcome_acknowledgement` pass.
- **Task 2 RED:** `8dec0c1` proved the old global-latest guess anchored Compact to the compatibility Prompt event instead of `CompactionCompleted`.
- **Task 2 GREEN:** `6d88399` made exact root, PartialCommit, and canonical Compact cancellation gates pass.
- **REFACTOR:** No separate refactor commit; focused verification fixes were committed explicitly.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Bound durable and transient transactions to the admitted operation id**

- **Found during:** Task 2 exact Compact association
- **Issue:** Prompt and Compact transactions independently generated ids, so emitted terminal evidence could not match admitted submitted provenance.
- **Fix:** Added an admitted-id transaction constructor and used capability snapshot ids for persistent Prompt, Compact, and transient Prompt execution.
- **Files modified:** `session_log/transaction.rs`, `session_service.rs`, `mod.rs`
- **Verification:** Exact Compact root and full public API suites pass.
- **Committed in:** `6d88399`, `490fbb7`

**2. [Rule 1 - Bug] Preserved PartialCommit after a non-leaf durable append**

- **Found during:** Task 2 deterministic manifest failure fixture
- **Issue:** Non-leaf transaction manifest failure was returned as a generic Session error even though events had already appended.
- **Fix:** Wrap post-append manifest failure as `PartialCommit` with the original admitted operation id.
- **Files modified:** `session_service.rs`
- **Verification:** `partial_commit_association` passes and records TerminalUncertain without retry fabrication.
- **Committed in:** `6d88399`

**3. [Rule 3 - Blocking] Added direct tokio-util dependency and synchronized structural guards**

- **Found during:** Task 2 compilation and full workspace verification
- **Issue:** The product crate needed `CancellationToken`, and existing exact source ledgers still expected pre-lease sync dispatch syntax or omitted the private cancellation helper.
- **Fix:** Added the already-workspace-standard `tokio-util` dependency and updated only the affected canonical-dispatch/owner-ledger assertions.
- **Files modified:** `Cargo.toml`, `Cargo.lock`, `intent_router.rs`, `api_boundary_guards.rs`, `product_runtime_boundary_guards.rs`
- **Verification:** Full `cargo test -p pi-coding-agent` and `cargo test --workspace` pass.
- **Committed in:** `6d88399`, `d6e8156`, `3d23b5f`, `4f2b660`

---

**Total deviations:** 3 auto-fixed (1 missing critical functionality, 1 bug, 1 blocking integration update).
**Impact on plan:** Each change is required to preserve admitted identity, durability truth, or repository-enforced boundaries; no public cancellation/control surface was added.

## Issues Encountered

- Workspace transport tests require loopback socket binding and were rerun outside the filesystem/network sandbox.
- A PTY workspace run caused the intentional TTY-only CLI integration test to enter interactive mode; final verification used a non-TTY persistent command session and passed.
- Existing dead-code, unused-import, and deprecated compatibility warnings remain unchanged and non-fatal.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plan 09-05 can drain active operations against exact admitted identities and terminal anchors without guessing or coupling shutdown to Compact cancellation.
- Plans 09-06 onward can project OutcomeOnly and TerminalUncertain state through adapters using the stable facade.
- No blockers; `docs/next stage.md` remains untouched and untracked.

## Self-Check: PASSED

- FOUND all created/modified implementation and integration-test files.
- FOUND task commits `b8aed5a`, `33559b3`, `8dec0c1`, and `6d88399` plus focused verification-fix commits.
- PASS: all five plan-level focused verification commands.
- PASS: full `cargo test -p pi-coding-agent --quiet`.
- PASS: full non-TTY `cargo test --workspace --quiet` with explicit exit code 0.
- PASS: `cargo fmt --all --check`, `cargo check --workspace --quiet`, source audit removing `current_event_sequence`, and `git diff --check`.
- PASS: unrelated `docs/next stage.md` remains untouched and untracked.

---
*Phase: 09-lifecycle-association-guards-and-closure*
*Completed: 2026-07-14*
