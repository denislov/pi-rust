# Changes

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
