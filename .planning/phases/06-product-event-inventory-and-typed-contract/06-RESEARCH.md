# Phase 6: Product Event Inventory and Typed Contract - Research

**Researched:** 2026-07-13
**Domain:** Rust product-event classification, public API projection, and Serde contract design
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Derive the public event contract from the existing internal `ProductEventKind`, family enums, durability model, operation identity, and terminal classification; do not invent an unrelated event taxonomy.
- **D-02:** Public product events must expose typed kind information instead of requiring consumers to parse string-only family/kind fields.
- **D-03:** Preserve sequence identity, operation identity where available, terminal status, durability, and explicit semantics for events that lack one of those fields.
- **D-04:** Keep `CodingAgentEvent` as an internal compatibility source only while the public typed projection is established; Phase 6 must define the replacement payload boundary rather than silently exposing the legacy enum.
- **D-05:** Preserve event ordering, replay compatibility, `PartialCommit` attribution, and adapter-visible behavior while changing the public projection.
- **D-06:** Use existing Rust/Serde/test patterns and add no external runtime dependency for this phase.

### the agent's Discretion

- Choose the exact public enum/module names and payload wrapper layout, provided they are exported through `pi_coding_agent::api` and keep implementation details private.
- Decide which payload variants are typed structs versus intentionally metadata-only variants based on the current emitter inventory.
- Choose whether to keep a narrowly scoped internal conversion helper during the transition, as long as no compatibility event is exposed through the new public contract.

### Deferred Ideas (OUT OF SCOPE)

- Migrating RPC and interactive consumers away from compatibility event unwrapping (Phase 7).
- Public reconnect/replay/client lifecycle APIs (Phase 8).
- Detach/close/shutdown and full operation/outcome/terminal association closure (Phase 9).
- Introducing a separately named `CodingAgentRuntime` owner type.
- New workflows, Lua Flow expansion, and `pi-web-ui` construction.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| EVENT-01 | Public consumers inspect event kind through a stable typed enum. | Existing internal `ProductEventKind` already maps all 45 `CodingAgentEvent` variants into 11 families; replace the string-only `Debug` projection with a public Serde-backed typed projection. |
| EVENT-02 | Public events expose sequence, operation identity, terminal status, durability, and documented payload semantics. | `ProductEvent` already owns sequence, optional operation ID, terminal status, and durability; `CodingAgentEvent` fields provide the payload source. Preserve absent values explicitly and remove public dependence on `compatibility_event`. |
| EVENT-03 | Inventory covers all emitted families and documents terminal/outcome association. | Exhaustive conversion in `ProductEvent::from_compat_event`, the 45-variant inventory below, and terminal-operation tests provide the implementation and verification anchor. |
</phase_requirements>

## Summary

The repository already has a complete internal classification layer. `CodingAgentEvent` has 45 variants, `ProductEventKind` maps them exhaustively into 11 families (`Session`, `Profile`, `Agent`, `Team`, `Message`, `Tool`, `Runtime`, `Delegation`, `Workflow`, `Diagnostic`, and `Capability`), and `ProductEvent` adds sequence, optional operation ID, terminal status, and durability metadata. [VERIFIED: `crates/pi-coding-agent/src/coding_session/event.rs:8-444`]

The public boundary is the remaining gap: `CodingAgentProductEvent` currently exposes only `sequence: u64`, `family: String`, and `kind: String`, with both strings generated using `format!("{:?}", ...)`. This makes the public contract dependent on Rust debug spelling and carries no payload, operation identity, terminal status, or durability. [VERIFIED: `crates/pi-coding-agent/src/coding_session/public_projection.rs:38-59`] Phase 6 should promote the existing classification into a public, Serde-backed typed payload model while retaining `CodingAgentEvent` only as a private conversion source. RPC and interactive migration and compatibility deletion remain Phase 7. [VERIFIED: `06-CONTEXT.md` Deferred Ideas; `05-STAGE-9-CLOSURE.md:Stage 10 Handoff`]

**Primary recommendation:** Add a focused public event contract module that converts the exhaustive internal `ProductEvent` into a typed `CodingAgentProductEvent` (`kind`/payload enum plus common metadata), derives stable `serde` names, and tests all 45 variants and metadata edge cases before any adapter migration.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Event classification and payload construction | `pi-coding-agent` runtime (`EventService` / `event.rs`) | `public_projection` | Product semantics and legacy-to-typed mapping belong to the product crate; lower crates must remain product-neutral. [VERIFIED: `AGENTS.md` architecture; `event_service.rs:174-196`] |
| Stable public event types and Serde names | `pi-coding-agent::api` facade | `coding_session` implementation | The API facade owns embedding contracts while internal event/source enums remain private. [VERIFIED: `crates/pi-coding-agent/src/lib.rs:64-80`] |
| Event sequence assignment and retained ordering | `EventService` | snapshot/replay consumers | `EventService::emit` assigns monotonically increasing sequence before retention and publication. [VERIFIED: `event_service.rs:174-185`] |
| Durable status attribution | `EventService` + session service | public projection | Pending/committed/skipped session writes are emitted by session finalization; projection must preserve `PartialCommit` identity and not infer durability from terminal status. [VERIFIED: `event_service.rs:336-380`; `session_service.rs:392-400`] |
| Adapter behavior | Protocol/interactive adapters | typed event contract | Phase 6 defines their target contract but does not migrate their legacy unwrapping. [VERIFIED: `06-CONTEXT.md` Phase Boundary] |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust | Edition 2024; toolchain `rustc 1.96.0` in this environment | Exhaustive enums and API types | Existing workspace language and compile-time exhaustiveness are the safest inventory guard. [VERIFIED: `Cargo.toml`; `rustc --version`] |
| `serde` | Workspace manifest `1` with `derive` | Stable typed event serialization | Already used throughout public wire models and session records; D-06 forbids adding a runtime dependency. [VERIFIED: `crates/pi-coding-agent/Cargo.toml`] |
| `serde_json` | Workspace manifest `1` | Projection/serialization contract tests | Already present and used by protocol tests. [VERIFIED: `crates/pi-coding-agent/Cargo.toml`; `tests/protocol_events.rs`] |
| Tokio broadcast | Workspace manifest `tokio 1` | Existing event receiver transport | Phase 6 changes event values, not subscription lifecycle. [VERIFIED: `event_service.rs:1-35`] |

### Supporting

No new supporting library is required. This is a code-only contract migration and must not add a crate or feature flag. [VERIFIED: `06-CONTEXT.md` D-06; `Cargo.toml`]

## Package Legitimacy Audit

Not applicable: Phase 6 installs no external package or runtime dependency.

## Architecture Patterns

### Current event flow

```text
CodingAgentEvent emitted by product services / AgentEvent mapper
        |
        v
EventService::emit -> assign ProductEventSequence -> ProductEvent::from_compat_event
        |                                      |
        |                                      +--> internal family/kind/status/durability
        |                                      +--> legacy compatibility source (Phase 6 only)
        v
retained product-event deque + product broadcast
        |
        +--> existing RPC/interactive/JSON consumers (legacy unwrapping; Phase 7)
        +--> CodingAgentProductEventReceiver (public facade; Phase 6 contract)
```

The sequence is assigned before both retained storage and broadcast, so conversion must never allocate a second sequence or reorder events. [VERIFIED: `event_service.rs:174-185`]

### Recommended public shape

Use one common metadata wrapper and a typed payload/kind enum. The exact names are discretionary, but the following constraints are mandatory:

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodingAgentProductEvent {
    pub sequence: u64,
    pub operation_id: Option<String>,
    pub terminal_status: Option<CodingAgentProductEventTerminalStatus>,
    pub durability: CodingAgentProductEventDurability,
    pub kind: CodingAgentProductEventKind,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "family", content = "payload", rename_all = "snake_case")]
pub enum CodingAgentProductEventKind {
    Session(CodingAgentSessionEvent),
    Profile(CodingAgentProfileEvent),
    Agent(CodingAgentAgentEvent),
    Team(CodingAgentTeamEvent),
    Message(CodingAgentMessageEvent),
    Tool(CodingAgentToolEvent),
    Runtime(CodingAgentRuntimeEvent),
    Delegation(CodingAgentDelegationEvent),
    Workflow(CodingAgentWorkflowEvent),
    Diagnostic(CodingAgentDiagnosticEvent),
    Capability(CodingAgentCapabilityEvent),
}
```

This shape keeps the existing 11-family taxonomy (D-01), makes the discriminant typed (EVENT-01), and gives each family room for typed fields without exposing `CodingAgentEvent` (D-04). Prefer private fields plus accessors if constructing arbitrary inconsistent `kind`/metadata combinations would be unsafe; if fields remain public for compatibility, conversion tests must be the authority and constructors should be limited.

### Conversion pattern

`From<ProductEvent>` should delegate to an exhaustive internal conversion that matches every `CodingAgentEvent` variant. Do not use `Debug` formatting, string parsing, wildcard arms, or a public `From<CodingAgentEvent>` implementation. The exhaustive match is the compile-time guard that a newly added internal event cannot silently disappear from the public inventory. [VERIFIED: `ProductEventKind::from_compat_event` currently uses an exhaustive 45-variant match at `event.rs:42-175`]

Payload fields can reuse already-public domain types (`ProfileId`, `ProfileKind`, `CodingSessionError`, self-healing structs) when their serialization is stable; otherwise define public projection structs with owned values. [VERIFIED: `profiles.rs:16-105`; `error.rs:7-85`; `self_healing_edit_flow.rs:42-190`]

### Serde naming and stability

Derive `Serialize` for public event enums and use explicit `rename_all = "snake_case"` (or explicit `rename` for names that must remain stable). Do not serialize Rust `Debug` output such as `Agent(InvocationStarted)`. Add JSON snapshot assertions for representative nested and metadata-only events, including `None` fields and durability variants. Existing public types use Serde derive and snake-case names as the local pattern. [VERIFIED: `profiles.rs:84-105`; `session_log/event.rs:88-257`]

### Metadata semantics

- `sequence` is live stream order, starts at 1 for the first emitted event, and is not durable session order. [VERIFIED: `event_service.rs:174-180`; `ProductEventSequence` tests]
- `operation_id` is optional: operation-bearing variants and diagnostics carry it; session-opened, profile-changed, and capability-changed variants do not. [VERIFIED: `CodingAgentEvent::operation_id` at `event.rs:773-815`]
- `terminal_status` is event-level completion/failure/abort/recovery, not necessarily root-operation completion. Tool completion and session-write commit are terminal at their event family but `terminal_operation()` intentionally returns `None`. [VERIFIED: `event.rs:823-861`, tests `product_event_wrapper_does_not_treat_family_completion_as_operation_terminal`]
- `durability` is independent from terminal status: normal events are `LiveOnly`; session write pending is `PendingSessionWrite { operation_id }`; committed writes are `Durable { session_id }`; skipped writes remain `LiveOnly`. [VERIFIED: `event.rs:417-475`, event wrapper tests]
- `PartialCommit` is an error/outcome fact and must retain its operation ID; Phase 6 should not fabricate a new event status or durability variant to represent it. [VERIFIED: `error.rs:26-31,72-75`; `session_service.rs:392-400`]

## Event Inventory

The current internal inventory contains 45 variants. The public conversion must cover each row; payload fields below are the source fields that must either become typed public data or be explicitly documented as metadata-only.

| Family | Internal variants | Payload/correlation notes |
|--------|-------------------|---------------------------|
| Session | `Opened`, `WritePending`, `WriteCommitted`, `WriteSkipped`, `CompactionCompleted` | Opened has `session_id`; writes have operation ID and pending/durable semantics; compaction has operation/turn and summary fields. |
| Profile | `DefaultChanged` | `profile_id`; no operation ID or terminal status. |
| Agent | `InvocationStarted`, `InvocationCompleted`, `InvocationFailed`, `InvocationAborted`, `TurnStarted`, `ProviderRequestStarted` | Invocation lifecycle has child/profile/task/result/error/reason; turn/provider events have operation and turn IDs plus provider/model. |
| Team | `Started`, `MemberStarted`, `MemberCompleted`, `Completed`, `Failed`, `Aborted` | Team/member IDs and child operation IDs; parent terminal status only on team completed/failed/aborted. |
| Message | `Started`, `Delta`, `ThinkingDelta`, `Completed` | operation/turn/message IDs, text/final text, and usage; message completion is family-terminal, not root-operation-terminal. |
| Tool | `Started`, `Updated`, `Completed`, `Failed` | operation/turn/tool/name/arguments or summary/message; tool completed/failed are family-terminal. |
| Runtime | `CompactionCompleted` | operation/turn/summary/first-kept-message/tokens; no terminal-operation mapping. |
| Delegation | `Requested`, `Rejected`, `Approved`, `ConfirmationRequired`, `Started`, `Completed`, `Failed` | request and child operation identity, profile target, task/reason/result/error; completed/failed are event-terminal but currently lack `terminal_operation()` mapping. |
| Workflow | `SelfHealingEditStarted`, `SelfHealingEditRepairAttempted`, `SelfHealingEditCompleted`, `SelfHealingEditFailed`, `PromptStarted`, `PromptCompleted`, `PromptFailed`, `PromptAborted`, `OperationRecovered` | Self-healing and prompt lifecycle payloads; operation recovered has recovery ID/reason and `Recovered` event status. |
| Diagnostic | `Diagnostic` | optional operation ID and message; no terminal status. |
| Capability | `Changed` | generation and revocation policy; no operation ID or terminal status. |

The grouping above is derived from the exhaustive internal mapper, not from the historical reference architecture's suggested future families. The current code has no `Operation`, `Plugin`, or `Pressure` family; plugin load currently emits diagnostics and capability changes rather than a dedicated plugin event. [VERIFIED: `ProductEventKind::from_compat_event`; `event_service.rs:237-248`]

## Terminal and Outcome Association Matrix

| Event variants | `terminal_status` | `terminal_operation()` today | Phase 6 rule |
|----------------|-------------------|------------------------------|---------------|
| Prompt completed/failed/aborted | completed/failed/aborted | `Prompt` | Preserve mapping and expose both event status and operation kind where available. |
| Agent invocation completed/failed/aborted | completed/failed/aborted | `AgentInvocation` | Preserve mapping. |
| Agent team completed/failed/aborted | completed/failed/aborted | `AgentTeam` | Preserve mapping. |
| Self-healing edit completed/failed | completed/failed | `SelfHealingEdit` | Preserve mapping. |
| Session compaction completed | completed | `Compact` | Preserve mapping. |
| Tool completed/failed; message completed; delegation completed/failed; session write committed | completed/failed | `None` | Expose event-level status; do not claim root operation completion. Phase 9 owns full association closure. |
| Operation recovered | recovered | `None` | Expose recovery status and recovery ID; do not invent operation-kind association in Phase 6. |

The existing `ProductEventTerminalOperation` has no tests for delegation, recovery, or all operation kinds, and is marked `allow(dead_code)`. [VERIFIED: `event.rs:306-310`, CodeGraph blast-radius report, current tests] Phase 6 must document this boundary rather than silently expanding it; Phase 9 owns the requirement that applicable terminal associations are fully tested.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Event kind identification | String parsing or `Debug` formatting | Exhaustive typed enum conversion | Debug spelling is not a stable API and loses payload semantics. [VERIFIED: `public_projection.rs:50-58`] |
| Event ordering | Per-adapter counters or timestamps | `EventService` assigned `ProductEventSequence` | A single owner already assigns sequence before retention/broadcast. [VERIFIED: `event_service.rs:174-185`] |
| Durable truth | Reconstructing durability from terminal status | Existing `ProductEventDurability` conversion and session-write events | Terminal status and persistence are independent dimensions. [VERIFIED: `event.rs:417-475`] |
| Legacy compatibility exposure | Publicly re-exporting `CodingAgentEvent` as the new payload | Private conversion helper plus public payload structs | Phase boundary requires the legacy enum to remain an internal source only. [VERIFIED: `06-CONTEXT.md` D-04] |

## Runtime State Inventory

This is a refactor/projection phase. No external runtime state migration is required.

| Category | Items Found | Action Required |
|----------|-------------|-----------------|
| Stored data | Session logs persist `SessionEventEnvelope`/`SessionEventData`, not live `ProductEvent` payloads. | No data migration. Preserve session-log schema and `PartialCommit` behavior. [VERIFIED: `session_log/event.rs`; `05-STAGE-9-CLOSURE.md`] |
| Live service config | No external service configuration contains the Rust enum names. | No live-service change; only source projection and tests. [VERIFIED: repository-local scope and no external service dependency in phase] |
| OS-registered state | No OS registration is tied to product-event type names. | No migration. |
| Secrets and env vars | No auth/env key is derived from product-event family or variant names. | No migration. |
| Build artifacts / installed packages | `target/` artifacts are regenerated by Cargo and are not contract state. | Rebuild/test only; do not edit generated artifacts. [VERIFIED: Cargo workspace] |

## Common Pitfalls

- **Leaving a public `CodingAgentEvent` payload inside the new type:** this violates D-04 and keeps Phase 7 unable to delete compatibility storage. Use owned typed payloads and keep conversion source private. [VERIFIED: `ProductEvent` currently stores `compatibility_event` at `event.rs:315-319`]
- **Keeping `family`/`kind` as strings for convenience:** current first-party tests rely on strings such as `Agent(InvocationStarted)`; that is exactly the boundary EVENT-01 replaces. [VERIFIED: `tests/agent_invocation.rs:304-305`; `tests/agent_team_flow.rs:384`]
- **Collapsing event-terminal and operation-terminal semantics:** tool/message/session-write/delegation statuses are not all root operation terminals. Preserve `terminal_status` separately from terminal operation association. [VERIFIED: `event.rs:823-861` and tests]
- **Using wildcard matches in conversion:** this makes new internal variants silently disappear from the public inventory. Require exhaustive matches and an inventory test. [VERIFIED: current exhaustive `ProductEventKind::from_compat_event`]
- **Serializing optional fields inconsistently:** operation-less events and `LiveOnly` events are valid; tests must assert explicit null/enum semantics rather than requiring every event to have operation/session IDs. [VERIFIED: `CodingAgentEvent::operation_id` and durability mapper]
- **Changing event sequence or retention while adding payloads:** replay and adapters depend on the existing sequence owner and retained deque. Keep projection pure and side-effect free. [VERIFIED: `EventService::emit` and `product_events_after`]
- **Pulling Phase 7 migration into Phase 6:** do not alter RPC/interactive compatibility consumers or delete receivers here; provide the stable target contract and projection tests first. [VERIFIED: `06-CONTEXT.md` Deferred Ideas and Phase Boundary]

## Code Examples

### Current internal conversion seam

```rust
pub(crate) fn from_compat_event(
    sequence: ProductEventSequence,
    compatibility_event: CodingAgentEvent,
) -> Self {
    let classification = compatibility_event.classification();
    let kind = ProductEventKind::from_compat_event(&compatibility_event);
    let operation_id = classification.operation_id.map(str::to_owned);
    let terminal_status = classification.terminal_status;
    let durability = ProductEventDurability::from_compat_event(&compatibility_event);
    // Phase 6 should keep this internal source only and add a typed payload projection.
    # ...
}
```

Source: [VERIFIED: `crates/pi-coding-agent/src/coding_session/event.rs:322-334`]

### Existing public projection to replace

```rust
impl From<ProductEvent> for CodingAgentProductEvent {
    fn from(event: ProductEvent) -> Self {
        Self {
            sequence: event.sequence().get(),
            family: format!("{:?}", event.family()),
            kind: format!("{:?}", event.kind()),
        }
    }
}
```

Source: [VERIFIED: `crates/pi-coding-agent/src/coding_session/public_projection.rs:50-59`]

### Verification test shape

```rust
let public = CodingAgentProductEvent::from(product_event);
assert_eq!(public.sequence(), 42);
assert_eq!(public.operation_id(), Some("op_prompt"));
assert!(matches!(public.kind(), CodingAgentProductEventKind::Workflow(_)));
assert_eq!(serde_json::to_value(&public).unwrap()["family"], "workflow");
```

The exact accessor names remain discretionary. The test must verify that public callers can match enum variants and serialize deterministic snake-case names without inspecting `CodingAgentEvent` or debug strings.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness plus Tokio tests |
| Config file | None; Cargo manifests define the workspace |
| Quick run command | `cargo test -p pi-coding-agent --lib product_event --quiet` |
| Focused integration command | `cargo test -p pi-coding-agent --test public_api --quiet` |
| Full suite command | `cargo test -p pi-coding-agent --quiet` then `cargo test --workspace` |

Existing focused product-event tests pass in this environment: 6 internal `product_event_wrapper` tests and the event-service sequence/durability test. [VERIFIED: commands run 2026-07-13; warnings are pre-existing dead-code/deprecation warnings]

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| EVENT-01 | Public typed family/kind matching and no Debug-string dependency | unit + public API integration | `cargo test -p pi-coding-agent product_event --lib`; `cargo test -p pi-coding-agent --test public_api` | Existing files; new contract tests required |
| EVENT-02 | Sequence, optional operation ID, terminal status, durability, payload and Serde projection | unit + serialization | `cargo test -p pi-coding-agent product_event --lib` | Existing internal tests; public serialization coverage is a Wave 0 gap |
| EVENT-03 | All 45 variants inventoried and terminal association documented | exhaustive conversion test + docs/source guard | `cargo test -p pi-coding-agent product_event --lib`; `cargo test -p pi-coding-agent --test event_boundary_guards` | Existing guard file; exhaustive public inventory test required |

### Wave 0 Gaps

- [ ] Add a focused public-event contract test module (prefer `crates/pi-coding-agent/tests/product_event_contract.rs` or a clearly named unit module) covering typed matching and Serde output.
- [ ] Add an exhaustive 45-variant fixture/conversion test. A compile-time exhaustive `match` is required; a count/assertion or table should make inventory drift visible in review.
- [ ] Add public API type assertions for every new enum/payload type in `tests/public_api.rs`.
- [ ] Add a source guard that forbids `format!("{:?}", event.family())`/`format!("{:?}", event.kind())` in the public event projection after migration.

## Security Domain

Phase 6 does not add authentication or cryptography, but it changes a public serialization boundary. [VERIFIED: `.planning/config.json` `security_enforcement: true`, ASVS level 1]

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | No | No authentication logic is changed. |
| V3 Session Management | Yes, limited | Preserve sequence/cursor and session ID semantics; do not expose internal session-log facts as live public truth. |
| V4 Access Control | Yes, limited | Keep internal `CodingAgentEvent`, services, and Flow nodes private; export only the curated `api` types. |
| V5 Input Validation | Yes | Public event types are output contracts; use Serde derives and explicit enum tags rather than accepting arbitrary family/kind strings. |
| V6 Cryptography | No | No cryptographic operation is introduced. |

### Known Threat Patterns

| Pattern | STRIDE | Mitigation |
|---------|--------|------------|
| Debug-string contract drift | Tampering / spoofing | Typed enum plus explicit Serde names; no `Debug` output as wire identity. |
| Accidental internal data exposure | Information disclosure | Public payload structs must select fields deliberately; never re-export `CodingAgentEvent`, Flow nodes, or internal services. |
| Incorrect durability claim | Tampering / repudiation | Preserve `ProductEventDurability` independently from terminal status and test pending/committed/skipped writes. |
| Sequence/order regression | Tampering / denial of service | Keep EventService sequence assignment and retained replay unchanged; add monotonic-order regression tests. |

## Sources

### Primary (HIGH confidence)

- [VERIFIED: `crates/pi-coding-agent/src/coding_session/event.rs`] — 45-variant `CodingAgentEvent`, exhaustive family/kind mapper, operation identity, terminal status, durability, and current terminal-operation mapping.
- [VERIFIED: `crates/pi-coding-agent/src/coding_session/event_service.rs`] — sequence assignment, retained event publication, emit helpers, session-write semantics, and receiver channels.
- [VERIFIED: `crates/pi-coding-agent/src/coding_session/public_projection.rs`] — current public string-only projection and receiver.
- [VERIFIED: `crates/pi-coding-agent/src/lib.rs`, `tests/public_api.rs`] — stable API export and public-surface test conventions.
- [VERIFIED: `06-CONTEXT.md`, `REQUIREMENTS.md`, `ROADMAP.md`] — locked phase boundary, requirements, and verification goals.

### Secondary (MEDIUM confidence)

- [CITED: internal architecture reference `docs/superpowers/specs/2026-07-07-operation-runtime-reference-architecture.md`] — target family-oriented ProductEvent, sequence, durability, and ordering principles. It is design input; current code and Phase 6 context take precedence where they differ.
- [CITED: archived closure `05-STAGE-9-CLOSURE.md`] — bounded Stage 10 handoff and behavior constraints.

### Tertiary (LOW confidence)

None. This research is codebase-only; no external package or web claim is needed.

## Project Constraints (from AGENTS.md)

- Communicate with the user in Chinese; technical documents may be English.
- Use CodeGraph before grep/find or direct file reading when the repository has `.codegraph/`; this research used `codegraph explore` first.
- Preserve dependency direction `pi-coding-agent -> pi-agent-core -> pi-ai` and `pi-coding-agent -> pi-tui`.
- Keep public contracts under `pi_coding_agent::api`; internal operations, services, and Flow nodes remain private.
- Preserve JSON/RPC/interactive behavior, event order, control, replay, navigation, typed session facts, and `PartialCommit` attribution.
- Use deterministic offline fixtures and retain existing behavior assertions.
- Use `apply_patch` for manual edits, ASCII by default, and concise comments only for non-obvious behavior.
- Verification gates include `cargo fmt --check`, focused `pi-coding-agent` tests, workspace tests/checks, source audits, and `git diff --check`.

## Open Questions

1. **Exact public payload granularity:** The 45 variants have heterogeneous fields. The planner should choose between one typed payload enum with 45 variants and family-specific structs/enums, preserving the 11 existing families either way. This is an implementation choice, not a reason to expose the legacy enum. [VERIFIED: internal inventory; decision remains discretionary in `06-CONTEXT.md`]
2. **Public field visibility:** Existing `CodingAgentProductEvent` has public fields, but making the new kind/payload fields private with accessors better prevents inconsistent metadata construction. Confirm the compatibility expectation in planning before selecting field visibility. [VERIFIED: `public_projection.rs:44-47`; discretion in context]
3. **Dedicated product-event contract documentation:** EVENT-03 requires documentation. The planner should decide whether rustdoc plus an inventory test is sufficient or whether to add a small stable `docs/` contract table. Do not treat the historical reference architecture as the current inventory. [VERIFIED: requirements and current docs layout]

## Assumptions Log

No `[ASSUMED]` claims. All findings are grounded in current repository files, commands, or explicitly labeled internal design documents.

## Environment Availability

This phase is code/config-only and has no external dependency. Available verification tools: `rustc 1.96.0`, `cargo 1.96.0`, and the workspace's existing Serde/Tokio dependencies. [VERIFIED: commands run 2026-07-13]

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all dependencies already exist in the crate manifest; no new package is needed.
- Architecture: HIGH — event ownership, conversion, and publication were inspected directly and cross-checked against archived closure evidence.
- Pitfalls: HIGH — current string projection, compatibility storage, terminal mapping gaps, and legacy test assumptions are all directly observable.

**Research date:** 2026-07-13
**Valid until:** 2026-08-12, unless Phase 7 changes the event source before Phase 6 implementation begins.
