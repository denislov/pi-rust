# pi-rust Architecture Index

## Status

This concise index is the normative entry point for `pi-rust` architecture from
the `0.4.x` train onward. Normative contracts, current implementation evidence,
decisions, migrations, and test procedures are intentionally separated.

The workspace is at `0.5.3`. Contracts introduced by completed version plans are
normative where the contract documents below say so; implementation facts remain
version-stamped in `current-state.md`. A document must label current facts
separately from target requirements.

## Completed Version Plan

- [`0.5.3-fullscreen-tui-runtime-hardening-plan.md`](0.5.3-fullscreen-tui-runtime-hardening-plan.md):
  fullscreen TUI runtime pressure/fault hardening and workspace `dead_code`
  convergence, completed with the workspace `0.5.3` release.

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
