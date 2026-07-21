# Public API Snapshot 0.5.5

This directory freezes the SHA-256 manifest for the normalized 0.5.5 Rust
public API surfaces and release toolchain. Full artifacts remain under
`target/release-artifacts/0.5.5/public-api/`.

Generate and verify this snapshot with:

```bash
PI_RUST_RELEASE_VERSION=0.5.5 scripts/release-api-snapshots.sh
PI_RUST_API_BASELINE_MANIFEST=docs/api-snapshots/0.5.5/SHA256SUMS \
  PI_RUST_RELEASE_VERSION=0.5.5 scripts/release-api-snapshots.sh
```

The normalized `pi-ai`, `pi-agent-core`, and `pi-tui` surfaces are
checksum-identical to 0.5.4. `pi-coding-agent` additively exposes
`CodingSessionError::SessionWriteRejected { message }`; its migration and
stable `session_write_rejected` code are documented in
`docs/0.5.5-migration-guide.md`.

RPC remains `2.1`, ProductEvent remains `2.2`, and UI snapshot remains `2.2`.
No session/outbox/manifest schema version changes are required.
