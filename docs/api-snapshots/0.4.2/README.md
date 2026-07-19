# `0.4.2` Public API Freeze

This directory records the release-manifest checksum after minimum
TypeScript/Wasm Extension kernel convergence. The release intentionally removes
the generic `pi-agent-core` Flow facade and the unreachable
`CodingAgentOperation::PluginCommand` operation/outcome. Contribution
productization remains Skipped; `PluginLoad` and trusted-host package activation
are the retained minimum Extension surface.

Generate release artifacts from the workspace root:

```bash
PI_RUST_RELEASE_VERSION=0.4.2 scripts/release-api-snapshots.sh
```

Verify them against this manifest:

```bash
PI_RUST_RELEASE_VERSION=0.4.2 \
PI_RUST_API_BASELINE_MANIFEST=docs/api-snapshots/0.4.2/SHA256SUMS \
scripts/release-api-snapshots.sh
```

Generated JSON and toolchain metadata remain under
`target/release-artifacts/0.4.2/public-api/`; only the release checksum manifest
is checked in.
