# Changes

## 0.4.0 - Unreleased

### Architecture And Planning

- Added the dependency-reviewed `0.4.x` release train and independent `0.4.0`
  through `0.4.5` release contracts.
- Split the monolithic architecture document into normative principles, runtime,
  extension-platform, dependency, and testing contracts, plus versioned current
  state, ADRs, and migration evidence.
- Accepted runtime-owner/finalization, `RecoveryPending`, extension grant/lease,
  state/fact, Workbench protocol, and isolated Wasm invocation decisions. Their
  disposable evidence now includes locked real-engine cancellation, epoch, fuel,
  memory/disposal, and TypeScript-to-WIT Component fixtures.

### Runtime Integrity

- Converged PluginLoad on the admitted operation identity and typed
  Completed/Failed/Aborted ProductEvent terminal evidence, with completion after
  capability generation installation.

### Release Status

- `RIF-005` and `RIF-006` are complete; `RIF-001` and `RIF-007` are next.
  Workspace packages remain at `0.3.1` until all `0.4.0` implementation, debt,
  and release gates close.

## 0.3.1 - 2026-07-18

### Added

- Added explicit known/unknown model-cost state and propagated it through
  transcripts, product events, replay, and RPC session statistics.
- Added AWS standard credential-chain and official SigV4 support for Bedrock,
  including named profiles, session credentials, cancellation, and redaction.
- Added a deterministic cross-provider contract matrix for every built-in API.

### Changed

- Provider streams now fail closed on truncation, malformed terminals, timeout,
  cancellation, and provider-declared failure; all HTTP providers share one
  retry, hook, option-validation, cancellation, and deadline implementation.
- SSE and OpenAI Responses parsing now support chunk-safe protocol processing,
  multiple interleaved outputs, structured failures, and safe unknown events.
- Compatibility metadata now exposes explicit request/runtime versus
  catalog-only dispositions, and generated catalog records are validated.
- RPC advances additively to protocol `2.1`; ProductEvent advances additively
  to `2.2`; UI Snapshot remains `2.1`.

### Removed

- Removed the DTO-only standalone image-generation category from the stable
  `pi_ai::api` facade; multimodal conversation images remain supported.
- Removed unused Codex WebSocket helpers. Explicit WebSocket or unknown
  transport selection now fails before network I/O.

### Migration

- See `docs/0.3.1-migration-guide.md` for public API, terminal, cost, transport,
  compatibility, and Bedrock credential changes.

### Release Status

- `PAIR-001` through `PAIR-012` and all completion criteria in
  `docs/0.3.1-pi-ai-remediation-plan.md` are closed.
- All workspace packages use version `0.3.1`; formatting, strict Clippy, full
  workspace tests, and the `0.3.1` public API freeze pass.
- The offline tmux TUI smoke suite was rerun successfully on 2026-07-18; no live
  provider credentials were used.

## 0.3.0 - 2026-07-17

### Added

- Added model-visible delegation target inventories, uniform asynchronous
  operation cancellation, durable session-tree labels, executable plugin UI
  actions, effective configuration handling, and runtime-backed tool
  authorization.
- Added a full-screen interactive application shell with responsive context,
  tips, composer, status, transcript interaction, mouse support, and focused
  overlays while retaining the supported inline path.
- Added truthful runtime-backed RPC capabilities, commands, state, statistics,
  multimodal prompts, compaction, session forks, cancellation, and
  authorization controls.

### Runtime Contracts

- Tool authorization is capability-bounded, cancellation-aware, reconnectable,
  redacted, and durably audited; unresolved requests recover as interrupted
  rather than approved.
- Product events and UI snapshots now project operation, change, delegation,
  usage, cost, context, and pending-authorization state from runtime owners.
- RPC remains protocol family `2.0`; ProductEvent and UI Snapshot advance
  additively to protocol family `2.1`. The durable session writer remains
  version `1`.

### Release Status

- All tasks and execution-debt entries in `docs/0.3-plan.md` are complete and
  closed.
- All workspace packages use the `0.3.0` workspace version.

## 0.2.0 - 2026-07-16

### Changed

- Unified all workspace packages under the root workspace version policy.
- Completed the breaking architecture convergence release train.
- Added reproducible architecture, public API snapshot, compatibility, and
  release gates.

### Boundaries

- The root `pi-rust` binary remains a placeholder and does not own provider,
  agent-runtime, session, product, or terminal UI behavior.
- The user-facing executable remains `pi-coding-agent`.

### Release Artifacts

- RPC, ProductEvent, and UI Snapshot protocol families are version `2.0`.
- The durable session writer remains version `1`.
- Public API freeze manifests are stored under `docs/api-snapshots/`.
- The completed architecture migration and release evidence are summarized in
  `docs/0.2-architecture-convergence-record.md`.
