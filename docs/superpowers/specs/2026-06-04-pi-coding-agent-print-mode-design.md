# Design: Rust `pi-coding-agent` minimal print-mode CLI

- Date: 2026-06-04
- Status: Draft (pending review)
- Scope: First Rust PoC for the `pi-coding-agent` crate.
- Depends on: `pi-ai`, `pi-agent-core`.

## 1. Context

The TypeScript `@earendil-works/pi-coding-agent` package is the user-facing CLI for pi. It
contains CLI parsing, print/json/rpc/interactive modes, session management, built-in coding
tools, extensions, skills, prompt templates, themes, settings, auth storage, and export flows.

The Rust workspace already has partial ports of lower layers:

- `pi-ai`: model types, provider registry, Anthropic provider skeleton, faux provider, stream helpers.
- `pi-agent-core`: agent state, tool calling loop, model stream integration.
- `pi-tui`: terminal rendering foundation.
- `pi-coding-agent`: currently a placeholder crate.

This design intentionally starts with a narrow vertical slice: a Rust print-mode command that can
send one prompt through `pi-agent-core` and print the final assistant text. It proves the CLI can
compose the lower Rust crates before porting sessions, tools, or interactive UI.

## 2. Goals and success criteria

Build a minimal `pi-coding-agent` Rust crate with a thin binary and testable library APIs.

The PoC is done when:

1. `cargo run -p pi-coding-agent -- -p "hello"` starts the print-mode path.
2. The crate exposes reusable library functions for argument parsing and print-mode execution.
3. Unit/integration tests can run without real provider keys by injecting a faux provider/model.
4. Text-mode output prints assistant text blocks and returns exit code `0` on `Stop` or `Length`.
5. Provider or agent errors return a non-zero exit code and a useful stderr message.
6. Focused tests cover:
   - `-p` and `--print` parsing;
   - positional prompt parsing;
   - `--model`, `--api-key`, `--system-prompt`, `--max-turns`, `--help`, and `--version`;
   - successful single-turn text output with a faux provider;
   - agent error exit behavior;
   - tool-call loop compatibility through an injected test tool/provider.
7. `cargo fmt --check`, `cargo test -p pi-coding-agent`, and `cargo test --workspace` pass.

## 3. Non-goals

This phase does not port:

- interactive TUI mode;
- RPC mode;
- JSON event streaming mode;
- persistent session files, resume, continue, fork, or branch navigation;
- built-in coding tools (`read`, `write`, `edit`, `bash`, `grep`, `find`, `ls`);
- extensions, skills, prompt templates, themes, context file discovery, or package manager flows;
- auth storage, OAuth, settings files, model registry config files, or `@file`/image inputs;
- stdin piping;
- HTML export or sharing.

Unknown flags should be rejected unless they are intentionally captured by a future extension
system. This PoC has no extension flag handling.

## 4. Key decisions

- **Use a library-first structure.** The binary should delegate to `pi_coding_agent::run_cli()`.
  This keeps behavior testable without spawning a process for every case.
- **Keep the binary thin.** `main.rs` should translate the async result into a process exit code.
- **Use existing lower crates.** The PoC should not duplicate agent loop or provider streaming
  logic already present in `pi-agent-core` and `pi-ai`.
- **Default to the static Rust model table.** `--model` resolves by `pi_ai::lookup_model(id)`.
  If omitted, use `claude-sonnet-4-5` if present.
- **Register built-in providers at CLI startup.** Runtime execution calls `pi_ai::providers::register_builtins()`.
- **Allow explicit API key but do not build auth storage yet.** `--api-key` sets
  `StreamOptions.api_key`; otherwise provider-specific env lookup remains in `pi-ai`.
- **Expose dependency injection for tests.** Tests should be able to supply a model and register a
  faux provider under a unique API key.
- **Return explicit exit codes.** Library functions should return a structured result rather than
  calling `std::process::exit()` directly.

## 5. Architecture

### 5.1 Crate layout

```text
crates/pi-coding-agent/
  Cargo.toml
  src/
    lib.rs          # public API, run_cli()
    args.rs         # CliArgs, parse_args(), help text
    runtime.rs      # model resolution and Agent construction
    print_mode.rs   # run_print_mode()
    error.rs        # typed error and display
    main.rs         # thin binary entrypoint
  tests/
    args.rs
    print_mode.rs
```

### 5.2 Public API shape

```rust
pub struct CliArgs {
    pub print: bool,
    pub prompt: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: u32,
    pub help: bool,
    pub version: bool,
}

pub struct CliOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<CliArgs, CliError>;
pub async fn run_cli(args: impl IntoIterator<Item = String>) -> CliOutput;
pub async fn run_print_mode(options: PrintModeOptions) -> Result<String, CliError>;
```

The exact Rust signatures may use borrowed string slices internally, but tests should be able to
call the parser and print-mode runner without invoking a subprocess.

### 5.3 CLI behavior

Supported arguments:

- `-p`, `--print`: required for this PoC.
- `--model <id>`: resolves against `pi_ai::lookup_model`.
- `--api-key <key>`: passed into `StreamOptions`.
- `--system-prompt <text>`: overrides the default system prompt.
- `--max-turns <n>`: defaults to `5`; rejects `0`.
- `--help`, `-h`: prints help and exits `0`.
- `--version`, `-v`: prints crate version and exits `0`.
- positional prompt text: parser produces one prompt string for this phase.

Behavioral rules:

- Missing prompt in print mode returns exit code `1`.
- Unknown flags return exit code `1`.
- Multiple positional shell words are joined with spaces into that one prompt string; quoted
  prompts already arrive as one argument from the shell.
- `--model` unknown returns exit code `1`.
- The default system prompt is short and explicit: `You are a helpful coding assistant.`

### 5.4 Runtime flow

1. Parse CLI arguments.
2. Handle `--help` and `--version`.
3. Require print mode and a prompt.
4. Register built-in `pi-ai` providers.
5. Resolve model:
   - use `--model` when provided;
   - otherwise `claude-sonnet-4-5`.
6. Build `AgentConfig`:
   - selected model;
   - system prompt;
   - max turns;
   - stream options with optional API key.
7. Create `pi_agent_core::Agent`.
8. Call `agent.prompt(prompt)` and collect events.
9. On `AgentDone`, print all final assistant text blocks, each separated by newlines.
10. On `AgentError`, return non-zero with the error message.

### 5.5 Test seams

The print-mode runner should accept an options struct that can override:

- model;
- provider registration key;
- optional tools to add to the agent before prompting.

This allows deterministic tests using `pi_ai::providers::faux::FauxProvider` or a small test
provider without real network access.

## 6. Error handling

Use a typed `CliError` for parser/runtime failures:

- invalid or missing argument value;
- unknown flag;
- unsupported mode;
- missing prompt;
- unknown model;
- invalid max turns;
- agent/provider failure.

`run_cli()` converts errors into `CliOutput { exit_code: 1, stdout: "", stderr }`. Lower-level
functions return `Result` so tests can assert exact error variants.

## 7. Testing strategy

Tests should stay offline and deterministic.

- Parser tests mirror the TypeScript `args.test.ts` subset relevant to this phase.
- Print-mode tests use unique provider API keys to avoid the global registry race already seen in
  `pi-ai` faux tests.
- A single-turn text test registers a faux provider that returns text and asserts stdout.
- An error test registers a provider response with `StopReason::Error` or uses an unknown model
  and asserts exit code `1`.
- A tool-loop compatibility test registers a faux provider that requests an injected `echo` tool,
  then returns a final assistant response after the tool result.

## 8. Future phases

After this PoC, likely next phases are:

1. Built-in tool library: `read`, `write`, `edit`, `bash`, then read-only search tools.
2. Session manager: JSONL session persistence, continue/resume, session IDs.
3. JSON/RPC print modes.
4. Resource loading: context files, prompt templates, skills.
5. Interactive TUI mode on top of the Rust `pi-tui` crate.
