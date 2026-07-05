# Lua Plugin Host Surface

This document defines the first-phase Lua plugin host surface for `pi-rust`.
It is a product/plugin boundary, not a raw runtime extension API.

Lua plugins are loaded by `PluginLoadFlow` from a manifest `entry` file. The
entry must define `register(host)`. The host table exposes only scoped
registration and metadata helpers.

## Stable First-Phase Host Methods

Registration methods:

- `host:tool(definition)`: registers a provider-visible tool.
- `host:command(definition)`: registers a plugin command runnable through the session-owned plugin service.
- `host:hook(definition)`: registers a prompt hook at supported hook points.
- `host:ui_action(definition)`: registers an interactive UI action definition.
- `host:dialog(definition)`: registers an interactive dialog definition.
- `host:keybind(definition)`: registers a keybinding definition.

Metadata methods:

- `host:api_version()`: returns the stable host API version string for feature gating.
- `host:plugin()`: returns manifest-scoped metadata: plugin id, name, version, and source.
- `host:workspace()`: returns path-scoped metadata such as plugin root and entry path.
- `host:capabilities()`: returns boolean feature flags for supported host methods.

The metadata helpers are read-only. They are intended for feature detection,
labels, diagnostics, and path-scoped plugin behavior.

## Dialog Field Surface

First-phase dialog fields may use these field types:

- text/string fields;
- boolean fields;
- integer/number fields;
- select/choice/enum fields with declared options.

Field metadata may include id, label, description, default value, required flag,
type, and options where applicable. Richer controls are follow-up work and do
not block Phase 6 closure.

## Execution Semantics

Plugin manifest load failures are fail-open diagnostics: one invalid plugin must
not prevent unrelated valid plugins from loading. Plugin command/action/hook/tool
execution failures are isolated to that provider/action and reported through the
session-owned diagnostics path.

Reload replaces the session-owned plugin service and emits capability-change
information through product events/capability reporting. Adapters consume the
loaded service through `CodingAgentSession` surfaces; they do not execute plugin
internals directly.

## Explicit Non-API Surface

The Lua host must not expose:

- raw `CodingAgentSession` or session services;
- raw session storage or event-log writers;
- raw operation contexts;
- provider/runtime internals;
- filesystem or shell handles;
- auth secrets or provider keys;
- arbitrary Flow node/subflow registration or graph mutation APIs.

When a future host method needs filesystem, shell, auth, provider, or Flow-like
behavior, it must be introduced as a capability-scoped API with explicit policy,
diagnostics, and tests proving it does not leak raw internals.
