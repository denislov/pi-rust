# P0 Crate Boundary Hardening Design

## Purpose

P0 turns the July 2026 TypeScript parity reframing into enforceable project boundaries before new Phase 6 product work expands the API surface again. The goal is not to remove every migrated public symbol immediately. The goal is to define a stable facade for each active crate, mark migration-private surfaces, add tests that make intended boundaries visible, and give P1-P4 a narrower contract to build against.

This design is based on the current Flow-centered direction:

- TypeScript `pi` remains a behavioral and fixture reference, not a parity checklist.
- `pi-coding-agent` owns product runtime/session/workflow semantics.
- `pi-agent-core` owns low-level agent runtime semantics.
- `pi-ai` owns provider/model/auth runtime semantics.
- `pi-tui` owns generic terminal UI primitives only.

## Non-Goals

P0 will not complete Phase 6 workflows, implement the full `AiClient` runtime, port TypeScript SDK exports, add TypeScript session compatibility, or remove every root module export in one sweep. Those changes require staged implementation after the public boundary is documented and covered by tests.

P0 also will not make `pi-tui` an application event-loop owner, make `pi-agent-core::AgentHarness` own product sessions, or expose plugin internals as a public extension SDK.

## Boundary Policy

### `pi-ai`

Stable direction:

- A future `pi_ai::api` facade should expose the Rust-native provider runtime surface: model lookup/catalog views, chat types, streaming helpers, provider response metadata, hooks, and a scoped `AiClient` or equivalent runtime once implemented.
- `register()` and `stream_model()` remain compatibility or test helpers until `AiClient` is ready; new product code should not grow additional global-registry coupling.
- Auth belongs to a centralized resolver that can merge API keys, bearer/OAuth state, base URLs, headers, and auth source diagnostics before provider execution.
- Catalog/register invariants should be testable: built-in models should not point at missing built-in provider APIs unless explicitly marked external or unsupported.

Migration-private or constrained:

- Provider module internals, wire types, conversion helpers, and transport details are not stable embedding APIs.
- `pi-ai` must not depend on `CodingAgentSession`, Rust-native session logs, CLI/RPC/TUI adapters, or product Flow types.

### `pi-agent-core`

Stable direction:

- Stable low-level entry points are `Agent`, `AgentConfig`, `AgentTool`, `AgentEvent`, `AgentHooks`, selected harness hook types, and `ExecutionEnv`.
- `AgentTurnFlow` is the low-level runtime implementation boundary. Its public usage should be narrowed to intentional extension/testing APIs, not product workflow ownership.
- Tool execution needs a Rust-native contract for schema validation, argument preparation, tool result details, and failure mapping that can also support plugin tools.
- Resources and first-party tool execution should increasingly go through `ExecutionEnv` instead of direct local filesystem assumptions.

Migration-private or constrained:

- `agent_loop` is a compatibility wrapper, not a stable API target.
- Legacy session JSONL modules and branch/session product behavior must not become the stable product session contract.
- `AgentEvent` remains a low-level event stream. Product message lifecycle and adapter wire semantics belong in `CodingAgentEvent` mapping inside `pi-coding-agent`.

### `pi-coding-agent`

Stable direction:

- `pi_coding_agent::api` is the stable facade for embedding and scripting.
- `CodingAgentSession`, `SessionService`, `RuntimeService`, `FlowService`, `EventService`, and `CapabilityService` are the product owner/service boundary.
- `CodingAgentCapabilities` and protocol versions should describe available adapter features before commands are exposed through RPC/TUI/JSON.
- Shared product operations such as manual compaction, branch summary, export, session switch, plugin command execution, UI/keybind dispatch, delegation-first child-agent orchestration, explicit team workflows, and self-healing edits should be implemented as session-owned services or product Flows.

Migration-private or constrained:

- Root modules and adapter internals can remain public temporarily, but are migration-private unless re-exported through `api` and documented as stable.
- Old JSONL session runner behavior is rejected input, not a compatibility path to revive.
- Plugin APIs must stay capability-scoped and must not expose raw session storage, provider internals, shell/filesystem primitives, or arbitrary Flow graph mutation.

### `pi-tui`

Stable direction:

- `pi-tui` is a generic terminal UI crate. Stable APIs should describe terminal, input, component, render, overlay, theme, image, autocomplete, and `VirtualTerminal` test primitives.
- Coding-agent product keybindings and actions should be injected from `pi-coding-agent`, not defined as default base-crate behavior.
- `Component` downcast behavior should not panic by default for downstream implementors.
- Plugin UI/keybind execution should enter through a controlled `pi-coding-agent` adapter that uses `pi-tui` primitives.

Migration-private or constrained:

- `pi-tui` must not know about model/session/tree/tool/plugin product semantics.
- TypeScript `TUI.start()/stop()/requestRender()` shape parity is not required.
- Global terminal-image cache/capability behavior from TypeScript is a reference, not a compatibility target.

## P0 Implementation Shape

P0 should land in small slices:

1. Add this boundary design and a P0 implementation plan to the project source documents.
2. Add or update public API smoke tests so each crate has an explicit stable facade expectation.
3. Add crate-level documentation comments or facade modules where the stable entry is already clear.
4. Mark compatibility/migration-private surfaces in documentation before removing or deprecating them.
5. Add focused compile-time or unit tests for the first hard boundaries:
   - `pi-coding-agent::api` remains importable for product owner types.
   - `pi-ai` has an explicit facade or compatibility classification for global registry calls.
   - `pi-agent-core` exposes low-level runtime types without product event/session ownership.
   - `pi-tui` default keybindings do not include coding-agent `app.*` product actions once that migration slice lands.

## Acceptance Criteria

P0 is complete when:

- `docs/TODO.md` links this spec and the P0 plan.
- The four cross-cutting boundary TODOs are marked in progress or complete with precise status.
- Each active crate has a written stable/migration-private boundary policy in docs or crate-level API docs.
- Focused tests cover the stable facade imports and the first executable boundary invariants.
- `cargo fmt --check`, focused crate tests for changed crates, `cargo check --workspace`, and `git diff --check` pass in an environment that sources `~/.cargo/env` on the remote host.

## Why P0 Precedes P1-P4

P1 depends on `pi-coding-agent` being the product owner rather than another adapter-specific state machine. P2 depends on `pi-agent-core` staying low-level. P3 depends on `pi-tui` remaining generic while application actions move upward. P4 depends on `pi-ai` providing scoped provider/auth runtime rather than expanding global registry behavior. Without P0, later work can pass tests while still making the final architecture less true.
