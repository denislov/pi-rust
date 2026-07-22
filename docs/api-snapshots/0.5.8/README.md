# Public API Snapshot 0.5.8

This directory freezes the SHA-256 manifest for the normalized 0.5.8 Rust
public API surfaces and release toolchain. Full artifacts remain under
`target/release-artifacts/0.5.8/public-api/`.

Generate and verify this snapshot with:

```bash
PI_RUST_RELEASE_VERSION=0.5.8 scripts/release-api-snapshots.sh
PI_RUST_API_BASELINE_MANIFEST=docs/api-snapshots/0.5.8/SHA256SUMS \
  PI_RUST_RELEASE_VERSION=0.5.8 scripts/release-api-snapshots.sh
```

All four normalized Rust public API surfaces are checksum-identical to 0.5.7.
The runtime interval and Context summary changes remain private adapter details.

RPC remains `2.1`, ProductEvent remains `2.2`, and UI snapshot remains `2.2`.
No persistence or Extension schema migration is required.
