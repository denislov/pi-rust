---
status: awaiting_human_verify
trigger: "Authorized Phase 7 Nyquist gap-fix continuation: remove stable raw CodingAgentEvent admission, restore typed interactive usage fallback, and migrate raw adapter/bridge tests to typed ProductEvent fixtures."
created: 2026-07-13T19:27:32+08:00
updated: 2026-07-13T20:06:00+08:00
---

## Current Focus

reasoning_checkpoint:
  hypothesis: Public raw adapter methods plus the stable CodingAgentEvent re-export let callers skip the ProductEvent envelope, and duplicated raw/typed UI matchers caused the typed matcher to omit the legacy component-sum fallback.
  confirming_evidence:
    - Current lib.rs re-exports CodingAgentEvent from api, protocol/events.rs exposes pub push(&CodingAgentEvent), and interactive/event_bridge.rs exposes pub handle(&CodingAgentEvent).
    - CodeGraph and direct source show the live production paths call push_product_event, while protocol and interactive integration suites call the parallel raw methods 14 and 21 times respectively.
    - The raw UI branch calls calculate_context_tokens, while the typed branch maps usage.total_tokens == 0 directly to None.
  falsification_test: If removing raw methods/export does not make old raw tests fail to compile, or if a typed completed-message fixture with zero aggregate and non-zero components already yields Some(component sum), the hypothesis is false.
  fix_rationale: Delete the parallel raw projections, expose only full typed public-envelope projection where external adapter tests need it, retain internal ProductEvent projection for production, and share one saturating usage helper in the typed matcher.
  blind_spots: JSON-mode pre-session synthetic failure currently enters the raw protocol adapter and must retain its exact wire error without reopening a raw projection API; full workspace tests may reveal additional downstream imports.
next_action: Parent/orchestrator should review the committed diff and decide whether to archive this debug record after accepting the deterministic verification evidence.

## Symptoms

expected: Stable consumers and first-party adapters accept only ProductEvent values carrying sequence/terminal/durability metadata; typed UI projection preserves legacy context-token fallback; all behavior tests exercise the typed production seam.
actual: CodingAgentEvent remains in the stable facade, raw adapter/bridge entry points remain public, typed usage maps total_tokens == 0 to None even when usage components are non-zero, and behavior tests still enter through raw methods.
errors: Phase 7 review CR-01/CR-02/WR-01 and latest Nyquist validation report COMPAT-01/COMPAT-02 coverage gaps.
reproduction: Inspect stable api exports and public signatures; send a typed Message::Completed ProductEvent with total_tokens 0 and non-zero components through CodingEventBridge::push_product_event; inspect protocol/interactive tests for raw adapter.push/bridge.handle calls.
started: Present in the current Phase 7 implementation identified by the post-phase review and validation audit.

## Eliminated

## Evidence

- timestamp: 2026-07-13T19:27:32+08:00
  checked: CodeGraph exploration of CodingAgentEvent, adapter push, bridge handle, typed product entry points, and usage projection.
  found: CodingEventBridge::handle is public and accepts CodingAgentEvent; its typed branch maps usage.total_tokens == 0 directly to None while the raw branch calls calculate_context_tokens; CodingProtocolEventAdapter::push has numerous callers including protocol tests.
  implication: The reported raw bypass and live typed usage regression are directly visible in the current source and require boundary narrowing plus test migration.

## Resolution

root_cause:
  Stable API cleanup stopped at retained ProductEvent storage/receiver deletion but left separate public raw projection APIs and raw-first behavior tests; duplicated UI projection logic then drifted specifically in the zero-aggregate usage branch.
fix:
  Removed CodingAgentEvent from the stable facade; deleted public raw protocol/UI projection methods and the duplicate raw UI matcher; added public full-envelope typed adapter methods plus private internal ProductEvent forwarding; restored typed saturating context-token fallback; migrated protocol/UI/public API tests and added fail-closed source guards.
verification:
  Focused Phase 7 gate, interactive loop tests, full pi-coding-agent suite, full workspace tests/check, rustfmt check, and git diff check all pass after commits 376b600 and 6a7eac1.
files_changed:
  - crates/pi-coding-agent/src/lib.rs
  - crates/pi-coding-agent/src/protocol/events.rs
  - crates/pi-coding-agent/src/protocol/json_mode.rs
  - crates/pi-coding-agent/src/protocol/rpc/events.rs
  - crates/pi-coding-agent/src/interactive/event_bridge.rs
  - crates/pi-coding-agent/src/interactive/prompt_task.rs
  - crates/pi-coding-agent/tests/event_boundary_guards.rs
  - crates/pi-coding-agent/tests/interactive_event_bridge.rs
  - crates/pi-coding-agent/tests/protocol_events.rs
  - crates/pi-coding-agent/tests/public_api.rs
  - crates/pi-coding-agent/tests/support/mod.rs
