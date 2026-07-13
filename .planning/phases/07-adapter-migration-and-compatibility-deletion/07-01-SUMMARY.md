---
phase: 07-adapter-migration-and-compatibility-deletion
plan: 01
subsystem: product-event-runtime
tags: [rust, typed-events, serde, compatibility]

requires:
  - phase: 06-product-event-inventory-and-typed-contract
    provides: Exhaustive 45-event typed public contract and five root-terminal associations
provides:
  - Private ProductEvent ownership of the exhaustive typed payload
  - Typed terminal-operation classification independent of compatibility storage
  - Public projection from the stored typed payload with unchanged Serde identity fields
affects: [07-02-adapter-migration, 07-03-interactive-migration, 07-04-compatibility-deletion]

tech-stack:
  added: []
  patterns: [single conversion at ProductEvent construction, typed payload projection]

key-files:
  created: []
  modified:
    - crates/pi-coding-agent/src/coding_session/event.rs
    - crates/pi-coding-agent/src/coding_session/public_event.rs
    - crates/pi-coding-agent/tests/product_event_contract.rs

key-decisions:
  - "Construct CodingAgentProductEventKind once at the internal ProductEvent boundary while retaining the raw compatibility field for later migration plans."
  - "Classify the existing five root-terminal associations from private typed ProductEventKind variants rather than the raw compatibility event."

patterns-established:
  - "Owned typed envelope: adapters and public projection read ProductEvent::event() instead of reconstructing payloads from compatibility storage."
  - "Terminal separation: event-level terminal status remains independent from typed root-operation association."

requirements-completed: [COMPAT-01, COMPAT-02]

coverage:
  - id: D1
    description: "ProductEvent owns the exhaustive typed payload and public projection consumes that stored value."
    requirement: COMPAT-01
    verification:
      - kind: unit
        ref: "cargo test -p pi-coding-agent --lib public_event --quiet"
        status: pass
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test product_event_contract --quiet"
        status: pass
    human_judgment: false
  - id: D2
    description: "Sequence, durability, terminal separation, and transitional Serde identity fields remain compatible with Phase 6."
    requirement: COMPAT-02
    verification:
      - kind: integration
        ref: "crates/pi-coding-agent/tests/product_event_contract.rs#public_receiver_preserves_typed_order_and_metadata"
        status: pass
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test event_boundary_guards --quiet"
        status: pass
      - kind: other
        ref: "cargo test --workspace --quiet"
        status: pass
    human_judgment: false

duration: 9 min
completed: 2026-07-13
status: complete
---

# Phase 07 Plan 01: Typed Product Event Ownership Summary

**Private product events now own the Phase 6 typed payload, and public receivers project it without consulting raw compatibility storage.**

## Performance

- **Duration:** 9 min
- **Started:** 2026-07-13T09:05:00Z
- **Completed:** 2026-07-13T09:13:58Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Added an owned `CodingAgentProductEventKind` payload and private typed accessor to every `ProductEvent` constructed from the exhaustive 45-variant conversion.
- Switched root-terminal classification and public projection away from `compatibility_event()` while preserving exactly the existing five root associations.
- Strengthened the real-receiver contract with explicit assertions for transitional `family`/`kind`, typed payload, operation identity, durability, and terminal serialization.

## Task Commits

| Task | Name | Commit | Files |
| --- | --- | --- | --- |
| 1 | Store the exhaustive typed payload in ProductEvent | `94155cb` | `event.rs`, `public_event.rs` |
| 2 | Lock typed envelope metadata and serialization behavior | `b62ae23` | `product_event_contract.rs` |

## Files Created/Modified

- `crates/pi-coding-agent/src/coding_session/event.rs` - Stores the typed payload and derives terminal associations from typed variants.
- `crates/pi-coding-agent/src/coding_session/public_event.rs` - Projects the already-owned payload into the public envelope.
- `crates/pi-coding-agent/tests/product_event_contract.rs` - Locks typed and transitional serialized event identity through real receivers.

## Decisions Made

- Kept the raw compatibility event and both receivers intact because later Phase 7 plans still require the transitional path.
- Used the existing private classification enum for root-terminal matching, avoiding string parsing and preserving the Phase 6 association boundary.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None. A proposed receiver assertion for `SessionOpened` was discarded before commit because the fixture subscribes after session creation; the committed test remains behavior-based and scoped to events observable through that receiver.

## Verification

- `cargo fmt --check` - pass
- `cargo test -p pi-coding-agent --lib public_event --quiet` - pass (3 tests)
- `cargo test -p pi-coding-agent --test product_event_contract --quiet` - pass
- `cargo test -p pi-coding-agent --test event_boundary_guards --quiet` - pass (19 tests)
- `cargo test --workspace --quiet` - pass
- `cargo check --workspace` - pass with existing dead-code warnings
- `git diff --check` - pass

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

The typed payload accessor is ready for protocol and interactive adapter migration. Compatibility storage and receivers remain deliberately available for the later deletion plans.

## Self-Check

- [x] All plan tasks completed and committed atomically.
- [x] All required and workspace verification commands passed.
- [x] Summary references both task commits and all three modified files.
- [x] `docs/next stage.md` was not modified or committed.

---
*Phase: 07-adapter-migration-and-compatibility-deletion*
*Completed: 2026-07-13*
