# `0.2.0-alpha.1` Public API Freeze Input

The checked-in manifest freezes the stable `public-api 0.52.0` projection of
the four active library crates. Full `*.public-api.json` and raw
`*.rustdoc.json` files remain release artifacts rather than source files.

| Crate | Public items |
| --- | ---: |
| `pi-ai` | 2,707 |
| `pi-agent-core` | 6,639 |
| `pi-tui` | 3,599 |
| `pi-coding-agent` | 13,034 |

Generation used Rust/Cargo `1.96.0` and
`scripts/release-api-snapshots.sh`. Validate the current tree with:

```bash
PI_RUST_API_BASELINE_MANIFEST=docs/api-snapshots/0.2.0-alpha.1/SHA256SUMS \
  scripts/release-api-snapshots.sh
```

A checksum change is not accepted automatically. Review the corresponding
full artifact diff and update the freeze manifest only when the API change is
intentional for the active prerelease phase.

`scripts/release-gates.sh` uses this alpha manifest as the default comparison
baseline for the first release candidate.
