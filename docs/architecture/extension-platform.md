# Extension Platform Contract

## Status

Normative target for the `0.4.x` train. `0.4.0` accepts the foundational security,
runtime-isolation, state/fact, and Workbench decisions; production extension
implementation begins in `0.4.2`.

## Authoring And Runtime

```text
third-party authoring language = TypeScript
installed artifact = Wasm Component
stable ABI = versioned WIT + language-neutral schemas
host/core implementation = Rust product code
```

Installed extensions run without Node.js or build tooling. The host never
directly executes TypeScript/JavaScript, loads native extension libraries, accepts
untrusted Rust trait objects, or selects among third-party runtime kinds. Built-in
Rust AI providers in `pi-ai` and trusted embedding closures are product/host code,
not a first-party extension tier.

## Capability Lifetime

```text
requested permissions
  -> persisted GrantRecord
  -> revocable ExtensionInstanceGrant(generation)
  -> frozen OperationCapabilityLease(operation, generation, scope, deadline)
```

Activation, contribution registration, and subscription definitions use an
instance grant. Every privileged tool, command, hook, action, view refresh, data
fetch, event callback, or background tick is a separately admitted operation with
a bounded lease. Each Host API call validates lease, operation, generation,
scope, deadline, cancellation, and quota.

Revocation blocks new admission, cancels matching old leases when policy requires,
closes registrations, and discards uncommitted late results. Dependency
permissions never transfer. Cross-extension calls use normal admission and the
callee's grant; there is no ambient transitive authority.

Extensions never receive repositories, provider clients, raw EventHub receivers,
Flow registries, core handler IDs, adapter state, mutable operation contexts,
unbounded channels, or detached task spawning.

## Wasm Invocation

- immutable compiled components may be cached by engine/version/target/digest;
- every operation receives an isolated Store/Instance/resource table;
- mutable guest memory is invocation-local, non-durable, and never shared across
  workspaces;
- async host bindings honor Tokio cancellation and deadlines;
- epoch interruption and deterministic fuel bound guest compute;
- memory, table, host-call allocation, output, progress, diagnostic, and log
  quotas are enforced;
- cancelled, trapped, OOM, out-of-fuel, invalid, or over-limit instances are
  destroyed and never reused;
- ambient WASI, native/dynamic loading, Node built-ins, and detached guest tasks
  are rejected.

Compiled-code caching cannot cache instance grants, mutable stores, guest memory,
resource tables, secrets, session handles, or client state.

## Contributions And Dispatch

The current minimum framework validates and projects handler targets but does
not expose contribution commands, UI actions, dialogs, keybindings, or matching
RPC/TUI dispatch. Those product surfaces are explicitly Skipped and require a
new version plan before implementation; no legacy `PluginCommand` compatibility
route is retained.

Manifest and schema-owned DTOs describe contributions. Shared projections use an
explicit `CoreHandlerRef` or `ExtensionHandlerRef`:

- core references invoke built-in Rust product behavior without extension grants;
- extension references always enter canonical operation admission and Wasm/WIT;
- extensions cannot address core handlers or receive raw Rust traits;
- IDs and references are validated before activation;
- interactive, RPC, JSON, and embedding paths consume the same semantics.

## State And Facts

State scopes are global, workspace, session, branch, and ephemeral invocation.

- global/workspace key-value state uses an extension state store with namespace,
  schema, quota, compare/transaction, export, inspection, and deletion;
- session/branch state uses generic versioned SessionEvents through
  SessionCoordinator and remains replayable without extension code;
- branch state inherits ancestor-visible mutations and diverges copy-on-write;
- `ExtensionFact` is immutable historical evidence, not mutable key-value state;
- ephemeral invocation state is discarded with the Wasm instance.

Core SessionEvents, session/branch state mutations, ExtensionFacts, committed
ProductEvent outbox records, and snapshot cursor advancement may share one
session transaction. Global/workspace state never participates in that atomic
transaction.

Migrations are bounded pure-data transformations and cannot call model, network,
shell, secrets, other extensions, or arbitrary operations. Session migration is
append-only and idempotent; history is never rewritten to simulate rollback.

## Package Update

Artifact registry, global/workspace state, and SessionEvent stores are coordinated
by a durable phase state machine, not a cross-store transaction. Each store is
atomic only within its boundary.

Candidate code/state is prepared and validated while externally non-admissible.
The old generation closes admission and drains/cancels under a deadline. A durable
activation record selects one generation; only then may the candidate accept
operations. Late old results fail generation checks. Failure before activation
reopens the old generation; failure after activation uses recorded forward
recovery or another explicit compatible generation transition.

## Workbench

Workbench is a host-rendered semantic retained tree:

```text
ViewSnapshot(view_instance_id, revision, root)
ViewPatch(view_instance_id, base_revision, typed operations)
```

The host validates stable node IDs, references, schema, depth, size, text/row/log
limits, actions, patch rate, and encoded size. A stale/gapped base revision causes
`ViewResyncRequired` and a fresh snapshot; patches are not heuristically merged.

Lists, trees, tables, diffs, and logs use paging, lazy loading, virtualization,
and bounded buffers. Every callback/fetch/action is an admitted leased operation.
Focus, selection, scroll, viewport, expansion, and transient form input are
client-local unless a submitted action persists business state.

Workbench is not a virtual DOM, CSS engine, raw `pi-tui` widget API, HTML/WebView,
or arbitrary JavaScript UI runtime. TUI, RPC, and embedding are peer renderers of
the same semantic protocol.
