# ADR-007: Extension Package Quarantine

- Status: Accepted 2026-07-19
- Date: 2026-07-19
- Owner: `0.4.2` `EKR-001`
- Implementation: `EKR-001` package integrity, `EKR-003` grant-backed activation,
  `EKR-004` Component boundary validation; package-update completion in `0.4.3`
  `ESS-006`

## Context

An extension artifact is untrusted input before activation. Treating a project
directory, npm package, or downloaded archive as executable would mix source,
trust, dependency resolution, and runtime admission. It would also permit path
escape, symlink substitution, mutable code after validation, and permission
transfer through dependencies.

## Decision

The installed unit is an immutable package identified by extension ID, semantic
version, and SHA-256 digest. The package contains a Manifest v2 document, exactly
one Wasm Component artifact, an optional bounded resource tree, and a resolved
dependency lock. TypeScript sources, Node modules, build scripts, native
libraries, and runtime-selection metadata are never installed.

Installation is a fail-closed pipeline:

```text
untrusted input
  -> bounded staging directory
  -> structural quarantine
  -> manifest/contract/dependency validation
  -> component import/export validation
  -> digest verification
  -> immutable content-addressed package store
  -> explicit workspace activation
```

Quarantine rejects absolute or parent-relative paths, symlinks and hard links,
duplicate/case-colliding entries, unsupported file types, undeclared files,
multiple components, excessive entry/count/depth/size limits, digest mismatch,
forbidden imports, dependency cycles, incompatible API/WIT ranges, and unknown
required permissions. Validation reads from the staged snapshot; activation
never reopens the original source path.

Source and trust are host facts derived from the installer/channel and are not
self-declared by the manifest. One workspace activates at most one version of an
extension ID. Dependencies are separately installed and admitted under their own
grants; their permissions never transfer to dependents.

`0.4.2` implements quarantine and immutable storage in `EKR-001`, permission and
explicit activation checks in `EKR-003`, and Component boundary checks in
`EKR-004`. Coordinated update/rollback across package and extension state is a
durable phase machine owned by `0.4.3`; this ADR fixes that direction now but does
not pretend a cross-store transaction exists in `0.4.2`.

## Alternatives Rejected

- executing directly from a project or npm directory;
- trusting a manifest-provided source/trust/runtime field;
- validating an archive and then executing from its mutable extraction path;
- granting dependency permissions transitively;
- loading TypeScript, JavaScript, native libraries, or Lua from an installed package;
- activating multiple versions of one extension ID in a workspace.

## Security And Failure Consequences

Unknown layout, imports, permissions, contracts, dependencies, or digests fail
closed before compilation or admission. Failed staging is non-admissible and may
be deleted idempotently. A package digest names bytes, not trust; trust and grant
records remain separate host-owned data. Diagnostics redact filesystem roots,
credentials, grant material, and package acquisition authorization.

## Compatibility And Verification

There is no Lua/native package compatibility runtime or automated migration tool.
Owned durable/session decoders are unaffected. `EKR-001`/`EKR-008` must cover
traversal, symlink, collision, size/count/depth, digest, dependency cycle/range,
forbidden import, trust-source, immutable-store, and one-active-version cases.
`ESS-006` later adds phase-crash, fencing, rollback, and forward-recovery evidence.
