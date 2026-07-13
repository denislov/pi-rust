# Phase 9: Lifecycle Association, Guards, and Closure - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md; this log preserves the alternatives considered.

**Date:** 2026-07-14
**Phase:** 09-lifecycle-association-guards-and-closure
**Areas discussed:** Detach/close/shutdown semantics, operation/event association, guard and final verification scope, RPC/Interactive lifecycle projection

---

## Detach / Close / Shutdown Semantics

| Question | Alternatives considered | Selected |
|----------|-------------------------|----------|
| State retained after detach | Preserve all recoverable state; preserve cursor/terminal only; clear all; separate detach/permanent close | Preserve all recoverable state |
| Active operation on detach | Continue; abort Prompt only; cancel any client-submitted operation | Continue under session ownership |
| Repeated/stale detach | Typed idempotent outcome; uniform success; errors after first success | Typed `Detached` / `AlreadyDetached` / `StaleGeneration` |
| Shutdown ordering | Close admission/control and drain; request Abort; close immediately | Close admission/control, drain active operation, publish terminal and lifecycle events, close receivers |

**User's choices:** Recommended option selected for all four questions.
**Notes:** Detach is a recoverable connection transition and must not become an operation cancellation policy.

---

## Operation / Event Association

| Question | Alternatives considered | Selected |
|----------|-------------------------|----------|
| Operation coverage | Closed three-way matrix; terminal events for all; existing five only | Closed `TerminalAssociated` / `OutcomeOnly` / `NotApplicable` matrix |
| Terminal cardinality | Exactly one; at least one; event optional | Exactly one root terminal event per admitted associated operation id |
| `PartialCommit` identity | Preserve id and uncertainty; remain running; create recovery id | Preserve original id and explicit durability/terminal uncertainty |
| `OutcomeOnly` acknowledgement | Outcome plus submitted terminal; synchronous only; generic completion event | Outcome plus submitted terminal with outcome acknowledgement |

**User's choices:** Recommended option selected for all four questions.
**Notes:** The matrix is fail-closed. Recovery cannot fabricate a second terminal event or operation id.

---

## Guard And Final Verification Scope

| Question | Alternatives considered | Selected |
|----------|-------------------------|----------|
| Adapter discovery | Recursive discovery/classification; fixed allowlist; forbidden-symbol scan only | Recursive discovery plus explicit classification |
| Compile-fail evidence | Exact diagnostic contract; symbol name only; any failure | File/span, code/symbol, diagnostic fragment, and positive fixture |
| Completion gates | All layers blocking; workspace tests only; risk-tiered | All focused/crate/workspace/format/audit/fixture/diff gates block |
| Security scope | Complete authority audit; dispatcher/control only; dependency/unsafe only | Complete public capability and authority audit |

**User's choices:** Recommended option selected for all four questions.
**Notes:** New adapter roots and unrelated compile failures must fail the guards rather than silently widening the boundary.

---

## Lifecycle Projection In RPC And Interactive Adapters

| Question | Alternatives considered | Selected |
|----------|-------------------------|----------|
| RPC detach trigger | Explicit command plus shared cleanup; transport only; explicit only | Explicit command and transport cleanup use one idempotent path |
| Wire compatibility | Separate typed lifecycle messages; extend existing state; generic success/error | Separate typed response/event; existing shapes unchanged |
| Interactive exit | Distinguish owner; always shutdown; detach only | UI detaches; explicit top-level runtime owner shuts down |
| Old handle behavior | Typed lifecycle rejection; auto-recovery; generic error | Typed rejection, closed receiver, no auto-reconnect/retry/retarget |

**User's choices:** Recommended option selected for all four questions.
**Notes:** Lifecycle projection is additive and must preserve existing RPC, JSON/print, replay, and control output.

---

## the agent's Discretion

- Exact Rust type and method names within established `CodingAgent*` conventions.
- Internal lock/drain algorithm and deterministic fixture layout, within existing ownership and ordering constraints.
- Concrete operation membership in the three-way association matrix, derived from live behavior and the Phase 6 inventory.

## Deferred Ideas

None. Separately named runtime ownership and multi-session daemon routing remain previously documented future work.

