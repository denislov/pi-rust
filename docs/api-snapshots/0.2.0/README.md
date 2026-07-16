# `0.2.0` Public API Freeze

This manifest is the final normalized public API freeze for the four active
library crates. It intentionally matches the reviewed `0.2.0-alpha.1` baseline;
the RC and GA version actions introduced no public item changes.

Full normalized `*.public-api.json`, raw `*.rustdoc.json`, workspace metadata,
compiler identity, and integrity checksums are generated under
`target/release-artifacts/0.2.0/public-api/` by:

```bash
PI_RUST_API_BASELINE_MANIFEST=docs/api-snapshots/0.2.0/SHA256SUMS \
  scripts/release-api-snapshots.sh
```

The generator is pinned by `tools/public-api-snapshot/Cargo.lock`; release
artifacts record the selected Rust/Cargo toolchain separately.
