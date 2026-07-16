# Changes

## 0.2.0 - 2026-07-16

### Breaking Changes

- Established categorized `pi_coding_agent::api` scenarios as the supported
  embedding surface.
- Removed root compatibility exports, implementation-module imports, the
  `coding_session/` migration container, and the centralized compatibility
  `CodingAgentEvent` path.
- Bumped RPC, ProductEvent, and UI Snapshot live protocol families to `2.0`.
  Protocol major `1` is rejected rather than supported through a fallback.

### Runtime And Ownership

- Converged operation admission, scheduling, dispatch, control, operation
  identity, and terminal outcome association onto one runtime path.
- Made operation-local capability snapshots the only authorization language for
  model, filesystem, shell, plugin, and delegation behavior.
- Kept `SessionEvent` as the durable source of session facts; the durable writer
  remains version `1`.
- Converged on one typed ProductEvent stream and one UI Snapshot/reconnect
  contract for all adapters.
- Moved configuration, session selection, and resource policy into the app
  layer; print, JSON, RPC, and interactive adapters are thin projections.

### Source Structure

- Runtime, operations, services, sessions, events, plugins, tools, profiles,
  resources, app, protocol, and adapters each have a dedicated owner tree.
- Product types do not leak into `pi-ai`, `pi-agent-core`, or `pi-tui`.

### Tests

- Consolidated product integration coverage into eleven test targets.
- Retained admission/concurrency, durability/recovery, protocol, capability,
  tool-safety, adapter, configuration, and terminal lifecycle contracts.
- Removed duplicate/private-topology tests and the ignored wall-clock render
  timing probe; the final crate suite has no ignored tests.
