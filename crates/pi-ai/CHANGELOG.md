# Changes

## 0.4.2 - 2026-07-20

### Changed

- Advanced to the workspace `0.4.2` version; provider contracts and behavior
  remain independent of the coding-agent Extension runtime.

## 0.3.1 - Unreleased

### Added

- Added `CompatibilityDisposition` and
  `compatibility_field_disposition()` for explicit runtime/catalog metadata.
- Added AWS standard credential-chain loading and official AWS SigV4 signing
  for Bedrock with isolated profile and redaction tests.
- Added generated-catalog validation, explicit unknown-price semantics, and a
  cross-provider behavioral matrix covering all nine built-in APIs.

### Changed

- `Done` is now reserved for provider-confirmed `Stop`, `Length`, and `ToolUse`;
  EOF, malformed streams, provider failures, timeout, and cancellation emit one
  error terminal, and `complete()` rejects invalid custom-provider terminals.
- `timeout_ms` is one invocation deadline across hooks, credential resolution,
  retries, requests, and body streaming. Unsupported explicit options return
  typed errors before network I/O.
- SSE parsing is incremental and bounded across arbitrary UTF-8 chunks and line
  endings. OpenAI Responses supports structured terminal failures, safe unknown
  events, multiple output items, and strict final tool arguments.
- Anthropic compatibility flags now suppress unsupported temperature, force
  adaptive thinking, and control tool cache metadata. OpenAI-compatible strict
  schemas, token fields, roles, usage, and reasoning fields remain typed.
- Secret-bearing options, auth values, and response headers are redacted from
  Debug/diagnostics and skipped from serialization.

### Removed

- Removed `pi_ai::api::images`; internal DTO experiments remain private and
  multimodal conversation image blocks are unchanged.
- Removed unused Codex WebSocket URL/frame helpers; only SSE is accepted.

### Migration And Evidence

- Migration guidance is in `docs/0.3.1-migration-guide.md`.
- The completed ledger and release evidence are in
  `docs/0.3.1-pi-ai-remediation-plan.md`; the API freeze manifest is in
  `docs/api-snapshots/0.3.1/SHA256SUMS`.

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
