# ADR-008: Extension Contract Versioning

- Status: Accepted 2026-07-19
- Date: 2026-07-19
- Owner: `0.4.2` `EKR-001`
- Implementation: `EKR-001`, generated SDK in `EKR-002`, service evolution in `0.4.3`

## Context

Manifest shape, Wasm ABI, Host APIs, contribution DTOs, and the TypeScript SDK
change for different reasons. A single workspace/package version would couple
unrelated compatibility decisions, while handwritten Rust and TypeScript types
would create competing ABI sources.

## Decision

Version five contracts independently:

| Contract | Initial candidate | Compatibility rule |
| --- | --- | --- |
| Manifest | integer schema `2` | unknown schema fails; additive optional fields stay within v2 |
| WIT package/world | `pi:extension@0.1.0`, world `extension` | semantic API range; breaking WIT change increments minor before 1.0 |
| Host API families | per-interface semantic versions | guest imports must fall within the host-supported range |
| contribution/resource schemas | namespaced integer revisions beginning at `1` | unknown required revision fails closed |
| TypeScript SDK | independent package semver beginning at `0.1.0` | SDK declares exact generated WIT/schema inputs |

Manifest v2 declares extension identity/version, compatible extension-API range,
component path and SHA-256 digest, expected WIT world, resolved lock reference,
activation conditions, requested permissions, contribution declarations, and
resource/limit requests. It cannot select a runtime or language and cannot
self-declare source, trust, persisted grants, or operation leases.

WIT and language-neutral schema files are authoritative. Rust bindings,
TypeScript declarations, SDK wrappers, fixture types, and contract hashes are
generated from those sources. Generated artifacts record generator/toolchain
versions and source hashes; handwritten bindings cannot become a second ABI.

Compatibility is negotiated before component compilation/admission. Unknown
required fields, worlds, imports, schemas, or API ranges fail closed. Unknown
optional contribution kinds may be ignored only when the manifest explicitly
marks them optional and activation remains meaningful. A host never guesses a
compatible major/minor contract from the workspace crate version.

The concrete candidate WIT functions and schema fields remain provisional until
the `EKR-004` package/admission/invocation/cancellation/disposal slice passes.
Freezing that candidate updates this ADR's evidence without changing the
independent-versioning decision.

## Alternatives Rejected

- one version shared by workspace crates, manifest, WIT, schemas, and SDK;
- workspace version as the extension ABI version;
- handwritten Rust/TypeScript DTO pairs;
- permissive decoding of unknown required fields/imports;
- runtime-selected ABI variants or manifest-selected implementation language;
- shipping multiple extension runtimes for compatibility.

## Security And Failure Consequences

Contract hashes and explicit ranges prevent silent type confusion and stale
generated bindings. Host-owned source/trust/grant data cannot be forged through
manifest fields. Diagnostics may report contract names/ranges/digests but must
not include secrets or installer authorization. Unsupported packages remain in
quarantine and never obtain an instance grant.

## Verification

`EKR-001` publishes candidate WIT/schema sources and strict Manifest v2 parsing.
`EKR-002` proves deterministic generated TypeScript bindings and hash matching.
`EKR-004` freezes the candidate after end-to-end invocation evidence. Tests cover
compatible ranges, incompatible versions, unknown required/optional fields,
world/import mismatch, stale generated hashes, and absence of runtime/language/
trust fields.
