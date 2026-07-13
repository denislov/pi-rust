# Phase 7: Adapter Migration and Compatibility Deletion - Research

**Researched:** 2026-07-13
**Domain:** Rust product-event adapter migration and legacy event-subscription removal
**Confidence:** HIGH

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| COMPAT-01 | RPC, interactive, JSON/print, and first-party tests consume typed product events without production calls to `compatibility_event()`. | The current adapter call sites and typed `CodingAgentProductEvent`/`ProductEvent` projection are inventoried below; the plan must migrate each production projection and test assertion to typed payload access. |
| COMPAT-02 | Compatibility event receivers, legacy subscriptions, and compatibility storage are deleted or narrowed to test-only migration fixtures after behavior-preserving coverage passes. | The legacy broadcast sender, `CodingAgentEventReceiver`, `CodingAgentSession::subscribe`, and `ProductEvent.compatibility_event` storage are identified with deletion order and guard updates. |
</phase_requirements>

## Summary

Phase 6 completed the typed public event contract and intentionally left a compatibility source in place so adapters could migrate against a stable target. The remaining legacy path is concrete: `ProductEvent` stores a `CodingAgentEvent` in `compatibility_event`; `EventService` publishes a second compatibility broadcast through `sender`; `CodingAgentSession::subscribe()` and `EventService::subscribe()` expose the old receiver; and production projections unwrap the stored value in the interactive bridge, protocol adapter, and typed public projection. [VERIFIED: `crates/pi-coding-agent/src/coding_session/event.rs:310-395`; `event_service.rs:27-35,174-185,714-725`; `interactive/event_bridge.rs:149-158`; `protocol/events.rs:35-40`; `public_event.rs:719-731`]

The migration should be staged by consumer boundary, preserving `EventService` sequence assignment, retained product-event replay, adapter state machines, overflow handling, and `PartialCommit` assertions. The typed public hierarchy already owns payload fields for all 45 event variants, while the internal `ProductEventKind` only carries classification. Therefore the safe implementation shape is to make one typed product-event representation the adapter input, move any required payload conversion into the event construction/projection boundary, and only then remove compatibility storage and the old broadcast. [VERIFIED: `06-VERIFICATION.md` goal evidence; `docs/product-event-contract.md` 45-row inventory; `event_service.rs:174-185`]

**Primary recommendation:** migrate protocol/JSON/print and interactive projections first, migrate co-located and integration tests to typed event access, add fail-closed source guards for zero production compatibility consumers, then delete the compatibility field, sender/receiver, and deprecated session subscription in that order.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Event payload projection | `pi-coding-agent` product runtime (`event.rs`/`public_event.rs`) | `EventService` | Product semantics and exhaustive event mapping belong in the product crate; lower crates remain product-neutral. [VERIFIED: `AGENTS.md` architecture; `public_event.rs`] |
| RPC/JSON/print wire projection | `pi-coding-agent` protocol adapters | typed product-event contract | Protocol adapters should consume typed product events and retain existing wire output/state machines. [VERIFIED: `protocol/events.rs`; `protocol/json_mode.rs`; `protocol/rpc/events.rs`] |
| Interactive UI projection | `pi-coding-agent` interactive adapter | typed product-event contract | UI projection owns presentation-only conversion and must preserve usage/delegation/error output. [VERIFIED: `interactive/event_bridge.rs`; `interactive/loop.rs`] |
| Event ordering/replay | `EventService` | RPC/interactive receivers | `EventService::emit` assigns sequence before retention and broadcast; adapters must not generate or reorder sequence values. [VERIFIED: `event_service.rs:111-185`] |
| Compatibility deletion guards | `pi-coding-agent` tests | source-audit helpers | Boundary tests already scan first-party roots and should be inverted from “deprecated or test-gated” to “absent except explicit fixtures.” [VERIFIED: `tests/event_boundary_guards.rs:513-566`] |

## Standard Stack

### Core

| Library/tool | Version | Purpose | Why Standard |
|---|---|---|---|
| Rust | Edition 2024 | Exhaustive typed event matching and compiler-guided deletion | The existing workspace language; exhaustive matches make missed payload variants compile failures. [VERIFIED: workspace `Cargo.toml`; Phase 6 event contract tests] |
| Serde / `serde_json` | Existing workspace dependencies | Preserve typed product-event and protocol serialization | Already owns public event/wire serialization; no new encoding layer is needed. [VERIFIED: `crates/pi-coding-agent/Cargo.toml`; `public_event.rs`; `protocol/jsonl.rs`] |
| Tokio broadcast/mpsc | Existing workspace dependency | Product-event subscription and bounded RPC handoff | Existing `EventService` and RPC queues use these transports; this phase removes only the duplicate legacy broadcast. [VERIFIED: `event_service.rs`; `protocol/rpc/event_queue.rs`] |
| Rust test harness | Installed toolchain | Unit, integration, source-guard, and offline behavior tests | Existing test organization covers adapter, contract, API, and boundary behavior. [VERIFIED: `crates/pi-coding-agent/tests/`; co-located test modules] |

### Supporting

No new supporting library, feature flag, or external service is required. [VERIFIED: Phase 7 scope and current Cargo manifests]

## Package Legitimacy Audit

Not applicable: Phase 7 installs no external package or runtime dependency.

## User Constraints (from AGENTS.md)

- Communicate with the user in Chinese; technical documents may be English.
- Use CodeGraph before grep/find or direct source reading when `.codegraph/` exists.
- Preserve dependency direction `pi-coding-agent -> pi-agent-core -> pi-ai` and `pi-coding-agent -> pi-tui`.
- Keep public contracts under `pi_coding_agent::api`; internal operations, services, receivers, and Flow nodes remain private.
- Preserve JSON output, print output, RPC responses, interactive projections, event ordering, replay, control, navigation, typed session facts, and `PartialCommit` attribution.
- Use deterministic offline fixtures and retain existing behavior assertions; migration is not permission to replace behavior checks with compile-only checks.
- Production adapters and tests migrate before broad workflow/compatibility methods are deleted.
- Required verification includes `cargo fmt --check`, focused `pi-coding-agent` tests, `cargo test --workspace`, `cargo check --workspace`, source audits, and `git diff --check`.
- Use `apply_patch` for manual edits, default to ASCII, and keep comments concise.
- Phase 8 owns public client lifecycle/replay/scoped control; Phase 9 owns COMPAT-03 ordering/association closure and guard hardening. Do not pull those changes into Phase 7.

## Current Compatibility Surface

| Legacy item | Current role | Migration/deletion action |
|---|---|---|
| `ProductEvent.compatibility_event: CodingAgentEvent` | Payload source for public event conversion, protocol adapter, and interactive bridge; also used by internal tests. [VERIFIED: `event.rs:319,392-395`; `public_event.rs:721`] | Replace all production reads with typed payload access. Remove field only after conversion and adapter tests pass; keep a test-only constructor/fixture only if an exhaustive migration assertion still requires raw source construction. |
| `EventService.sender: broadcast::Sender<CodingAgentEvent>` | Legacy live broadcast. [VERIFIED: `event_service.rs:29-31,181-183`] | Stop cloning/sending compatibility events, then delete sender and its capacity/setup state once no receiver remains. |
| `EventService::subscribe()` | Deprecated compatibility receiver factory. [VERIFIED: `event_service.rs:714-719`] | Delete after all session/tests migrate; do not retain a production deprecation shim. |
| `CodingAgentEventReceiver` and `CodingAgentSession::subscribe()` | Old public/root receiver path used by co-located tests and `public_api.rs`. [VERIFIED: `coding_session/mod.rs:58,392-400`; `tests/public_api.rs:712,1012`] | Migrate tests to `subscribe_product_events_public()` or typed internal receiver, then remove type/export and method. |
| `ProductEventReceiver` | Internal typed receiver used by projections and replay tests. [VERIFIED: `event_service.rs:987-1007`; `public_projection.rs`] | Retain as the internal transport while adapters migrate; it is not the legacy path targeted for deletion. |
| `#[allow(deprecated)]` around compatibility calls | Suppresses old API warnings in public projection and tests. [VERIFIED: `public_event.rs:718-721`; `coding_session/mod.rs:394-396`] | Remove local suppressions with the old symbols, and add a source guard rejecting new compatibility suppressions in production roots. |

## Consumer Inventory and Migration Order

1. **Protocol and JSON/print adapters.** `CodingProtocolEventAdapter::push_product_event` currently calls `event.compatibility_event()` and then the existing `push(&CodingAgentEvent)` matcher. RPC delegates to that adapter through `RpcCodingEventAdapter`; JSON mode calls the same product-event push path. [VERIFIED: `protocol/events.rs:35-40`; `protocol/rpc/events.rs:16-19`; `protocol/json_mode.rs:224-237`] Preserve protocol event ordering, provider/model updates, message/tool state, and final `AgentEnd` output while switching the matcher to typed family/payload variants.
2. **Interactive bridge and loop.** `CodingEventBridge::push_product_event` unwraps the compatibility source. Several co-located loop tests inspect `compatibility_event()` for partial-commit prompt failure, startup recovery, session-opened, and profile-change behavior. [VERIFIED: `interactive/event_bridge.rs:149-158`; `interactive/loop.rs:2895-3048`] Keep these checks as typed kind/payload assertions; preserve snapshot/replay cursor checks and do not change Phase 8/9 lifecycle semantics.
3. **Public projection.** `CodingAgentProductEvent::from_internal` converts metadata and then calls `CodingAgentProductEventKind::from(source.compatibility_event())`. [VERIFIED: `public_event.rs:714-731`] Make this conversion consume the typed payload owned by the internal event; legacy `family`/`kind` strings remain transitional only until this phase's output compatibility assertions pass.
4. **First-party tests and fixtures.** `coding_session/mod.rs` has eight old `session.subscribe()` test sites and four `compatibility_event()` assertions; `tests/public_api.rs` has two old subscription calls; event and event-service unit tests assert raw compatibility storage. [VERIFIED: `rg` inventory on 2026-07-13] Migrate behavior assertions rather than deleting them. Use `CodingAgentProductEvent` accessors for external tests and internal typed `ProductEvent` accessors for co-located tests.
5. **Boundary guards.** Existing guards currently accept a deprecated or test-gated compatibility subscription. [VERIFIED: `tests/event_boundary_guards.rs:513-566`] Update them to scan production `src/protocol`, `src/interactive`, and non-fixture tests for `.subscribe()`, `CodingAgentEventReceiver`, `compatibility_event()`, and `#[allow(deprecated)]`; explicitly allow only named test fixture locations when unavoidable.

## Architecture Patterns

### Typed adapter projection

Keep sequence and replay ownership in `EventService`; make each adapter match the typed payload enum and read common metadata through accessors. The adapter should remain stateful where it already is (`CodingProtocolEventAdapter` accumulates assistant/tool state; `CodingEventBridge` emits UI deltas), but it must not reconstruct a `CodingAgentEvent` merely to reuse the old matcher. [VERIFIED: `event_service.rs:174-185`; `protocol/events.rs:11-31`; `interactive/event_bridge.rs:130-158`]

```rust
// Target shape (names are illustrative; use the existing typed API names).
match event.event() {
    CodingAgentProductEventKind::Message(payload) => match payload {
        CodingAgentMessageProductEvent::Delta { text, .. } => { /* existing output */ }
        CodingAgentMessageProductEvent::Completed { usage, .. } => { /* existing usage */ }
        _ => Vec::new(),
    },
    CodingAgentProductEventKind::Workflow(payload) => { /* prompt status */ }
    _ => Vec::new(),
}
```

The exact enum variant names and field accessors must come from `public_event.rs`; do not invent a parallel event taxonomy or expose `CodingAgentEvent` through `pi_coding_agent::api`. [VERIFIED: `06-CONTEXT.md` D-01/D-04; `public_event.rs` typed hierarchy]

### Deletion sequence

Use a compiler-guided sequence: (a) migrate production adapters, (b) migrate tests and fixtures, (c) add/flip source guards, (d) remove public compatibility method/type, (e) remove `EventService` legacy sender/subscription, and (f) remove `ProductEvent` compatibility storage and conversion helpers. Compile after each deletion boundary so missed callers are surfaced instead of restoring deleted methods. [VERIFIED: project deletion-order constraint in `AGENTS.md`; current symbol graph from `codegraph explore`]

### Behavior-preserving evidence

For every adapter, retain existing assertions for event order and output shape. RPC overflow must still emit `event_stream_lag` with `fresh_snapshot` recovery; typed event queue sequence ordering must remain monotonic; interactive projections must preserve delegation blocks, usage deltas, compaction notices, and partial-commit messages. [VERIFIED: `protocol/rpc.rs:73-100`; `protocol/rpc/event_queue.rs:72-99`; `interactive/event_bridge.rs:167-404`; `interactive/loop.rs:2895-3048`]

## Don't Hand-Roll

| Problem | Don't build | Use instead | Why |
|---|---|---|---|
| Event identity | String parsing or `Debug` matching | `CodingAgentProductEventKind` and typed payload accessors | Phase 6 froze a 45-row exhaustive contract; debug spelling is transitional and unstable. [VERIFIED: `docs/product-event-contract.md`; `06-VERIFICATION.md`] |
| Ordering/replay | Adapter-local sequence counters or a second queue | `EventService` sequence/retained replay and existing `ProductEventReceiver` | Sequence is assigned before retention/broadcast; duplicating it risks gaps and reorder. [VERIFIED: `event_service.rs:111-185`] |
| Compatibility cleanup | Broad search-and-delete without behavior tests | Compiler-guided deletion plus boundary guards and existing fixtures | Co-located tests cover durable/recovery/control edge cases that source scans alone miss. [VERIFIED: `coding_session/mod.rs` tests; `event_boundary_guards.rs`] |

## Runtime State Inventory

| Category | Items Found | Action Required |
|---|---|---|
| Stored data | No external datastore or persisted key stores the Rust receiver/type name; durable session logs store typed session facts, not live `CodingAgentEventReceiver` values. [VERIFIED: architecture and `session_log` contract] | No data migration; verify session replay fixtures remain unchanged. |
| Live service config | None; this is an in-process Tokio broadcast/mpsc path with no external service configuration. [VERIFIED: `event_service.rs`; `protocol/rpc/event_queue.rs`] | No external change. |
| OS-registered state | None found; no service/unit/task registration references compatibility symbols. [VERIFIED: repository scope and runtime architecture] | No action. |
| Secrets/env vars | None; compatibility names are Rust symbols, not credential or environment keys. [VERIFIED: source inventory] | No action. |
| Build artifacts/installed packages | None requiring migration; Cargo rebuilds affected crates from source. [VERIFIED: workspace manifests and lockfile] | Run the required workspace checks after deletion. |

## Common Pitfalls

### Pitfall 1: Deleting the source before migrating projections
**What goes wrong:** Removing `compatibility_event` first breaks the public projection and adapter matchers, encouraging a compatibility shim to be restored. [VERIFIED: current call graph]
**How to avoid:** Migrate each production consumer and its tests first, then delete storage in compiler-guided steps.
**Warning signs:** New `#[allow(deprecated)]`, a conversion back to `CodingAgentEvent`, or a new adapter-local event queue.

### Pitfall 2: Preserving identity but dropping payload fields
**What goes wrong:** Matching only typed family/kind can lose text, usage, tool arguments, delegation IDs, recovery reasons, or partial-commit details. [VERIFIED: `docs/product-event-contract.md` payload columns]
**How to avoid:** For each old matcher arm, map the exact typed payload fields and retain the existing output assertions.
**Warning signs:** Protocol/UI tests pass only on event counts or kind names, with no payload/output assertions.

### Pitfall 3: Confusing event terminal status with root operation completion
**What goes wrong:** Adapter migration may infer a root completion from message/tool/delegation/session-write events, changing output/control behavior. [VERIFIED: `docs/product-event-contract.md` Root Terminal Associations]
**How to avoid:** Preserve the existing five root associations; leave Phase 9 association closure out of this phase.

### Pitfall 4: Removing replay/overflow behavior with the old receiver
**What goes wrong:** Replacing the typed receiver transport while deleting compatibility code can change lag handling or cursor ordering. [VERIFIED: `event_service.rs:132-170`; `protocol/rpc.rs:73-100`]
**How to avoid:** Keep `ProductEventReceiver`, retained replay, and RPC queue behavior intact until adapter migration is green.

### Pitfall 5: Treating tests as disposable compatibility code
**What goes wrong:** Deleting co-located tests to satisfy source guards reduces coverage of startup recovery, durability, forks, delegation, and partial commits. [VERIFIED: `coding_session/mod.rs` test inventory; `06-VERIFICATION.md`]
**How to avoid:** Rewrite assertions against typed events and retain deterministic fixtures; only test-gate a raw source constructor when there is a documented migration-only reason.

## Validation Architecture

### Test Framework

| Property | Value |
|---|---|
| Framework | Rust built-in test harness plus Tokio async tests |
| Config file | None; Cargo manifests and colocated `#[cfg(test)]` modules |
| Quick run command | `cargo test -p pi-coding-agent --test event_boundary_guards --test product_event_contract --test protocol_events --quiet` |
| Focused adapter command | `cargo test -p pi-coding-agent protocol::events interactive::event_bridge --lib --quiet` (or the nearest existing target filters) |
| Full suite command | `cargo test --workspace --quiet` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|---|---|---|---|---|
| COMPAT-01 | RPC/JSON/print and interactive adapters consume typed payloads with unchanged wire/UI behavior | integration + unit | `cargo test -p pi-coding-agent --test protocol_events --test interactive_event_bridge --quiet` plus targeted lib tests | Yes; extend existing adapter suites |
| COMPAT-01 | No production `compatibility_event()` consumer or local compatibility deprecation suppression | source audit | `cargo test -p pi-coding-agent --test event_boundary_guards --quiet` | Yes; invert existing guards |
| COMPAT-02 | Legacy receiver/subscription/storage absent or explicitly test-only | source audit + compile | `cargo test -p pi-coding-agent --test event_boundary_guards --test public_api --quiet` | Yes; update public API expectations |
| COMPAT-02 | Existing event ordering, replay, overflow, durability, and partial-commit assertions remain green | integration | `cargo test -p pi-coding-agent --lib --quiet` and focused protocol/interactive tests | Yes; preserve current fixtures |

### Sampling Rate

- **Per task commit:** focused adapter and boundary-guard tests.
- **Per wave merge:** `cargo test -p pi-coding-agent --quiet`.
- **Phase gate:** `cargo fmt --check`, focused tests, `cargo test --workspace --quiet`, `cargo check --workspace`, source audits, and `git diff --check`.

### Wave 0 Gaps

- Add or extend a deterministic typed-payload adapter fixture for every old `CodingEventBridge::handle`/`CodingProtocolEventAdapter::push` arm that currently relies on the compatibility enum.
- Add a guard that rejects production `compatibility_event()` and compatibility sender/receiver symbols, while allowing only explicitly named migration fixtures.
- Add a regression assertion for unchanged RPC overflow/recovery and interactive partial-commit projection if the existing test does not bind the typed payload fields directly.

## Security Domain

This phase is code-only and does not add authentication, authorization, cryptography, or external input parsers. Security enforcement remains enabled because adapters serialize and project untrusted model/tool text. [VERIFIED: `.planning/config.json` `security_enforcement: true`]

| ASVS Category | Applies | Standard Control |
|---|---|---|
| V2 Authentication | No new behavior | Existing session/provider auth boundaries remain unchanged |
| V3 Session Management | Yes, behavior preservation | Keep session event ordering, replay, recovery markers, and ownership in existing services |
| V4 Access Control | Yes, indirectly | Do not bypass capability-scoped product projections or expose internal receivers through the API |
| V5 Input Validation | Yes | Preserve existing Serde/protocol validation and avoid interpolating typed payload text into new control paths |
| V6 Cryptography | No new behavior | No cryptographic code or dependency introduced |

## Open Questions (RESOLVED)

1. **Where should typed payload ownership live after compatibility storage deletion? RESOLVED.** `ProductEvent` owns one `CodingAgentProductEventKind` value constructed once at the internal event boundary; adapters and the public projection read that owned typed payload. No hidden raw-event clone is retained. [VERIFIED: `public_event.rs`; `public_projection.rs`]
2. **Which raw-event fixtures, if any, are genuinely migration-only? RESOLVED.** Raw input may remain only at the internal `EventService::emit` boundary or in specifically justified `cfg(test)` fixtures with documented coverage. Runtime storage, rebroadcast, and adapter matching use typed payloads. [VERIFIED: `coding_session/mod.rs`; `event_service.rs` tests]
3. **Should transitional `family`/`kind` strings remain serialized? RESOLVED.** Transitional `family`/`kind` wire fields remain during Phase 7 so existing JSON/RPC and public contract assertions stay compatible; this phase does not remove or renegotiate them. [VERIFIED: `docs/product-event-contract.md` Envelope]

## Sources

### Primary (HIGH confidence)

- `crates/pi-coding-agent/src/coding_session/event.rs` - ProductEvent storage, classification, terminal/durability metadata, compatibility accessor.
- `crates/pi-coding-agent/src/coding_session/event_service.rs` - sequence assignment, retained typed events, legacy sender/receiver, typed receiver.
- `crates/pi-coding-agent/src/coding_session/public_event.rs` and `public_projection.rs` - typed public payload hierarchy and receiver projection.
- `crates/pi-coding-agent/src/protocol/events.rs`, `protocol/rpc/events.rs`, `protocol/json_mode.rs` - protocol/JSON adapter call paths.
- `crates/pi-coding-agent/src/interactive/event_bridge.rs`, `interactive/loop.rs` - interactive projection and behavior assertions.
- `.planning/phases/06-product-event-inventory-and-typed-contract/06-VERIFICATION.md` - completed typed contract evidence.
- `docs/product-event-contract.md` - authoritative 45-event inventory and terminal association semantics.
- `crates/pi-coding-agent/tests/event_boundary_guards.rs` and `tests/public_api.rs` - current compatibility guards and external API fixtures.

### Secondary (MEDIUM confidence)

- `AGENTS.md`, `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `.planning/STATE.md` - repository constraints, phase scope, and milestone decisions.

### Tertiary (LOW confidence)

None. This is codebase-only research; no external package or web claim is needed.

## Assumptions Log

No `[ASSUMED]` claims. Findings are grounded in current source, planning artifacts, or repository verification output.

## Environment Availability

This phase has no external runtime dependency or package installation. Cargo/Rust and the existing workspace test fixtures are the only required tools. [VERIFIED: workspace manifests; `.planning/config.json`]

| Dependency | Required By | Available | Version | Fallback |
|---|---|---|---|---|
| Rust/Cargo | compile and tests | Yes | workspace toolchain; `rustc 1.96.0` observed in Phase 6 research | None needed |
| CodeGraph CLI | source discovery | Yes | repository `.codegraph/` index present | `rg` after CodeGraph exploration |

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies; existing Rust/Serde/Tokio stack is directly present.
- Architecture: HIGH - all compatibility producers, consumers, and guards were inspected with CodeGraph and source audits.
- Pitfalls: HIGH - ordering, payload, replay, and test-coverage hazards are evidenced by current implementations and Phase 6 verification.

**Research date:** 2026-07-13
**Valid until:** 2026-08-12, unless Phase 7 implementation changes the event representation.
