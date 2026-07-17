# Changes

## 0.3.0 - 2026-07-17

### Changed

- Advanced to the workspace `0.3.0` version.

### Boundaries

- No provider API or wire-contract changes are introduced by this release.
- Product profiles, delegation policy, authorization, sessions, RPC, and TUI
  behavior remain outside `pi-ai`; provider requests continue to receive
  ordinary canonical tool schemas and conversation content.

## 0.2.0 - 2026-07-16

### Breaking Changes

- Replaced flat, root, and implementation-module access with the categorized
  `pi_ai::api` facade.
- Removed process-global provider runtime mutation from the supported API.
- Provider registries and clients are now explicitly scoped values.

### API Categories

- `conversation`: canonical messages, content blocks, contexts, and usage.
- `model`: model metadata, lookup, capability, and cost contracts.
- `provider`: provider construction and provider-neutral execution inputs.
- `registry`: scoped provider registry, client, and authentication resolution.
- `transport`: HTTP/SSE transport, retry, timeout, and error contracts.
- `stream`: request options, assistant stream events, and terminal outcomes.
- `testing`: faux providers and deterministic fixtures behind `test-support`.

### Boundaries

- The crate remains independent of agent sessions, product events, CLI/RPC,
  persistence, plugins, and TUI behavior.
- Downstream crates must use their documented dependency-edge allowlists rather
  than treating the entire facade as implicitly available.

### Tests

- Provider request/response mapping, authentication/redaction, transport,
  model catalog, and scoped registry behavior remain owner-tested here.
- Live provider access is not required by the ordinary test suite.
