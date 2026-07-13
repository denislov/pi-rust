---
phase: 06-product-event-inventory-and-typed-contract
plan: 03
subsystem: testing
tags: [rust, product-events, contract-tests, source-guards, gap-closure]

requires:
  - phase: 06-product-event-inventory-and-typed-contract
    provides: 45-variant conversion fixture and event contract documentation from Plan 06-02
provides:
  - Exact public family/kind identity and representative typed payload assertions for all 45 events
  - Four-layer drift guard spanning internal enum, executable fixture, expected inventory, and documentation
affects: [phase-07-adapter-migration]

tech-stack:
  added: []
  patterns: [source-fixture-document set equality, marked inventory regions, typed payload coverage]

key-files:
  modified:
    - crates/pi-coding-agent/src/coding_session/public_event.rs
    - crates/pi-coding-agent/tests/event_boundary_guards.rs
    - docs/product-event-contract.md

key-decisions:
  - "Keep the executable inventory in source order while compare-only drift guards use set equality against the enum declaration, whose declaration order is independent."
  - "Keep the authoritative 45-row table in the contract document and require exact identity equality with the executable inventory."

patterns-established:
  - "Every current public event has an explicit family, kind, and representative typed payload assertion."
  - "Marked source regions make fixture and documentation guards resilient to unrelated test code."

requirements-completed: [EVENT-03]

coverage:
  - id: D1
    description: "Every projected event is checked against exact family/kind identity and a typed payload assertion."
    requirement: "EVENT-03"
    verification:
      - kind: unit
        ref: "coding_session::public_event::tests::exhaustive_inventory_covers_all_current_variants"
        status: pass
    human_judgment: false
  - id: D2
    description: "Internal enum, fixture constructors, executable expected inventory, and documented 45-row table remain complete and equal."
    requirement: "EVENT-03"
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/event_boundary_guards.rs#full_event_inventory_is_source_fixture_and_document_complete"
        status: pass
    human_judgment: false

duration: 8min
completed: 2026-07-13
status: complete
---

# Phase 6 Plan 03: Product Event Inventory Gap Closure Summary

**The verifier gap is closed: the 45-event public contract now has exact identity, payload, and four-layer drift evidence.**

## Accomplishments

- Added an explicit 45-row executable inventory binding each internal event variant to its typed public family and snake-case kind.
- Added representative typed payload assertions for every public event variant, including identifiers, terminal data, durability data, delegation context, self-healing details, and capability metadata.
- Added an authoritative marked 45-row table to `docs/product-event-contract.md`.
- Added a fail-closed guard comparing the internal `CodingAgentEvent` enum, fixture constructors, executable inventory, and documented inventory.

## Task Commits

1. **Task 1: Bind all public event identities and typed payloads** - `04366f6` (RED), `7704a01` (GREEN)
2. **Task 2: Close source/fixture/expected/document inventory drift** - `235a388` (RED), `71a4c3c` (GREEN)

## Verification

- `cargo fmt --check` passed.
- `cargo test -p pi-coding-agent --test event_boundary_guards --quiet` passed (19 tests).
- `cargo test -p pi-coding-agent --test product_event_contract --quiet` passed.
- `cargo test -p pi-coding-agent --lib public_event::tests::exhaustive_inventory_covers_all_current_variants --quiet` passed.
- `cargo check -p pi-coding-agent --all-targets` passed.
- `git diff --check` passed.

## Deviations

None. This plan stayed within the Phase 6 event-contract verification boundary and did not change runtime behavior.

## Next Phase Readiness

EVENT-03 now has the missing exact-inventory and drift-closure evidence. The Phase 6 verifier should be rerun before advancing to Phase 7.

## Self-Check: PASSED

- All planned files and task commits exist.
- Focused unit, contract, boundary, formatting, build, and whitespace checks passed.

---
*Phase: 06-product-event-inventory-and-typed-contract*
*Completed: 2026-07-13*
