# Public API Snapshot 0.5.1

This directory freezes the SHA-256 manifest for the 0.5.1 Rust public API and
rustdoc artifacts. The full generated artifacts remain under
`target/release-artifacts/0.5.1/public-api/`.

Generate and verify this snapshot with:

```bash
PI_RUST_RELEASE_VERSION=0.5.1 scripts/release-api-snapshots.sh
PI_RUST_API_BASELINE_MANIFEST=docs/api-snapshots/0.5.1/SHA256SUMS \
  PI_RUST_RELEASE_VERSION=0.5.1 scripts/release-api-snapshots.sh
```

The 0.5.1 `pi-agent-core` diff removes the retired core Branch Summary,
test-only Session Context, Agent-turn node, Harness/Proxy, and `TreeFilterMode`
facade contracts. It adds the canonical `BeforeProviderRequestHook` alias to
`api::agent`; the other stable package surfaces are unchanged.
