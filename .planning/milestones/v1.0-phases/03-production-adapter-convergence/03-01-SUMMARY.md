---
phase: 03-production-adapter-convergence
plan: 01
subsystem: protocol-adapter
tags: [rust, canonical-operations, json-mode, print-mode, adapter-convergence, boundary-guards]

requires:
  - phase: 02-canonical-facade-correctness
    plan: 03
    provides: Canonical run durability, exhaustive public outcome projection, and closed facade ledger
provides:
  - JSON adapter submits Prompt through CodingAgentSession::run with preserved event ordering and projection
  - Persistent and transient print adapters submit Prompt through CodingAgentSession::run with preserved text/error/session semantics
  - Narrow executable boundary guard rejecting deprecated broad workflow calls and production deprecation suppression in JSON/print source
affects: [rpc-migration, interactive-migration, phase-04, phase-05]

tech-stack:
  added: []
  patterns:
    - "Exhaustive public outcome extraction at the adapter boundary: run(CodingAgentOperation::Prompt) -> match CodingAgentOperationOutcome::Prompt, unreachable! on impossible variants"
    - "Sanitize-source boundary guard that strips comments/strings, skips #[cfg(test)] modules, and reports file:line for deprecated broad workflow calls or production #[allow(deprecated)]"

key-files:
  created: []
  modified:
    - crates/pi-coding-agent/src/protocol/json_mode.rs
    - crates/pi-coding-agent/src/print_mode.rs
    - crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs

key-decisions:
  - "Import CodingAgentOperation and CodingAgentOperationOutcome through crate::api per D-16, leaving existing concrete type imports from crate::coding_session unchanged."
  - "Treat an unexpected public outcome variant as an internal invariant (unreachable!) rather than a new user-visible error, matching T-03-04 accept disposition and the closed-enum discipline from Phase 2."
  - "Scope the JSON/print guard to the eight CodingAgentSession methods deprecated in favor of run, excluding crate-private lifecycle helpers like fork_session and the non-deprecated subscribe_product_events receiver."
  - "Grow the guard incrementally: json_mode.rs in Task 1, print_mode.rs in Task 3, so each adapter's parity gate is independently green before the combined D-06 gate closes."

patterns-established:
  - "Adapter canonical-call pattern: replace session.<broad>(opts) with session.run(CodingAgentOperation::<Variant>(opts)) and exhaustive outcome extraction, preserving the surrounding select/drain/projection shell."
  - "Narrow source guard pattern: sanitize_rust_source + line_is_cfg_test_gated to flag production deprecation suppression and deprecated broad calls while preserving test-only allowances and compatibility definitions."

requirements-completed: [ADAPT-01, ADAPT-02, ADAPT-03, ADAPT-04]

coverage:
  - id: D1
    description: "JSON adapter executes prompt work through CodingAgentSession::run(CodingAgentOperation::Prompt) with preserved header/AgentStart ordering, event select/drain, provider-failure projection, exit codes, and persistent session effects."
    requirement: ADAPT-01
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/json_mode.rs#json_mode_emits_session_header_and_lifecycle_events"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/json_mode.rs#json_mode_emits_tool_execution_events"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/json_mode.rs#json_mode_maps_provider_failure_to_error_output"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/json_mode.rs#json_mode_enabled_session_uses_rust_native_log"
        status: pass
    human_judgment: false
  - id: D2
    description: "Persistent print adapter executes prompt work through the canonical operation facade with unchanged session target/open/fork lifecycle, semantic manifest/event facts, replay, and CliError mapping."
    requirement: ADAPT-02
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/session_print_mode.rs#persists_new_print_mode_session"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/session_print_mode.rs#enabled_session_with_name_uses_rust_native_log"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/session_print_mode.rs#continues_most_recent_rust_native_session"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/print_mode.rs#explicit_new_session_writes_rust_native_session_events"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/print_mode.rs#fork_target_routes_through_rust_native_session"
        status: pass
    human_judgment: false
  - id: D3
    description: "Transient print adapter executes prompt work through the canonical operation facade, creates no session artifacts, and preserves target rejection, final-text/error conversion, and diagnostic text."
    requirement: ADAPT-02
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/print_mode.rs#prints_single_turn_text_response"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/print_mode.rs#disabled_session_print_uses_non_persistent_runtime_without_session_files"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/print_mode.rs#returns_agent_failure_on_error_stop_reason"
        status: pass
      - kind: integration
        ref: "crates/pi-coding-agent/tests/print_mode.rs#supports_tool_call_loop_with_injected_tool"
        status: pass
    human_judgment: false
  - id: D4
    description: "JSON and print production code contains no replaced broad workflow calls or local #[allow(deprecated)] attributes, enforced by an executable boundary guard that distinguishes executable receiver calls from comments/strings and preserves test-only allowances."
    requirement: ADAPT-04
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs#production_json_and_print_use_canonical_operations"
        status: pass
    human_judgment: false

duration: 10 min
completed: 2026-07-11
status: complete
---

# Phase 03 Plan 01: JSON And Print Adapter Convergence Summary

**JSON and print prompt paths now route through CodingAgentSession::run(CodingAgentOperation::Prompt) with exhaustive outcome extraction, locked by a narrow production source guard.**

## Performance

- **Duration:** 10 min
- **Started:** 2026-07-11T19:48:55Z
- **Completed:** 2026-07-11T19:58:38Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- Migrated the JSON prompt future to `session.run(CodingAgentOperation::Prompt(prompt_options))` with exhaustive `CodingAgentOperationOutcome::Prompt` extraction inside the existing spawned task, preserving receiver-before-run ordering, header/`AgentStart` emission, every `tokio::select!` branch, completion-time drain, synthetic `PromptFailed`, stderr/exit projection, and persistent session effects.
- Migrated the persistent print branch to the canonical `run(Prompt)` path with exact outcome extraction, leaving target resolution, open/continue/fork lifecycle behavior, semantic event persistence, output text, and `CliError` mapping unchanged.
- Migrated the transient print branch to the canonical `run(Prompt)` path, preserving non-persistent target rejection, absence of session files, final-text/error conversion, and existing diagnostic text.
- Added the `production_json_and_print_use_canonical_operations` boundary guard covering both `src/protocol/json_mode.rs` and `src/print_mode.rs`; it sanitizes Rust source (stripping comments and string contents), skips `#[cfg(test)]` modules, and reports file:line for any deprecated broad workflow call or production `#[allow(deprecated)]` attribute.
- Closed the combined D-06 parity gate: `json_mode`, `print_mode`, `session_print_mode`, the boundary guard, `cargo check -p pi-coding-agent`, `cargo fmt --check`, and `git diff --check` are all green together.

## Task Commits

Each task was committed atomically:

1. **Task 1: Migrate the JSON prompt future and lock streaming parity** - `6717a87` (feat)
2. **Task 2: Migrate the persistent print prompt path** - `036c413` (feat)
3. **Task 3: Migrate transient print and close the combined adapter gate** - `e2ddba7` (feat)

## Files Created/Modified

- `crates/pi-coding-agent/src/protocol/json_mode.rs` - JSON prompt future now calls `session.run(CodingAgentOperation::Prompt(...))` and extracts the Prompt outcome; imports operation types via `crate::api`; production `#[allow(deprecated)]` removed.
- `crates/pi-coding-agent/src/print_mode.rs` - Persistent and transient print branches now call `session.run(CodingAgentOperation::Prompt(...))` with exhaustive outcome extraction; both production `#[allow(deprecated)]` attributes removed; imports operation types via `crate::api`.
- `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` - New `production_json_and_print_use_canonical_operations` test enforcing canonical operations and no production deprecation suppression across JSON/print adapter source.

## Decisions Made

- Imported `CodingAgentOperation` and `CodingAgentOperationOutcome` through `crate::api` (the in-crate equivalent of `pi_coding_agent::api`) per D-16, while leaving the existing concrete type imports (`CodingAgentSession`, `PromptTurnOptions`, `PromptTurnOutcome`) from `crate::coding_session` untouched to keep the diff minimal.
- Used `unreachable!` for the impossible `CodingAgentOperationOutcome` variant in each adapter, matching the closed-enum discipline established in Phase 2 and the T-03-04 accept disposition rather than introducing a new user-visible error string.
- Grew the boundary guard file list incrementally (json_mode.rs in Task 1, then print_mode.rs in Task 3) so each adapter's parity gate is independently green before the combined gate closes.
- Scoped the guard's deprecated method list to the eight `CodingAgentSession` methods marked `#[deprecated(note = "use CodingAgentSession::run instead")]`, deliberately excluding the crate-private `fork_session` lifecycle helper and the non-deprecated `subscribe_product_events` receiver.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## Known Stubs

None. The `unreachable!` branches are exhaustive invariant handling for the closed `CodingAgentOperationOutcome` enum (T-03-04 accept), not placeholder behavior.

## Threat Flags

None. The migration introduces no new network endpoints, auth paths, file access patterns, or trust-boundary schema changes. The plan's threat register (T-03-01 through T-03-04) is addressed: canonical admission and the source guard mitigate T-03-01; preserved subscription-before-run, final drains, event assertions, and session-log parity mitigate T-03-02; retained typed error conversion and serialized fields mitigate T-03-03; the closed-enum mismatch is handled as an internal invariant per T-03-04.

## User Setup Required

None - all verification uses deterministic offline faux providers and tempfile sessions.

## Next Phase Readiness

- ADAPT-01 through ADAPT-04 are satisfied: every JSON and print prompt path uses `CodingAgentSession::run(CodingAgentOperation::Prompt)` and retains its exact external behavior.
- The JSON/print adapter boundary is the lowest-risk tier and its behavior-preservation gate is closed, unblocking RPC migration (Plans 03-02 and 03-03) per D-01/D-02/D-03.
- The narrow source guard is green and ready to be extended to RPC and interactive source in later plans without refactoring the source parser.

## Self-Check: PASSED

- All three modified files exist on disk.
- Commits `6717a87`, `036c413`, and `e2ddba7` exist in repository history.
- `cargo test -p pi-coding-agent --test json_mode --test print_mode --test session_print_mode`, `cargo test -p pi-coding-agent --test product_runtime_boundary_guards production_json_and_print_use_canonical_operations -- --exact`, `cargo check -p pi-coding-agent`, `cargo fmt --check`, and `git diff --check` all pass.
- The full `cargo test -p pi-coding-agent` suite passes with 0 failures (644 lib tests + all integration suites).
- No tracked files were deleted by any task commit, and no new production threat surface or dependency was introduced.

---
*Phase: 03-production-adapter-convergence*
*Completed: 2026-07-11*
