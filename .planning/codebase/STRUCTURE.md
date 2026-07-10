# Codebase Structure

**Analysis Date:** 2026-07-10

## Directory Layout

```text
pi-rust/
|-- Cargo.toml                         # Workspace membership plus placeholder root package
|-- Cargo.lock                         # Locked Rust dependency graph
|-- src/
|   `-- main.rs                        # Placeholder root binary; prints Hello, world!
|-- crates/
|   |-- pi-ai/                         # Model, provider, auth, streaming, and HTTP transport
|   |-- pi-agent-core/                 # Product-neutral agent loop, Flow engine, tools, hooks
|   |-- pi-coding-agent/               # Active coding-agent product, CLI, runtime, adapters
|   |-- pi-tui/                        # Generic terminal/component/render/input library
|   |-- pi-mom/                        # Placeholder crate
|   |-- pi-pods/                       # Placeholder crate
|   `-- pi-web-ui/                     # Placeholder crate
|-- docs/
|   |-- roadmap/                       # Current roadmap supporting documents
|   |-- superpowers/specs/             # Design specifications and reference architecture
|   |-- superpowers/plans/             # Implementation plans
|   `-- archive/                       # Historical roadmaps, ideas, and parity reports
|-- scripts/
|   `-- tui-smoke.sh                   # Manual/automated terminal smoke harness
|-- .qoder/repowiki/                   # Committed generated repository wiki
|-- .codegraph/                        # Local generated CodeGraph index; gitignored
`-- .planning/codebase/                # Generated GSD codebase map
```

Active source is concentrated in four crates: `crates/pi-ai`, `crates/pi-agent-core`, `crates/pi-coding-agent`, and `crates/pi-tui`. The `crates/pi-mom`, `crates/pi-pods`, and `crates/pi-web-ui` crates contain only the Cargo template `add` function and test.

## Directory Purposes

**`crates/pi-ai/`:**
- Purpose: Own shared AI types, model metadata, provider selection/authentication, provider-specific wire conversion, streaming normalization, images, and network transport.
- Contains: Library source in `crates/pi-ai/src`, provider fixtures and integration tests in `crates/pi-ai/tests`, examples in `crates/pi-ai/examples`, and model-generation tooling in `crates/pi-ai/tools`.
- Key files: `crates/pi-ai/src/lib.rs`, `crates/pi-ai/src/registry.rs`, `crates/pi-ai/src/models.rs`, `crates/pi-ai/src/providers/mod.rs`, `crates/pi-ai/src/transport/mod.rs`, `crates/pi-ai/src/types/mod.rs`

**`crates/pi-ai/src/providers/`:**
- Purpose: Isolate provider-specific APIs behind the shared `ApiProvider` and `EventStream` contracts.
- Contains: Provider directories such as `crates/pi-ai/src/providers/anthropic`, `crates/pi-ai/src/providers/openai`, `crates/pi-ai/src/providers/google`, `crates/pi-ai/src/providers/mistral`, `crates/pi-ai/src/providers/deepseek`, and `crates/pi-ai/src/providers/bedrock`.
- Key files: Each substantial provider follows `mod.rs` plus `convert.rs`, `wire.rs`, and `process.rs`; Anthropic also uses `crates/pi-ai/src/providers/anthropic/sse.rs`, and shared processing lives in `crates/pi-ai/src/providers/process_framework.rs`.

**`crates/pi-ai/src/transport/`:**
- Purpose: Centralize network behavior that must remain consistent across providers.
- Contains: HTTP execution, headers, retry policy, and normalized provider errors.
- Key files: `crates/pi-ai/src/transport/http.rs`, `crates/pi-ai/src/transport/retry.rs`, `crates/pi-ai/src/transport/headers.rs`, `crates/pi-ai/src/transport/error.rs`

**`crates/pi-agent-core/`:**
- Purpose: Provide the reusable low-level agent runtime without coding-agent product/session/UI ownership.
- Contains: Agent and Flow source in `crates/pi-agent-core/src`, product-neutral integration tests in `crates/pi-agent-core/tests`, and a loop example in `crates/pi-agent-core/examples`.
- Key files: `crates/pi-agent-core/src/lib.rs`, `crates/pi-agent-core/src/agent.rs`, `crates/pi-agent-core/src/flow.rs`, `crates/pi-agent-core/src/types.rs`, `crates/pi-agent-core/src/agent_turn_flow/runtime.rs`

**`crates/pi-agent-core/src/agent_turn_flow/`:**
- Purpose: Implement the explicit low-level turn graph and its mutable operation context.
- Contains: Public module exports, context/state transfer, node wrappers, and runtime/tool/provider node behavior.
- Key files: `crates/pi-agent-core/src/agent_turn_flow/mod.rs`, `crates/pi-agent-core/src/agent_turn_flow/context.rs`, `crates/pi-agent-core/src/agent_turn_flow/nodes.rs`, `crates/pi-agent-core/src/agent_turn_flow/runtime.rs`

**`crates/pi-agent-core/src/compaction/`:**
- Purpose: Estimate context usage, split history, summarize old messages, and manage runtime/session compaction primitives.
- Contains: Estimation, preparation, provider summarization, error, and session helpers.
- Key files: `crates/pi-agent-core/src/compaction/estimate.rs`, `crates/pi-agent-core/src/compaction/prepare.rs`, `crates/pi-agent-core/src/compaction/summarize.rs`, `crates/pi-agent-core/src/compaction/session.rs`

**`crates/pi-agent-core/src/resources/`:**
- Purpose: Parse and format skills, prompt templates, context frontmatter, and system prompt resources.
- Contains: Resource loaders and formatters.
- Key files: `crates/pi-agent-core/src/resources/skills.rs`, `crates/pi-agent-core/src/resources/prompt_templates.rs`, `crates/pi-agent-core/src/resources/system_prompt.rs`, `crates/pi-agent-core/src/resources/frontmatter.rs`

**`crates/pi-agent-core/src/session_context/` and `crates/pi-agent-core/src/transcript/`:**
- Purpose: Hold product-neutral in-memory context and transcript primitives used by the agent runtime.
- Contains: Context/memory/error modules and typed transcript IDs/entries.
- Key files: `crates/pi-agent-core/src/session_context/context.rs`, `crates/pi-agent-core/src/session_context/memory.rs`, `crates/pi-agent-core/src/transcript/types.rs`, `crates/pi-agent-core/src/transcript/id.rs`

**`crates/pi-coding-agent/`:**
- Purpose: Own the active coding-agent product, including CLI/process bootstrap, configuration, tools, plugins, sessions, workflows, adapters, and product-specific UI.
- Contains: Binary/library source in `crates/pi-coding-agent/src`, broad product tests in `crates/pi-coding-agent/tests`, and a manual example in `crates/pi-coding-agent/examples`.
- Key files: `crates/pi-coding-agent/src/main.rs`, `crates/pi-coding-agent/src/lib.rs`, `crates/pi-coding-agent/src/args.rs`, `crates/pi-coding-agent/src/request.rs`, `crates/pi-coding-agent/src/runtime.rs`

**`crates/pi-coding-agent/src/coding_session/`:**
- Purpose: Act as the product-runtime migration container and own typed operations, Flow orchestration, services, durable session facts, events, projections, capabilities, plugins, profiles, teams, and delegation.
- Contains: `CodingAgentSession` facade, `*_flow.rs` workflow graphs, `*_service.rs` side-effect owners, operation/event contracts, capability snapshots, and the Rust-native session log.
- Key files: `crates/pi-coding-agent/src/coding_session/mod.rs`, `crates/pi-coding-agent/src/coding_session/operation.rs`, `crates/pi-coding-agent/src/coding_session/public_operation.rs`, `crates/pi-coding-agent/src/coding_session/flow_service.rs`, `crates/pi-coding-agent/src/coding_session/prompt_flow.rs`, `crates/pi-coding-agent/src/coding_session/event_service.rs`

**`crates/pi-coding-agent/src/coding_session/session_log/`:**
- Purpose: Define the only durable product-session fact format and the append/replay/transaction mechanics around it.
- Contains: Typed event envelopes, IDs, manifest schema, JSONL store, replay/fold, and transaction staging.
- Key files: `crates/pi-coding-agent/src/coding_session/session_log/event.rs`, `crates/pi-coding-agent/src/coding_session/session_log/manifest.rs`, `crates/pi-coding-agent/src/coding_session/session_log/store.rs`, `crates/pi-coding-agent/src/coding_session/session_log/replay.rs`, `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`

**`crates/pi-coding-agent/src/interactive/`:**
- Purpose: Implement coding-agent-specific terminal behavior on top of generic `pi-tui` primitives.
- Contains: Terminal app/loop, root component, transcript rendering, input pump, slash commands, model/session/tree/profile selectors, delegation confirmation, event bridge, clipboard, and key hints.
- Key files: `crates/pi-coding-agent/src/interactive/app.rs`, `crates/pi-coding-agent/src/interactive/loop.rs`, `crates/pi-coding-agent/src/interactive/root.rs`, `crates/pi-coding-agent/src/interactive/event_bridge.rs`, `crates/pi-coding-agent/src/interactive/prompt_task.rs`

**`crates/pi-coding-agent/src/protocol/`:**
- Purpose: Expose machine-readable JSON and JSONL RPC adapters over the product operation/event boundary.
- Contains: Shared wire types/versioning, JSON mode, JSONL I/O, protocol event mapping, and RPC command/state/event submodules.
- Key files: `crates/pi-coding-agent/src/protocol/types.rs`, `crates/pi-coding-agent/src/protocol/events.rs`, `crates/pi-coding-agent/src/protocol/json_mode.rs`, `crates/pi-coding-agent/src/protocol/jsonl.rs`, `crates/pi-coding-agent/src/protocol/rpc.rs`, `crates/pi-coding-agent/src/protocol/rpc/`

**`crates/pi-coding-agent/src/plugins/`:**
- Purpose: Define controlled product extension contracts.
- Contains: Registries and provider traits for capabilities, tools, commands, hooks, keybinds, UI actions/dialogs, and limited flow extension points.
- Key files: `crates/pi-coding-agent/src/plugins/registry.rs`, `crates/pi-coding-agent/src/plugins/capability.rs`, `crates/pi-coding-agent/src/plugins/tool.rs`, `crates/pi-coding-agent/src/plugins/hook.rs`, `crates/pi-coding-agent/src/plugins/flow_extension.rs`

**`crates/pi-coding-agent/src/tools/`:**
- Purpose: Implement the builtin coding filesystem/shell tool set under operation-local capabilities.
- Contains: Tool constructors/executors plus shared path, truncation, diff, and file-mutation queue helpers.
- Key files: `crates/pi-coding-agent/src/tools/mod.rs`, `crates/pi-coding-agent/src/tools/read.rs`, `crates/pi-coding-agent/src/tools/write.rs`, `crates/pi-coding-agent/src/tools/edit.rs`, `crates/pi-coding-agent/src/tools/bash.rs`, `crates/pi-coding-agent/src/tools/grep.rs`, `crates/pi-coding-agent/src/tools/find.rs`, `crates/pi-coding-agent/src/tools/ls.rs`

**`crates/pi-coding-agent/src/config/` and `crates/pi-coding-agent/src/theme/`:**
- Purpose: Resolve layered product settings/auth/paths and coding-agent theme tokens/resources.
- Contains: TOML settings/auth path logic plus builtin JSON themes, schema, resolution, reload/watch, syntax, and token modules.
- Key files: `crates/pi-coding-agent/src/config/mod.rs`, `crates/pi-coding-agent/src/config/settings.rs`, `crates/pi-coding-agent/src/config/auth.rs`, `crates/pi-coding-agent/src/theme/mod.rs`, `crates/pi-coding-agent/src/theme/theme-schema.json`

**`crates/pi-tui/`:**
- Purpose: Supply a generic terminal UI library that is independent of coding-agent sessions and protocols.
- Contains: Terminal/component source in `crates/pi-tui/src`, behavior/contract tests in `crates/pi-tui/tests`, and a rendering example in `crates/pi-tui/examples`.
- Key files: `crates/pi-tui/src/lib.rs`, `crates/pi-tui/src/tui.rs`, `crates/pi-tui/src/terminal.rs`, `crates/pi-tui/src/component.rs`, `crates/pi-tui/src/input/mod.rs`

**`crates/pi-tui/src/components/`:**
- Purpose: Hold reusable UI widgets only; product-specific transcript/session/profile behavior remains in `crates/pi-coding-agent/src/interactive`.
- Contains: Editor, input, markdown, text, box, spacer, loader, selector dialog, select/settings lists, image, and truncated text components.
- Key files: `crates/pi-tui/src/components/editor.rs`, `crates/pi-tui/src/components/input.rs`, `crates/pi-tui/src/components/markdown.rs`, `crates/pi-tui/src/components/selector_dialog.rs`

**`docs/`:**
- Purpose: Record current roadmap material, design contracts, implementation plans, operational notes, and historical references.
- Contains: Current design artifacts under `docs/superpowers`, smoke documentation in `docs/tui-smoke.md`, and explicitly historical content under `docs/archive`.
- Key files: `docs/superpowers/ARCHITECTURE.md`, `docs/superpowers/specs/2026-07-07-operation-runtime-reference-architecture.md`, `docs/lua-plugin-host.md`, `docs/agent-profiles.md`

**`scripts/`:**
- Purpose: Hold repository-level operational/test scripts that do not belong in a Rust crate.
- Contains: TUI smoke automation.
- Key files: `scripts/tui-smoke.sh`

## Key File Locations

**Entry Points:**
- `crates/pi-coding-agent/src/main.rs`: Active product process entry point and RPC/stdin bootstrap.
- `crates/pi-coding-agent/src/lib.rs`: Library entry point, stable `api` facade, and common CLI dispatcher.
- `crates/pi-coding-agent/src/interactive/app.rs`: Interactive TTY adapter entry point.
- `crates/pi-coding-agent/src/print_mode.rs`: One-shot print adapter entry point.
- `crates/pi-coding-agent/src/protocol/json_mode.rs`: One-shot JSON adapter entry point.
- `crates/pi-coding-agent/src/protocol/rpc.rs`: Streaming JSONL RPC adapter entry point.
- `src/main.rs`: Placeholder root binary, not the product entry point.

**Configuration:**
- `Cargo.toml`: Workspace member list and empty root package.
- `Cargo.lock`: Reproducible dependency resolution.
- `crates/pi-coding-agent/src/args.rs`: CLI grammar and mode selection.
- `crates/pi-coding-agent/src/config/paths.rs`: User/project config path resolution.
- `crates/pi-coding-agent/src/config/settings.rs`: Layered settings model and parsing.
- `crates/pi-coding-agent/src/config/auth.rs`: Product auth-store loading/selection.
- `crates/pi-coding-agent/src/runtime.rs`: Runtime/session defaults and model selection.
- `crates/pi-coding-agent/src/theme/theme-schema.json`: Product theme schema.

**Core Logic:**
- `crates/pi-coding-agent/src/coding_session/mod.rs`: Product owner/facade and operation dispatch.
- `crates/pi-coding-agent/src/coding_session/operation.rs`: Internal operation vocabulary and metadata.
- `crates/pi-coding-agent/src/coding_session/prompt_flow.rs`: Primary product prompt graph.
- `crates/pi-coding-agent/src/coding_session/flow_service.rs`: Product flow construction/running boundary.
- `crates/pi-coding-agent/src/coding_session/session_service.rs`: Persistent/transient session behavior.
- `crates/pi-agent-core/src/flow.rs`: Generic graph engine.
- `crates/pi-agent-core/src/agent_turn_flow/runtime.rs`: Low-level provider/tool turn loop.
- `crates/pi-ai/src/registry.rs`: Scoped provider selection and auth application.
- `crates/pi-tui/src/tui.rs`: Generic TUI runtime and rendering ownership.

**Testing:**
- `crates/pi-ai/tests/`: Provider, transport, serialization, model registry, cost, fixtures, and API boundary tests.
- `crates/pi-agent-core/tests/`: Flow, agent loop, hooks, resources, compaction, harness, session context, and boundary tests.
- `crates/pi-coding-agent/tests/`: CLI/modes, sessions, protocol, tools, plugins, delegation, profiles/teams, runtime, and boundary tests.
- `crates/pi-tui/tests/`: Components, input, rendering, terminal lifecycle, styling, image, and public API tests.
- `scripts/tui-smoke.sh`: Cross-terminal smoke harness documented by `docs/tui-smoke.md`.

## Naming Conventions

**Files:**
- Rust modules use `snake_case.rs`: `crates/pi-coding-agent/src/coding_session/intent_router.rs`.
- Workflow graph modules use the `_flow.rs` suffix: `crates/pi-coding-agent/src/coding_session/prompt_flow.rs`.
- Side-effect/coordination owners use the `_service.rs` suffix: `crates/pi-coding-agent/src/coding_session/session_service.rs`.
- Provider implementations group roles as `mod.rs`, `convert.rs`, `wire.rs`, and `process.rs`: `crates/pi-ai/src/providers/google/`.
- Integration tests use behavior names in `snake_case.rs`: `crates/pi-coding-agent/tests/session_print_mode.rs`.
- Architectural dependency checks use `_boundary_guards.rs`: `crates/pi-agent-core/tests/api_boundary_guards.rs`.
- Generated data names its status explicitly where practical: `crates/pi-ai/src/models_generated.json`.

**Directories:**
- Cargo crate directories use kebab-case package names: `crates/pi-coding-agent`, `crates/pi-agent-core`.
- Rust module directories use snake_case: `crates/pi-agent-core/src/agent_turn_flow`, `crates/pi-coding-agent/src/coding_session/session_log`.
- Provider directory names follow API/provider identifiers: `crates/pi-ai/src/providers/openai_codex_responses`.
- Historical documents are isolated under `docs/archive`; current design work belongs under `docs/superpowers/specs` or `docs/superpowers/plans`.

## Where to Add New Code

**New Product Feature:**
- Primary code: Add a focused operation/flow/service module under `crates/pi-coding-agent/src/coding_session`; extend typed operation contracts in `crates/pi-coding-agent/src/coding_session/public_operation.rs` and `crates/pi-coding-agent/src/coding_session/operation.rs`, then register construction/running through `crates/pi-coding-agent/src/coding_session/flow_service.rs`.
- Tests: Add focused integration coverage under `crates/pi-coding-agent/tests` and narrow unit tests beside the new module where private Flow-node behavior needs access.

**New Product Adapter Command:**
- Primary code: Keep command parsing/projection in the owning adapter under `crates/pi-coding-agent/src/interactive`, `crates/pi-coding-agent/src/protocol`, or `crates/pi-coding-agent/src/print_mode.rs`; submit typed operations rather than calling internal services.
- Tests: Use the matching adapter integration files under `crates/pi-coding-agent/tests`, such as `protocol_*.rs`, `interactive_*.rs`, or `print_mode.rs`.

**New Model Provider:**
- Primary code: Create `crates/pi-ai/src/providers/<provider>/` with `mod.rs`, `wire.rs`, `convert.rs`, and `process.rs` as applicable; register the API in `crates/pi-ai/src/providers/mod.rs` and keep shared HTTP/retry behavior in `crates/pi-ai/src/transport`.
- Tests: Add request/stream fixtures and provider tests under `crates/pi-ai/tests`; use `crates/pi-ai/tests/support/mod.rs` for shared deterministic helpers.

**New Low-Level Agent Behavior:**
- Primary code: Put generic turn orchestration in `crates/pi-agent-core/src/agent_turn_flow`, generic graph mechanics in `crates/pi-agent-core/src/flow.rs`, and product-neutral helper state in `crates/pi-agent-core/src/loop_runtime`.
- Tests: Add integration tests under `crates/pi-agent-core/tests`; preserve product-boundary checks in `crates/pi-agent-core/tests/*boundary*.rs`.

**New Builtin Coding Tool:**
- Primary code: Add `crates/pi-coding-agent/src/tools/<tool>.rs`, expose/register it in `crates/pi-coding-agent/src/tools/mod.rs`, and require the narrowest capability from `crates/pi-coding-agent/src/coding_session/capability_snapshot.rs`.
- Tests: Add `crates/pi-coding-agent/tests/tool_<tool>.rs` plus end-to-end coverage in `crates/pi-coding-agent/tests/tools_e2e.rs` when appropriate.

**New Plugin Extension:**
- Primary code: Add or extend a provider/registration contract under `crates/pi-coding-agent/src/plugins`; collect/execute it through `crates/pi-coding-agent/src/coding_session/plugin_service.rs` and load Lua declarations through `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs`.
- Tests: Place registry/host tests beside private plugin modules and product integration coverage under `crates/pi-coding-agent/tests`.

**New Generic TUI Component:**
- Primary code: Add `crates/pi-tui/src/components/<component>.rs`, export it from `crates/pi-tui/src/components/mod.rs` and the stable facade in `crates/pi-tui/src/lib.rs` when it is intended as public API.
- Tests: Add component behavior under `crates/pi-tui/tests`; use `crates/pi-tui/src/virtual_terminal.rs` for deterministic terminal behavior.

**New Coding-Agent UI Behavior:**
- Primary code: Put product-specific state/event handling in `crates/pi-coding-agent/src/interactive`, not `crates/pi-tui/src`; translate product events through `crates/pi-coding-agent/src/interactive/event_bridge.rs`.
- Tests: Add interactive integration tests under `crates/pi-coding-agent/tests` and deterministic loop/component tests beside `crates/pi-coding-agent/src/interactive/app.rs` or `loop.rs` when private access is required.

**New Durable Session Fact:**
- Primary code: Define the typed event in `crates/pi-coding-agent/src/coding_session/session_log/event.rs`, stage it through `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`, fold it in `crates/pi-coding-agent/src/coding_session/session_log/replay.rs`, and commit through `crates/pi-coding-agent/src/coding_session/session_service.rs`.
- Tests: Add replay, serialization/version, commit/recovery, and adapter projection coverage under `crates/pi-coding-agent/src/coding_session` and `crates/pi-coding-agent/tests`.

**New Shared Utility:**
- Shared helpers: Keep utilities in the narrowest owner: provider/network helpers in `crates/pi-ai/src/util` or `transport`; low-level agent helpers in `crates/pi-agent-core/src`; coding-product helpers in `crates/pi-coding-agent/src`; terminal text/layout helpers in `crates/pi-tui/src/utils`.

## Special Directories

**`.codegraph/`:**
- Purpose: Local CodeGraph index used for symbol/call-path exploration.
- Generated: Yes.
- Committed: No; ignored by `.gitignore`.

**`.planning/codebase/`:**
- Purpose: Structured GSD codebase reference consumed by planning and execution workflows.
- Generated: Yes, by `$gsd-map-codebase`.
- Committed: Intended to be committed by the mapping orchestrator; currently newly generated in the working tree.

**`.qoder/repowiki/`:**
- Purpose: Generated repository wiki/knowledge artifacts, primarily Chinese documentation and metadata.
- Generated: Yes.
- Committed: Yes.

**`target/`:**
- Purpose: Cargo build artifacts and incremental compilation output.
- Generated: Yes.
- Committed: No; ignored by `.gitignore`.

**`crates/pi-ai/src/models_generated.json`:**
- Purpose: Generated model registry data consumed by `crates/pi-ai/src/models.rs`.
- Generated: Yes, via `crates/pi-ai/tools/generate_models.cjs`.
- Committed: Yes.

**`crates/pi-coding-agent/src/theme/`:**
- Purpose: Runtime theme implementation plus embedded `dark.json`, `light.json`, and `theme-schema.json` assets.
- Generated: No; theme JSON assets are source inputs.
- Committed: Yes.

**`docs/archive/`:**
- Purpose: Preserve historical ideas, completed roadmaps, and TypeScript parity reports without treating them as current runtime contracts.
- Generated: No.
- Committed: Yes.

**`crates/pi-mom/`, `crates/pi-pods/`, `crates/pi-web-ui/`:**
- Purpose: Reserved workspace crate names; current contents are Cargo template placeholders only.
- Generated: No.
- Committed: Yes.

---

*Structure analysis: 2026-07-10*
