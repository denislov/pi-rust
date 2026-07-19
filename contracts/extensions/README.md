# Extension Contract Candidates

This directory contains the authoritative candidate inputs for the `0.4.2`
TypeScript/Wasm extension kernel. Contract families version independently from
the Rust workspace. Generated Rust/TypeScript bindings must record these source
hashes and may not become a second ABI source.

`0.1.0/` remains a release candidate until the `EKR-004` end-to-end Wasm slice
freezes it. Installed packages contain a Wasm Component and data resources only;
these authoring/build inputs are repository contracts, not runtime dependencies.

Verify the checked-in inputs from this directory:

```bash
sha256sum -c 0.1.0/SHA256SUMS
```
