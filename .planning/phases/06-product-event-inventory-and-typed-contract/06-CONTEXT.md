# Phase 6: Product Event Inventory and Typed Contract - Context

**Created:** 2026-07-13
**Source:** v1.1 milestone kickoff and codebase event-boundary audit

## Decisions

- **D-01:** Derive the public event contract from the existing internal `ProductEventKind`, family enums, durability model, operation identity, and terminal classification; do not invent an unrelated event taxonomy.
- **D-02:** Public product events must expose typed kind information instead of requiring consumers to parse string-only family/kind fields.
- **D-03:** Preserve sequence identity, operation identity where available, terminal status, durability, and explicit semantics for events that lack one of those fields.
- **D-04:** Keep `CodingAgentEvent` as an internal compatibility source only while the public typed projection is established; Phase 6 must define the replacement payload boundary rather than silently exposing the legacy enum.
- **D-05:** Preserve event ordering, replay compatibility, `PartialCommit` attribution, and adapter-visible behavior while changing the public projection.
- **D-06:** Use existing Rust/Serde/test patterns and add no external runtime dependency for this phase.

## Discretion

- Choose the exact public enum/module names and payload wrapper layout, provided they are exported through `pi_coding_agent::api` and keep implementation details private.
- Decide which payload variants are typed structs versus intentionally metadata-only variants based on the current emitter inventory.
- Choose whether to keep a narrowly scoped internal conversion helper during the transition, as long as no compatibility event is exposed through the new public contract.

## Deferred Ideas

- Migrating RPC and interactive consumers away from compatibility event unwrapping (Phase 7).
- Public reconnect/replay/client lifecycle APIs (Phase 8).
- Detach/close/shutdown and full operation/outcome/terminal association closure (Phase 9).
- Introducing a separately named `CodingAgentRuntime` owner type.
- New workflows, Lua Flow expansion, and `pi-web-ui` construction.

## Source Anchors

- `crates/pi-coding-agent/src/coding_session/event.rs`
- `crates/pi-coding-agent/src/coding_session/public_projection.rs`
- `crates/pi-coding-agent/src/coding_session/event_service.rs`
- `crates/pi-coding-agent/tests/event_boundary_guards.rs`
- `crates/pi-coding-agent/tests/protocol_events.rs`
- `crates/pi-coding-agent/tests/public_api.rs`

## Phase Boundary

Phase 6 freezes the inventory and implements the public typed event model. It does not delete compatibility consumers or build client lifecycle APIs; those consumers must have a stable typed contract to target first.
