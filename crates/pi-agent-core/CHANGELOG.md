# Changes

## 0.3.0 - 2026-07-17

### Runtime

- Extended provider-neutral user input handling so product adapters can submit
  canonical multimodal content without introducing RPC types into the core.
- Made the asynchronous before-tool hook gate cancellation-aware and associated
  with the active execution context.
- Delayed tool-start events until hooks and product authorization complete, so
  a proposed or blocked call is never reported as executing.

### Boundaries

- Product authorization policy, pending decisions, durable audit facts,
  operation scheduling, and adapter presentation remain owned by
  `pi-coding-agent`.

## 0.2.0 - 2026-07-16

### Breaking Changes

- Replaced flat and implementation-module access with the categorized
  `pi_agent_core::api` facade.
- Removed legacy session storage and coding-agent product ownership from the
  supported core surface.
- Removed obsolete `PrepareContextNode` and `DecideStopOrToolsNode` test seams
  after migrating tests to the canonical agent-turn Flow nodes.

### Runtime

- Converged on one provider-neutral agent loop and one canonical agent-turn
  Flow.
- Kept tools, hooks, resources, compaction, transcripts, Flow primitives, and
  execution-environment contracts provider-neutral.
- Narrowed the `pi-ai` dependency to explicit categorized items for canonical
  conversation, model, streaming, and scoped client behavior.

### Boundaries

- Product sessions, durable persistence, operation scheduling, product events,
  plugins, RPC, CLI, and TUI policy remain owned by `pi-coding-agent`.
- Provider-specific wire representations and registries are not re-exported as
  downstream escape hatches.
- Deterministic harness and node-level facilities are available only through
  the non-default `api::testing` facade.

### Tests

- Coverage is focused on agent state transitions, tool ordering/cancellation,
  hooks, Flow semantics, compaction, resources, execution, and transcripts.
- Duplicate immediate-completion and private-topology tests were removed.
