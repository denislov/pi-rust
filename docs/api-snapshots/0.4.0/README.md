# `0.4.0` Public API Freeze

This manifest records the normalized public API of the four active library
crates after the 0.4.0 runtime ownership, operation descriptor, recovery, and
projection convergence work.

Regenerate the release artifacts under the repository `target/` directory with:

```bash
PI_RUST_RELEASE_VERSION=0.4.0 scripts/release-api-snapshots.sh
```

Verify the generated artifacts against this manifest with:

```bash
PI_RUST_RELEASE_VERSION=0.4.0 \
PI_RUST_API_BASELINE_MANIFEST=docs/api-snapshots/0.4.0/SHA256SUMS \
scripts/release-api-snapshots.sh
```

The snapshot tool records the selected Rust/Cargo toolchain in the generated
artifact directory. API changes after this freeze require an explicit release
plan and compatibility decision.
