# Changes

## 0.5.0 - 2026-07-20

### Changed

- Advanced with the workspace to `0.5.0` and retained scoped registration for
  all eight remaining `pi-ai` built-in providers. Bedrock is no longer
  registered or advertised by the lower-level provider runtime.
- Product runtime, adapters, protocols, Extension behavior, and TUI behavior
  are otherwise unchanged by the `pi-ai` lean-runtime release.

## 0.5.2 - Unreleased

### Planned

- Remove the stateless `WorkflowService`, simplify fixed typed operation
  pipelines, converge operation/outcome/adapter projections, contract the stable
  facade, separate TUI-local state from runtime projection, and consolidate
  product tool output and test infrastructure.
- Retain the complete current TypeScript/Wasm Extension framework and PluginLoad
  lifecycle, including package quarantine, grants/leases, Host-call checks,
  per-invocation Wasmtime isolation, durability, restart, and public contracts.
- Track implementation, migration, API/protocol snapshots, architecture,
  Extension conformance, TUI smoke, and release evidence in
  `docs/0.5.2-pi-coding-agent-lean-runtime-plan.md`.

## 0.4.2 - 2026-07-20

### Extension Kernel Replacement

- Added strict Manifest/package/lock quarantine, immutable installation,
  generated TypeScript SDK conformance, grant-backed activation, revocable
  operation leases, and lease-only Host API handles for the replacement kernel.
- Added the ADR-012 core/extension handler target boundary. Validated extension
  inventories now project package-bound data-only handler references and cannot
  deserialize or address built-in Rust handlers.
- Added the minimum Wasmtime `46.0.1` Component runtime: pre-admission immutable
  compilation cache, isolated invocation stores, WIT-generated async Host/guest
  bindings, lease checks, epoch/fuel/memory/deadline/output limits, and an offline
  TypeScript-to-Wasm invocation gate. The crate now requires Rust 1.94.
- Removed the Lua/`mlua` and native Rust contribution-provider extension paths.
  The retained Extension surface is the minimum Wasm framework only; no empty
  PluginService, prompt-hook forwarding, or plugin capability set remains.
- Removed generic Flow wrappers from product operations and renamed their owner
  to `WorkflowService`; fixed operations now expose only typed runners.
- Removed the unreachable `PluginCommand` operation/outcome, RPC and TUI
  presentation paths, adapter-only contribution DTOs, and empty plugin
  capability carrier. `PluginLoad` remains the minimum Wasm activation owner.

## 0.4.0 - Unreleased

### Runtime Integrity

- PluginLoad now reuses its admitted operation identity through persistent
  transactions, public outcomes, projection, and ProductEvents.
- PluginLoad publishes one typed Completed, Failed, or Aborted root terminal;
  cancellation is no longer represented as failure, and completion follows
  capability generation installation.

### Release Status

- This is partial `RIF-003` evidence. The owning task remains planned behind the
  `0.4.0` supervisor, finalization, and outbox prerequisites.

## 0.3.0 - 2026-07-17

### Agent Runtime

- Projected deterministic, policy-filtered agent and team inventories into
  model-visible delegation tool schemas while retaining runtime validation.
- Unified typed cancellation across prompts, tools, plugins, delegation,
  teams, compaction, branch summaries, self-healing edits, and session
  operations.
- Persisted session-tree label edits through the session owner and made them
  survive replay and reopen.
- Enforced reference-only plugin UI actions: actions and keybindings resolve
  to validated commands or dialogs, and dialog submission executes the typed
  command path directly.
- Connected supported shell and runtime settings to their production consumers
  and rejected or removed inert configuration surfaces.

### Tool Authorization

- Added typed, capability-bounded tool authorization with allow-once,
  operation-scoped grants, denial, stale-decision protection, cancellation,
  redacted previews, and deterministic pending queues.
- Added durable authorization request/resolution facts, reconnect snapshots,
  fail-closed persistence, and startup interruption of unresolved requests.
- Added automatic keyboard-complete authorization and delegation confirmation
  surfaces for inline and full-screen modes, plus typed RPC discovery and
  resolution commands. Print and JSON modes fail closed instead of waiting.

### RPC

- Replaced adapter-local placeholders with runtime-backed capabilities,
  operation control, session state, statistics, settings, pending
  authorizations, and recovery snapshots.
- Added multimodal prompts, manual compaction, parent-session forks, and
  uniform abort handling while preserving stdout/stderr and JSONL cleanliness.
- Kept RPC at protocol family `2.0` and advanced ProductEvent and UI Snapshot
  additively to protocol family `2.1`.

### Interactive TUI

- Added a responsive full-screen shell with Conversation, Context, Tips,
  Composer, and Status regions plus shared focused overlays.
- Added selectable and foldable transcript blocks, tool/thinking/delegation
  previews, image rendering, independent scrolling, stable scroll anchoring,
  keyboard navigation, mouse hit testing, and responsive context views for
  operations, changes, agents, and usage.
- Preserved the inline interaction path and terminal cleanup guarantees.

### Release Status

- Completed all acceptance criteria and closed all execution debt recorded in
  `docs/0.3-plan.md`.

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
