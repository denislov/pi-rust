# P1/P3/P4 Boundary Closure Design

## Purpose

The Flow-centered runtime, Phase 1-6 workflow implementation, P0 public API boundary gate, scoped `pi-ai` runtime/auth boundary, and Flow core abstraction hardening are complete. The remaining active TODO items are deeper boundary-closure work:

- P1 product runtime boundary in `pi-coding-agent`;
- P3 tool boundary across `pi-agent-core` and `pi-coding-agent` plugin ingress;
- P4 UI boundary across `pi-tui` and `pi-coding-agent` adapters.

This design closes those items by turning the already-established architecture into source-guarded invariants plus small contract hardening slices. The goal is not a new product feature. The goal is to make the desired end state hard to regress: product operations stay session-owned, provider-visible tools stay validated and capability-scoped, and generic TUI primitives stay free of coding-agent product semantics.

## Current State

`docs/TODO.md` shows only P1, P3, and P4 as active `[~]` boundary items. Recent audits show:

- product prompt, workflow model calls, and adapter commands already route mostly through `CodingAgentSession`, services, `FlowService`, capabilities, and `CodingAgentEvent`;
- normal provider streaming uses scoped `AiClient`/`ProviderStreamer` paths, while remaining global-provider helpers are compatibility boundaries with guard coverage;
- `AgentTool::validate()` and `Agent::try_add_tool()` exist, and plugin tools are collected through `PluginService`;
- plugin UI action/dialog/keybind definitions are collected in `pi-coding-agent` and synchronized into the interactive adapter;
- `pi_tui::api` is the stable generic facade, and `pi-tui` keybinding defaults are already product-free.

The remaining work is therefore closure work: add missing source guards, route unvalidated or bypass-prone paths through named helpers, and update the TODO when evidence proves each stop condition.

## Design Principles

1. **Close by evidence, not by assertion**
   Each boundary item must finish with tests or source guards that would fail if new code reintroduced the bypass.

2. **Prefer guard-backed minimal changes**
   These slices should not refactor broad subsystems. They should add focused helpers only when the helper removes an actual bypass or creates an enforceable convention.

3. **Keep compatibility explicit**
   Remaining global provider runtime paths are allowed only where already declared as compatibility boundaries. New product/runtime paths must not expand those allowlists.

4. **Keep product ownership in `pi-coding-agent`**
   `pi-agent-core` remains low-level and `pi-tui` remains generic. Product session, protocol, adapter, plugin, and workflow semantics stay in `pi-coding-agent`.

## P1 Product Runtime Boundary

### Boundary

`CodingAgentSession` remains the product runtime owner. Adapters and product workflows must not directly own provider calls, raw low-level agent loops, session persistence, or product event construction.

Allowed ownership:

- `CodingAgentSession` coordinates product operations and active-operation guards.
- `SessionService` owns persistence and session-state finalization.
- `RuntimeService` owns runtime snapshots, scoped provider streamers, and agent construction.
- `FlowService` owns product Flow and subflow execution entrypoints.
- `EventService` owns product event construction and low-level event mapping.
- `CapabilityService` owns adapter-visible capability status.
- Workflow-specific services own workflow execution plumbing for compaction, branch summary, plugin load, delegation, and self-healing edit.

Disallowed drift:

- adapters constructing or running `Agent` directly for product commands;
- adapters or workflows calling `pi_ai::stream_model()`/`pi_ai::registry::stream_model()` directly;
- adapters directly constructing product session persistence events;
- product workflows directly running nested Flow types instead of `FlowService` runners;
- owner methods rebuilding workflow outcome branches that already belong to service/flow helpers.

### Implementation Shape

P1 should add a focused `product_runtime_boundary_guards.rs` integration test under `crates/pi-coding-agent/tests/`. The guard should scan production source and prove:

- direct `Agent::new`, `Agent::with_messages`, `Agent::run`, and `Agent::prompt` usages in `pi-coding-agent/src` are confined to runtime/flow implementation files, not adapters;
- global provider streaming calls remain confined to existing compatibility boundary files;
- adapter command files use `CodingAgentSession` owner APIs and do not call low-level `Agent` or global provider runtime APIs;
- product event construction in adapters remains behind the protocol/interactive adapters and `EventService`, not ad hoc operation logic.

If the guard finds an existing broad allowlist, the implementation should narrow it only after confirming the path is a current intentional boundary.

### Stop Condition Evidence

P1 is complete when a source audit plus focused tests prove that:

- a normal prompt can be traced through `CodingAgentSession::prompt`, `RuntimeService`, `FlowService`, and `EventService`;
- workflow model calls use scoped/provider-streamer-aware runtime paths;
- adapter commands do not bypass the owner with raw core/runtime calls;
- remaining global-provider compatibility helpers are documented, deprecated, and source-guarded.

## P3 Tool Boundary

### Boundary

Low-level tool execution belongs in `pi-agent-core`; product/plugin tool ingress belongs in `pi-coding-agent`. Provider-visible tools must be validated before entering an `Agent`, and plugin tool registration must not expose raw session, runtime, provider, filesystem, shell, auth, or Flow internals.

Allowed ownership:

- `AgentTool`, `AgentToolOutput`, `AgentToolResult`, `ToolExecutionMode`, hooks, and low-level tool events live in `pi-agent-core`.
- `ExecutionEnv` remains the low-level abstraction for filesystem/shell side effects.
- `PluginService` collects plugin-provided tools and diagnostics.
- `RuntimeService` merges profile policy tools, plugin tools, and delegation tools into the provider-visible agent runtime.

Disallowed drift:

- adding unvalidated tools to the product runtime with `Agent::add_tool`;
- plugin hosts exposing `CodingAgentSession`, `SessionService`, provider clients, auth material, raw `ExecutionEnv`, or Flow graph mutation;
- plugin or product tool ingress constructing product `CodingAgentEvent` directly;
- low-level `pi-agent-core` tool execution depending on `pi-coding-agent` product concepts.

### Implementation Shape

P3 should add two kinds of hardening:

1. A low-level contract helper in `pi-agent-core` if needed, such as a named validated tool-addition path or result helper. The current `try_add_tool()` should be preferred in product runtime code where tools enter an `Agent`.
2. A `tool_boundary_guards.rs` test layer that proves product runtime code uses validation before adding profile/plugin/delegation tools, plugin host traits stay capability-scoped, and `pi-agent-core` source remains free of coding-agent product imports.

A likely production change is to replace `RuntimeService`'s product-runtime `agent.add_tool(tool)` loop with `agent.try_add_tool(tool)` and map validation failure into `CodingSessionError::Tool`. This turns existing validation into a real product ingress gate instead of relying on providers to supply valid tools.

### Stop Condition Evidence

P3 is complete when tests prove that:

- invalid product/plugin tools fail at runtime construction with a typed product error instead of entering the agent;
- `RuntimeService` validates all provider-visible tools through the named low-level contract;
- plugin registration hosts remain capability-scoped and do not expose raw product/runtime internals;
- `pi-agent-core` has no dependency or source import on `pi-coding-agent` product ownership.

## P4 UI Boundary

### Boundary

`pi-tui` is a generic terminal UI crate. Coding-agent session/model/tree/tool/plugin semantics belong in `pi-coding-agent` interactive adapters.

Allowed ownership:

- `pi-tui` owns terminal, input, style, component, overlay, theme, autocomplete, render, image, editor, fuzzy, and virtual terminal primitives.
- `pi-coding-agent` owns app keybindings, slash commands, profile/team/delegation/session/model workflows, plugin UI action routing, plugin dialog validation, and plugin keybinding dispatch.

Disallowed drift:

- adding `app.*` actions to `pi-tui` defaults;
- adding coding-agent session/model/tree/tool/profile/team/plugin data models to `pi-tui` source;
- routing plugin UI actions directly through generic `pi-tui` primitives without `pi-coding-agent` adapter control;
- making `pi-tui` depend on `pi-coding-agent` or `pi-agent-core`.

### Implementation Shape

P4 should add a source guard under `crates/pi-tui/tests/` that scans `pi-tui/src` for product-owned terms and dependency imports. Existing tests already assert `TUI_KEYBINDINGS` is product-free; the new guard should be broader and allow generic examples/tests to use neutral labels like `model` only where they are test fixture text, not source-owned product semantics.

P4 should also add a `plugin_ui_boundary_guards.rs` test under `crates/pi-coding-agent/tests/` that proves plugin UI action/dialog/keybind dispatch stays in the interactive adapter and continues to resolve action IDs through controlled plugin command/dialog paths.

### Stop Condition Evidence

P4 is complete when tests prove that:

- `pi-tui` source has no coding-agent product dependencies or product workflow concepts;
- `pi-tui` default and facade keybinding surfaces remain generic/product-free;
- plugin UI action, dialog, and keybinding execution is routed through `pi-coding-agent` interactive adapter state, not raw TUI callbacks or plugin-owned arbitrary code;
- existing interactive plugin UI integration tests still pass.

## Execution Order

Execute in this order:

1. P1 product runtime boundary guards and any small runtime-owner cleanup they expose.
2. P3 tool validation and plugin ingress guards.
3. P4 generic TUI and plugin UI routing guards.
4. TODO closure update and full verification.

This order keeps ownership first, then hardens the tool and UI surfaces that depend on that ownership.

## Verification

Focused verification should include:

```bash
cargo fmt --check
cargo test -p pi-coding-agent --test product_runtime_boundary_guards
cargo test -p pi-coding-agent --test provider_registry_boundary_guards
cargo test -p pi-coding-agent --test event_boundary_guards
cargo test -p pi-agent-core --test tool_boundary_guards
cargo test -p pi-coding-agent --test tool_boundary_guards
cargo test -p pi-tui --test ui_boundary_guards
cargo test -p pi-coding-agent --test plugin_ui_boundary_guards
cargo test -p pi-coding-agent --test interactive_sessions
cargo test -p pi-agent-core
cargo test -p pi-coding-agent
cargo check --workspace
cargo test --workspace
git diff --check
```
