# `0.3.1` Public API Freeze

This manifest freezes the normalized public API of the four active library
crates after the `pi-ai` reliability remediation release. Intentional changes
include explicit known/unknown cost state, compatibility-field dispositions,
removal of the DTO-only image category from `pi_ai::api`, and additive RPC and
ProductEvent usage metadata.

Full normalized `*.public-api.json`, raw `*.rustdoc.json`, workspace metadata,
compiler identity, and integrity checksums are generated under
`target/release-artifacts/0.3.1/public-api/` by:

```bash
PI_RUST_RELEASE_VERSION=0.3.1 scripts/release-api-snapshots.sh
```

Verify a regenerated artifact set against this manifest with:

```bash
PI_RUST_RELEASE_VERSION=0.3.1 \
PI_RUST_API_BASELINE_MANIFEST=docs/api-snapshots/0.3.1/SHA256SUMS \
scripts/release-api-snapshots.sh
```

The generator is pinned by `tools/public-api-snapshot/Cargo.lock`; release
artifacts record the selected Rust/Cargo toolchain separately.

