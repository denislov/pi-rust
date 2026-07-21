# Public API Snapshot 0.5.6

This directory freezes the SHA-256 manifest for the normalized 0.5.6 Rust
public API surfaces and release toolchain. Full artifacts remain under
`target/release-artifacts/0.5.6/public-api/`.

Generate and verify this snapshot with:

```bash
PI_RUST_RELEASE_VERSION=0.5.6 scripts/release-api-snapshots.sh
PI_RUST_API_BASELINE_MANIFEST=docs/api-snapshots/0.5.6/SHA256SUMS \
  PI_RUST_RELEASE_VERSION=0.5.6 scripts/release-api-snapshots.sh
```

The normalized `pi-ai`, `pi-agent-core`, and `pi-coding-agent` surfaces are
checksum-identical to 0.5.5. `pi-tui` additively exposes
`Editor::render_input(width)` and `Editor::render_assistance(width)` so an
embedding shell can lay out editor input and autocomplete assistance in
separate rectangles. Existing `Component::render(width)` composition remains
unchanged; migration guidance is in `docs/0.5.6-migration-guide.md`.

RPC remains `2.1`, ProductEvent remains `2.2`, and UI snapshot remains `2.2`.
No session/outbox/manifest or Extension schema version changes are required.
