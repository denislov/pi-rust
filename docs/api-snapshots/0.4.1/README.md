# `0.4.1` Public API Freeze

This directory records the release-manifest checksum for the stable library
surfaces after Agent/workflow convergence. The implementation changes remain
internal: no extension-facing execution seam or protocol-major change is part
of `0.4.1`.

Generate release artifacts from the workspace root:

```bash
PI_RUST_RELEASE_VERSION=0.4.1 scripts/release-api-snapshots.sh
```

Verify them against this manifest:

```bash
PI_RUST_RELEASE_VERSION=0.4.1 \
PI_RUST_API_BASELINE_MANIFEST=docs/api-snapshots/0.4.1/SHA256SUMS \
scripts/release-api-snapshots.sh
```

Generated JSON and toolchain metadata remain under
`target/release-artifacts/0.4.1/public-api/`; only the release checksum manifest
is checked in.
