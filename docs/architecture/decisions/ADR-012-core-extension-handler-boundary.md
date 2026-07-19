# ADR-012: Core And Extension Handler Boundary

- Status: Accepted 2026-07-19
- Date: 2026-07-19
- Owner: `0.4.2` `EKR-006`
- Implementation: `EKR-006`, contribution dispatch in `EKR-005`

## Context

The product has built-in Rust behavior and a legacy plugin registry whose Rust
traits can also be implemented by Lua-backed providers. The replacement kernel
must represent built-in and third-party contribution targets in one product
projection without turning built-in code into a privileged extension tier or
letting an extension name a Rust function, trait object, service, or repository.

The Wasm vertical slice needs this boundary before it can bind a manifest
contribution to an invocation. Deferring the distinction to the dispatcher
would make package data authoritative over the core execution path.

## Decision

The product-owned projection uses one exhaustive target enum:

```text
HandlerTarget = Core(CoreHandlerRef) | Extension(ExtensionHandlerRef)
```

`CoreHandlerRef` contains a contribution kind and a product-owned handler ID.
Only Rust product assembly may construct it. It executes directly through the
owning product dispatcher and does not acquire an extension grant, instantiate
Wasm, or enter an extension Host API.

`ExtensionHandlerRef` contains the validated extension ID, immutable package
digest, contribution kind, manifest handler ID, and schema revision. The host
derives it from a quarantined package and its validated contribution inventory;
the guest cannot supply or replace package identity. Dispatch always enters
operation admission, obtains the current generation-bound capability lease,
and invokes the declared WIT export in a fresh isolated instance.

Both references are language-neutral data and contain no executable authority.
They may be projected for diagnostics and client inventory, but the target enum
is not deserialized from extension-controlled data. Runtime generation,
operation identity, lease, cancellation, deadline, and Host handles remain
invocation state rather than fields in either reference.

Contribution kind matching is exhaustive for tools, commands, Prompt hooks, UI
actions, dialogs, and keybindings. New kinds require an explicit product enum
change and dispatch review; unknown manifest revisions fail closed.

## Prohibited Edges

- extension manifests, WIT values, or client messages constructing core refs;
- extensions addressing core handler IDs or choosing `Core` as a target;
- core behavior passing through Wasm, extension permissions, or guest limits;
- extension refs containing Rust trait objects, callbacks, services,
  repositories, provider clients, channels, registries, or adapter state;
- dispatcher fallback from a missing extension handler to a same-named core
  handler;
- a first-party/native extension tier that bypasses Wasm admission.

## Alternatives Rejected

- execute built-in behavior through Wasm for uniformity;
- retain a privileged first-party Rust extension ABI;
- use an untyped handler string and decide core versus extension at runtime;
- expose Rust contribution traits or callbacks across the extension boundary;
- allow manifests to select a runtime or target kind.

## Security And Failure Consequences

Package identity and handler identity are bound before admission, preventing a
guest from redirecting an invocation to core code or another installed package.
An invalid identity, digest, kind, or schema revision rejects activation. A
missing/revoked generation, deadline, cancellation, trap, or Host denial fails
the extension operation without attempting core fallback. Core failures use the
owning product error path and cannot be reclassified as extension failures.

## Compatibility

This is an intentional replacement boundary. Legacy Rust provider traits and
Lua-backed contribution providers were removed in `EKR-007`; contribution
productization in `EKR-005` was Skipped. `CLC-042-002` also removed the
unreachable PluginCommand and adapter presentation compatibility surface rather
than treating it as an implementation of either new target. Built-in `pi-ai`
provider registration remains product infrastructure and is outside the
extension target model.

## Verification

`EKR-006` provides the DTO/dispatch slice and proves that validated manifest
contributions project only to package-bound `ExtensionHandlerRef` values, target
dispatch is exhaustive, serialized projections contain no executable authority,
and malformed identity/digest/revision data fails closed. Boundary tests reject
deserialization authority, raw Rust traits/services, and core-handler fields in
extension contracts. `EKR-004` consumes extension refs through real WIT
admission. Full contribution-family dispatch was Skipped with `EKR-005` and
must be replanned before this boundary is extended into product adapters.
