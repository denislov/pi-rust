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
