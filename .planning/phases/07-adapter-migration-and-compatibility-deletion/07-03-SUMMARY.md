---
phase: 07-adapter-migration-and-compatibility-deletion
plan: 03
subsystem: interactive-adapter
tags: [rust, typed-events, tui, compatibility]

requires:
  - phase: 07-adapter-migration-and-compatibility-deletion
    provides: Owned typed ProductEvent payload and typed protocol adapters from Plans 07-01 and 07-02
provides:
  - Interactive UI projection directly matching CodingAgentProductEventKind payload families
  - Typed interactive loop assertions for PartialCommit, fork continuity, profile navigation, and recovery
affects: [07-04-compatibility-deletion]

tech-stack:
  added: []
  patterns:
    - "Interactive bridge consumes ProductEvent::event() and keeps legacy CodingAgentEvent handling only for isolated bridge fixtures."
    - "Navigation and recovery tests assert typed payload identity without resubscribing receivers."

key-files:
  created: []
  modified:
    - crates/pi-coding-agent/src/interactive/event_bridge.rs
    - crates/pi-coding-agent/src/interactive/loop.rs

key-decisions:
  - "Preserved all existing UiEvent text, delegation identifiers, tool argument parsing, usage deltas, compaction reset, and no-op variants while replacing the live projection source with typed payloads."
  - "Retained the original pre-fork receiver in the continuity test and matched SessionOpened/Profile changes through typed event variants."

requirements-completed: [COMPAT-01]

coverage:
  - id: D1
    description: "Interactive bridge projects typed message, tool, delegation, compaction, self-healing, and recovery payloads with existing UiEvent shapes."
    requirement: COMPAT-01
    verification:
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test interactive_event_bridge --quiet"
        status: pass
  - id: D2
    description: "Interactive loop retains exact PartialCommit attribution, failed-fork receiver continuity, profile change, and cursor behavior."
    requirement: COMPAT-01
    verification:
      - kind: unit
        ref: "cargo test -p pi-coding-agent --lib interactive::r#loop::tests --quiet"
        status: pass
      - kind: integration
        ref: "cargo test -p pi-coding-agent --test interactive_sessions --quiet"
        status: pass

duration: 8 min
completed: 2026-07-13
status: complete
---

# Phase 07 Plan 03: Interactive Typed Event Projection Summary

**Interactive UI adapters now consume the owned typed product-event payload while preserving visible transcript and recovery behavior.**

## Accomplishments

- Added exhaustive typed-family matching to `CodingEventBridge::push_product_event` for assistant usage, tools, delegation lifecycle, compaction, self-healing notices, prompt failures, and operation recovery.
- Preserved legacy no-op behavior for session, profile, provider, lifecycle, diagnostic, capability, and write events; no new root-terminal associations were introduced.
- Migrated loop assertions for PartialCommit identity, failed-fork continuity, and profile navigation to `ProductEvent::event()` typed variants while retaining the original receiver.
- Confirmed the interactive production tree has no `compatibility_event()` consumers.

## Task Commits

| Task | Name | Commit | Files |
| --- | --- | --- | --- |
| 1-2 | Typed interactive bridge and loop assertions | `6db8eb2` | `event_bridge.rs`, `loop.rs` |

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

The shared worktree contained unrelated modifications to `.planning/STATE.md` and an untracked `docs/next stage.md`; these files were left untouched and excluded from the plan commit.

## Verification

- `cargo test -p pi-coding-agent --test interactive_event_bridge --quiet` - pass (11 tests)
- `cargo test -p pi-coding-agent --lib interactive::r#loop::tests --quiet` - pass (9 tests)
- `cargo test -p pi-coding-agent --test interactive_sessions --quiet` - pass (30 tests)
- `cargo fmt --check` - pass
- `git diff --check` - pass
- `rg -n "compatibility_event\\(" crates/pi-coding-agent/src/interactive` - no matches

## Self-Check: PASSED

- [x] Typed bridge and loop behavior are implemented and committed.
- [x] Required focused tests and formatting/diff checks pass.
- [x] No protocol files, `.planning/STATE.md`, or `docs/next stage.md` were modified by this plan.
