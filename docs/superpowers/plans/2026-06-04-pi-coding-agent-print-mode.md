# Rust `pi-coding-agent` Print-Mode CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first Rust `pi-coding-agent` vertical slice: a minimal print-mode CLI that sends one prompt through `pi-agent-core` and prints the final assistant text.

**Architecture:** The crate is library-first with a thin binary. `args.rs` owns CLI parsing, `runtime.rs` owns model/config construction, `print_mode.rs` owns agent execution, and `lib.rs` composes them into `run_cli()`/`run_cli_with_options()` for tests and the binary.

**Tech Stack:** Rust 2024, `tokio`, `futures`, `thiserror`, `serde_json`, existing workspace crates `pi-ai` and `pi-agent-core`.

---

## File Structure

- Modify `crates/pi-coding-agent/Cargo.toml`: add dependencies and binary-compatible runtime dependencies.
- Replace `crates/pi-coding-agent/src/lib.rs`: public API and CLI orchestration.
- Create `crates/pi-coding-agent/src/error.rs`: typed CLI/runtime errors.
- Create `crates/pi-coding-agent/src/args.rs`: `CliArgs`, `parse_args()`, `help_text()`.
- Create `crates/pi-coding-agent/src/runtime.rs`: default model/system prompt, model resolution, `AgentConfig` construction.
- Create `crates/pi-coding-agent/src/print_mode.rs`: `PrintModeOptions`, `run_print_mode()`.
- Create `crates/pi-coding-agent/src/main.rs`: thin async binary entrypoint.
- Create `crates/pi-coding-agent/tests/public_api.rs`: import and API shape smoke test.
- Create `crates/pi-coding-agent/tests/args.rs`: parser behavior tests.
- Create `crates/pi-coding-agent/tests/runtime.rs`: model/config tests.
- Create `crates/pi-coding-agent/tests/print_mode.rs`: offline faux provider execution tests.
- Create `crates/pi-coding-agent/tests/cli.rs`: `run_cli_with_options()` behavior tests.

Do not commit during execution unless the user explicitly asks for a commit.

## Task 1: Configure Crate Dependencies And Public Skeleton

**Files:**
- Modify: `crates/pi-coding-agent/Cargo.toml`
- Replace: `crates/pi-coding-agent/src/lib.rs`
- Create: `crates/pi-coding-agent/src/error.rs`
- Create: `crates/pi-coding-agent/src/args.rs`
- Create: `crates/pi-coding-agent/src/runtime.rs`
- Create: `crates/pi-coding-agent/src/print_mode.rs`
- Create: `crates/pi-coding-agent/tests/public_api.rs`

- [ ] **Step 1: Write the failing public API smoke test**

Create `crates/pi-coding-agent/tests/public_api.rs`:

```rust
use pi_ai::types::Model;
use pi_coding_agent::{
    help_text, parse_args, CliArgs, CliError, CliOutput, CliRunOptions, PrintModeOptions,
};

fn model(api: &str) -> Model {
    Model {
        id: "test-model".into(),
        name: "Test Model".into(),
        api: api.into(),
        provider: "test".into(),
        base_url: String::new(),
        reasoning: false,
        input: 0.0,
        output: 0.0,
        cache_read: None,
        cache_write: None,
        context_window: 0,
        max_tokens: None,
        headers: None,
    }
}

#[test]
fn public_api_symbols_are_importable() {
    let args = CliArgs::default();
    assert_eq!(args.max_turns, 5);

    let parsed = parse_args(vec!["-p".to_string(), "hello".to_string()]).unwrap();
    assert!(parsed.print);
    assert_eq!(parsed.prompt.as_deref(), Some("hello"));

    let print_options = PrintModeOptions::new("hello", model("public-api-test"));
    assert_eq!(print_options.prompt, "hello");
    assert!(!print_options.register_builtins);

    let output = CliOutput {
        exit_code: 0,
        stdout: "ok\n".into(),
        stderr: String::new(),
    };
    assert_eq!(output.exit_code, 0);

    let runtime_options = CliRunOptions::default();
    assert!(runtime_options.register_builtins);

    let err = CliError::MissingPrompt;
    assert_eq!(err.to_string(), "missing prompt");

    assert!(help_text().contains("Usage:"));
}
```

- [ ] **Step 2: Run the public API smoke test and verify it fails**

Run:

```bash
cargo test -p pi-coding-agent --test public_api
```

Expected: FAIL to compile with unresolved imports such as `CliArgs`, `CliError`, and `PrintModeOptions`.

- [ ] **Step 3: Add crate dependencies**

Replace `crates/pi-coding-agent/Cargo.toml` with:

```toml
[package]
name = "pi-coding-agent"
version = "0.1.0"
edition = "2024"

[dependencies]
futures = "0.3"
pi-agent-core = { path = "../pi-agent-core" }
pi-ai = { path = "../pi-ai" }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

- [ ] **Step 4: Add typed errors**

Create `crates/pi-coding-agent/src/error.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CliError {
    #[error("missing value for {0}")]
    MissingValue(String),
    #[error("unknown flag: {0}")]
    UnknownFlag(String),
    #[error("unsupported mode: {0}")]
    UnsupportedMode(String),
    #[error("missing prompt")]
    MissingPrompt,
    #[error("unknown model: {0}")]
    UnknownModel(String),
    #[error("invalid max turns: {0}")]
    InvalidMaxTurns(String),
    #[error("agent failure: {0}")]
    AgentFailure(String),
}
```

- [ ] **Step 5: Add minimal argument API**

Create `crates/pi-coding-agent/src/args.rs`:

```rust
use crate::CliError;

pub const DEFAULT_MAX_TURNS: u32 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
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

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            print: false,
            prompt: None,
            model: None,
            api_key: None,
            system_prompt: None,
            max_turns: DEFAULT_MAX_TURNS,
            help: false,
            version: false,
        }
    }
}

pub fn help_text() -> String {
    format!(
        "pi-coding-agent {}\n\nUsage:\n  pi-coding-agent -p <prompt>\n\nOptions:\n  -p, --print              Run one prompt and print the assistant response\n  --model <id>             Model id from the built-in Rust model table\n  --api-key <key>          API key passed to the selected provider\n  --system-prompt <text>   System prompt override\n  --max-turns <n>          Maximum agent loop turns (default: 5)\n  -h, --help               Show help\n  -v, --version            Show version\n",
        env!("CARGO_PKG_VERSION")
    )
}

pub fn parse_args<I>(args: I) -> Result<CliArgs, CliError>
where
    I: IntoIterator<Item = String>,
{
    let mut parsed = CliArgs::default();
    let mut prompt_parts = Vec::new();
    let raw: Vec<String> = args.into_iter().collect();
    let mut i = 0;

    while i < raw.len() {
        let arg = &raw[i];
        match arg.as_str() {
            "-p" | "--print" => {
                parsed.print = true;
                if let Some(next) = raw.get(i + 1) {
                    if !next.starts_with('-') || next.starts_with("---") {
                        prompt_parts.push(next.clone());
                        i += 1;
                    }
                }
            }
            "-h" | "--help" => parsed.help = true,
            "-v" | "--version" => parsed.version = true,
            value if value.starts_with("--") => {
                return Err(CliError::UnknownFlag(value.to_string()));
            }
            value if value.starts_with('-') => {
                return Err(CliError::UnknownFlag(value.to_string()));
            }
            value => prompt_parts.push(value.to_string()),
        }
        i += 1;
    }

    if !prompt_parts.is_empty() {
        parsed.prompt = Some(prompt_parts.join(" "));
    }

    Ok(parsed)
}
```

- [ ] **Step 6: Add minimal runtime API**

Create `crates/pi-coding-agent/src/runtime.rs`:

```rust
use pi_agent_core::{AgentConfig, AgentTool};
use pi_ai::types::{Model, StreamOptions};

pub const DEFAULT_MODEL_ID: &str = "claude-sonnet-4-5";
pub const DEFAULT_SYSTEM_PROMPT: &str = "You are a helpful coding assistant.";

#[derive(Clone)]
pub struct CliRunOptions {
    pub model_override: Option<Model>,
    pub tools: Vec<AgentTool>,
    pub register_builtins: bool,
}

impl Default for CliRunOptions {
    fn default() -> Self {
        Self {
            model_override: None,
            tools: Vec::new(),
            register_builtins: true,
        }
    }
}

pub fn build_agent_config(
    model: Model,
    system_prompt: Option<String>,
    max_turns: u32,
    api_key: Option<String>,
) -> AgentConfig {
    let stream_options = api_key.map(|api_key| StreamOptions {
        api_key: Some(api_key),
        ..Default::default()
    });
    AgentConfig {
        model,
        system_prompt: Some(system_prompt.unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_string())),
        max_turns,
        stream_options,
    }
}
```

- [ ] **Step 7: Add minimal print-mode API**

Create `crates/pi-coding-agent/src/print_mode.rs`:

```rust
use crate::CliError;
use pi_agent_core::AgentTool;
use pi_ai::types::Model;

pub struct PrintModeOptions {
    pub prompt: String,
    pub model: Model,
    pub api_key: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: u32,
    pub tools: Vec<AgentTool>,
    pub register_builtins: bool,
}

impl PrintModeOptions {
    pub fn new(prompt: impl Into<String>, model: Model) -> Self {
        Self {
            prompt: prompt.into(),
            model,
            api_key: None,
            system_prompt: None,
            max_turns: 5,
            tools: Vec::new(),
            register_builtins: false,
        }
    }
}

pub async fn run_print_mode(_options: PrintModeOptions) -> Result<String, CliError> {
    Ok(String::new())
}
```

- [ ] **Step 8: Replace public library exports**

Replace `crates/pi-coding-agent/src/lib.rs`:

```rust
pub mod args;
pub mod error;
pub mod print_mode;
pub mod runtime;

pub use args::{help_text, parse_args, CliArgs, DEFAULT_MAX_TURNS};
pub use error::CliError;
pub use print_mode::{run_print_mode, PrintModeOptions};
pub use runtime::{build_agent_config, CliRunOptions, DEFAULT_MODEL_ID, DEFAULT_SYSTEM_PROMPT};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub async fn run_cli(args: impl IntoIterator<Item = String>) -> CliOutput {
    run_cli_with_options(args, CliRunOptions::default()).await
}

pub async fn run_cli_with_options(
    args: impl IntoIterator<Item = String>,
    _options: CliRunOptions,
) -> CliOutput {
    match parse_args(args) {
        Ok(parsed) if parsed.help => CliOutput {
            exit_code: 0,
            stdout: help_text(),
            stderr: String::new(),
        },
        Ok(_) => CliOutput {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        },
        Err(error) => CliOutput {
            exit_code: 1,
            stdout: String::new(),
            stderr: format!("{error}\n"),
        },
    }
}
```

- [ ] **Step 9: Run the public API smoke test and verify it passes**

Run:

```bash
cargo test -p pi-coding-agent --test public_api
```

Expected: PASS.

## Task 2: Implement CLI Argument Parsing

**Files:**
- Create: `crates/pi-coding-agent/tests/args.rs`
- Replace: `crates/pi-coding-agent/src/args.rs`

- [ ] **Step 1: Write parser tests**

Create `crates/pi-coding-agent/tests/args.rs`:

```rust
use pi_coding_agent::{parse_args, CliError};

fn parse(values: &[&str]) -> Result<pi_coding_agent::CliArgs, CliError> {
    parse_args(values.iter().map(|value| value.to_string()))
}

#[test]
fn parses_short_print_with_prompt() {
    let args = parse(&["-p", "hello"]).unwrap();

    assert!(args.print);
    assert_eq!(args.prompt.as_deref(), Some("hello"));
}

#[test]
fn parses_long_print_with_prompt() {
    let args = parse(&["--print", "hello"]).unwrap();

    assert!(args.print);
    assert_eq!(args.prompt.as_deref(), Some("hello"));
}

#[test]
fn parses_prompt_after_flags() {
    let args = parse(&[
        "--model",
        "claude-haiku-4-5",
        "--api-key",
        "sk-test",
        "--system-prompt",
        "Be terse.",
        "--max-turns",
        "7",
        "-p",
        "say hi",
    ])
    .unwrap();

    assert_eq!(args.model.as_deref(), Some("claude-haiku-4-5"));
    assert_eq!(args.api_key.as_deref(), Some("sk-test"));
    assert_eq!(args.system_prompt.as_deref(), Some("Be terse."));
    assert_eq!(args.max_turns, 7);
    assert_eq!(args.prompt.as_deref(), Some("say hi"));
}

#[test]
fn joins_multiple_positional_words_into_one_prompt() {
    let args = parse(&["-p", "say", "hello", "now"]).unwrap();

    assert_eq!(args.prompt.as_deref(), Some("say hello now"));
}

#[test]
fn print_does_not_consume_following_option_as_prompt() {
    let args = parse(&["-p", "--model", "claude-haiku-4-5", "hello"]).unwrap();

    assert!(args.print);
    assert_eq!(args.model.as_deref(), Some("claude-haiku-4-5"));
    assert_eq!(args.prompt.as_deref(), Some("hello"));
}

#[test]
fn print_consumes_yaml_frontmatter_prompt() {
    let prompt = "---\ntitle: hello\n---\nSay hi.";
    let args = parse(&["-p", prompt]).unwrap();

    assert_eq!(args.prompt.as_deref(), Some(prompt));
}

#[test]
fn parses_help_and_version() {
    let help = parse(&["--help"]).unwrap();
    let version = parse(&["-v"]).unwrap();

    assert!(help.help);
    assert!(version.version);
}

#[test]
fn rejects_missing_flag_values() {
    assert_eq!(
        parse(&["--model"]).unwrap_err(),
        CliError::MissingValue("--model".into())
    );
    assert_eq!(
        parse(&["--api-key"]).unwrap_err(),
        CliError::MissingValue("--api-key".into())
    );
    assert_eq!(
        parse(&["--system-prompt"]).unwrap_err(),
        CliError::MissingValue("--system-prompt".into())
    );
    assert_eq!(
        parse(&["--max-turns"]).unwrap_err(),
        CliError::MissingValue("--max-turns".into())
    );
}

#[test]
fn rejects_invalid_max_turns() {
    assert_eq!(
        parse(&["--max-turns", "0"]).unwrap_err(),
        CliError::InvalidMaxTurns("0".into())
    );
    assert_eq!(
        parse(&["--max-turns", "abc"]).unwrap_err(),
        CliError::InvalidMaxTurns("abc".into())
    );
}

#[test]
fn rejects_unknown_flags() {
    assert_eq!(
        parse(&["--json"]).unwrap_err(),
        CliError::UnknownFlag("--json".into())
    );
    assert_eq!(
        parse(&["-x"]).unwrap_err(),
        CliError::UnknownFlag("-x".into())
    );
}
```

- [ ] **Step 2: Run parser tests and verify they fail**

Run:

```bash
cargo test -p pi-coding-agent --test args
```

Expected: FAIL. The missing-value, long option, and invalid max-turn tests fail because the skeleton parser does not implement those behaviors.

- [ ] **Step 3: Replace parser implementation**

Replace `crates/pi-coding-agent/src/args.rs`:

```rust
use crate::CliError;

pub const DEFAULT_MAX_TURNS: u32 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
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

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            print: false,
            prompt: None,
            model: None,
            api_key: None,
            system_prompt: None,
            max_turns: DEFAULT_MAX_TURNS,
            help: false,
            version: false,
        }
    }
}

pub fn help_text() -> String {
    format!(
        "pi-coding-agent {}\n\nUsage:\n  pi-coding-agent -p <prompt>\n\nOptions:\n  -p, --print              Run one prompt and print the assistant response\n  --model <id>             Model id from the built-in Rust model table\n  --api-key <key>          API key passed to the selected provider\n  --system-prompt <text>   System prompt override\n  --max-turns <n>          Maximum agent loop turns (default: 5)\n  -h, --help               Show help\n  -v, --version            Show version\n",
        env!("CARGO_PKG_VERSION")
    )
}

fn take_value(raw: &[String], index: &mut usize, flag: &str) -> Result<String, CliError> {
    let next_index = *index + 1;
    let value = raw
        .get(next_index)
        .ok_or_else(|| CliError::MissingValue(flag.to_string()))?;
    *index = next_index;
    Ok(value.clone())
}

fn parse_max_turns(value: String) -> Result<u32, CliError> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| CliError::InvalidMaxTurns(value.clone()))?;
    if parsed == 0 {
        return Err(CliError::InvalidMaxTurns(value));
    }
    Ok(parsed)
}

pub fn parse_args<I>(args: I) -> Result<CliArgs, CliError>
where
    I: IntoIterator<Item = String>,
{
    let mut parsed = CliArgs::default();
    let mut prompt_parts = Vec::new();
    let raw: Vec<String> = args.into_iter().collect();
    let mut i = 0;

    while i < raw.len() {
        let arg = &raw[i];
        match arg.as_str() {
            "-p" | "--print" => {
                parsed.print = true;
                if let Some(next) = raw.get(i + 1) {
                    if !next.starts_with('-') || next.starts_with("---") {
                        prompt_parts.push(next.clone());
                        i += 1;
                    }
                }
            }
            "-h" | "--help" => parsed.help = true,
            "-v" | "--version" => parsed.version = true,
            "--model" => parsed.model = Some(take_value(&raw, &mut i, "--model")?),
            "--api-key" => parsed.api_key = Some(take_value(&raw, &mut i, "--api-key")?),
            "--system-prompt" => {
                parsed.system_prompt = Some(take_value(&raw, &mut i, "--system-prompt")?)
            }
            "--max-turns" => {
                let value = take_value(&raw, &mut i, "--max-turns")?;
                parsed.max_turns = parse_max_turns(value)?;
            }
            value if value.starts_with("--") => {
                return Err(CliError::UnknownFlag(value.to_string()));
            }
            value if value.starts_with('-') => {
                return Err(CliError::UnknownFlag(value.to_string()));
            }
            value => prompt_parts.push(value.to_string()),
        }
        i += 1;
    }

    if !prompt_parts.is_empty() {
        parsed.prompt = Some(prompt_parts.join(" "));
    }

    Ok(parsed)
}
```

- [ ] **Step 4: Run parser tests and verify they pass**

Run:

```bash
cargo test -p pi-coding-agent --test args
```

Expected: PASS.

## Task 3: Implement Runtime Model Resolution And Agent Config

**Files:**
- Create: `crates/pi-coding-agent/tests/runtime.rs`
- Replace: `crates/pi-coding-agent/src/runtime.rs`
- Modify: `crates/pi-coding-agent/src/lib.rs`

- [ ] **Step 1: Write runtime tests**

Create `crates/pi-coding-agent/tests/runtime.rs`:

```rust
use pi_coding_agent::{
    build_agent_config, parse_args, select_model, CliError, DEFAULT_MODEL_ID, DEFAULT_SYSTEM_PROMPT,
};

#[test]
fn selects_default_model_when_no_override_is_provided() {
    let args = parse_args(vec!["-p".to_string(), "hello".to_string()]).unwrap();
    let model = select_model(&args, None).unwrap();

    assert_eq!(model.id, DEFAULT_MODEL_ID);
}

#[test]
fn selects_explicit_model_from_static_table() {
    let args = parse_args(vec![
        "--model".to_string(),
        "claude-haiku-4-5".to_string(),
        "-p".to_string(),
        "hello".to_string(),
    ])
    .unwrap();
    let model = select_model(&args, None).unwrap();

    assert_eq!(model.id, "claude-haiku-4-5");
}

#[test]
fn unknown_model_returns_typed_error() {
    let args = parse_args(vec![
        "--model".to_string(),
        "missing-model".to_string(),
        "-p".to_string(),
        "hello".to_string(),
    ])
    .unwrap();

    assert_eq!(
        select_model(&args, None).unwrap_err(),
        CliError::UnknownModel("missing-model".into())
    );
}

#[test]
fn model_override_is_used_when_cli_model_is_absent() {
    let args = parse_args(vec!["-p".to_string(), "hello".to_string()]).unwrap();
    let mut override_model = select_model(&args, None).unwrap();
    override_model.id = "override-model".into();

    let model = select_model(&args, Some(override_model)).unwrap();

    assert_eq!(model.id, "override-model");
}

#[test]
fn builds_agent_config_with_defaults() {
    let args = parse_args(vec!["-p".to_string(), "hello".to_string()]).unwrap();
    let model = select_model(&args, None).unwrap();
    let config = build_agent_config(
        model,
        args.system_prompt.clone(),
        args.max_turns,
        args.api_key.clone(),
    );

    assert_eq!(config.system_prompt.as_deref(), Some(DEFAULT_SYSTEM_PROMPT));
    assert_eq!(config.max_turns, 5);
    assert!(config.stream_options.is_none());
}

#[test]
fn builds_agent_config_with_cli_overrides() {
    let args = parse_args(vec![
        "--api-key".to_string(),
        "sk-test".to_string(),
        "--system-prompt".to_string(),
        "Be brief.".to_string(),
        "--max-turns".to_string(),
        "9".to_string(),
        "-p".to_string(),
        "hello".to_string(),
    ])
    .unwrap();
    let model = select_model(&args, None).unwrap();
    let config = build_agent_config(
        model,
        args.system_prompt.clone(),
        args.max_turns,
        args.api_key.clone(),
    );

    assert_eq!(config.system_prompt.as_deref(), Some("Be brief."));
    assert_eq!(config.max_turns, 9);
    assert_eq!(
        config.stream_options.unwrap().api_key.as_deref(),
        Some("sk-test")
    );
}
```

- [ ] **Step 2: Run runtime tests and verify they fail**

Run:

```bash
cargo test -p pi-coding-agent --test runtime
```

Expected: FAIL to compile because `select_model` is not implemented or exported.

- [ ] **Step 3: Replace runtime implementation**

Replace `crates/pi-coding-agent/src/runtime.rs`:

```rust
use crate::{CliArgs, CliError};
use pi_agent_core::{AgentConfig, AgentTool};
use pi_ai::types::{Model, StreamOptions};

pub const DEFAULT_MODEL_ID: &str = "claude-sonnet-4-5";
pub const DEFAULT_SYSTEM_PROMPT: &str = "You are a helpful coding assistant.";

#[derive(Clone)]
pub struct CliRunOptions {
    pub model_override: Option<Model>,
    pub tools: Vec<AgentTool>,
    pub register_builtins: bool,
}

impl Default for CliRunOptions {
    fn default() -> Self {
        Self {
            model_override: None,
            tools: Vec::new(),
            register_builtins: true,
        }
    }
}

pub fn select_model(args: &CliArgs, model_override: Option<Model>) -> Result<Model, CliError> {
    if let Some(model_id) = &args.model {
        return pi_ai::lookup_model(model_id)
            .ok_or_else(|| CliError::UnknownModel(model_id.clone()));
    }

    if let Some(model) = model_override {
        return Ok(model);
    }

    pi_ai::lookup_model(DEFAULT_MODEL_ID)
        .ok_or_else(|| CliError::UnknownModel(DEFAULT_MODEL_ID.to_string()))
}

pub fn build_agent_config(
    model: Model,
    system_prompt: Option<String>,
    max_turns: u32,
    api_key: Option<String>,
) -> AgentConfig {
    let stream_options = api_key.map(|api_key| StreamOptions {
        api_key: Some(api_key),
        ..Default::default()
    });
    AgentConfig {
        model,
        system_prompt: Some(system_prompt.unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_string())),
        max_turns,
        stream_options,
    }
}
```

- [ ] **Step 4: Export `select_model`**

Modify `crates/pi-coding-agent/src/lib.rs` re-export line for runtime:

```rust
pub use runtime::{
    build_agent_config, select_model, CliRunOptions, DEFAULT_MODEL_ID, DEFAULT_SYSTEM_PROMPT,
};
```

- [ ] **Step 5: Run runtime tests and verify they pass**

Run:

```bash
cargo test -p pi-coding-agent --test runtime
```

Expected: PASS.

## Task 4: Implement Offline Print Mode Runner

**Files:**
- Create: `crates/pi-coding-agent/tests/print_mode.rs`
- Replace: `crates/pi-coding-agent/src/print_mode.rs`

- [ ] **Step 1: Write print-mode tests**

Create `crates/pi-coding-agent/tests/print_mode.rs`:

```rust
use pi_agent_core::AgentTool;
use pi_ai::providers::faux::{FauxCall, FauxProvider, FauxResponse, FauxToolCall};
use pi_ai::registry;
use pi_ai::types::{ContentBlock, Model, StopReason};
use pi_coding_agent::{run_print_mode, CliError, PrintModeOptions};
use std::sync::Arc;

fn faux_model(api: &str) -> Model {
    Model {
        id: "faux-model".into(),
        name: "Faux Model".into(),
        api: api.into(),
        provider: "faux".into(),
        base_url: String::new(),
        reasoning: false,
        input: 0.0,
        output: 0.0,
        cache_read: None,
        cache_write: None,
        context_window: 0,
        max_tokens: None,
        headers: None,
    }
}

fn text_response(text: &str) -> FauxResponse {
    FauxResponse {
        text_deltas: vec![text.to_string()],
        thinking_deltas: vec![],
        tool_calls: vec![],
    }
}

fn echo_tool() -> AgentTool {
    AgentTool {
        name: "echo".into(),
        description: "echoes input".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "text": { "type": "string" }
            }
        }),
        execute: Arc::new(|args| {
            let text = args
                .get("text")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let result = vec![ContentBlock::Text {
                text: format!("echo: {text}"),
                text_signature: None,
            }];
            Box::pin(async move { Ok(result) })
        }),
    }
}

#[tokio::test]
async fn prints_single_turn_text_response() {
    let api = "pi-coding-print-text";
    registry::register(api, Arc::new(FauxProvider::new(vec![text_response("Hello")])));

    let output = run_print_mode(PrintModeOptions {
        prompt: "hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: 5,
        tools: Vec::new(),
        register_builtins: false,
    })
    .await
    .unwrap();

    assert_eq!(output, "Hello");
    registry::unregister(api);
}

#[tokio::test]
async fn treats_length_as_successful_final_text() {
    let api = "pi-coding-print-length";
    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![FauxCall {
            responses: vec![text_response("Partial final text")],
            stop_reason: StopReason::Length,
        }])),
    );

    let output = run_print_mode(PrintModeOptions {
        prompt: "hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: 5,
        tools: Vec::new(),
        register_builtins: false,
    })
    .await
    .unwrap();

    assert_eq!(output, "Partial final text");
    registry::unregister(api);
}

#[tokio::test]
async fn returns_agent_failure_on_error_stop_reason() {
    let api = "pi-coding-print-error";
    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![FauxCall {
            responses: vec![FauxResponse {
                text_deltas: vec![],
                thinking_deltas: vec![],
                tool_calls: vec![],
            }],
            stop_reason: StopReason::Error,
        }])),
    );

    let error = run_print_mode(PrintModeOptions {
        prompt: "hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: 5,
        tools: Vec::new(),
        register_builtins: false,
    })
    .await
    .unwrap_err();

    assert_eq!(error, CliError::AgentFailure("LLM error".into()));
    registry::unregister(api);
}

#[tokio::test]
async fn supports_tool_call_loop_with_injected_tool() {
    let api = "pi-coding-print-tool-loop";
    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxCall {
                responses: vec![FauxResponse {
                    text_deltas: vec![],
                    thinking_deltas: vec![],
                    tool_calls: vec![FauxToolCall {
                        id: "tool_1".into(),
                        name: "echo".into(),
                        deltas: vec!["{\"text\":".into(), "\"hi\"}".into()],
                        final_arguments: serde_json::json!({ "text": "hi" }),
                    }],
                }],
                stop_reason: StopReason::ToolUse,
            },
            FauxCall {
                responses: vec![text_response("Tool completed")],
                stop_reason: StopReason::Stop,
            },
        ])),
    );

    let output = run_print_mode(PrintModeOptions {
        prompt: "echo hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: 5,
        tools: vec![echo_tool()],
        register_builtins: false,
    })
    .await
    .unwrap();

    assert_eq!(output, "Tool completed");
    registry::unregister(api);
}
```

- [ ] **Step 2: Run print-mode tests and verify they fail**

Run:

```bash
cargo test -p pi-coding-agent --test print_mode
```

Expected: FAIL. The success tests return an empty string because the skeleton `run_print_mode()` does not run the agent.

- [ ] **Step 3: Replace print-mode implementation**

Replace `crates/pi-coding-agent/src/print_mode.rs`:

```rust
use crate::{build_agent_config, CliError};
use futures::StreamExt;
use pi_agent_core::{Agent, AgentEvent, AgentTool};
use pi_ai::types::{AssistantMessage, ContentBlock, Model};

pub struct PrintModeOptions {
    pub prompt: String,
    pub model: Model,
    pub api_key: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: u32,
    pub tools: Vec<AgentTool>,
    pub register_builtins: bool,
}

impl PrintModeOptions {
    pub fn new(prompt: impl Into<String>, model: Model) -> Self {
        Self {
            prompt: prompt.into(),
            model,
            api_key: None,
            system_prompt: None,
            max_turns: 5,
            tools: Vec::new(),
            register_builtins: false,
        }
    }
}

fn assistant_text(message: &AssistantMessage) -> String {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub async fn run_print_mode(options: PrintModeOptions) -> Result<String, CliError> {
    if options.register_builtins {
        pi_ai::providers::register_builtins();
    }

    let config = build_agent_config(
        options.model,
        options.system_prompt,
        options.max_turns,
        options.api_key,
    );
    let agent = Agent::new(config);
    for tool in options.tools {
        agent.add_tool(tool);
    }

    let mut stream = agent.prompt(&options.prompt);
    let mut final_message: Option<AssistantMessage> = None;

    while let Some(event) = stream.next().await {
        match event {
            AgentEvent::AgentDone { message } => final_message = Some(message),
            AgentEvent::AgentError { error } => return Err(CliError::AgentFailure(error)),
            _ => {}
        }
    }

    let message = final_message.ok_or_else(|| {
        CliError::AgentFailure("agent stream ended without completion".to_string())
    })?;
    Ok(assistant_text(&message))
}
```

- [ ] **Step 4: Run print-mode tests and verify they pass**

Run:

```bash
cargo test -p pi-coding-agent --test print_mode
```

Expected: PASS.

## Task 5: Implement CLI Orchestration And Binary Entrypoint

**Files:**
- Create: `crates/pi-coding-agent/tests/cli.rs`
- Replace: `crates/pi-coding-agent/src/lib.rs`
- Create: `crates/pi-coding-agent/src/main.rs`

- [ ] **Step 1: Write CLI orchestration tests**

Create `crates/pi-coding-agent/tests/cli.rs`:

```rust
use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::Model;
use pi_coding_agent::{run_cli_with_options, CliRunOptions};
use std::sync::Arc;

fn faux_model(api: &str) -> Model {
    Model {
        id: "faux-model".into(),
        name: "Faux Model".into(),
        api: api.into(),
        provider: "faux".into(),
        base_url: String::new(),
        reasoning: false,
        input: 0.0,
        output: 0.0,
        cache_read: None,
        cache_write: None,
        context_window: 0,
        max_tokens: None,
        headers: None,
    }
}

#[tokio::test]
async fn help_returns_success_with_help_text() {
    let output = run_cli_with_options(
        vec!["--help".to_string()],
        CliRunOptions {
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert!(output.stdout.contains("Usage:"));
    assert!(output.stderr.is_empty());
}

#[tokio::test]
async fn version_returns_success_with_package_version() {
    let output = run_cli_with_options(
        vec!["--version".to_string()],
        CliRunOptions {
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert_eq!(output.stdout, format!("{}\n", env!("CARGO_PKG_VERSION")));
    assert!(output.stderr.is_empty());
}

#[tokio::test]
async fn missing_print_mode_is_rejected() {
    let output = run_cli_with_options(
        vec!["hello".to_string()],
        CliRunOptions {
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 1);
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, "unsupported mode: interactive\n");
}

#[tokio::test]
async fn missing_prompt_is_rejected() {
    let output = run_cli_with_options(
        vec!["-p".to_string()],
        CliRunOptions {
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 1);
    assert_eq!(output.stderr, "missing prompt\n");
}

#[tokio::test]
async fn unknown_model_is_rejected() {
    let output = run_cli_with_options(
        vec![
            "--model".to_string(),
            "missing-model".to_string(),
            "-p".to_string(),
            "hello".to_string(),
        ],
        CliRunOptions {
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 1);
    assert_eq!(output.stderr, "unknown model: missing-model\n");
}

#[tokio::test]
async fn print_mode_uses_injected_model_and_returns_stdout() {
    let api = "pi-coding-cli-success";
    registry::register(api, Arc::new(FauxProvider::simple_text("Hello from CLI")));

    let output = run_cli_with_options(
        vec!["-p".to_string(), "hello".to_string()],
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert_eq!(output.stdout, "Hello from CLI\n");
    assert!(output.stderr.is_empty());
    registry::unregister(api);
}
```

- [ ] **Step 2: Run CLI tests and verify they fail**

Run:

```bash
cargo test -p pi-coding-agent --test cli
```

Expected: FAIL. The skeleton `run_cli_with_options()` does not reject unsupported mode, does not resolve models, and does not call print mode.

- [ ] **Step 3: Replace library orchestration**

Replace `crates/pi-coding-agent/src/lib.rs`:

```rust
pub mod args;
pub mod error;
pub mod print_mode;
pub mod runtime;

pub use args::{help_text, parse_args, CliArgs, DEFAULT_MAX_TURNS};
pub use error::CliError;
pub use print_mode::{run_print_mode, PrintModeOptions};
pub use runtime::{
    build_agent_config, select_model, CliRunOptions, DEFAULT_MODEL_ID, DEFAULT_SYSTEM_PROMPT,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CliOutput {
    fn success(stdout: String) -> Self {
        Self {
            exit_code: 0,
            stdout,
            stderr: String::new(),
        }
    }

    fn failure(error: CliError) -> Self {
        Self {
            exit_code: 1,
            stdout: String::new(),
            stderr: format!("{error}\n"),
        }
    }
}

fn stdout_with_trailing_newline(text: String) -> String {
    if text.is_empty() {
        String::new()
    } else if text.ends_with('\n') {
        text
    } else {
        format!("{text}\n")
    }
}

pub async fn run_cli(args: impl IntoIterator<Item = String>) -> CliOutput {
    run_cli_with_options(args, CliRunOptions::default()).await
}

pub async fn run_cli_with_options(
    args: impl IntoIterator<Item = String>,
    options: CliRunOptions,
) -> CliOutput {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(error) => return CliOutput::failure(error),
    };

    if parsed.help {
        return CliOutput::success(help_text());
    }

    if parsed.version {
        return CliOutput::success(format!("{}\n", env!("CARGO_PKG_VERSION")));
    }

    if !parsed.print {
        return CliOutput::failure(CliError::UnsupportedMode("interactive".into()));
    }

    let prompt = match parsed.prompt.clone() {
        Some(prompt) if !prompt.trim().is_empty() => prompt,
        _ => return CliOutput::failure(CliError::MissingPrompt),
    };

    let model = match select_model(&parsed, options.model_override) {
        Ok(model) => model,
        Err(error) => return CliOutput::failure(error),
    };

    match run_print_mode(PrintModeOptions {
        prompt,
        model,
        api_key: parsed.api_key,
        system_prompt: parsed.system_prompt,
        max_turns: parsed.max_turns,
        tools: options.tools,
        register_builtins: options.register_builtins,
    })
    .await
    {
        Ok(text) => CliOutput::success(stdout_with_trailing_newline(text)),
        Err(error) => CliOutput::failure(error),
    }
}
```

- [ ] **Step 4: Add binary entrypoint**

Create `crates/pi-coding-agent/src/main.rs`:

```rust
#[tokio::main]
async fn main() {
    let output = pi_coding_agent::run_cli(std::env::args().skip(1)).await;

    if !output.stdout.is_empty() {
        print!("{}", output.stdout);
    }
    if !output.stderr.is_empty() {
        eprint!("{}", output.stderr);
    }

    std::process::exit(output.exit_code);
}
```

- [ ] **Step 5: Run CLI tests and verify they pass**

Run:

```bash
cargo test -p pi-coding-agent --test cli
```

Expected: PASS.

- [ ] **Step 6: Run the binary help path**

Run:

```bash
cargo run -p pi-coding-agent -- --help
```

Expected: exit `0`, stdout includes `Usage:` and `--model <id>`.

## Task 6: Full Verification

**Files:**
- No file edits.

- [ ] **Step 1: Run focused crate tests**

Run:

```bash
cargo test -p pi-coding-agent
```

Expected: PASS for unit tests and all `pi-coding-agent` integration tests.

- [ ] **Step 2: Run formatting check**

Run:

```bash
cargo fmt --check
```

Expected: exit `0`, no diff output.

- [ ] **Step 3: Run workspace tests**

Run:

```bash
cargo test --workspace
```

Expected: PASS across all workspace crates.

- [ ] **Step 4: Run workspace compile check**

Run:

```bash
cargo check --workspace
```

Expected: exit `0`.

- [ ] **Step 5: Inspect final diff**

Run:

```bash
git diff -- crates/pi-coding-agent Cargo.toml Cargo.lock
```

Expected: diff contains only the `pi-coding-agent` implementation and dependency lockfile updates caused by the new crate dependencies.
