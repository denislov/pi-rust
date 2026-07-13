# Phase 3: Production Adapter Convergence - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md; this log preserves the alternatives considered.

**Date:** 2026-07-11
**Phase:** 3-production-adapter-convergence
**Areas discussed:** Migration waves and commit boundaries

---

## Migration Axis

| Option | Description | Selected |
|--------|-------------|----------|
| Adapter risk order | JSON/print -> RPC -> interactive ordinary work -> interactive navigation, completing behavior gates between risk layers | Yes |
| Operation-family migration | Migrate Prompt, Agent/Team, Profile/Delegation, Plugin, and Navigation across all adapters | |
| Mixed migration | Complete JSON/print, then group RPC and interactive work by operation family | |

**User's choice:** Adapter risk order.
**Notes:** The migration should make regressions attributable to one adapter/control boundary rather than one operation spread across several products.

---

## JSON And Print Split

| Option | Description | Selected |
|--------|-------------|----------|
| One plan, separate tasks | JSON, persistent print, and transient print use separate tasks/commits followed by one parity gate | Yes |
| Two plans | JSON separate from persistent/transient print | |
| Three plans | One plan for each path | |

**User's choice:** One plan with separate tasks and atomic commits.
**Notes:** Shared prompt semantics justify one plan; per-path commits retain diagnosis and rollback precision.

---

## RPC Split

| Option | Description | Selected |
|--------|-------------|----------|
| Split by control model | Background/select-driven operations separate from mutation/command operations | Yes |
| One RPC plan | Migrate all RPC commands together | |
| Per-operation plans | Prompt, agent/team, delegation, profile, and plugin/self-healing each separate | |

**User's choice:** Two plans split by control model.
**Notes:** `tokio::select!` control handling and product-event forwarding require an independent verification boundary.

---

## Interactive Split

| Option | Description | Selected |
|--------|-------------|----------|
| Three risk tiers | Background operations, profile/delegation mutations, then navigation transitions | Yes |
| Ordinary work plus navigation | Combine background and mutations; keep navigation separate | |
| One interactive plan | Migrate all interactive paths together | |

**User's choice:** Three risk-increasing plans.
**Notes:** Navigation is last and atomic because owner replacement, subscriber continuity, event sequence, snapshot refresh, and projection refresh form one compatibility boundary.

---

## the agent's Discretion

- Exact task names and additional atomic commits inside the six locked plan boundaries.
- Small typed helper use when it reduces duplication without creating a replacement operation facade.
- Focused deterministic verification commands and fixture reuse.

## Deferred Ideas

- Test migration and workflow-method deletion remain in Phase 4.
- Final guard hardening and Stage 9 closure remain in Phase 5.
- Stage 10 event contract convergence remains outside this milestone.
