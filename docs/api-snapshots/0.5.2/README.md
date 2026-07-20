# Public API Snapshot 0.5.2

This directory freezes the SHA-256 manifest for the normalized 0.5.2 Rust
public API surfaces and release toolchain. The full generated artifacts remain under
`target/release-artifacts/0.5.2/public-api/`.

Raw rustdoc JSON and workspace metadata are retained there as diagnostic inputs.
They are intentionally excluded from the checked-in manifest because they
contain checkout paths and rustdoc/Cargo-internal identifiers that are not API
contracts and are not byte-stable across clean worktrees.

Generate and verify this snapshot with:

```bash
PI_RUST_RELEASE_VERSION=0.5.2 scripts/release-api-snapshots.sh
PI_RUST_API_BASELINE_MANIFEST=docs/api-snapshots/0.5.2/SHA256SUMS \
  PI_RUST_RELEASE_VERSION=0.5.2 scripts/release-api-snapshots.sh
```

The `pi-coding-agent` snapshot contracts `api::cli` to the high-level runner
surface, recategorizes runtime/operation/print types under their owning API
categories, and removes the empty testing category. Direct facade exports fall
from 292 to 189 (35.3%); generated public-api items fall from 15,533 to 12,143.
The `pi-ai`, `pi-agent-core`, and `pi-tui` public API inventories are unchanged.

RPC `2.1`, ProductEvent `2.2`, and UI snapshot `2.1` remain the protocol
inventory for this release; their compatibility and serialization snapshots
are enforced by the `events_snapshot`, RPC negotiation, and release gates.
