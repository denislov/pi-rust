# Technology Stack

**Analysis Date:** 2026-07-10

## Languages

**Primary:**
- Rust, edition 2024 - All production libraries and binaries live in `src/main.rs` and `crates/*/src/**/*.rs`; the edition is declared in `Cargo.toml` and every crate manifest under `crates/*/Cargo.toml`.

**Secondary:**
- JavaScript (CommonJS), version unpinned - Optional Node.js model-catalog conversion utility in `crates/pi-ai/tools/generate_models.cjs`; it is not part of the Cargo build.
- Bash, version unpinned - Tmux-driven interactive smoke automation in `scripts/tui-smoke.sh`.
- TOML and JSON - Workspace/package configuration in `Cargo.toml` and `crates/*/Cargo.toml`, runtime settings/auth schemas in `crates/pi-coding-agent/src/config/`, plugin manifests consumed by `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs`, and the generated model catalog in `crates/pi-ai/src/models_generated.json`.

## Runtime

**Environment:**
- Native Rust executable with Tokio 1.52.3 asynchronous runtime - `crates/pi-coding-agent/src/main.rs` uses `#[tokio::main]`, while provider streaming and agent control use Tokio tasks, channels, timers, process I/O, and cancellation throughout `crates/pi-ai/src/`, `crates/pi-agent-core/src/`, and `crates/pi-coding-agent/src/`.
- Rust stable toolchain, not repository-pinned - Edition 2024 in `Cargo.toml` requires Rust 1.85 or newer; no `rust-toolchain.toml` accompanies the workspace manifests.
- The operational application is the `pi-coding-agent` binary in `crates/pi-coding-agent/src/main.rs`; the workspace-root binary in `src/main.rs` is only a `Hello, world!` placeholder.

**Package Manager:**
- Cargo, resolver supplied by the installed Rust toolchain - Workspace membership and package metadata are defined in `Cargo.toml` and `crates/*/Cargo.toml`.
- Lockfile: present as `Cargo.lock` format version 4, with exact third-party versions committed.

## Frameworks

**Core:**
- Tokio 1.52.3 - Async runtime for the CLI entry point, provider streams, cancellation, filesystem/process work, broadcasts, and stdio RPC in `crates/pi-coding-agent/src/main.rs`, `crates/pi-ai/src/`, and `crates/pi-coding-agent/src/protocol/rpc.rs`.
- Reqwest 0.12.28 with Rustls TLS - JSON HTTP requests and streamed response bodies for provider clients in `crates/pi-ai/Cargo.toml`, `crates/pi-ai/src/providers/`, and `crates/pi-ai/src/transport/http.rs`.
- Crossterm 0.28.1 plus the in-house `pi-tui` component/runtime layer - Terminal control, input, rendering, overlays, Markdown, and inline images in `crates/pi-tui/Cargo.toml` and `crates/pi-tui/src/`.
- Serde 1.0.228, serde_json 1.0.150, serde_yaml 0.9.34, and toml 0.8.23 - Wire models, settings, auth entries, plugin manifests, resources, and durable session records in `crates/pi-ai/src/types/`, `crates/pi-agent-core/src/resources.rs`, `crates/pi-coding-agent/src/config/`, and `crates/pi-coding-agent/src/coding_session/session_log/`.
- No web application framework is active - `crates/pi-web-ui/src/lib.rs` is a placeholder and `crates/pi-web-ui/Cargo.toml` has no dependencies.

**Testing:**
- Rust built-in test harness plus Tokio `#[tokio::test]` - Unit tests are colocated under `#[cfg(test)]` modules and integration suites live in `crates/pi-ai/tests/`, `crates/pi-agent-core/tests/`, `crates/pi-coding-agent/tests/`, and `crates/pi-tui/tests/`.
- Tempfile 3.27.0 and custom deterministic guards - Filesystem fixtures and environment/provider isolation are used from crate dev-dependencies and helpers such as `crates/pi-coding-agent/tests/support/mod.rs` and `crates/pi-agent-core/tests/common/mod.rs`.

**Build/Dev:**
- Cargo - Build, test, example, and package selection are driven by the workspace and crate manifests in `Cargo.toml` and `crates/*/Cargo.toml`.
- Rustfmt and Clippy from the Rust toolchain - Source contains targeted Clippy allowances in `crates/pi-coding-agent/src/lib.rs`; no repository-specific `rustfmt.toml` or `clippy.toml` is present next to `Cargo.toml`.
- Node.js - Only required to regenerate `crates/pi-ai/src/models_generated.json` with `crates/pi-ai/tools/generate_models.cjs`; the repository does not pin a Node version or include a JavaScript package manifest.
- Bash and tmux - Required only for the TUI capture suite in `scripts/tui-smoke.sh`, which builds `pi-coding-agent` and writes captures below `target/tui-smoke/`.

## Key Dependencies

**Critical:**
- `pi-ai` 0.1.0 - Provider registry, 921-model catalog, request conversion, authentication injection, HTTP/SSE transport, Bedrock event-stream handling, and response normalization in `crates/pi-ai/`.
- `pi-agent-core` 0.1.0 - Agent loop, turn flows, tools, compaction, queues, resource loading, and provider-facing context in `crates/pi-agent-core/`.
- `pi-coding-agent` 0.1.0 - Product CLI, configuration, session/runtime orchestration, stdio protocols, built-in coding tools, profiles, delegation, plugins, and interactive TUI integration in `crates/pi-coding-agent/`.
- `pi-tui` 0.1.0 - Terminal abstraction and reusable TUI components in `crates/pi-tui/`.
- Futures 0.3.32, async-stream 0.3.6, and tokio-util 0.7.18 - Stream construction/composition and cancellation across `crates/pi-ai/Cargo.toml` and `crates/pi-agent-core/Cargo.toml`.
- Thiserror 2.0.18 - Typed error surfaces across provider, core-agent, CLI, and TUI crates declared in `crates/pi-ai/Cargo.toml`, `crates/pi-agent-core/Cargo.toml`, `crates/pi-coding-agent/Cargo.toml`, and `crates/pi-tui/Cargo.toml`.

**Infrastructure:**
- mlua 0.10.5 with vendored Lua 5.4 - Sandboxed local plugin execution and host capability registration in `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs` and `crates/pi-coding-agent/src/plugins/`.
- ring 0.17.14 and base64 0.22.1 - PKCE hashing, token/JWT decoding, request signing support, and encoded image content in `crates/pi-ai/src/util/oauth.rs`, `crates/pi-ai/src/providers/bedrock/`, `crates/pi-ai/src/providers/openai_codex_responses/mod.rs`, and `crates/pi-coding-agent/src/input.rs`.
- image 0.25.10 - PNG/JPEG/GIF/WebP decoding and resizing before multimodal requests in `crates/pi-coding-agent/src/input.rs`.
- notify 7.0.0 - Debounced custom-theme filesystem watching in `crates/pi-coding-agent/src/theme/reload.rs`.
- ignore 0.4.25, globset 0.4.18, and regex 1.12.3 - Gitignore-aware traversal, filtering, resource discovery, and coding tools in `crates/pi-agent-core/src/resources.rs` and `crates/pi-coding-agent/src/tools/`.
- pulldown-cmark 0.12.2, syntect 5.3.0, unicode-segmentation 1.13.3, and unicode-width 0.2.2 - Markdown parsing, syntax highlighting, and terminal-safe text layout in `crates/pi-tui/src/components/markdown.rs`, `crates/pi-coding-agent/src/interactive/`, and `crates/pi-tui/src/`.
- dirs 6.0.0, time 0.3.47, and uuid 1.23.2 - User-directory resolution, timestamps, and UUID v7 identifiers in `crates/pi-coding-agent/src/config/paths.rs`, `crates/pi-coding-agent/src/session.rs`, and `crates/pi-agent-core/src/`.

## Configuration

**Environment:**
- Global runtime state defaults to `~/.pi-rust/` and can be relocated with `PI_RUST_DIR`; project overrides live below `<cwd>/.pi-rust/` as implemented in `crates/pi-coding-agent/src/config/paths.rs`.
- Settings merge global `settings.toml` with project `.pi-rust/settings.toml`, with project values winning, in `crates/pi-coding-agent/src/config/settings.rs`.
- Provider credentials resolve from CLI arguments, provider-specific environment variables, then global `auth.toml`; `$VAR` and `${VAR}` references inside auth entries are expanded by `crates/pi-coding-agent/src/config/auth.rs` and provider mappings live in `crates/pi-ai/src/util/env_keys.rs`.
- Session storage defaults to `${PI_RUST_DIR:-~/.pi-rust}/sessions` and can be overridden by CLI/runtime configuration or `PI_SESSION_DIR` in `crates/pi-coding-agent/src/session.rs`.
- Skills, prompt templates, themes, profiles, and Lua plugins are loaded from user and project roots by `crates/pi-coding-agent/src/resources.rs`, `crates/pi-coding-agent/src/coding_session/profiles.rs`, and `crates/pi-coding-agent/src/coding_session/mod.rs`.

**Build:**
- Workspace topology is declared in `Cargo.toml`; dependency features and crate-local dev dependencies are declared in `crates/*/Cargo.toml`.
- Exact dependency resolution is committed in `Cargo.lock`; there are no workspace `build.rs` scripts or non-Cargo build-system manifests alongside `Cargo.toml`.
- The checked-in provider/model data file is `crates/pi-ai/src/models_generated.json`; regenerate it explicitly with `crates/pi-ai/tools/generate_models.cjs` rather than as an implicit Cargo build step.

## Platform Requirements

**Development:**
- Use a Rust stable toolchain supporting edition 2024 and Cargo lockfile version 4 to build the manifests in `Cargo.toml` and `crates/*/Cargo.toml`.
- Run the main application with Cargo package selection for `pi-coding-agent`, whose entry point is `crates/pi-coding-agent/src/main.rs`; building the root package alone only compiles `src/main.rs`.
- Node.js is optional for model-catalog regeneration through `crates/pi-ai/tools/generate_models.cjs`; Bash plus tmux is optional for `scripts/tui-smoke.sh`.
- Provider integration tests should remain deterministic/offline unless explicitly opted into a real-provider path; fake providers and boundary suites are available in `crates/pi-ai/src/providers/faux.rs`, `crates/pi-ai/tests/`, and `crates/pi-coding-agent/tests/`.

**Production:**
- Deployment target is a local native CLI/TUI executable from `crates/pi-coding-agent/src/main.rs`, not a hosted server; automation can communicate through JSONL over stdin/stdout in `crates/pi-coding-agent/src/protocol/rpc.rs`.
- Interactive use requires an ANSI-capable terminal supported by `crates/pi-tui/src/terminal.rs`; inline images use Kitty or iTerm2 protocols when detected by `crates/pi-tui/src/terminal_image.rs`.
- Model use requires outbound HTTPS access to the selected provider endpoint from `crates/pi-ai/src/models_generated.json`; streaming is handled by `crates/pi-ai/src/providers/` and `crates/pi-ai/src/transport/http.rs`.
- Coding tools require local filesystem and process permissions in the selected working directory through `crates/pi-coding-agent/src/tools/`; optional clipboard commands are selected per OS in `crates/pi-coding-agent/src/interactive/clipboard.rs`.

---

*Stack analysis: 2026-07-10*
