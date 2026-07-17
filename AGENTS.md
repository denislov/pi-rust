## 语言选择

使用中文跟用户沟通，专业、常用的术语可以使用英文。技术文档可完全使用英文。

<!-- CODEGRAPH_START -->
## CodeGraph

In repositories indexed by CodeGraph (a `.codegraph/` directory exists at the repo root), reach for it BEFORE grep/find or reading files when you need to understand or locate code:

- **MCP tool** (when available): `codegraph_explore` answers most code questions in one call - the relevant symbols' verbatim source plus the call paths between them, including dynamic-dispatch hops grep cannot follow. Name a file or symbol in the query to read its current line-numbered source. If it is listed but deferred, load it by name via tool search.
- **Shell** (always works): `codegraph explore "<symbol names or question>"` prints the same output.

If there is no `.codegraph/` directory, skip CodeGraph entirely - indexing is the user's decision.
<!-- CODEGRAPH_END -->

## Project Overview

`pi-rust` is a Rust 2024 workspace for an AI coding-agent runtime. The active implementation is split into reusable provider, agent-runtime, terminal UI, and product layers. The target runtime contract and layer boundaries are documented in `docs/architecture.md`; statements marked as contracts or invariants there are normative.

The root `pi-rust` binary currently only prints `Hello, world!`. The user-facing executable is `pi-coding-agent`.

## Workspace Map

- `crates/pi-ai`: model metadata, provider registry, authentication inputs, HTTP transports, request/response mapping, and streaming. It must remain independent of agent sessions, product events, CLI, RPC, and TUI concerns.
- `crates/pi-agent-core`: provider-neutral agent loop, Flow primitives, tools, hooks, resources, compaction, transcripts, and execution environment. It may depend on `pi-ai`, but not on coding-agent product policy or adapters.
- `crates/pi-tui`: generic terminal lifecycle, input normalization, rendering, editor, menu, dialog, markdown, and image components. Keep product semantics out of this crate.
- `crates/pi-coding-agent`: product runtime and `pi-coding-agent` binary. It owns CLI/configuration, `CodingAgentSession`, product Flow orchestration, session persistence, plugins, tools, profiles/teams, print/JSON/RPC/interactive adapters, and product events.
- `crates/pi-mom`, `crates/pi-pods`, and `crates/pi-web-ui`: current placeholder crates. Do not assume behavior that is not present in their source.
- `docs/architecture.md`: reference architecture for convergence toward an operation runtime. Read it before changing runtime ownership, event contracts, persistence, Flow/service boundaries, or adapter behavior.
- `scripts/tui-smoke.sh`: tmux-based interactive smoke suite; captures output under `target/tui-smoke/`.

The architectural layers run from `pi-ai` through `pi-agent-core` to `pi-coding-agent`. Cargo dependencies point from the product toward the lower layers: `pi-coding-agent -> pi-agent-core -> pi-ai`, and `pi-coding-agent -> pi-tui`. Do not introduce reverse dependencies or product types into lower-level crates.

## Development Workflow

- Run commands from the workspace root unless a task explicitly requires another directory.
- Use stable Rust with Rust 2024 edition support (Rust 1.85 or newer). There is no checked-in toolchain override.
- Prefer the stable embedding facade under `pi_coding_agent::api`. Root-level exports in `pi-coding-agent` are compatibility exports and many are deprecated.
- Keep changes scoped to the crate that owns the behavior. When changing a public type or cross-crate contract, inspect downstream call paths with CodeGraph and update boundary/public-API tests.
- Follow existing module organization and error types. Do not add a new abstraction when an existing Flow, service, adapter, provider, or component boundary already owns the concern.
- Keep Flow nodes focused on orchestration and operation-local state; durable side effects belong behind services. Rust-native `SessionEvent` records remain the durable source of session facts, while product adapters consume product events rather than raw Flow or agent events.
- Preserve async cancellation, streaming, and operation association semantics when modifying runtime paths. Avoid blocking I/O in async code.
- Never commit credentials or hard-code provider tokens. Provider tests use fake credentials and local/mocked transports; ordinary test runs should not require live provider access.

## Build And Validation

Use the smallest relevant check while iterating, then broaden validation according to the change's blast radius:

```bash
# Fast compile check for one crate
cargo check -p pi-ai
cargo check -p pi-agent-core
cargo check -p pi-tui
cargo check -p pi-coding-agent

# Run one integration test target or one named test
cargo test -p pi-coding-agent --test cli
cargo test -p pi-agent-core agent_turn

# Full workspace validation
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Run `cargo fmt --all` after editing Rust. New behavior requires focused tests in the owning crate; fixes should include a regression test when practical. Keep deterministic and boundary-guard tests intact: they enforce layering, public APIs, provider/tool boundaries, and stable event/protocol contracts.

For interactive terminal changes, also run:

```bash
scripts/tui-smoke.sh
```

The smoke suite requires `tmux`, builds `pi-coding-agent`, and writes captures to `target/tui-smoke/`. Set `PI_RUST_TUI_SMOKE_REAL_PROMPT` only when an explicitly authorized live-provider check is needed; the default suite must remain offline.

## Rust Conventions

- Match the repository's standard Rust style and let `rustfmt` decide formatting.
- Prefer explicit domain types and structured serialization over ad hoc strings, especially for product events, protocol messages, session logs, and provider payloads.
- Return typed errors with useful context; avoid `unwrap`/`expect` in production paths unless an invariant is both local and undeniable.
- Keep public APIs intentionally small. Add exports through the appropriate crate facade and update `public_api` tests when the supported surface changes.
- Unit tests belong beside focused implementation details; cross-module behavior and public contracts belong in the crate's `tests/` directory.
- Tests that mutate process environment must use the repository's environment guards because Rust 2024 marks environment mutation unsafe and tests may execute concurrently.
- Do not weaken crate-level Clippy allowances or boundary guards merely to make a change pass. Address the owning design issue or explain why a narrowly scoped exception is required.

## Runtime And Configuration Notes

- `pi-coding-agent` supports interactive, print/JSON, and RPC stdio paths. Preserve stdout/stderr and JSONL protocol cleanliness; diagnostics must not corrupt machine-readable output.
- User/project configuration, sessions, themes, skills, templates, plugins, and profiles are resolved by `pi-coding-agent`. Tests should isolate them with temporary directories and `PI_RUST_DIR` rather than touching a developer's real configuration.
- Provider credentials are resolved from environment/configuration by `pi-ai` and `pi-coding-agent`. Never print secret values in diagnostics, snapshots, fixtures, or test failures.
- TUI code must preserve terminal cleanup, cursor placement, Unicode display width, resize behavior, and unrelated scrollback. Cover rendering logic with virtual-terminal tests where possible, then use the smoke script for lifecycle behavior.

## 工作原则
1. 要有大局观，不要为了兼容性测试写冗余代码，可以做执行债务记录，推迟收敛，但在计划完整收敛时，所有债务记录需要处理掉。
2. 每一个任务必须进入版本计划进行收敛，可以多个任务组合在一起推进一个大版本迭代，也可以一个小任务推进一个小版本迭代。
3. 每推进一个版本计划，必须同步更新项目的版本信息、以及CHANGELOG的更新。