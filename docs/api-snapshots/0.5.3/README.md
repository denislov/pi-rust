# Public API Snapshot 0.5.3

This directory freezes the SHA-256 manifest for the normalized 0.5.3 Rust
public API surfaces and release toolchain. Full artifacts remain under
`target/release-artifacts/0.5.3/public-api/`.

Generate and verify this snapshot with:

```bash
PI_RUST_RELEASE_VERSION=0.5.3 scripts/release-api-snapshots.sh
PI_RUST_API_BASELINE_MANIFEST=docs/api-snapshots/0.5.3/SHA256SUMS \
  PI_RUST_RELEASE_VERSION=0.5.3 scripts/release-api-snapshots.sh
```

The `pi-ai`, `pi-agent-core`, and `pi-coding-agent` normalized surfaces are
checksum-identical to 0.5.2. The `pi-tui` surface changes only by adding the
derived `UnwindSafe` and `RefUnwindSafe` auto traits to `ProcessTerminal` after
the independent progress writer state was removed.

RPC `2.1`, ProductEvent `2.2`, and UI snapshot `2.1` remain unchanged.
