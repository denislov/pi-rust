# Public API Snapshot 0.5.0

This directory freezes the SHA-256 manifest for the 0.5.0 Rust public API and
rustdoc artifacts. The full generated artifacts remain under
`target/release-artifacts/0.5.0/public-api/`.

Generate and verify this snapshot with:

```bash
PI_RUST_RELEASE_VERSION=0.5.0 scripts/release-api-snapshots.sh
PI_RUST_API_BASELINE_MANIFEST=docs/api-snapshots/0.5.0/SHA256SUMS \
  PI_RUST_RELEASE_VERSION=0.5.0 scripts/release-api-snapshots.sh
```

The 0.5.0 `pi-ai` diff intentionally removes Bedrock-specific `StreamOptions`
and `ProviderAuth` fields. The private image-generation experiment does not
appear in the public snapshot.
