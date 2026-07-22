# pi-rust Architecture Index

## Status

This concise index is the normative entry point for `pi-rust` architecture from
the `0.4.x` train onward. Normative contracts, current implementation evidence,
decisions, migrations, and test procedures are intentionally separated.

The workspace is at `0.5.7`. Contracts introduced by completed version plans are
normative where the contract documents below say so; implementation facts remain
version-stamped in `current-state.md`. A document must label current facts
separately from target requirements.

## Completed Version Plan

- [`0.5.3-fullscreen-tui-runtime-hardening-plan.md`](0.5.3-fullscreen-tui-runtime-hardening-plan.md):
  fullscreen TUI runtime pressure/fault hardening and workspace `dead_code`
  convergence, completed with the workspace `0.5.3` release.
- [`0.5.4-delegation-runtime-and-child-agent-tui-plan.md`](0.5.4-delegation-runtime-and-child-agent-tui-plan.md):
  awaited terminal delegation results, operation-scoped child authorization,
  bounded child projections, and fullscreen child conversation pages,
  completed with the workspace `0.5.4` release.
- [`0.5.5-operation-tree-runtime-pressure-and-fault-hardening-plan.md`](0.5.5-operation-tree-runtime-pressure-and-fault-hardening-plan.md):
  operation-tree cancellation, authorization/provider/persistence fault
  injection, bounded reconnect pressure, child-page lifecycle hardening, and
  deterministic soak/performance evidence, completed with workspace `0.5.5`.
- [`0.5.6-fullscreen-tui-visual-hierarchy-and-interaction-polish-plan.md`](0.5.6-fullscreen-tui-visual-hierarchy-and-interaction-polish-plan.md):
  fullscreen visual hierarchy, responsive Context, completion geometry,
  semantic theme, transcript/table, accessibility, and release evidence,
  completed with workspace `0.5.6`. The frozen vocabulary and responsive policy
  are recorded in
  [`architecture/fullscreen-visual-contract-0.5.6.md`](architecture/fullscreen-visual-contract-0.5.6.md).
- [`0.5.7-built-in-helper-read-only-filesystem-capability-plan.md`](0.5.7-built-in-helper-read-only-filesystem-capability-plan.md):
  built-in delegated helper read-only filesystem capability correction,
  least-privilege regression coverage, and release convergence, completed with
  workspace `0.5.7`.

## Active Version Plan

No version plan is active after completion of 0.5.7.

## Queued Version Plan

No later version plan is queued.

## Normative Contracts

- [`principles.md`](architecture/principles.md): invariants and authority rules.
- [`dependency-rules.md`](architecture/dependency-rules.md): crate ownership,
  allowed dependency directions, and public boundary rules.
- [`runtime.md`](architecture/runtime.md): operation, session, event, recovery,
  adapter, and shutdown contracts.
- [`extension-platform.md`](architecture/extension-platform.md): capability,
  TypeScript/Wasm/WIT, state/fact, package, and Workbench contracts.
- [`testing.md`](architecture/testing.md): deterministic validation and evidence
  requirements.

## Evidence And Change History

- [`current-state.md`](architecture/current-state.md): version-stamped facts
  derived from source and tests.
- [`0.5.4-release-evidence.md`](0.5.4-release-evidence.md): delegation,
  child-page, protocol/API, soak, Extension, and workspace release gates.
- [`0.5.5-release-evidence.md`](0.5.5-release-evidence.md): operation-tree fault
  matrix, bounded pressure, protocol/API audit, repeated schedules, performance,
  and workspace release gates.
- [`0.5.6-release-evidence.md`](0.5.6-release-evidence.md): fullscreen visual,
  responsive, theme/accessibility, table geometry, smoke, performance, and
  workspace release gates.
- [`0.5.7-release-evidence.md`](0.5.7-release-evidence.md): built-in helper
  capability correction, least-privilege tests, API/protocol audit, repeated
  schedules, and workspace/Extension release gates.
- [`decisions/`](architecture/decisions/README.md): accepted, proposed, and
  scheduled ADRs with task/test traceability.
- [`migrations/`](architecture/migrations/README.md): historical baselines and
  migration records.

## Authority

If documents conflict, authority is:

1. accepted ADRs for the decision they own;
2. normative contract documents listed above;
3. source and tests for claims marked current;
4. version completion records and migration evidence;
5. plans and historical documents.

Plans cannot waive a normative invariant. Current-state evidence cannot silently
change a target contract. A deliberate change requires a superseding ADR, updated
downstream plans, tests, migration decisions, and protocol/public-API evidence.

## Workspace Shape

```text
pi-coding-agent -> pi-agent-core -> pi-ai
        |
        +-----------------------> pi-ai
        +-----------------------> pi-tui
```

`pi-mom`, `pi-pods`, and `pi-web-ui` remain reserved placeholders until activated
through their declared stable product boundaries. The root `pi-rust` binary is a
placeholder; `pi-coding-agent` is the user-facing executable.
