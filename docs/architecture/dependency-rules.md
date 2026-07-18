# Dependency And Boundary Rules

## Workspace Dependencies

| Crate | May depend on | Must not depend on |
| --- | --- | --- |
| `pi-ai` | third-party transport/protocol libraries | agent sessions, product events, CLI, RPC, TUI, product policy |
| `pi-agent-core` | `pi-ai` provider-neutral API | coding-agent sessions, product operations/events, plugins, adapters, TUI |
| `pi-tui` | generic terminal/rendering libraries | product commands, sessions, providers, plugins, RPC |
| `pi-coding-agent` | `pi-ai`, `pi-agent-core`, `pi-tui` public facades | lower-crate private modules or reverse ownership |
| reserved product crates | stable `pi-coding-agent` API/protocol when activated | lower-layer internals and storage implementations |

No lower crate may acquire a product type to save an adapter. Product semantics
belong in `pi-coding-agent`; provider-neutral agent behavior belongs in
`pi-agent-core`; provider wire behavior belongs in `pi-ai`; terminal mechanics
belong in `pi-tui`.

## Edge Allowlist

Cross-crate use goes through categorized `api` facades and an item-level
allowlist for that dependency edge. A symbol being public, or present in a broad
provider facade, does not automatically authorize every consumer to use it.

Allowed cross-boundary shapes are:

- immutable domain values and closed DTOs;
- minimal object-safe behavior traits where dynamic dispatch is useful;
- cancellation-aware callbacks;
- purpose-specific handles with explicit lifetime and authority;
- test-only builders/faux implementations behind non-default `test-support`.

Forbidden shapes include:

- `ServiceContainer`, `RuntimeHost`, repositories, registries, or mutable contexts
  passed as convenience dependencies;
- provider-specific wire types crossing into `pi-agent-core` or public product
  protocols;
- Flow nodes, node IDs, `AgentEvent`, or `FlowEvent` exposed to product adapters;
- `pi-tui` component/widget types exposed to product extensions;
- raw Rust trait objects as an untrusted extension ABI;
- test-support APIs in production signatures.

## Ownership Matrix

| Concern | Owner |
| --- | --- |
| model metadata, auth inputs, HTTP/SSE, provider wire mapping | `pi-ai` |
| provider-neutral agent loop, generic Flow primitives, generic tools/resources | `pi-agent-core` |
| generic terminal lifecycle, input, layout, components | `pi-tui` |
| operation admission/policy, sessions, ProductEvents, snapshots, extensions, adapters | `pi-coding-agent` |
| browser presentation | `pi-web-ui` when activated |
| cross-session orchestration | `pi-mom` when activated |
| isolated/remote runtime hosting | `pi-pods` when activated |

## Public API Rule

The stable embedding surface is `pi_coding_agent::api`. Root-level compatibility
exports are not a place to grow new contracts. Public APIs expose product verbs,
typed outcomes/events, snapshots, protocol negotiation, and narrow testing
fixtures—not scheduler, Flow, service, repository, provider-client, plugin-host,
or adapter internals.

## Enforcement

Boundary tests must reject reverse dependencies, forbidden paths, facade drift,
raw services, compatibility backdoors, and production use of test support. When a
public type or cross-crate contract changes, update its owning facade, edge
allowlist, public-API snapshot, protocol inventory, migration decision, and
downstream tests together.
