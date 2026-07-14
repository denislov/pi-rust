---
phase: 09-lifecycle-association-guards-and-closure
verified: 2026-07-14T10:14:29Z
status: gaps_found
score: 7/10 must-haves verified
behavior_unverified: 0
overrides_applied: 0
gaps:
  - truth: "Detach/close and shutdown are explicit, idempotent, and preserve session/event ownership invariants."
    status: failed
    reason: "After Phase B reaches ShutDown, CodingAgentReconnectReceiver rejects every queued delivery except Runtime.ShutDown itself. A slow reconnect receiver can therefore lose an admitted operation's terminal event that was published before shutdown."
    artifacts:
      - path: "crates/pi-coding-agent/src/coding_session/public_projection.rs"
        issue: "ensure_delivery_live permits only Runtime.ShutDown after validate_receiver reports RuntimeShutDown; it has no authoritative shutdown sequence/drain boundary for earlier queued events."
      - path: "crates/pi-coding-agent/tests/public_api.rs"
        issue: "The shutdown ordering test uses subscribe_product_events_public(), not CodingAgentReconnectReceiver, so it does not exercise the rejecting delivery gate."
    missing:
      - "Record an authoritative shutdown event sequence or equivalent drain boundary and allow queued events through that boundary before receiver closure."
      - "Add a deterministic slow reconnect-consumer test proving terminal event, then Runtime.ShutDown, then closure after Phase B has completed."
  - truth: "Operation id, submitted state, terminal outcome, and terminal event associations are tested for applicable operations, including PartialCommit failure paths."
    status: failed
    reason: "fail_non_leaf_transaction flushes OperationFailed session events and then returns a raw manifest-update error, losing the admitted operation id and PartialCommit classification after a real partial write."
    artifacts:
      - path: "crates/pi-coding-agent/src/coding_session/session_service.rs"
        issue: "Lines 728-732 call transaction.fail() and then update_manifest(...)? without the PartialCommit map_err used by commit_non_leaf_transaction."
      - path: "crates/pi-coding-agent/tests/operation_association.rs"
        issue: "The current manifest-failure fixture covers the successful Compact commit path, not a failed Prompt/Compact/self-healing non-leaf transaction."
    missing:
      - "Map post-fail manifest-update errors to CodingSessionError::PartialCommit with the original transaction operation id."
      - "Add a deterministic failed-transaction manifest-failure test that proves the same operation id and TerminalUncertain association."
  - truth: "Required formatting, focused tests, full workspace checks, security checks, source audits, and diff checks pass."
    status: failed
    reason: "Formatting, focused/full tests, workspace check, source guards, and diff checks passed, but security enforcement is active and Phase 09 has no required 09-SECURITY.md with threats_open: 0. The GSD verify/ship contracts explicitly block advancement in this state."
    artifacts:
      - path: ".planning/phases/09-lifecycle-association-guards-and-closure/09-SECURITY.md"
        issue: "Missing while the active verify:post secure-phase hook is configured with onError: halt."
      - path: ".planning/phases/09-lifecycle-association-guards-and-closure/09-VALIDATION.md"
        issue: "Claims security closure from source tests but cannot substitute for the configured independent SECURITY.md threat audit."
    missing:
      - "Run $gsd-secure-phase 9 and produce 09-SECURITY.md."
      - "Resolve all high-severity Phase 09 threat-register findings so SECURITY.md reports threats_open: 0."
---

# Phase 9: Lifecycle Association, Guards, and Closure Verification Report

**Phase Goal:** Close client lifecycle ownership and operation/event association, harden boundary guards, and verify the complete v1.1 contract.
**Verified:** 2026-07-14T10:14:29Z
**Status:** gaps_found
**Re-verification:** No - initial verification
**Execution mode:** generic-agent workaround for `gsd-verifier`; typed GSD agent dispatch was unavailable.

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | Detach/close and shutdown are explicit, idempotent, and preserve session/event ownership invariants. | FAILED | Detach and normal shutdown tests pass, but `public_projection.rs:622-644` rejects a queued pre-shutdown terminal event after coordinator state becomes `ShutDown`. The existing shutdown test reads the non-reconnect product receiver (`public_api.rs:853,906-925`) and misses this path. |
| 2 | Operation id, submitted state, terminal outcome, and terminal event associations are tested for applicable operations. | FAILED | Exact Compact root association passes, but `session_service.rs:707-732` loses `PartialCommit { operation_id }` when a failed non-leaf transaction flushes and the following manifest update fails. |
| 3 | Adapter-root and compile-fixture guard debt is closed with fail-closed tests. | VERIFIED | Recursive discovery uses exact discovered/classified set equality (`product_runtime_boundary_guards.rs:1816-1821`); the unclassified/stale fixture passed. Cargo fixtures require the first rustc error, exact code, primary `src/main.rs` span, symbol/path fragments, and a compiling adjacent facade (`api_boundary_guards.rs:176-239,297-369`). |
| 4 | Formatting, focused tests, full workspace checks, security checks, source audits, and diff checks pass. | FAILED | Fresh orchestrator evidence passed format, workspace test/check, and diff checks. The active `verify:post` security hook requires `SECURITY.md`, but Phase 09 has none; GSD verification explicitly blocks advancement when enforcement is enabled and the file is absent. |
| 5 | Compile-ready lifecycle outcomes, rejection categories, and terminal anchors are typed and externally observable without private authority. | VERIFIED | `lifecycle_values_are_exhaustive_and_importable` passed; Plan artifact verification found `public_api.rs` and `api_boundary_guards.rs` substantive and wired to `pi_coding_agent::api`. |
| 6 | Detached/stale/shutdown handles fail closed across state, acknowledgement, draft, submission, replay, and control paths. | VERIFIED | `detach_outcomes_and_lifecycle_rejection_paths_are_typed_and_preserve_state` passed; source routes receiver and connection validation through the coordinator lifecycle gate. |
| 7 | All 15 public operations occur exactly once in a fail-closed association matrix with exact root semantics. | VERIFIED | `association_matrix_classifies_all_public_operations_exactly_once` and `terminal_association_uses_the_exact_compact_root_event` passed. |
| 8 | OutcomeOnly completion has a distinct acknowledgement domain and no guessed event sequence. | VERIFIED | `outcome_acknowledgement_is_distinct_from_event_acknowledgement` passed; public anchors remain separate ProductEvent, OutcomeOnly, and TerminalUncertain variants. |
| 9 | RPC lifecycle projection is additive and restores the unique owner before Phase B shutdown. | VERIFIED | `rpc_lifecycle_shutdown_waits_for_owner_restoration_and_uses_stable_rejection_code` passed; legacy/full RPC compatibility was also covered by the fresh workspace run. |
| 10 | Interactive loop exit is detach-only while the process owner alone performs final shutdown. | VERIFIED | `embedded_interactive_lifecycle_is_detach_only_and_owner_shutdown_is_top_level` passed. |

**Score:** 7/10 truths verified (0 present-but-behavior-unverified)

No item is deferred: Phase 9 is the final phase in the v1.1 roadmap, so all three gaps are current closure work.

## Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/pi-coding-agent/src/coding_session/public_projection.rs` | Typed lifecycle values, connection detach, lifecycle-aware reconnect delivery | FAILED | Exists, substantive, and wired, but its post-shutdown delivery gate drops queued pre-shutdown business/terminal events. |
| `crates/pi-coding-agent/src/coding_session/snapshot_coordinator.rs` | Sole lifecycle and submitted-state authority | VERIFIED | Exists, substantive, wired through connection validation and EventService; no second lifecycle registry found. |
| `crates/pi-coding-agent/src/coding_session/public_operation.rs` | Exhaustive 15-operation association descriptor | VERIFIED | Exists and substantive; exact matrix test passed. |
| `crates/pi-coding-agent/src/coding_session/session_service.rs` | Durable transaction finalization and PartialCommit preservation | FAILED | Successful non-leaf commit wraps post-append manifest failure, but failed non-leaf finalization does not. |
| `crates/pi-coding-agent/src/coding_session/operation_control.rs` | Scoped active identity and private Compact cancellation | VERIFIED | Exact kind/id/generation/token design is private and covered by phase source guards and the fresh full suite. |
| `crates/pi-coding-agent/src/protocol/rpc/state.rs` | Shared RPC detach cleanup path | VERIFIED | Exists, substantive, and behavior is covered by the RPC lifecycle suites. |
| `crates/pi-coding-agent/src/interactive/loop.rs` / `app.rs` | Detach-only loop and process-owner shutdown boundary | VERIFIED | Both exist, substantive, wired, and the named ownership test passed. |
| `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` | Recursive adapter discovery/classification | VERIFIED | Exact set-equality implementation and unclassified/stale negative fixture are present and passing. |
| `crates/pi-coding-agent/tests/api_boundary_guards.rs` | Diagnostic-bound external compile fixtures | VERIFIED | Structured Cargo JSON matcher, 12-case matrix, first-error rule, and positive neighbor are present. |
| `.planning/phases/09-lifecycle-association-guards-and-closure/09-SECURITY.md` | Independent ASVS Level 1 threat closure with `threats_open: 0` | MISSING | Required by the active security hook; no Phase 09 security report exists. |

## Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `CodingAgentClientConnection::detach` | `SnapshotCoordinator` | Connection-derived private handle | WIRED | Named detach behavior test passed; no arbitrary client/generation selector is exposed. |
| `EventService::emit` | submitted terminal anchor | Exact id/kind root observation | WIRED | Exact Compact root test passed; event retention and root observation occur under the coordinator state lock. |
| failed non-leaf transaction flush | public recovery error | `PartialCommit { operation_id }` conversion | NOT WIRED | `transaction.fail()` flushes, but the subsequent manifest error escapes through raw `?`. |
| `CodingAgentSession::shutdown` | reconnect receiver drain | terminal event -> shutdown event -> closure | PARTIAL | Publication ordering exists, but reconnect delivery validation cannot drain queued terminal events once runtime state is `ShutDown`. |
| RPC/Interactive owner task | Phase A handle and restored owner | request before owner return; Phase B afterward | WIRED | Named RPC and Interactive ownership tests passed. |
| production source discovery | adapter classification ledger | exact set equality | WIRED | Unclassified and stale rows are independently rejected. |
| compile-fail fixture | intended forbidden surface | first Cargo/rustc JSON diagnostic and primary span | WIRED | Exact matcher test passed; positive stable facade is compiled before negative cases. |

## Data-Flow Trace

No frontend or dynamic rendering artifact is in scope. The relevant runtime traces are:

| Flow | Source | Sink | Status |
|---|---|---|---|
| Product event delivery | `EventService::emit` retained/broadcast event | `CodingAgentReconnectReceiver::recv/try_recv` | FAILED after Phase B for queued non-shutdown events |
| Durable failed operation | `TurnTransaction::fail` flushed envelopes | caller-visible `CodingSessionError` and submitted uncertainty | FAILED on following manifest update |
| Adapter discovery | recursive sanitized `src/**/*.rs` structural candidates | exact ownership classification | FLOWING |
| External fixture diagnostic | Cargo JSON compiler messages | code/span/symbol/path assertion | FLOWING |

## Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Detach idempotency/preservation | `cargo test -p pi-coding-agent --test public_api detach_outcomes_and_lifecycle_rejection_paths_are_typed_and_preserve_state -- --exact` | 1 passed | PASS |
| Existing shutdown ordering fixture | `cargo test -p pi-coding-agent --test public_api shutdown_drains_admitted_work_before_lifecycle_event_and_receiver_close -- --exact` | 1 passed, but exercises the non-reconnect receiver | PASS WITH COVERAGE GAP |
| Closed operation matrix | `cargo test -p pi-coding-agent association_matrix_classifies_all_public_operations_exactly_once -- --exact` | named test passed | PASS |
| Exact Compact root | `cargo test -p pi-coding-agent --test operation_association terminal_association_uses_the_exact_compact_root_event -- --exact` | 1 passed | PASS |
| Outcome acknowledgement separation | `cargo test -p pi-coding-agent --test public_api outcome_acknowledgement_is_distinct_from_event_acknowledgement -- --exact` | 1 passed | PASS |
| Adapter fail-closed fixture | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards adapter_discovery_fixture_rejects_unclassified_and_stale_ownership -- --exact` | 1 passed | PASS |
| Diagnostic attribution | `cargo test -p pi-coding-agent --test api_boundary_guards external_diagnostic_matcher_requires_code_primary_span_and_forbidden_surface -- --exact` | 1 passed | PASS |
| RPC owner restoration | `cargo test -p pi-coding-agent --test rpc_mode rpc_lifecycle_shutdown_waits_for_owner_restoration_and_uses_stable_rejection_code -- --exact` | 1 passed | PASS |
| Interactive ownership | `cargo test -p pi-coding-agent --test interactive_mode embedded_interactive_lifecycle_is_detach_only_and_owner_shutdown_is_top_level -- --exact` | 1 passed | PASS |
| Full workspace gates | Fresh orchestrator: `cargo fmt --all --check`, `cargo test --workspace`, `cargo check --workspace`, `git diff --check` | all passed in non-TTY mode | PASS |

No phase-declared or conventional `scripts/**/tests/probe-*.sh` probes exist, so probe execution is not applicable.

## Requirements Coverage

| Requirement | Source Plans | Status | Evidence |
|---|---|---|---|
| COMPAT-03 | 09-04..09-08 | BLOCKED | Existing adapter/workspace compatibility tests pass, but queued terminal loss and failed-transaction PartialCommit identity violate ordering/durability compatibility. |
| CLIENT-04 | 09-01, 09-03, 09-05..09-08 | BLOCKED | Explicit idempotent detach/shutdown exists; reconnect receiver ownership invariant fails for a slow consumer after shutdown. |
| CONTROL-02 | 09-01..09-05, 09-08 | BLOCKED | Matrix/exact roots/outcome acknowledgements pass; a failed durable operation can lose its original id and uncertainty classification. |
| GUARD-01 | 09-08 | SATISFIED | Recursive discovery plus exact classification equality and unclassified/stale negative fixture passed. |
| GUARD-02 | 09-08 | SATISFIED | 12 diagnostic-bound fixtures and positive stable facade are structurally enforced; exact matcher test passed. |

No Phase 9 requirement is orphaned from plan frontmatter.

## Security and Anti-Patterns

No `TODO`, `FIXME`, `XXX`, `HACK`, or placeholder marker was found in the critical modified source/guard files. The important anti-patterns are behavioral:

| File | Line | Pattern | Severity | Impact |
|---|---:|---|---|---|
| `public_projection.rs` | 622 | Post-shutdown delivery whitelist recognizes only the shutdown event, without a drain sequence boundary | BLOCKER | Drops valid queued terminal evidence for slow reconnect consumers. |
| `session_service.rs` | 729 | Raw `?` after durable failed-operation flush | BLOCKER | Converts partial durability into an unattributed generic error. |
| `09-VALIDATION.md` | final evidence | Declares security closure without configured `SECURITY.md` output | BLOCKER | Security enforcement remains incomplete and phase advancement must halt. |

The active `verify:post` hook includes `secure-phase`, produces `SECURITY.md`, and uses `onError: halt`. Source/security tests are useful mitigation evidence, but they do not replace the configured independent threat-register audit.

## Disconfirmation Pass

- **Partially met requirement:** CLIENT-04 has correct explicit/idempotent APIs but fails the slow reconnect-consumer shutdown invariant.
- **Misleading passing test:** `shutdown_drains_admitted_work_before_lifecycle_event_and_receiver_close` passes because it uses `CodingAgentProductEventReceiver`; it does not traverse `CodingAgentReconnectReceiver::ensure_delivery_live`, where the defect resides.
- **Uncovered error path:** no test injects `UpdateManifest` failure after `fail_non_leaf_transaction` has flushed failed-operation envelopes.

## Human Verification Required

None. The three gaps are deterministically observable in source/control flow and can be covered by offline automated tests; no visual, external-service, or subjective check is needed.

## Gaps Summary

Phase 9 is not complete despite green workspace gates. The implementation has two production correctness gaps at explicit roadmap boundaries: shutdown does not preserve queued terminal delivery for reconnect consumers, and failed non-leaf durable transactions can lose `PartialCommit` operation identity. In addition, the configured blocking security workflow has not produced its required Phase 09 audit artifact. These gaps are not deferred by any later v1.1 phase.

---

_Verified: 2026-07-14T10:14:29Z_
_Verifier: generic-agent workaround (gsd-verifier)_
