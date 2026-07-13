# Phase 7 Validation Strategy

**Phase:** Adapter Migration and Compatibility Deletion
**Requirements:** COMPAT-01, COMPAT-02
**Validation mode:** Nyquist enabled; deterministic offline Rust tests and source audits

## Observable Contract

1. RPC, JSON/print, and interactive adapters consume typed product-event payloads directly; no production adapter calls `compatibility_event()`.
2. First-party tests assert typed identity/payload and retain existing ordering, output, replay, overflow, durability, control, recovery, and `PartialCommit` behavior.
3. `CodingAgentEventReceiver`, `CodingAgentSession::subscribe`, the legacy broadcast sender, and `ProductEvent` compatibility storage are absent or explicitly `cfg(test)` migration fixtures.

## Validation Matrix

| Requirement | Evidence | Command | Gate |
|---|---|---|---|
| COMPAT-01 | Typed protocol/JSON adapter output and state transitions | `cargo test -p pi-coding-agent --test protocol_events --quiet` | Must pass before deleting storage |
| COMPAT-01 | Typed interactive projection, usage/delegation/error output, and cursor monotonicity | `cargo test -p pi-coding-agent --test interactive_event_bridge --quiet` and `cargo test -p pi-coding-agent --lib interactive::r#loop::tests --quiet` | Must pass before deleting receivers |
| COMPAT-01 | No production compatibility consumer/suppression | `cargo test -p pi-coding-agent --test event_boundary_guards --quiet` | Fail closed on any new call or suppression |
| COMPAT-02 | Receiver-dependent session and EventService behavior uses typed assertions | `cargo test -p pi-coding-agent --lib coding_session::tests --quiet` and `cargo test -p pi-coding-agent --lib coding_session::event_service::tests --quiet` | Both must pass before deleting the legacy receiver/broadcast |
| COMPAT-02 | Public facade no longer exposes legacy subscription/type | `cargo test -p pi-coding-agent --test public_api --quiet` | Must pass after API deletion |
| COMPAT-02 | Compatibility storage deletion preserves session white-box behavior | `cargo test -p pi-coding-agent --lib coding_session::event --quiet`, `cargo test -p pi-coding-agent --lib coding_session::event_service::tests --quiet`, and `cargo test -p pi-coding-agent --lib coding_session::tests --quiet` | Must pass at the 07-05 storage-deletion boundary |
| COMPAT-02 | Existing product-event serialization, durability, terminal separation, and ordering | `cargo test -p pi-coding-agent --test product_event_contract --quiet` | Must pass at each deletion step |
| COMPAT-02 | Full workspace compatibility | `cargo test --workspace --quiet` and `cargo check --workspace` | Final phase gate |
| All | Formatting and whitespace | `cargo fmt --check` and `git diff --check` | Final phase gate |

## Migration Evidence Requirements

- Record a typed assertion for every production matcher family and preserve representative payload field checks, not only event counts.
- Preserve the RPC `event_stream_lag`/`fresh_snapshot` response and bounded queue sequence tests.
- Preserve interactive startup recovery, partial-commit, delegation, compaction, and profile/session navigation assertions.
- Run source scans over `crates/pi-coding-agent/src/protocol`, `src/interactive`, and first-party tests after each deletion wave; only explicitly named `cfg(test)` fixtures may mention legacy symbols.
- Do not add Phase 8 reconnect/client lifecycle behavior or Phase 9 terminal-association/guard closure to this validation artifact.

## Wave 0 Coverage Tasks

- **07-01 Task 1-2:** establish owned typed payload construction and bind it to the exhaustive inventory, real receiver metadata, and Serde contract before adapter migration.
- **07-02 Task 1-2:** add typed protocol fixtures for old matcher arms lacking direct payload assertions and retain JSON/RPC output plus overflow recovery tests.
- **07-03 Task 1-2:** add typed bridge/loop fixtures, including exact PartialCommit attribution, recovery, navigation, and no-op projection behavior.
- **07-04 Task 1:** migrate and execute all receiver-dependent session and EventService tests before deletion (`coding_session::tests`, `coding_session::event_service::tests`, and `public_api`).
- **07-04 Task 2:** after Task 1 passes, flip the scoped receiver guard so it rejects the legacy receiver, duplicate sender, and path-specific local suppressions; defer compatibility storage/accessor rejection to 07-05.

## Security Notes

No new dependencies, network calls, authentication paths, or cryptographic code are introduced. Validation must still ensure typed projection does not bypass existing capability/session boundaries or alter protocol input validation.

## State A Nyquist Audit (2026-07-13)

**Audit mode:** GENERIC-AGENT WORKAROUND for `gsd-nyquist-auditor`
**Result:** ESCALATE — focused suites are green, but the phase has two implementation blockers and one material automated-coverage gap.
**Requirement status:** 0 COVERED, 2 PARTIAL (`COMPAT-01`, `COMPAT-02`)
**Plan-task status:** 6 COVERED, 3 PARTIAL, 1 MISSING (10 total)

The audit used CodeGraph before direct source inspection and traced the live adapter paths. It then ran 158 focused tests successfully:

- `cargo test -p pi-coding-agent --test event_boundary_guards --test product_event_contract --test protocol_events --test interactive_event_bridge --test public_api --quiet` — 68 passed.
- `cargo test -p pi-coding-agent --lib interactive::r#loop::tests --quiet` — 9 passed.
- `cargo test -p pi-coding-agent --lib coding_session::event_service::tests --quiet` — 25 passed.
- `cargo test -p pi-coding-agent --lib coding_session::tests --quiet` — 56 passed.

These passing commands are not sufficient evidence for phase closure. The protocol integration suite makes 14 calls to the public raw `CodingProtocolEventAdapter::push(&CodingAgentEvent)` entry point, and the interactive integration suite makes 21 calls to the public raw `CodingEventBridge::handle(&CodingAgentEvent)` entry point. Neither suite exercises its claimed behavior matrix through the live `ProductEvent` projection path.

### Requirement-to-Task/Test Map

| Plan task | Requirement | Automated evidence | Status | Audit conclusion |
|---|---|---|---|---|
| 07-01 Task 1 | COMPAT-01, COMPAT-02 | `public_event` unit tests; `product_event_contract` | COVERED | Typed payload ownership and exhaustive conversion are behavior-tested. |
| 07-01 Task 2 | COMPAT-01, COMPAT-02 | `product_event_contract`; `event_boundary_guards` | COVERED | Sequence, durability, terminal separation, and Serde shape are tested through a real product-event receiver. |
| 07-02 Task 1 | COMPAT-01 | `protocol_events` | PARTIAL | Production RPC/JSON forwarding reaches typed projection, but the public raw `push` entry point remains and most behavior assertions enter through it. |
| 07-02 Task 2 | COMPAT-01 | `protocol_events`; existing JSON/RPC suites and queue tests recorded by the plan | PARTIAL | Wire behavior and overflow recovery are covered, but representative protocol payload assertions do not prove the `ProductEvent -> push_product_event` path. |
| 07-03 Task 1 | COMPAT-01 | `interactive_event_bridge`; one co-located typed assistant-delta test | MISSING | The broad suite exercises the raw matcher, while the live typed matcher has a confirmed context-token fallback regression. |
| 07-03 Task 2 | COMPAT-01 | `interactive::r#loop::tests`; session tests | COVERED | Typed loop consumption, cursor, recovery, navigation, and `PartialCommit` assertions remain covered. |
| 07-04 Task 1 | COMPAT-01, COMPAT-02 | `coding_session::tests`; `event_service::tests`; `public_api` receiver assertions | COVERED | Legacy receiver-dependent tests were migrated to typed receivers with payload/identity assertions. |
| 07-04 Task 2 | COMPAT-02 | `event_boundary_guards`; `public_api`; `product_event_contract` | COVERED | Legacy receiver/subscription and duplicate broadcast deletion are covered. |
| 07-05 Task 1 | COMPAT-02 | event, EventService, and session unit suites | COVERED | Raw `ProductEvent` storage/accessor deletion and typed retained transport are covered. |
| 07-05 Task 2 | COMPAT-01, COMPAT-02 | `event_boundary_guards`; adapter/contract suites | PARTIAL | The guard rejects deleted storage/receiver symbols but does not reject the stable `CodingAgentEvent` facade export or public raw adapter signatures. |

### Requirement Status

| Requirement | Status | Covered evidence | Unclosed evidence |
|---|---|---|---|
| COMPAT-01 | PARTIAL | Live production RPC/JSON and interactive loops receive `ProductEvent`; typed receiver, ordering, durability, replay, recovery, and `PartialCommit` tests pass. | First-party protocol/interactive behavior suites predominantly consume raw events; the stable facade and public adapters still expose that path; live typed interactive usage behavior regresses when aggregate usage is absent. |
| COMPAT-02 | PARTIAL | Legacy receiver, subscription, duplicate broadcast, and raw retained storage are deleted and guarded. | A supported raw compatibility API remains through `pi_coding_agent::api::CodingAgentEvent`, `CodingProtocolEventAdapter::push`, and `CodingEventBridge::handle`; the final guard does not reject those surfaces. |

### Escalated Findings and Required Tests

1. **CR-01 — BLOCKER (implementation/public-boundary bug):** `crates/pi-coding-agent/src/lib.rs` still re-exports `CodingAgentEvent` from the stable `api` facade; `crates/pi-coding-agent/src/protocol/events.rs` retains public `push(&CodingAgentEvent)`; `crates/pi-coding-agent/src/interactive/event_bridge.rs` retains public `handle(&CodingAgentEvent)`. `public_api.rs` explicitly imports the raw enum, so current tests certify the bypass rather than reject it.
   - Required implementation direction: remove the stable raw export and production-public raw adapter entry points, retaining raw input only at the private `EventService::emit` admission boundary or narrowly `cfg(test)` conversion fixtures.
   - Required automated test: extend `event_boundary_guards.rs` (and, if appropriate, an external compile-fail facade fixture) to reject `CodingAgentEvent` in the stable facade and reject public adapter signatures accepting it, while allowlisting only the private enum definition, exhaustive conversion, and `EventService::emit`.
   - Required behavioral migration: move the existing protocol assertions to events observed from a real `CodingAgentSession` product-event receiver or a narrowly test-only typed `ProductEvent` fixture, then call the same typed adapter entry used by production.

2. **CR-02 — BLOCKER (confirmed live behavior regression):** the raw interactive matcher uses `calculate_context_tokens`, which prefers `total_tokens` and otherwise saturating-sums input/output/cache counters. The live typed matcher instead maps `usage.total_tokens == 0` directly to `None`. Providers that omit the aggregate but populate components therefore lose the footer context-token value on the actual typed path.
   - Required implementation direction: apply the same component-sum fallback to `CodingAgentProductEventUsage` in the typed matcher.
   - Required failing regression test: construct an `AssistantMessageCompleted` `ProductEvent` with `total_tokens = 0` and non-zero input/output/cache counters, enter through `UiProjection::apply_product_event` or `CodingEventBridge::push_product_event`, and assert `UiEvent::UsageUpdate.context_tokens == Some(saturating component sum)`. This test would fail against the audited implementation and must not be weakened to expect `None`.

3. **WR-01 — WARNING (material coverage gap):** the live typed interactive matcher duplicates the raw matcher, but only a simple assistant delta is tested through `ProductEvent`; 21 integration calls cover the raw matcher instead.
   - Required behavioral tests: migrate the existing usage, tool start/update/finish, delegation lifecycle, compaction, self-healing, prompt failure/recovery, and no-op cases to `ProductEvent -> UiProjection::apply_product_event` or the internal production typed entry point, preserving exact payload assertions.
   - Completion criterion: no production raw bridge remains solely to support tests, and every established interactive matcher family has at least one fail-capable assertion through the live typed path.

No test files or production implementation were changed during this audit. Per the State A workflow, the orchestrator must obtain the required user choice before generating the missing tests; the confirmed implementation bug must be fixed before Phase 7 can be marked covered.

## State B Nyquist Re-audit After Authorized Gap Fixes (2026-07-13)

**Audit mode:** GENERIC-AGENT WORKAROUND for `gsd-nyquist-auditor`
**Audited fixes:** `376b600` (`fix(07): close raw event adapter bypasses`) and `6a7eac1` (`test(07): exercise typed product event adapters`)
**Result:** GAPS FILLED — both requirements and all ten planned tasks have passing automated evidence.
**Requirement status:** 2 COVERED, 0 PARTIAL (`COMPAT-01`, `COMPAT-02`)
**Plan-task status:** 10 COVERED, 0 PARTIAL, 0 MISSING

The re-audit independently read the Phase 7 plans, summaries, review, prior validation, requirements, both fix commits, and current implementation/tests. It used CodeGraph before direct source inspection and verified the following current-state properties:

1. The stable `pi_coding_agent::api` region does not export `CodingAgentEvent`. Production prefixes of `protocol/events.rs` and `interactive/event_bridge.rs` contain no `CodingAgentEvent` reference and expose only `CodingAgentProductEvent` public projection entry points. The protocol and interactive integration suites contain no raw-event reference.
2. `CodingEventBridge` computes typed context tokens by preferring non-zero `total_tokens`, then saturating-summing input, output, cache-read, and cache-write components. The typed-envelope regression supplies `30 + 20 + 5 + 0` with `total_tokens = 0` and asserts `context_tokens: Some(55)`.
3. Protocol and interactive behavior suites construct complete `CodingAgentProductEvent` envelopes and call `push_product_event` / `handle_product_event`, preserving the previous ordered output and exact payload assertions for messages, thinking, tools, delegation, compaction, self-healing, failures, recovery, capability changes, and terminal output. Internal live adapters continue to consume private `ProductEvent` values.
4. The boundary suite scopes the stable facade, production adapter signatures, adapter behavior tests, receiver/storage/transport paths, exactly-once raw admission conversion, and local deprecation suppressions. The raw enum remains permitted only at the private event definition/conversion/`EventService::emit` boundary and explicitly test-gated internal fixtures.

### Re-audit Commands

- `cargo test -p pi-coding-agent --test event_boundary_guards --test product_event_contract --test protocol_events --test interactive_event_bridge --test public_api --test json_mode --test rpc_mode --test interactive_sessions --quiet` — pass, 143 tests.
- `cargo test -p pi-coding-agent --lib public_event --quiet` — pass, 3 tests.
- `cargo test -p pi-coding-agent --lib coding_session::event_service::tests --quiet` — pass, 25 tests.
- `cargo test -p pi-coding-agent --lib coding_session::tests --quiet` — pass, 56 tests.
- `cargo test -p pi-coding-agent --lib interactive::r#loop::tests --quiet` — pass, 9 tests.
- `cargo test -p pi-coding-agent --lib protocol::rpc::event_queue --quiet` — pass, 2 tests.
- `cargo test -p pi-coding-agent --test interactive_event_bridge coding_event_bridge_maps_assistant_events --quiet` — pass; directly includes the typed-envelope `Some(55)` regression.
- `cargo test -p pi-coding-agent --test event_boundary_guards stable_facade_and_adapters_reject_raw_event_projection --quiet` — pass.
- `cargo fmt --check` — pass.
- `git diff --check` — pass.

The focused runs executed 238 distinct tests across the listed targets. Existing `load_plugins` and `ensure_idle` dead-code warnings remain non-blocking and are unrelated to Phase 7 compatibility behavior.

### Final Requirement-to-Task/Test Map

| Plan task | Requirement | Automated evidence | Status | Re-audit conclusion |
|---|---|---|---|---|
| 07-01 Task 1 | COMPAT-01, COMPAT-02 | `public_event`; `product_event_contract` | COVERED | Exhaustive typed payload ownership/conversion and the five terminal associations remain tested. |
| 07-01 Task 2 | COMPAT-01, COMPAT-02 | `product_event_contract`; `event_boundary_guards` | COVERED | Real receiver metadata, sequence, durability, terminal separation, and Serde shape remain green. |
| 07-02 Task 1 | COMPAT-01 | `protocol_events`; typed adapter source guard | COVERED | The production matcher and behavior suite both enter through typed product-event payloads; no public raw `push` path remains. |
| 07-02 Task 2 | COMPAT-01 | `protocol_events`; `json_mode`; `rpc_mode`; RPC queue units | COVERED | Ordered wire output, representative typed payloads, and lag/fresh-snapshot recovery remain green. |
| 07-03 Task 1 | COMPAT-01 | `interactive_event_bridge`; typed adapter source guard | COVERED | Usage, tool, delegation, compaction, self-healing, failure, recovery, and no-op behavior enter the typed envelope path; component fallback asserts `Some(55)`. |
| 07-03 Task 2 | COMPAT-01 | `interactive::r#loop::tests`; `interactive_sessions` | COVERED | Typed loop projection, cursor, recovery, navigation, fork continuity, profile changes, and exact `PartialCommit` attribution remain green. |
| 07-04 Task 1 | COMPAT-01, COMPAT-02 | `coding_session::tests`; `event_service::tests`; `public_api` | COVERED | Session/EventService/facade receiver behavior uses typed receivers and retains exact identity/payload assertions. |
| 07-04 Task 2 | COMPAT-02 | `event_boundary_guards`; `public_api`; `product_event_contract` | COVERED | Legacy subscription/receiver and duplicate broadcast remain deleted and guarded. |
| 07-05 Task 1 | COMPAT-02 | event ownership via `public_event`, EventService, and session units | COVERED | Retained/broadcast storage is typed-only; raw conversion occurs once at private admission, with only explicitly test-gated raw fixtures. |
| 07-05 Task 2 | COMPAT-01, COMPAT-02 | `event_boundary_guards`; all adapter/contract/public suites; format/diff checks | COVERED | Final guards now reject the stable raw export, known public raw adapter signatures, and raw first-party integration fixtures in addition to deleted storage/transport paths. |

### Final Requirement Status

| Requirement | Status | Automated closure evidence |
|---|---|---|
| COMPAT-01 | COVERED | Protocol, JSON/RPC, interactive, and first-party behavior suites consume typed product events; exact output/order/payload assertions pass, including the typed usage fallback regression. |
| COMPAT-02 | COVERED | Legacy receiver/subscription, duplicate broadcast, retained raw storage, stable raw export, and production raw adapter entry points are absent and source-guarded; the private raw admission boundary remains intentionally narrow. |

No production or test files were modified during this re-audit. `.planning/STATE.md` and `docs/next stage.md` were not touched.
