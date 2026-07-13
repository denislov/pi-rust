---
phase: 06-product-event-inventory-and-typed-contract
verified: 2026-07-13T06:08:00Z
status: gaps_found
score: 9/10 must-haves verified
behavior_unverified: 0
overrides_applied: 0
gaps:
  - truth: "The checked inventory freezes all 45 current event variants and their typed family/kind/payload mappings against silent drift."
    status: partial
    reason: "The fixture constructs 45 variants and checks total/family counts, but it does not assert the expected public kind or representative payload for each item. The source guard checks documented families, not documented event variants. After adding an internal event and its required exhaustive conversion arm, the fixture and document can remain stale while all current guards pass."
    artifacts:
      - path: "crates/pi-coding-agent/src/coding_session/public_event.rs"
        issue: "exhaustive_inventory_covers_all_current_variants validates 45, family counts, sequence, and selected metadata only; it has no 45-entry expected kind/payload table."
      - path: "crates/pi-coding-agent/tests/event_boundary_guards.rs"
        issue: "typed_public_event_boundary_is_fail_closed requires 11 family rows but does not compare the 45 event variants to docs/product-event-contract.md."
    missing:
      - "Add an explicit 45-entry expected public family/kind inventory and compare every projected fixture item in source order."
      - "Assert representative correlation/payload fields per variant or use typed per-variant matches that fail on a wrong conversion target."
      - "Extend the documentation guard to compare the full public event variant inventory, not only family names."
---

# Phase 6: Product Event Inventory and Typed Contract Verification Report

**Phase Goal:** Freeze the emitted event inventory and implement the stable typed public product-event model, including identity, durability, terminal semantics, and payload boundaries.
**Verified:** 2026-07-13T06:08:00Z
**Status:** gaps_found
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | Every current internal event has an exhaustive typed public conversion with an owned payload. | VERIFIED | `public_event.rs:828` exhaustively matches all `CodingAgentEvent` variants without a wildcard; public family payload enums are defined at `public_event.rs:149-463`. |
| 2 | Public callers identify family and kind through stable typed enums instead of parsing strings. | VERIFIED | `CodingAgentProductEvent::event()` and `family()` expose `CodingAgentProductEventKind`/`Family`; `tests/public_api.rs` imports and matches all 11 family branches. Transitional strings are deprecated only. |
| 3 | Sequence, optional operation identity, terminal status, root terminal association, and durability remain independent explicit dimensions. | VERIFIED | Accessors at `public_event.rs:685-711`; conversion at lines 715-734; focused unit and public integration tests pass. |
| 4 | Public payloads do not expose internal events, services, session logs, or Flow nodes. | VERIFIED | Public declarations contain no `CodingAgentEvent`; internal conversion is private; boundary guard passes. Errors, usage, profiles, and edit details are copied into public-owned mirrors. |
| 5 | EventService remains the sole ordering owner and the public receiver performs one pure projection per receive. | VERIFIED | `public_projection.rs:53-64` maps each `ProductEventReceiver` result exactly once through `from_internal`; no sequence assignment occurs in the projection. Real persistent/non-persistent prompt test proves contiguous monotonic sequence. |
| 6 | Existing compatibility consumers continue to compile and transitional string spelling is not Debug-derived. | VERIFIED | Full workspace test/check pass. Explicit legacy-name functions supply transitional fields; no `format!("{:?}", ...)` occurs in public projection code. |
| 7 | The documented operation/outcome matrix covers every public request and outcome exactly once while preserving five root-terminal mappings. | VERIFIED | `operation_outcome_documentation_matches_public_enums_exactly` parses both Rust enums and checks unique set equality against the 15-row marked matrix. |
| 8 | Offline tests cover public serialization, ordering, pending/committed/skipped durability, terminal separation, and valid absence. | VERIFIED | `product_event_contract` drives real faux-provider prompt flows; co-located tests cover operation-less and recovery cases. All focused tests pass without network access. |
| 9 | Source boundaries reject Debug identity, public compatibility payloads, conversion wildcards, omitted families, and operation/outcome document drift. | VERIFIED | `typed_public_event_boundary_is_fail_closed` and matrix drift test pass in `event_boundary_guards` (18/18 target tests). |
| 10 | The checked inventory individually freezes all 45 public family/kind/payload mappings. | FAILED | The fixture constructs 45 values but only checks total, family distribution, sequence, and selected metadata. It never compares all 45 `kind_name()` values or per-variant typed payloads; the doc guard checks only 11 families. |

**Score:** 9/10 truths verified (0 behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/pi-coding-agent/src/coding_session/public_event.rs` | Typed public hierarchy and exhaustive private conversion | VERIFIED | Substantive implementation, exported through the stable facade, invoked by the public receiver, and covered by unit tests. Inventory fixture evidence is partial as described in the gap. |
| `crates/pi-coding-agent/src/coding_session/public_projection.rs` | Typed public receiver projection | VERIFIED | `recv` and `try_recv` each map one internal event through `from_internal`. |
| `crates/pi-coding-agent/src/lib.rs` | Curated stable API exports | VERIFIED | All event families, metadata, payload, durability, and terminal types are re-exported through `pi_coding_agent::api`; public API tests pass. |
| `crates/pi-coding-agent/tests/public_api.rs` | Downstream signature closure | VERIFIED | Imports public types only and matches all 11 family branches. |
| `crates/pi-coding-agent/tests/product_event_contract.rs` | External receiver/Serde/order contract | VERIFIED | Real persistent and non-persistent operations verify monotonic sequence and durability/terminal distinctions. |
| `crates/pi-coding-agent/tests/event_boundary_guards.rs` | Fail-closed identity/privacy/inventory guards | PARTIAL | Identity, privacy, wildcard, family, and operation/outcome checks are substantive; full 45-event documentation drift is not enforced. |
| `docs/product-event-contract.md` | Current event and operation/outcome contract | VERIFIED | Documents 11 families, 45 named variants, envelope semantics, five root mappings, absence, sequence authority, durability, and `PartialCommit`. |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| Internal `ProductEvent` | Public `CodingAgentProductEvent` | private exhaustive `from_internal` conversion | VERIFIED | Receiver uses the conversion exactly once per result. |
| `coding_session` exports | `pi_coding_agent::api` | curated `pub use` lists | VERIFIED | Downstream integration tests compile without implementation-module imports. |
| 45-event source inventory | Checked fixture/document inventory | fixture counts and source guards | PARTIAL | Current 45 inputs exist, but exact kind/payload and documented variant set equality are not wired. |
| Public operation/outcome enums | Documentation matrix | enum parser and unique set equality | VERIFIED | Additions, renames, omissions, and duplicate rows fail the guard. |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Typed 45-event projection unit target | `cargo test -p pi-coding-agent --lib public_event` | 3 passed | PASS |
| Stable downstream facade | `cargo test -p pi-coding-agent --test public_api` | 23 passed | PASS |
| Real receiver order/Serde/durability | `cargo test -p pi-coding-agent --test product_event_contract` | 1 passed | PASS |
| Boundary and documentation guards | `cargo test -p pi-coding-agent --test event_boundary_guards` | 18 passed | PASS |
| Full workspace regression | `cargo test --workspace` | All workspace targets passed; intentional performance baseline remained ignored | PASS |
| Workspace compile boundary | `cargo check --workspace` | Passed with pre-existing dead-code/deprecation warnings | PASS |
| Formatting and diff hygiene | `cargo fmt --check`; `git diff --check` | Passed | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| EVENT-01 | 06-01, 06-02 | Typed event inspection rather than string-only identity | SATISFIED | Typed hierarchy/accessors, stable facade tests, no-Debug guard. |
| EVENT-02 | 06-01, 06-02 | Sequence, identity, terminal, durability, and payload semantics | SATISFIED | Public metadata projection plus real receiver/Serde/durability tests. |
| EVENT-03 | 06-02 | Complete emitted inventory and operation terminal/outcome documentation | PARTIAL | Current inventory and matrix are documented correctly, but event-variant fixture/document drift is not fully fail-closed. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---|---|---|---|
| `crates/pi-coding-agent/src/coding_session/public_event.rs` | 1842 | Inventory test asserts count/family distribution rather than every typed variant | BLOCKER | A wrong conversion target or newly added converted event can leave the frozen inventory evidence stale. |
| `.planning/phases/06-product-event-inventory-and-typed-contract/06-VALIDATION.md` | 1 | Validation metadata remains `status: draft`, `wave_0_complete: false`, task rows `pending` | INFO | Execution evidence exists elsewhere, but the validation contract itself was not refreshed after completion. |

### Human Verification Required

None. All intended Phase 6 behavior is deterministic and offline; the remaining issue requires stronger automated inventory assertions rather than manual approval.

### Gaps Summary

The production projection appears correct and all focused/workspace tests pass. The remaining gap is evidence strength at the phase's central freeze boundary. Plan 06-02 promised that all 45 variants would be individually asserted and that inventory drift would fail closed. The current fixture constructs all variants but does not bind each input to an expected typed kind/payload, and the document guard checks families only. Add a 45-entry expected inventory with typed/per-payload assertions and compare the full documented variant set before treating EVENT-03 and Phase 6 as fully verified.

Git status after verification contains only the user-owned untracked `docs/next stage.md` plus this verification report; no runtime file was modified by verification.

---

_Verified: 2026-07-13T06:08:00Z_
_Verifier: the agent (gsd-verifier)_
