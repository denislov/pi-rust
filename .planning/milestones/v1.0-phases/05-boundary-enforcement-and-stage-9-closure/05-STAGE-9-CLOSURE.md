# Stage 9 Canonical Operation Runtime Closure

**Authority:** This is the single authoritative Stage 9 closure record. Historical plans and design documents link here rather than serving as completion evidence.

## Closed Boundary

Stage 9 closes the public live-session operation boundary at `pi_coding_agent::api` and `CodingAgentSession::run(CodingAgentOperation)`. All JSON, print, RPC, interactive, owner, public API, and integration callers use typed canonical operations. The 16 replaced broad workflow methods are deleted; retained construction, open/resume, snapshot/query, event-subscription, control, and static repository helpers are not alternate operation facades.

The final boundary is enforced by:

- a recursive, fail-closed adapter inventory and receiver-aware deleted-method/source scan;
- production deprecation-suppression rejection with test-only regions excluded;
- external dependent-crate compile-pass coverage for all 15 public operations and outcome/support families;
- external compile-fail coverage for internal operation/dispatch, services, plugin options/registries, and Flow contracts.

## Verification Identity

- Verification window (UTC): `2026-07-13T02:07:54Z` to `2026-07-13T02:24:00Z`
- Verified pre-report-write HEAD: `fa1317d093c635a0c24bb23c712f79820657f811`
- Verified worktree identity: `fa1317d093c635a0c24bb23c712f79820657f811` plus the authority-document edits listed in the report; the report write itself is the only subsequent uncommitted artifact.
- Report self-reference: the main evidence covers the authority-edited tree immediately before this report's final evidence write. Because writing this report changes the worktree, a separate post-write `git diff --check` hygiene result is recorded below. No clean-worktree claim is made for an uncommitted report-bearing tree.

## Source Audits

| UTC | Exact command | Scope | Result |
|---|---|---|---|
| `2026-07-13T02:07:54Z` | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards -- --nocapture` | Recursive JSON/print/RPC/interactive production inventory; receiver-aware 16-method deletion ledger; compatibility synonyms; production `allow(deprecated)` | PASS; 16 tests after code-review fix, zero guard violations |
| `2026-07-13T02:08:26Z` | `rg -n 'allow\\(deprecated\\)' crates/pi-coding-agent/src/{print_mode.rs,protocol,interactive}` plus receiver-aware guard assertions | `crates/pi-coding-agent/src/{lib.rs,print_mode.rs,protocol,interactive,coding_session}` | PASS; no production local suppression; guard inventory reports zero unexpected calls/definitions |

## Command Evidence

| UTC | Exact command | Status | Meaningful conclusion |
|---|---|---|---|
| `2026-07-13T02:07:54Z` | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --test api_boundary_guards --test public_api -- --nocapture` | PASS | 45 focused tests after code-review fix: 16 + 6 + 23 |
| `2026-07-13T02:07:54Z` | `cargo fmt --check` | PASS | Formatting clean |
| `2026-07-13T02:08:26Z` | `cargo test -p pi-coding-agent` | PASS | 653 passed, 1 ignored |
| `2026-07-13T02:08:26Z` | `cargo check -p pi-coding-agent` | PASS | Crate compiled; only existing dead-code/deprecation warnings |
| `2026-07-13T02:19:00Z` | `cargo test --workspace` | PASS (elevated, outside restricted sandbox) | All workspace tests passed; restricted run's socket PermissionDenied was environment-only |
| `2026-07-13T02:24:00Z` | `cargo check --workspace` | PASS (elevated, outside restricted sandbox) | All workspace crates and doctests compiled |
| `2026-07-13T02:24:00Z` | `git diff --check` | PASS | Pre-report-write diff hygiene clean |
| `2026-07-13T02:24:00Z` | `git diff --check` | PASS | Post-report-write self-reference hygiene clean |

Complete Cargo logs are intentionally omitted. Each row records the exact command, status, and concise conclusion needed to reproduce the evidence.

## Requirement Closure

| Requirement | Closure evidence |
|---|---|
| GUARD-01 | Recursive centralized adapter guard rejects replaced broad workflow calls. |
| GUARD-02 | Production adapter scan rejects local deprecation suppression. |
| GUARD-03 | Independent external consumer compiles against the complete stable facade. |
| GUARD-04 | Compiler visibility and sealed contracts reject internal runtime categories; scanning is retained only for repository/source boundaries. |
| CLOSE-01 | Final source audits report zero unexpected definitions, calls, compatibility synonyms, or production suppressions. |
| CLOSE-02 | Format, focused/crate tests and checks, and diff checks pass. |
| CLOSE-03 | Workspace tests and checks pass. |
| CLOSE-04 | Current authority documents link here and bound Stage 10 explicitly. |

## Stage 10 Handoff

Stage 10 is deferred and was not implemented or planned in Stage 9. Its bounded inventory is:

- `crates/pi-coding-agent/src/coding_session/event.rs`: `ProductEvent` still stores `compatibility_event: CodingAgentEvent`; family/kind, terminal status, operation identity, and durability are classified from that compatibility payload. Stage 10 must introduce typed payload families without changing event ordering, sequence identity, terminal classification, durability facts, or replay behavior.
- `crates/pi-coding-agent/src/coding_session/mod.rs`: deprecated `CodingAgentSession::subscribe()` and `CodingAgentEventReceiver` remain beside the product-event subscription. Stage 10 must migrate remaining consumers before deleting compatibility subscription paths.
- `crates/pi-coding-agent/src/coding_session/event_service.rs` and first-party projections under `src/protocol/` and `src/interactive/`: publication, replay window, protocol mapping, and UI projection must preserve response shapes, subscriber continuity, event/control multiplexing, and user-visible ordering.
- `crates/pi-coding-agent/tests/event_boundary_guards.rs`, `tests/public_api.rs`, and event/session integration coverage: re-verify compatibility consumers, public event signatures, sequencing, replay, and terminal/durability semantics during Stage 10.

Stage 10 does not reopen the Stage 9 operation dispatcher, restore broad workflow methods, redesign RPC wire commands, or alter interactive presentation semantics.
