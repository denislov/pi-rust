---
phase: 06-product-event-inventory-and-typed-contract
plan: 01
subsystem: api
tags: [rust, serde, product-events, public-api, event-projection]

requires:
  - phase: 05-boundary-enforcement-and-stage-9-closure
    provides: Canonical CodingAgentSession operation facade and compiler-enforced API boundary
provides:
  - Stable typed public product-event hierarchy for all 11 families and 45 variants
  - Exhaustive private projection with sequence, operation, terminal, and durability metadata
  - Typed public receiver and curated pi_coding_agent::api exports
affects: [phase-07-adapter-migration, phase-08-client-replay, phase-09-association-closure]

tech-stack:
  added: []
  patterns: [exhaustive private event projection, family-oriented public payload enums, independent event/root terminal metadata]

key-files:
  created:
    - crates/pi-coding-agent/src/coding_session/public_event.rs
  modified:
    - crates/pi-coding-agent/src/coding_session/public_projection.rs
    - crates/pi-coding-agent/src/coding_session/mod.rs
    - crates/pi-coding-agent/src/lib.rs
    - crates/pi-coding-agent/tests/public_api.rs

key-decisions:
  - "Keep event-level terminal status independent from the five existing root-operation terminal associations."
  - "Retain transitional family/kind strings with explicit legacy-name mapping while typed enums and snake_case Serde become authoritative."
  - "Copy errors, usage, profile identifiers, diagnostics, and edit data into stable owned public payloads instead of exposing compatibility events."

patterns-established:
  - "Typed projection: match every CodingAgentEvent variant without a wildcard and copy only curated public fields."
  - "Compatibility identity: transitional strings use explicit names, never Debug formatting."

requirements-completed: [EVENT-01, EVENT-02]

coverage:
  - id: D1
    description: "Public callers can match all 11 product-event families and 45 current variants through typed payload enums."
    requirement: "EVENT-01"
    verification:
      - kind: unit
        ref: "cargo test -p pi-coding-agent --lib public_event --quiet"
        status: pass
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test public_api --quiet"
        status: pass
    human_judgment: false
  - id: D2
    description: "Public events preserve sequence and expose operation identity, event terminal status, root terminal association, and durability independently."
    requirement: "EVENT-02"
    verification:
      - kind: unit
        ref: "coding_session::public_event::tests"
        status: pass
    human_judgment: false
  - id: D3
    description: "The public receiver projects each retained/broadcast ProductEvent once while compatibility consumers continue to compile."
    requirement: "EVENT-02"
    verification:
      - kind: other
        ref: "cargo check -p pi-coding-agent --all-targets"
        status: pass
    human_judgment: false

duration: 18min
completed: 2026-07-13
status: complete
---

# Phase 6 Plan 01: Typed Product Event Contract Summary

**A Serde-capable, exhaustive typed product-event boundary now projects all 45 compatibility events without exposing internal runtime ownership.**

## Performance

- **Duration:** 18 min
- **Started:** 2026-07-13T13:21:51+08:00
- **Completed:** 2026-07-13T13:39:13+08:00
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added family-oriented public payload enums covering all 11 internal families and 45 emitted variants.
- Preserved sequence, optional operation identity, event terminal status, root-operation terminal association, and durability as separate typed dimensions.
- Wired the public receiver and stable facade to the exhaustive private projection while keeping transitional string consumers source-compatible.
- Closed the downstream API boundary with imports, 11-family matching, type-name checks, and deterministic Serde assertions.

## Task Commits

Each task used explicit RED and GREEN commits:

1. **Task 1: Define the complete typed event and payload boundary** - `10488cb` (RED), `650283b` (GREEN)
2. **Task 2: Wire typed projection through the public receiver and stable facade** - `85ccd9e` (RED), `6d6f46b` (GREEN)
3. **Task 3: Close the public API signature and privacy boundary** - `8255a64` (RED), `c29ed81` (GREEN)

## Files Created/Modified

- `crates/pi-coding-agent/src/coding_session/public_event.rs` - Public event metadata, 11 family payload enums, exhaustive conversion, and focused tests.
- `crates/pi-coding-agent/src/coding_session/public_projection.rs` - Receiver projection into the authoritative typed event.
- `crates/pi-coding-agent/src/coding_session/mod.rs` - Curated coding-session re-exports.
- `crates/pi-coding-agent/src/lib.rs` - Stable `pi_coding_agent::api` facade exports.
- `crates/pi-coding-agent/tests/public_api.rs` - Downstream signature, matching, and serialization closure.

## Decisions Made

- Tool, message, delegation, and session-write completion remains event-terminal without being mislabeled as root-operation completion.
- Transitional `family` and `kind` fields retain their prior CamelCase values until Phase 7 migrates consumers, but explicit mapping replaces `Debug` identity.
- Public payloads own selected values; errors are projected to stable code/message values and no internal service, Flow node, `ProductEvent`, or `CodingAgentEvent` is exported.

## Deviations from Plan

None - plan executed within the specified event-contract and receiver boundaries.

## Issues Encountered

- Initial receiver verification exposed existing CamelCase string assertions. Explicit legacy-name mapping preserved those assertions without restoring Debug-derived identity.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plan 06-02 can freeze the inventory and operation/outcome association matrix against the typed contract.
- Phase 7 can migrate adapters from transitional strings and compatibility-event unwrapping to the new family payload enums.
- No blockers.

## Self-Check: PASSED

- Created public event module and summary exist.
- All six RED/GREEN task commits exist.
- Focused unit tests, public API integration tests, all-target check, formatting, and diff checks pass.

---
*Phase: 06-product-event-inventory-and-typed-contract*
*Completed: 2026-07-13*
