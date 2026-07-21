# Changes

## 0.5.3 - 2026-07-21

### Changed

- Advanced with the workspace to `0.5.3`; provider-neutral agent, tool,
  transcript, resource, compaction, and execution contracts are unchanged.
- Documented the one retained shared integration-fixture lint scope used by
  Cargo's separately compiled core test targets.

## 0.5.2 - 2026-07-20

### Changed

- Advanced with the workspace to `0.5.2`; the provider-neutral Agent,
  transcript, resource, compaction, and execution contracts remain unchanged
  by product runtime convergence.

## 0.5.0 - 2026-07-20

### Changed

- Removed retired Bedrock fields from the test-support `StreamOptionsPatch`
  surface after `pi-ai` removed the provider and its authentication options.
- Runtime behavior remains provider-neutral; the crate advances with the
  workspace to `0.5.0`.

## 0.5.1 - 2026-07-20

### Runtime And API

- Removed the unused core Branch Summary workflow alternative and the test-only
  Session Context/Memory subsystem while retaining provider-neutral
  summarization, conversion, transcript, and execution contracts.
- Completed enum-only Agent-turn transitions with compile-time variant
  coverage, reduced message/config/resource
  cloning and duplicate event retention, replaced the parallel Harness/Proxy
  runtime with narrow production-path fixtures, and consolidated resource-loader
  mechanics.
- Removed the retired branch/session/node/harness/proxy facade contracts, moved
  product-specific tree filtering out of the core, and added
  `BeforeProviderRequestHook` to `api::agent`.
- Recorded implementation, downstream migration, public API, architecture, and
  release evidence in `docs/0.5.1-pi-agent-core-lean-runtime-plan.md`.

## 0.4.2 - 2026-07-20

### Breaking Changes

- Removed the generic `api::flow` graph API and test facade after all production
  consumers converged on typed runners.
- Removed compatibility agent-turn node wrappers and `AgentTurnFlow`; the agent
  loop now runs only through `AgentTurnRunner` and `AgentTurnDecision`.

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
