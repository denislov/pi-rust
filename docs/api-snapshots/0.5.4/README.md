# Public API Snapshot 0.5.4

This directory freezes the SHA-256 manifest for the normalized 0.5.4 Rust
public API surfaces and release toolchain. Full artifacts remain under
`target/release-artifacts/0.5.4/public-api/`.

Generate and verify this snapshot with:

```bash
PI_RUST_RELEASE_VERSION=0.5.4 scripts/release-api-snapshots.sh
PI_RUST_API_BASELINE_MANIFEST=docs/api-snapshots/0.5.4/SHA256SUMS \
  PI_RUST_RELEASE_VERSION=0.5.4 scripts/release-api-snapshots.sh
```

The normalized `pi-ai`, `pi-agent-core`, `pi-tui`, and `pi-coding-agent`
surfaces are checksum-identical to 0.5.3. The delegation result and child-page
projection remain product-runtime internals rather than expanding the stable
embedding facade.

RPC remains `2.1` and ProductEvent remains `2.2`. UI snapshot advances from
`2.1` to `2.2` with bounded retained child ProductEvents for child-page
reconnect hydration.
