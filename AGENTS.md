## 语言选择
使用中文跟用户沟通，专业、常用的术语可以使用英文. 技术文档可完全使用英文.

<!-- CODEGRAPH_START -->
## CodeGraph

In repositories indexed by CodeGraph (a `.codegraph/` directory exists at the repo root), reach for it BEFORE grep/find or reading files when you need to understand or locate code:

- **MCP tool** (when available): `codegraph_explore` answers most code questions in one call - the relevant symbols' verbatim source plus the call paths between them, including dynamic-dispatch hops grep cannot follow. Name a file or symbol in the query to read its current line-numbered source. If it is listed but deferred, load it by name via tool search.
- **Shell** (always works): `codegraph explore "<symbol names or question>"` prints the same output.

If there is no `.codegraph/` directory, skip CodeGraph entirely - indexing is the user's decision.
<!-- CODEGRAPH_END -->

<!-- GSD:project-start source:PROJECT.md -->

## Project

**Canonical Operation Runtime Convergence**

This project completes the Stage 9 runtime convergence for the existing `pi-rust` coding-agent workspace. It will audit the current implementation, then make `CodingAgentSession::run(CodingAgentOperation)` the single public live-session operation dispatcher used by every first-party adapter and test before deleting the replaced workflow-specific session methods.

The existing implementation plan at `docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md` is design input and historical evidence, not the new execution structure. Current code, tests, source guards, and repository history determine what is actually complete and how the remaining work is phased.

**Core Value:** Every first-party live-session product operation follows one typed, admitted, behavior-preserving runtime path through `CodingAgentSession::run`.

### Constraints

- **Architecture**: Preserve the dependency direction `pi-coding-agent -> pi-agent-core -> pi-ai` and `pi-coding-agent -> pi-tui`; product semantics must not move into lower-level crates
- **Public API**: New and migrated callers use contracts exported by `pi_coding_agent::api`; internal `Operation`, dispatch metadata, plugin load options, services, and Flow nodes remain private
- **Behavior compatibility**: JSON output, print output, RPC responses, interactive projections, event ordering, control handling, session replay, and persistent navigation behavior must not regress
- **Durability**: Preserve typed session facts, replay authority, append/manifest ordering, operation identifiers, recovery markers, and explicit `PartialCommit` reporting
- **Testing**: Use deterministic offline fixtures and retain existing assertions; migration is not permission to reduce coverage or replace behavior checks with compile-only checks
- **Deletion order**: Production adapters and tests migrate before broad workflow methods are deleted; missed callers must be migrated rather than restoring deleted methods
- **Verification**: Completion requires `cargo fmt --check`, focused `pi-coding-agent` tests, `cargo test --workspace`, `cargo check --workspace`, source audits, and `git diff --check`
- **Scope**: Stage 10 event compatibility work and unrelated reliability/security/performance initiatives remain outside this milestone

<!-- GSD:project-end -->

<!-- GSD:stack-start source:codebase/STACK.md -->

## Technology Stack

## Languages

- Rust, edition 2024 - All production libraries and binaries live in `src/main.rs` and `crates/*/src/**/*.rs`; the edition is declared in `Cargo.toml` and every crate manifest under `crates/*/Cargo.toml`.
- JavaScript (CommonJS), version unpinned - Optional Node.js model-catalog conversion utility in `crates/pi-ai/tools/generate_models.cjs`; it is not part of the Cargo build.
- Bash, version unpinned - Tmux-driven interactive smoke automation in `scripts/tui-smoke.sh`.
- TOML and JSON - Workspace/package configuration in `Cargo.toml` and `crates/*/Cargo.toml`, runtime settings/auth schemas in `crates/pi-coding-agent/src/config/`, plugin manifests consumed by `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs`, and the generated model catalog in `crates/pi-ai/src/models_generated.json`.

## Runtime

- Native Rust executable with Tokio 1.52.3 asynchronous runtime - `crates/pi-coding-agent/src/main.rs` uses `#[tokio::main]`, while provider streaming and agent control use Tokio tasks, channels, timers, process I/O, and cancellation throughout `crates/pi-ai/src/`, `crates/pi-agent-core/src/`, and `crates/pi-coding-agent/src/`.
- Rust stable toolchain, not repository-pinned - Edition 2024 in `Cargo.toml` requires Rust 1.85 or newer; no `rust-toolchain.toml` accompanies the workspace manifests.
- The operational application is the `pi-coding-agent` binary in `crates/pi-coding-agent/src/main.rs`; the workspace-root binary in `src/main.rs` is only a `Hello, world!` placeholder.
- Cargo, resolver supplied by the installed Rust toolchain - Workspace membership and package metadata are defined in `Cargo.toml` and `crates/*/Cargo.toml`.
- Lockfile: present as `Cargo.lock` format version 4, with exact third-party versions committed.

## Frameworks

- Tokio 1.52.3 - Async runtime for the CLI entry point, provider streams, cancellation, filesystem/process work, broadcasts, and stdio RPC in `crates/pi-coding-agent/src/main.rs`, `crates/pi-ai/src/`, and `crates/pi-coding-agent/src/protocol/rpc.rs`.
- Reqwest 0.12.28 with Rustls TLS - JSON HTTP requests and streamed response bodies for provider clients in `crates/pi-ai/Cargo.toml`, `crates/pi-ai/src/providers/`, and `crates/pi-ai/src/transport/http.rs`.
- Crossterm 0.28.1 plus the in-house `pi-tui` component/runtime layer - Terminal control, input, rendering, overlays, Markdown, and inline images in `crates/pi-tui/Cargo.toml` and `crates/pi-tui/src/`.
- Serde 1.0.228, serde_json 1.0.150, serde_yaml 0.9.34, and toml 0.8.23 - Wire models, settings, auth entries, plugin manifests, resources, and durable session records in `crates/pi-ai/src/types/`, `crates/pi-agent-core/src/resources.rs`, `crates/pi-coding-agent/src/config/`, and `crates/pi-coding-agent/src/coding_session/session_log/`.
- No web application framework is active - `crates/pi-web-ui/src/lib.rs` is a placeholder and `crates/pi-web-ui/Cargo.toml` has no dependencies.
- Rust built-in test harness plus Tokio `#[tokio::test]` - Unit tests are colocated under `#[cfg(test)]` modules and integration suites live in `crates/pi-ai/tests/`, `crates/pi-agent-core/tests/`, `crates/pi-coding-agent/tests/`, and `crates/pi-tui/tests/`.
- Tempfile 3.27.0 and custom deterministic guards - Filesystem fixtures and environment/provider isolation are used from crate dev-dependencies and helpers such as `crates/pi-coding-agent/tests/support/mod.rs` and `crates/pi-agent-core/tests/common/mod.rs`.
- Cargo - Build, test, example, and package selection are driven by the workspace and crate manifests in `Cargo.toml` and `crates/*/Cargo.toml`.
- Rustfmt and Clippy from the Rust toolchain - Source contains targeted Clippy allowances in `crates/pi-coding-agent/src/lib.rs`; no repository-specific `rustfmt.toml` or `clippy.toml` is present next to `Cargo.toml`.
- Node.js - Only required to regenerate `crates/pi-ai/src/models_generated.json` with `crates/pi-ai/tools/generate_models.cjs`; the repository does not pin a Node version or include a JavaScript package manifest.
- Bash and tmux - Required only for the TUI capture suite in `scripts/tui-smoke.sh`, which builds `pi-coding-agent` and writes captures below `target/tui-smoke/`.

## Key Dependencies

- `pi-ai` 0.1.0 - Provider registry, 921-model catalog, request conversion, authentication injection, HTTP/SSE transport, Bedrock event-stream handling, and response normalization in `crates/pi-ai/`.
- `pi-agent-core` 0.1.0 - Agent loop, turn flows, tools, compaction, queues, resource loading, and provider-facing context in `crates/pi-agent-core/`.
- `pi-coding-agent` 0.1.0 - Product CLI, configuration, session/runtime orchestration, stdio protocols, built-in coding tools, profiles, delegation, plugins, and interactive TUI integration in `crates/pi-coding-agent/`.
- `pi-tui` 0.1.0 - Terminal abstraction and reusable TUI components in `crates/pi-tui/`.
- Futures 0.3.32, async-stream 0.3.6, and tokio-util 0.7.18 - Stream construction/composition and cancellation across `crates/pi-ai/Cargo.toml` and `crates/pi-agent-core/Cargo.toml`.
- Thiserror 2.0.18 - Typed error surfaces across provider, core-agent, CLI, and TUI crates declared in `crates/pi-ai/Cargo.toml`, `crates/pi-agent-core/Cargo.toml`, `crates/pi-coding-agent/Cargo.toml`, and `crates/pi-tui/Cargo.toml`.
- mlua 0.10.5 with vendored Lua 5.4 - Sandboxed local plugin execution and host capability registration in `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs` and `crates/pi-coding-agent/src/plugins/`.
- ring 0.17.14 and base64 0.22.1 - PKCE hashing, token/JWT decoding, request signing support, and encoded image content in `crates/pi-ai/src/util/oauth.rs`, `crates/pi-ai/src/providers/bedrock/`, `crates/pi-ai/src/providers/openai_codex_responses/mod.rs`, and `crates/pi-coding-agent/src/input.rs`.
- image 0.25.10 - PNG/JPEG/GIF/WebP decoding and resizing before multimodal requests in `crates/pi-coding-agent/src/input.rs`.
- notify 7.0.0 - Debounced custom-theme filesystem watching in `crates/pi-coding-agent/src/theme/reload.rs`.
- ignore 0.4.25, globset 0.4.18, and regex 1.12.3 - Gitignore-aware traversal, filtering, resource discovery, and coding tools in `crates/pi-agent-core/src/resources.rs` and `crates/pi-coding-agent/src/tools/`.
- pulldown-cmark 0.12.2, syntect 5.3.0, unicode-segmentation 1.13.3, and unicode-width 0.2.2 - Markdown parsing, syntax highlighting, and terminal-safe text layout in `crates/pi-tui/src/components/markdown.rs`, `crates/pi-coding-agent/src/interactive/`, and `crates/pi-tui/src/`.
- dirs 6.0.0, time 0.3.47, and uuid 1.23.2 - User-directory resolution, timestamps, and UUID v7 identifiers in `crates/pi-coding-agent/src/config/paths.rs`, `crates/pi-coding-agent/src/session.rs`, and `crates/pi-agent-core/src/`.

## Configuration

- Global runtime state defaults to `~/.pi-rust/` and can be relocated with `PI_RUST_DIR`; project overrides live below `<cwd>/.pi-rust/` as implemented in `crates/pi-coding-agent/src/config/paths.rs`.
- Settings merge global `settings.toml` with project `.pi-rust/settings.toml`, with project values winning, in `crates/pi-coding-agent/src/config/settings.rs`.
- Provider credentials resolve from CLI arguments, provider-specific environment variables, then global `auth.toml`; `$VAR` and `${VAR}` references inside auth entries are expanded by `crates/pi-coding-agent/src/config/auth.rs` and provider mappings live in `crates/pi-ai/src/util/env_keys.rs`.
- Session storage defaults to `${PI_RUST_DIR:-~/.pi-rust}/sessions` and can be overridden by CLI/runtime configuration or `PI_SESSION_DIR` in `crates/pi-coding-agent/src/session.rs`.
- Skills, prompt templates, themes, profiles, and Lua plugins are loaded from user and project roots by `crates/pi-coding-agent/src/resources.rs`, `crates/pi-coding-agent/src/coding_session/profiles.rs`, and `crates/pi-coding-agent/src/coding_session/mod.rs`.
- Workspace topology is declared in `Cargo.toml`; dependency features and crate-local dev dependencies are declared in `crates/*/Cargo.toml`.
- Exact dependency resolution is committed in `Cargo.lock`; there are no workspace `build.rs` scripts or non-Cargo build-system manifests alongside `Cargo.toml`.
- The checked-in provider/model data file is `crates/pi-ai/src/models_generated.json`; regenerate it explicitly with `crates/pi-ai/tools/generate_models.cjs` rather than as an implicit Cargo build step.

## Platform Requirements

- Use a Rust stable toolchain supporting edition 2024 and Cargo lockfile version 4 to build the manifests in `Cargo.toml` and `crates/*/Cargo.toml`.
- Run the main application with Cargo package selection for `pi-coding-agent`, whose entry point is `crates/pi-coding-agent/src/main.rs`; building the root package alone only compiles `src/main.rs`.
- Node.js is optional for model-catalog regeneration through `crates/pi-ai/tools/generate_models.cjs`; Bash plus tmux is optional for `scripts/tui-smoke.sh`.
- Provider integration tests should remain deterministic/offline unless explicitly opted into a real-provider path; fake providers and boundary suites are available in `crates/pi-ai/src/providers/faux.rs`, `crates/pi-ai/tests/`, and `crates/pi-coding-agent/tests/`.
- Deployment target is a local native CLI/TUI executable from `crates/pi-coding-agent/src/main.rs`, not a hosted server; automation can communicate through JSONL over stdin/stdout in `crates/pi-coding-agent/src/protocol/rpc.rs`.
- Interactive use requires an ANSI-capable terminal supported by `crates/pi-tui/src/terminal.rs`; inline images use Kitty or iTerm2 protocols when detected by `crates/pi-tui/src/terminal_image.rs`.
- Model use requires outbound HTTPS access to the selected provider endpoint from `crates/pi-ai/src/models_generated.json`; streaming is handled by `crates/pi-ai/src/providers/` and `crates/pi-ai/src/transport/http.rs`.
- Coding tools require local filesystem and process permissions in the selected working directory through `crates/pi-coding-agent/src/tools/`; optional clipboard commands are selected per OS in `crates/pi-coding-agent/src/interactive/clipboard.rs`.

<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->

## Conventions

## Naming Patterns

- Use lowercase `snake_case.rs` for Rust modules and focused implementation units, such as `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs` and `crates/pi-agent-core/src/agent_turn_flow/runtime.rs`.
- Use `mod.rs` only for directory module roots and re-export surfaces, as in `crates/pi-agent-core/src/resources/mod.rs` and `crates/pi-tui/src/components/mod.rs`.
- Name integration-test files after the behavior or boundary they cover, such as `crates/pi-agent-core/tests/agent_loop.rs`, `crates/pi-ai/tests/http_retry.rs`, and `crates/pi-coding-agent/tests/api_boundary_guards.rs`.
- Use explicit suffixes for architectural enforcement tests: `*_boundary_guards.rs`, `public_api.rs`, and `deterministic_boundary.rs` identify contract tests rather than feature examples.
- Use `snake_case` for functions and methods, with behavior-oriented names such as `parse_retry_after_ms` in `crates/pi-ai/src/util/http.rs` and `resolve_prompt_request` exported by `crates/pi-coding-agent/src/lib.rs`.
- Test functions read as complete behavioral statements, for example `provider_guard_restores_existing_provider_on_drop` in `crates/pi-ai/tests/support_guards.rs` and `root_public_modules_are_marked_migration_private` in `crates/pi-agent-core/tests/api_boundary_guards.rs`.
- Use `new` for primary constructors and `with_*` consuming builders for optional configuration, as shown by `SelfHealingEditRequest::new`, `with_check_command`, and `with_repair_attempts` in `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs`.
- Use verb prefixes consistently: `load_*`, `resolve_*`, `parse_*`, `build_*`, `run_*`, `format_*`, and `register_*` communicate side effects and ownership across `crates/pi-agent-core/src/lib.rs` and `crates/pi-coding-agent/src/lib.rs`.
- Use short conventional names only when scope is small (`ctx`, `opts`, `cfg`, `req`); otherwise prefer semantic names such as `release_rx`, `provider_streamer`, and `previous_non_empty` in `crates/pi-agent-core/tests/agent_loop.rs` and `crates/pi-agent-core/tests/api_boundary_guards.rs`.
- Use suffixes to show cloned or adapted ownership in async closures, such as `calls_for_streamer` in `crates/pi-agent-core/tests/agent_loop.rs`.
- Prefix intentionally retained ownership guards with `_`, such as `_lock` and `_guard` in `crates/pi-coding-agent/tests/support/mod.rs` and `crates/pi-ai/tests/support_guards.rs`.
- Name constants in `SCREAMING_SNAKE_CASE`; test-only source snapshots and timing anchors follow this convention in `crates/pi-tui/tests/deterministic_boundary.rs`.
- Use `UpperCamelCase` for structs, enums, traits, aliases, and enum variants: `RetryConfig`, `ApiProvider`, `AssistantMessageEvent::Done`, and `CodingAgentSession` in `crates/pi-ai/src/util/http.rs`, `crates/pi-ai/tests/support_guards.rs`, and `crates/pi-coding-agent/src/lib.rs`.
- Use domain suffixes to make roles explicit: `*Config`, `*Options`, `*Request`, `*Outcome`, `*Error`, `*Provider`, `*Guard`, `*Flow`, and `*Service` recur throughout `crates/pi-agent-core/src/` and `crates/pi-coding-agent/src/coding_session/`.
- Derive standard traits explicitly according to value semantics. Configuration/data types commonly derive `Debug`, `Clone`, `PartialEq`, serialization traits, and sometimes `Default`, as in `PartialCompaction` in `crates/pi-coding-agent/src/config/settings.rs`.

## Code Style

- Use standard `rustfmt` through `cargo fmt`; no `rustfmt.toml` or `.rustfmt.toml` is present, so default toolchain formatting is authoritative.
- Keep formatter-produced multiline imports, derives, match arms, and builder expressions. Representative formatting appears in `crates/pi-agent-core/src/lib.rs` and `crates/pi-ai/tests/openai_completions.rs`.
- Verify formatting with `cargo fmt --check`; this command is part of the recurring project verification recorded in `docs/TODO.md` and phase plans under `docs/superpowers/plans/`.
- Use trailing commas in multiline structs, calls, enum variants, and collection literals so `rustfmt` produces stable diffs, as in `crates/pi-ai/tests/openai_completions.rs`.
- Use Clippy for static analysis. The project documents `cargo clippy --fix` across `pi-tui`, `pi-ai`, `pi-agent-core`, and `pi-coding-agent` in `docs/TODO.md`.
- Keep lint suppression narrow and documented by code location. `crates/pi-coding-agent/src/lib.rs` has crate-level allowances for intentionally large error/enum types, argument-heavy APIs, and compatibility conditionals; most other allowances are item- or module-scoped.
- Preserve explicit compatibility allowances such as `#[allow(deprecated)]` only around migration shims and compatibility tests in `crates/pi-ai/src/lib.rs`, `crates/pi-agent-core/src/lib.rs`, and `crates/pi-coding-agent/tests/support/mod.rs`.
- Do not add broad warning suppression to new crates. Match the smallest existing scope in the owning module, and add a boundary test when the allowance protects a deliberate migration surface.

## Import Organization

- Rust module paths are used directly; no custom path-alias configuration is present.
- Inside a crate, prefer `crate::...` for cross-module references, as in `crates/pi-coding-agent/src/config/settings.rs` and `crates/pi-ai/src/util/http.rs`.
- Downstream and integration-test code should prefer stable `pi_ai::api`, `pi_agent_core::api`, and `pi_coding_agent::api` facades where available. Root-level compatibility exports remain deliberately constrained by tests such as `crates/pi-agent-core/tests/api_boundary_guards.rs`.
- Use `super::*` primarily in small co-located `#[cfg(test)]` modules such as `crates/pi-coding-agent/src/config/paths.rs`; integration tests use explicit public imports.

## Error Handling

- Return `Result<T, E>` for recoverable failures and use typed domain errors where the API crosses a component boundary. `thiserror` is used by `pi-agent-core`, `pi-coding-agent`, and `pi-tui` via their `Cargo.toml` manifests.
- Convert lower-level failures at the ownership boundary with `map_err`, attaching domain context. `PluginError::Execution` conversion in `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs` is representative.
- Use early returns for invalid conditions and keep error messages actionable, as in `parse_retry_after_ms` in `crates/pi-ai/src/util/http.rs`.
- In tests, use `expect` with a state-specific message when failure indicates a broken fixture or invariant, and use `unwrap` only for setup that cannot meaningfully recover. Examples appear in `crates/pi-ai/tests/support_guards.rs` and `crates/pi-agent-core/tests/agent_loop.rs`.
- Model provider-stream failures as events where streaming APIs require continued protocol consistency; scripted error events in `crates/pi-agent-core/tests/common/mod.rs` exercise this behavior.

## Logging

- Send user-facing errors and diagnostics to stderr at executable/interactive boundaries, as in `crates/pi-coding-agent/src/main.rs`, `crates/pi-coding-agent/src/interactive/loop.rs`, and `crates/pi-coding-agent/src/resources.rs`.
- Keep reusable library logic free of incidental console output. Return typed errors, diagnostics, outcomes, or events from modules under `crates/pi-ai/src/`, `crates/pi-agent-core/src/`, and `crates/pi-coding-agent/src/coding_session/`.
- Emit progress through established event types such as `AgentEvent`, `AssistantMessageEvent`, and `CodingAgentProductEvent`, exported from `crates/pi-agent-core/src/lib.rs`, `crates/pi-ai/src/lib.rs`, and `crates/pi-coding-agent/src/lib.rs`.
- Reserve `println!` in library-adjacent code for examples, benchmarks, and intentional command output; examples include `crates/pi-coding-agent/src/interactive/app.rs` and `crates/pi-agent-core/examples/loop_example.rs`.

## Comments

- Explain architectural intent, compatibility constraints, unsafe/global-state handling, and non-obvious protocol behavior. Stable-facade rationale is documented in `crates/pi-ai/src/lib.rs`, `crates/pi-agent-core/src/lib.rs`, and `crates/pi-coding-agent/src/lib.rs`.
- Use short inline comments to explain fixture ordering or protocol structure, such as the message-order comment in `crates/pi-ai/tests/openai_completions.rs`.
- Avoid narrating straightforward assignments or control flow. Prefer expressive type and test names throughout `crates/pi-tui/tests/` and `crates/pi-agent-core/tests/`.
- Not applicable; this is a Rust workspace.
- Use Rust doc comments (`///` and `//!`) for public contracts, module intent, migration guidance, and helper caveats, as shown in `crates/pi-agent-core/src/agent_loop.rs`, `crates/pi-agent-core/src/lib.rs`, and `crates/pi-agent-core/tests/common/mod.rs`.
- Use `#[deprecated(note = "...")]` for machine-visible migration guidance rather than relying only on prose comments, as in the crate root facades.

## Function Design

- Accept borrowed inputs (`&str`, `&Path`, slices, `Option<&T>`) when ownership is unnecessary, as in `crates/pi-ai/src/util/http.rs` and `crates/pi-coding-agent/src/config/paths.rs`.
- Accept `impl Into<String>` or `impl AsRef<OsStr>` at ergonomic construction/configuration boundaries, as in `SelfHealingEditRequest` and `EnvGuard` helpers.
- Group evolving optional behavior into `*Config` and `*Options` structs rather than extending long positional argument lists. Existing exceptions are explicitly covered by scoped Clippy allowances in `crates/pi-coding-agent/src/lib.rs`.
- Return owned domain values from constructors and transforms, `Option<T>` for absence, and `Result<T, E>` for recoverable failure.
- Return borrowed views (`&str`, slices, `Option<&T>`) from accessors, following `SelfHealingEditRequest` in `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs`.
- Use iterators and streams for incremental data. `EventStream` and async stream implementations in `crates/pi-ai/src/lib.rs` and `crates/pi-agent-core/tests/common/mod.rs` establish the streaming pattern.

## Module Design

- Keep implementation modules private by default (`mod`) and expose intentional contracts through crate-root `api` modules and curated `pub use` lists in `crates/pi-ai/src/lib.rs`, `crates/pi-agent-core/src/lib.rs`, and `crates/pi-coding-agent/src/lib.rs`.
- Mark migration-only public modules `#[doc(hidden)]`; boundary tests such as `crates/pi-agent-core/tests/api_boundary_guards.rs` enforce this convention.
- Add compatibility exports only with `#[deprecated(note = "...")]` and a named replacement path. Do not expand root-level facades casually.
- Rust crate roots and selected `mod.rs` files serve as controlled barrels. Use them to declare modules and re-export a deliberate stable surface, not every internal symbol.
- Add new stable public APIs to the owning crate's `api` module and update public-API/boundary tests under that crate's `tests/` directory.
- Keep test helpers in crate-local support modules such as `crates/pi-ai/tests/support/mod.rs`, `crates/pi-agent-core/tests/common/mod.rs`, and `crates/pi-coding-agent/tests/support/mod.rs`; do not promote them into production facades.

<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->

## Architecture

## System Overview

```text
| Product entry points and adapters                                     |
| Interactive terminal | Print / JSON         | JSONL RPC               |
| `crates/pi-coding-   | `crates/pi-coding-  | `crates/pi-coding-      |
| agent/src/interactive`| agent/src/print_mode.rs` | agent/src/protocol` |
| Product operation runtime                                             |
| `crates/pi-coding-agent/src/coding_session`                            |
| typed Operation -> admission/capability snapshot -> product Flow       |
| -> services -> ProductEvent -> session transaction                     |
| Low-level agent runtime            |  | Generic terminal UI           |
| `crates/pi-agent-core/src`          |  | `crates/pi-tui/src`           |
| AgentTurnFlow, tools, hooks, queues |  | components, input, rendering  |
| Model/provider and transport runtime                                  |
| `crates/pi-ai/src`                                                     |
| model registry -> scoped provider registry -> provider adapter -> HTTP |
| External model APIs and local durable session storage                  |
| provider HTTPS/SSE; `session.json` + `events.jsonl` via `session_log`   |
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| Binary bootstrap | Parse process arguments, special-case streaming RPC, collect piped stdin, install builtin tools, and convert `CliOutput` into process output/exit status | `crates/pi-coding-agent/src/main.rs` |
| CLI facade | Parse modes, resolve prompt requests, and dispatch interactive, print, JSON, or RPC-compatible work | `crates/pi-coding-agent/src/lib.rs` |
| Stable product API | Re-export intended embedding contracts while root modules and deprecated exports remain available during migration | `crates/pi-coding-agent/src/lib.rs` |
| Product runtime owner | Own session state, operation admission, services, profiles, plugins, capability generations, and public operation execution | `crates/pi-coding-agent/src/coding_session/mod.rs` |
| Operation contract | Classify each product action by kind, origin, class, and async/sync dispatch mode | `crates/pi-coding-agent/src/coding_session/operation.rs` |
| Public operation facade | Convert public `CodingAgentOperation` values to internal operations and convert internal outcomes back to stable results | `crates/pi-coding-agent/src/coding_session/public_operation.rs` |
| Intent admission | Resolve client intents into typed operations and enforce operation metadata/capability rules | `crates/pi-coding-agent/src/coding_session/intent_router.rs` |
| Operation control | Enforce active-operation exclusivity and carry abort/follow-up/steering control handles | `crates/pi-coding-agent/src/coding_session/operation_control.rs` |
| Product flow factory | Construct and run prompt, compaction, export, plugin load, branch summary, agent/team, and self-healing-edit graphs | `crates/pi-coding-agent/src/coding_session/flow_service.rs` |
| Prompt orchestration | Execute the ordered product prompt graph from request resolution through session commit and completion events | `crates/pi-coding-agent/src/coding_session/prompt_flow.rs` |
| Runtime assembly | Build and hydrate low-level `Agent` instances from immutable runtime snapshots, tools, plugins, auth, and resources | `crates/pi-coding-agent/src/coding_session/runtime_service.rs` |
| Capability snapshots | Derive operation-local model, tool, filesystem, shell, session, UI, and plugin permissions | `crates/pi-coding-agent/src/coding_session/capability_snapshot.rs` |
| Session persistence | Create/open/list/replay/fork/clone sessions and commit typed transactions to the Rust-native log | `crates/pi-coding-agent/src/coding_session/session_service.rs` |
| Durable log primitives | Store manifests and JSONL envelopes, replay/fold state, and stage transactional operation facts | `crates/pi-coding-agent/src/coding_session/session_log` |
| Event publication | Map low-level agent events into product events, assign sequences, retain a replay window, and broadcast to adapters | `crates/pi-coding-agent/src/coding_session/event_service.rs` |
| Plugin host | Register capability-scoped tools, commands, hooks, keybinds, dialogs, UI actions, and Lua-backed providers | `crates/pi-coding-agent/src/plugins` |
| Builtin coding tools | Implement read/write/edit/bash/grep/find/ls behind filesystem or shell capabilities | `crates/pi-coding-agent/src/tools` |
| Low-level flow engine | Run typed node graphs with actions, transitions, cancellation, step limits, and flow events | `crates/pi-agent-core/src/flow.rs` |
| Agent turn runtime | Orchestrate context preparation, provider streaming, stop/tool decisions, tool execution, and subsequent turns | `crates/pi-agent-core/src/agent_turn_flow` |
| Provider runtime | Resolve model API identifiers and auth into scoped providers, then return normalized assistant event streams | `crates/pi-ai/src/registry.rs` |
| Provider adapters | Convert shared request types to provider wire formats and process provider-specific streaming responses | `crates/pi-ai/src/providers` |
| Transport | Own HTTP headers, retry/error classification, and request execution shared by providers | `crates/pi-ai/src/transport` |
| Generic TUI | Provide terminal lifecycle, normalized input, component trees, overlays, differential rendering, themes, and image protocols | `crates/pi-tui/src` |
| Product TUI adapter | Project product events/snapshots into coding-agent-specific transcript, menus, commands, and footer state | `crates/pi-coding-agent/src/interactive` |

## Pattern Overview

- Product actions enter as typed operations at `crates/pi-coding-agent/src/coding_session/public_operation.rs` and are classified before execution by `crates/pi-coding-agent/src/coding_session/operation.rs`.
- Workflows are explicit graphs built on the generic `Flow<C>` engine in `crates/pi-agent-core/src/flow.rs`; product graphs stay in `pi-coding-agent`, while the reusable agent loop stays in `pi-agent-core`.
- Side effects are service-owned: `SessionService` persists facts, `RuntimeService` assembles agents, `EventService` publishes semantic events, and `PluginService` mediates extensions under `crates/pi-coding-agent/src/coding_session`.
- Persistent session truth is reconstructed from typed `SessionEventEnvelope` records under `crates/pi-coding-agent/src/coding_session/session_log`; adapters consume product events and snapshots instead of reading the log directly.
- Crate dependencies form a one-way DAG: `pi-coding-agent` depends on `pi-agent-core`, `pi-ai`, and `pi-tui`; `pi-agent-core` depends on `pi-ai`; `pi-ai` and `pi-tui` remain product-neutral.
- Compatibility surfaces remain visible but are marked deprecated in `crates/pi-coding-agent/src/lib.rs` and `crates/pi-ai/src/lib.rs`; new consumers should use each crate's `api` module.

## Layers

- Purpose: Translate process I/O, CLI commands, TTY input, JSON, and JSONL RPC messages into product operations; project product events back to users or clients.
- Location: `crates/pi-coding-agent/src/main.rs`, `crates/pi-coding-agent/src/lib.rs`, `crates/pi-coding-agent/src/interactive`, `crates/pi-coding-agent/src/print_mode.rs`, `crates/pi-coding-agent/src/protocol`
- Contains: Mode dispatch, request resolution, interactive event loop, UI projection, JSON serialization, and RPC command state.
- Depends on: The stable/product facade in `crates/pi-coding-agent/src/coding_session` and generic terminal primitives from `crates/pi-tui/src`.
- Used by: Shell users, embedding callers, scripted JSON consumers, and long-lived RPC clients.
- Purpose: Own coding-agent semantics, operation admission, workflow composition, profiles/teams/delegation, plugins, session navigation, and product events.
- Location: `crates/pi-coding-agent/src/coding_session`
- Contains: `CodingAgentSession`, typed operations, Flow contexts, product flows, service boundaries, capability snapshots, event publication, projections, and session logging.
- Depends on: `pi-agent-core` for agent/flow primitives, `pi-ai` types/providers, local config/resources, plugins, and builtin tools.
- Used by: All adapters in `crates/pi-coding-agent/src/interactive`, `crates/pi-coding-agent/src/print_mode.rs`, and `crates/pi-coding-agent/src/protocol`.
- Purpose: Execute a provider/tool loop without knowing coding-agent sessions, UI protocols, profile policy, or persistence rules.
- Location: `crates/pi-agent-core/src`
- Contains: `Agent`, `AgentTurnFlow`, `Flow<C>`, tool contracts, hooks, queues, context conversion, compaction, resources, execution environments, and low-level agent events.
- Depends on: Shared model/message/event contracts and streaming from `crates/pi-ai/src`.
- Used by: Product runtime construction in `crates/pi-coding-agent/src/coding_session/runtime_service.rs` and agent invocations in product flows.
- Purpose: Normalize model metadata, authentication, request conversion, streaming event parsing, cost/usage, and HTTP behavior across providers.
- Location: `crates/pi-ai/src`
- Contains: `ProviderRegistry`, `AiClient`, `ApiProvider`, model registry, provider-specific `convert`/`wire`/`process` modules, transport and retry helpers, compatibility adapters, and image APIs.
- Depends on: Network/runtime libraries only; it does not depend on product or TUI crates.
- Used by: `pi-agent-core` provider streaming and product model selection/configuration.
- Purpose: Provide reusable terminal mechanics without coding-agent business semantics.
- Location: `crates/pi-tui/src`
- Contains: `Terminal`, `ProcessTerminal`, `VirtualTerminal`, `Tui<T>`, `Component`, editor/input/list/dialog components, overlay/focus management, render scheduling, ANSI styles, and terminal image support.
- Depends on: Terminal and rendering libraries; it does not depend on `pi-coding-agent`.
- Used by: Product-specific interactive code under `crates/pi-coding-agent/src/interactive`.
- Purpose: Persist session identity, active leaf, operation boundaries, messages, compaction, delegation, runtime generations, and navigation as typed facts.
- Location: `crates/pi-coding-agent/src/coding_session/session_log` and `crates/pi-coding-agent/src/coding_session/session_service.rs`
- Contains: `session.json` manifest handling, `events.jsonl` append/read, transaction staging, replay/fold, recovery markers, and fork/clone/export helpers.
- Depends on: Local filesystem and typed product/session contracts.
- Used by: `CodingAgentSession` only through `SessionService`; adapters receive derived views and snapshots.

## Data Flow

### Primary Request Path

### Tool Call Loop

### Session Commit and Replay

### Interactive Projection

- Product ownership is explicit mutable state on `CodingAgentSession` in `crates/pi-coding-agent/src/coding_session/mod.rs`; services are fields rather than process-wide service locators.
- Each operation receives an immutable `OperationCapabilitySnapshot` from `crates/pi-coding-agent/src/coding_session/capability_snapshot.rs`; runtime-affecting permission state is generation-tracked.
- Low-level agent state is shared as `Arc<RwLock<AgentState>>` while a turn copies state into `AgentTurnContext` and applies it back after graph execution in `crates/pi-agent-core/src/agent_turn_flow/runtime.rs`.
- Live adapter events use Tokio broadcast channels plus a bounded retained deque in `crates/pi-coding-agent/src/coding_session/event_service.rs`; sequence gaps require snapshot recovery.
- Durable state is the Rust-native manifest/event log under `crates/pi-coding-agent/src/coding_session/session_log`; interactive and protocol state are projections.

## Key Abstractions

- Purpose: Represent workflow nodes and action-selected transitions with cancellation, maximum-step protection, and lifecycle events.
- Examples: `crates/pi-agent-core/src/flow.rs`, `crates/pi-coding-agent/src/coding_session/prompt_flow.rs`, `crates/pi-coding-agent/src/coding_session/agent_team_flow.rs`
- Pattern: Generic state-machine/graph runner; operation-specific context carries temporary state.
- Purpose: Define the complete product action vocabulary and attach admission/dispatch metadata.
- Examples: `crates/pi-coding-agent/src/coding_session/public_operation.rs`, `crates/pi-coding-agent/src/coding_session/operation.rs`
- Pattern: Stable public command mapped to an internal discriminated operation and typed outcome.
- Purpose: Act as the product runtime owner and stable coordination boundary for sessions, services, operations, clients, events, and capabilities.
- Examples: `crates/pi-coding-agent/src/coding_session/mod.rs`, re-exported from `crates/pi-coding-agent/src/lib.rs`
- Pattern: Facade/service container during the operation-runtime convergence; new callers use `run` rather than adding operation-specific public methods.
- Purpose: Carry temporary workflow state, diagnostics, transactions, capability handles, and outcomes between Flow nodes.
- Examples: `PromptTurnContext` in `crates/pi-coding-agent/src/coding_session/prompt.rs`, `AgentTeamContext` in `crates/pi-coding-agent/src/coding_session/agent_team_flow.rs`, `AgentTurnContext` in `crates/pi-agent-core/src/agent_turn_flow/context.rs`
- Pattern: Mutable context object scoped to one graph execution; durable facts leave through services/transactions.
- Purpose: Preserve typed durable facts and atomic operation boundaries independent of UI/protocol formats.
- Examples: `crates/pi-coding-agent/src/coding_session/session_log/event.rs`, `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`
- Pattern: Append-only event log plus replay/fold; manifest stores session-level index state.
- Purpose: Provide an adapter-facing, sequenced semantic stream with family classification, operation IDs, terminal status, durability, and compatibility events.
- Examples: `crates/pi-coding-agent/src/coding_session/event.rs`, `crates/pi-coding-agent/src/coding_session/event_service.rs`
- Pattern: Publish/subscribe event bus with bounded replay and snapshot recovery.
- Purpose: Select a provider by `Model.api`, inject scoped auth, and normalize all providers to `EventStream`.
- Examples: `crates/pi-ai/src/registry.rs`, `crates/pi-ai/src/providers/mod.rs`
- Pattern: Registry/strategy; prefer scoped `AiClient` or `ProviderRegistry` over deprecated global registration.
- Purpose: Separate terminal I/O and generic renderable/input-aware UI components from coding-agent semantics.
- Examples: `crates/pi-tui/src/component.rs`, `crates/pi-tui/src/terminal.rs`, `crates/pi-tui/src/tui.rs`
- Pattern: Trait-based generic presentation runtime with a virtual terminal test double.
- Purpose: Restrict extensions to declared tools, commands, hooks, keybinds, dialogs, UI actions, and registered flow extension points.
- Examples: `crates/pi-coding-agent/src/plugins/tool.rs`, `crates/pi-coding-agent/src/plugins/hook.rs`, `crates/pi-coding-agent/src/plugins/registry.rs`
- Pattern: Capability-scoped provider registry; Lua plugins are adapted into the same host contracts by `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs`.

## Entry Points

- Location: `crates/pi-coding-agent/src/main.rs`
- Triggers: Running the `pi-coding-agent` package binary.
- Responsibilities: Process-level stdin/TTY handling, RPC fast path, builtin tool setup, CLI facade invocation, and exit codes.
- Location: `crates/pi-coding-agent/src/lib.rs`
- Triggers: Embedders call `run_cli`, `run_cli_with_options`, or `run_cli_with_options_and_stdin`.
- Responsibilities: Parse/resolve/dispatch without taking direct ownership of process exit.
- Location: `crates/pi-coding-agent/src/lib.rs` (`api` module)
- Triggers: Rust callers embedding product sessions or typed operations.
- Responsibilities: Expose the intended stable product surface, including `CodingAgentSession`, operations, outcomes, events, snapshots, resources, and tools.
- Location: `crates/pi-coding-agent/src/interactive/app.rs`
- Triggers: No explicit print/JSON/RPC mode and stdin/stdout are TTYs.
- Responsibilities: Start/stop terminal mode, create the product UI, run input/event/render scheduling, and project product state.
- Location: `crates/pi-coding-agent/src/print_mode.rs`
- Triggers: `--print` or explicit print mode.
- Responsibilities: Create/open a persistent or transient coding session, run one prompt, and return final text.
- Location: `crates/pi-coding-agent/src/protocol/json_mode.rs`
- Triggers: Explicit JSON mode.
- Responsibilities: Run a prompt and serialize protocol events/outcome for one-shot machine consumption.
- Location: `crates/pi-coding-agent/src/protocol/rpc.rs`
- Triggers: Explicit RPC mode from the binary fast path.
- Responsibilities: Process JSONL commands, manage running operation/event subscriptions, report stream lag, and serve snapshots/responses over stdio.
- Location: `src/main.rs`
- Triggers: Running the root `pi-rust` package.
- Responsibilities: Prints `Hello, world!` only; do not treat it as the product runtime.

## Architectural Constraints

- **Threading:** Tokio's multi-thread runtime drives product binaries and async provider/tool workflows; `AgentTurnFlow` streams events while operating on `Arc<RwLock<AgentState>>` in `crates/pi-agent-core/src/agent_turn_flow/runtime.rs`. The interactive adapter remains a single coordinating event loop over terminal input, prompt tasks, product events, and render deadlines in `crates/pi-coding-agent/src/interactive/loop.rs`.
- **Global state:** Scoped registries/services are preferred. Deprecated compatibility globals remain in `crates/pi-ai/src/registry.rs`; environment-backed auth and terminal/color detection also read process state. Tests serialize environment/provider registry mutation through guards in `crates/pi-coding-agent/src/lib.rs`.
- **Circular imports:** No crate-level circular dependency is possible in the current Cargo DAG. Preserve `pi-coding-agent -> pi-agent-core -> pi-ai` and `pi-coding-agent -> pi-tui`; never add reverse dependencies from `pi-ai`, `pi-agent-core`, or `pi-tui` to product code.
- **Durability:** `SessionEvent` is the durable fact boundary in `crates/pi-coding-agent/src/coding_session/session_log/event.rs`; `ProductEvent`, raw `FlowEvent`, raw `AgentEvent`, and TUI state are not substitutes for session persistence.
- **Capabilities:** Filesystem, shell, tool, model, session, UI, and plugin access must come from `OperationCapabilitySnapshot` in `crates/pi-coding-agent/src/coding_session/capability_snapshot.rs`, not from unscoped paths/services passed into arbitrary nodes or plugins.
- **Event recovery:** Product event channels and retained replay are bounded in `crates/pi-coding-agent/src/coding_session/event_service.rs`; adapters must detect lag/gaps and recover from `UiSnapshot`/fresh state rather than assuming lossless broadcast delivery.
- **Extension boundary:** Plugins may register through the explicit contracts in `crates/pi-coding-agent/src/plugins`; generic Flow internals, session storage, adapter state, and raw services are not plugin-owned extension surfaces.
- **Root package:** `Cargo.toml` includes all crates in one workspace but the root package has no dependencies and `src/main.rs` is a scaffold. Product work belongs in an owned crate under `crates/`.

## Anti-Patterns

### Expanding Compatibility Surfaces

### Bypassing Services or Transactions

### Leaking Product Semantics Downward

### Implementing Product Work in the Root Scaffold

## Error Handling

- `FlowError` in `crates/pi-agent-core/src/flow.rs` covers graph construction, cancellation, missing transitions, step limits, and node failure; product `FlowService` maps it to `CodingSessionError::Flow`.
- `CodingSessionError` in `crates/pi-coding-agent/src/coding_session/error.rs` groups config, auth, input, resource, session, partial commit, provider, tool, flow, plugin, capability, busy, event-gap/lag, protocol, and cancellation failures.
- `CliError` in `crates/pi-coding-agent/src/error.rs` is the adapter/process boundary; `crates/pi-coding-agent/src/coding_session/error.rs` maps product errors to user-facing CLI categories.
- Provider failures are normalized by `ProviderError` under `crates/pi-ai/src/transport/error.rs` and by terminal `AssistantMessageEvent::Error` values consumed in `crates/pi-agent-core/src/agent_turn_flow/nodes.rs`.
- Persistent commit uncertainty is not collapsed into a generic error; `CodingSessionError::PartialCommit` and startup recovery markers in `crates/pi-coding-agent/src/coding_session/session_service.rs` preserve the ambiguous operation state.
- Adapters treat event-stream lag/gaps as recoverable protocol conditions requiring a fresh snapshot, as implemented in `crates/pi-coding-agent/src/protocol/rpc.rs` and `crates/pi-coding-agent/src/interactive/loop.rs`.

## Cross-Cutting Concerns

<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->

## Project Skills

No project skills found. Add skills to any of: `.claude/skills/`, `.agents/skills/`, `.cursor/skills/`, `.github/skills/`, or `.codex/skills/` with a `SKILL.md` index file.
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->

## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:

- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->

<!-- GSD:profile-start -->

## Developer Profile

> Profile not yet configured. Run `/gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
