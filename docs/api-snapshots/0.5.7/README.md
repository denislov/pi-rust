# Public API Snapshot 0.5.7

This directory freezes the SHA-256 manifest for the normalized 0.5.7 Rust
public API surfaces and release toolchain. Full artifacts remain under
`target/release-artifacts/0.5.7/public-api/`.

Generate and verify this snapshot with:

```bash
PI_RUST_RELEASE_VERSION=0.5.7 scripts/release-api-snapshots.sh
PI_RUST_API_BASELINE_MANIFEST=docs/api-snapshots/0.5.7/SHA256SUMS \
  PI_RUST_RELEASE_VERSION=0.5.7 scripts/release-api-snapshots.sh
```

All four normalized Rust public API surfaces are checksum-identical to 0.5.6.
The built-in helper correction changes private profile data and child capability
composition only.

RPC remains `2.1`, ProductEvent remains `2.2`, and UI snapshot remains `2.2`.
No persistence, profile-schema, or Extension schema migration is required.
