# pi-coding-agent M1 Search Tools Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the remaining M1 built-in tools `grep`, `find`, and `ls` to `pi-coding-agent` so the Rust agent exposes all seven coding tools.

**Architecture:** Follow the existing tool pattern in `crates/pi-coding-agent/src/tools`: one module per tool with `*_execute` for direct tests and `*_tool` for `AgentTool` registration. Use local Rust filesystem traversal and matching so tests remain deterministic and do not require `rg`, `fd`, provider keys, or network access at runtime.

**Tech Stack:** Rust edition 2024, Tokio filesystem APIs where existing tools use async APIs, `ignore`/`globset` for traversal and glob filtering, `regex` for grep regex matching, existing `pi-agent-core::AgentTool`, existing `pi-ai::ContentBlock`, existing `tools::truncate`.

---

## Context

Read these files before editing:

- `docs/superpowers/specs/2026-06-11-pi-coding-agent-m1-search-tools-design.md`
- `crates/pi-coding-agent/src/tools/mod.rs`
- `crates/pi-coding-agent/src/tools/read.rs`
- `crates/pi-coding-agent/src/tools/path.rs`
- `crates/pi-coding-agent/src/tools/truncate.rs`
- `crates/pi-coding-agent/tests/tools_e2e.rs`
- `pi/packages/coding-agent/src/core/tools/grep.ts`
- `pi/packages/coding-agent/src/core/tools/find.ts`
- `pi/packages/coding-agent/src/core/tools/ls.ts`

Current baseline:

- `builtin_tools` returns `read`, `write`, `edit`, `bash`.
- `tests/tools_e2e.rs::builtin_tools_has_four` asserts that four-tool order.
- Tool unit tests call public `*_execute` helpers directly with `tempfile`.
- Tool end-to-end tests use faux providers and must stay offline.

## File Structure

- Modify `crates/pi-coding-agent/Cargo.toml`
  - Add `globset`, `ignore`, and `regex` dependencies.
- Modify `crates/pi-coding-agent/src/tools/mod.rs`
  - Add `grep`, `find`, and `ls` modules.
  - Register all seven built-in tools.
- Create `crates/pi-coding-agent/src/tools/ls.rs`
  - Implement directory listing.
- Create `crates/pi-coding-agent/src/tools/find.rs`
  - Implement glob search.
- Create `crates/pi-coding-agent/src/tools/grep.rs`
  - Implement content search.
- Create `crates/pi-coding-agent/tests/tool_ls.rs`
  - Focused `ls_execute` tests.
- Create `crates/pi-coding-agent/tests/tool_find.rs`
  - Focused `find_execute` tests.
- Create `crates/pi-coding-agent/tests/tool_grep.rs`
  - Focused `grep_execute` tests.
- Modify `crates/pi-coding-agent/tests/tools_e2e.rs`
  - Update built-in order assertion.
  - Add one faux-provider E2E test for a search tool result.
- Modify `crates/pi-coding-agent/src/lib.rs`
  - Update `default_cli_options_include_builtin_tools` expected names.

## Task 1: Add Dependencies and Red Registration Tests

**Files:**
- Modify: `crates/pi-coding-agent/Cargo.toml`
- Modify: `crates/pi-coding-agent/src/lib.rs`
- Modify: `crates/pi-coding-agent/tests/tools_e2e.rs`

- [ ] **Step 1: Add local search dependencies**

In `crates/pi-coding-agent/Cargo.toml`, add these dependencies under `[dependencies]`:

```toml
globset = "0.4"
ignore = "0.4"
regex = "1"
```

- [ ] **Step 2: Update the public built-in tool expectation**

In `crates/pi-coding-agent/src/lib.rs`, update the `default_cli_options_include_builtin_tools` assertion:

```rust
assert_eq!(
    names,
    vec!["read", "write", "edit", "bash", "grep", "find", "ls"]
);
```

- [ ] **Step 3: Update the E2E built-in tool order test**

In `crates/pi-coding-agent/tests/tools_e2e.rs`, rename `builtin_tools_has_four` to `builtin_tools_has_seven` and update the assertion:

```rust
#[test]
fn builtin_tools_has_seven() {
    let tools = pi_coding_agent::builtin_tools(std::path::PathBuf::from("."));
    let names: Vec<_> = tools.iter().map(|t| t.name.clone()).collect();
    assert_eq!(names, vec!["read", "write", "edit", "bash", "grep", "find", "ls"]);
}
```

- [ ] **Step 4: Run red registration tests**

Run:

```bash
cargo test -p pi-coding-agent default_cli_options_include_builtin_tools
cargo test -p pi-coding-agent --test tools_e2e builtin_tools_has_seven
```

Expected: FAIL because `grep`, `find`, and `ls` modules are not registered yet.

## Task 2: Implement `ls`

**Files:**
- Create: `crates/pi-coding-agent/src/tools/ls.rs`
- Create: `crates/pi-coding-agent/tests/tool_ls.rs`
- Modify: `crates/pi-coding-agent/src/tools/mod.rs`

- [ ] **Step 1: Write focused `ls` tests**

Create `crates/pi-coding-agent/tests/tool_ls.rs`:

```rust
use pi_ai::types::ContentBlock;
use pi_coding_agent::tools::ls::ls_execute;
use tempfile::tempdir;

fn text(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn lists_entries_sorted_with_directory_suffix_and_dotfiles() {
    let dir = tempdir().unwrap();
    tokio::fs::write(dir.path().join("b.txt"), "b").await.unwrap();
    tokio::fs::write(dir.path().join(".env"), "x").await.unwrap();
    tokio::fs::create_dir(dir.path().join("Alpha")).await.unwrap();

    let result = ls_execute(dir.path(), serde_json::json!({})).await.unwrap();

    assert_eq!(text(&result), ".env\nAlpha/\nb.txt");
}

#[tokio::test]
async fn limit_adds_notice() {
    let dir = tempdir().unwrap();
    tokio::fs::write(dir.path().join("a.txt"), "a").await.unwrap();
    tokio::fs::write(dir.path().join("b.txt"), "b").await.unwrap();

    let result = ls_execute(dir.path(), serde_json::json!({"limit": 1}))
        .await
        .unwrap();

    let output = text(&result);
    assert_eq!(output, "a.txt\n\n[1 entries limit reached. Use limit=2 for more]");
}

#[tokio::test]
async fn empty_directory_message() {
    let dir = tempdir().unwrap();

    let result = ls_execute(dir.path(), serde_json::json!({})).await.unwrap();

    assert_eq!(text(&result), "(empty directory)");
}

#[tokio::test]
async fn missing_and_file_paths_error() {
    let dir = tempdir().unwrap();
    tokio::fs::write(dir.path().join("file.txt"), "x").await.unwrap();

    let missing = ls_execute(dir.path(), serde_json::json!({"path": "missing"}))
        .await
        .unwrap_err();
    assert!(missing.starts_with("ls: path not found:"), "{missing}");

    let file = ls_execute(dir.path(), serde_json::json!({"path": "file.txt"}))
        .await
        .unwrap_err();
    assert!(file.starts_with("ls: not a directory:"), "{file}");
}
```

- [ ] **Step 2: Run red `ls` tests**

Run:

```bash
cargo test -p pi-coding-agent --test tool_ls
```

Expected: FAIL because `pi_coding_agent::tools::ls` does not exist.

- [ ] **Step 3: Implement `ls.rs`**

Create `crates/pi-coding-agent/src/tools/ls.rs` with:

```rust
use crate::tools::path::resolve_to_cwd;
use crate::tools::truncate::{DEFAULT_MAX_BYTES, TruncationOptions, format_size, truncate_head};
use pi_agent_core::{AgentTool, ToolFn};
use pi_ai::types::ContentBlock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const DESCRIPTION: &str = "List directory contents. Returns entries sorted alphabetically, with '/' suffix for directories. Includes dotfiles. Output is truncated to 500 entries or 50KB (whichever is hit first).";
const DEFAULT_LIMIT: usize = 500;

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "Directory to list (default: current directory)" },
            "limit": { "type": "number", "description": "Maximum number of entries to return (default: 500)" }
        }
    })
}

fn text_block(text: String) -> Vec<ContentBlock> {
    vec![ContentBlock::Text {
        text,
        text_signature: None,
    }]
}

fn limit_arg(args: &serde_json::Value, default: usize) -> usize {
    args.get("limit")
        .and_then(|v| v.as_u64())
        .map(|n| n.max(1) as usize)
        .unwrap_or(default)
}

pub async fn ls_execute(cwd: &Path, args: serde_json::Value) -> Result<Vec<ContentBlock>, String> {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let limit = limit_arg(&args, DEFAULT_LIMIT);
    let abs = resolve_to_cwd(path, cwd);

    let meta = tokio::fs::metadata(&abs)
        .await
        .map_err(|_| format!("ls: path not found: {}", abs.display()))?;
    if !meta.is_dir() {
        return Err(format!("ls: not a directory: {}", abs.display()));
    }

    let mut entries = Vec::new();
    let mut read_dir = tokio::fs::read_dir(&abs)
        .await
        .map_err(|e| format!("ls: cannot read directory {}: {e}", abs.display()))?;
    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|e| format!("ls: cannot read directory {}: {e}", abs.display()))?
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
        entries.push(if is_dir { format!("{name}/") } else { name });
    }

    entries.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    });

    if entries.is_empty() {
        return Ok(text_block("(empty directory)".to_string()));
    }

    let entry_limit_reached = entries.len() > limit;
    let mut output = entries.into_iter().take(limit).collect::<Vec<_>>().join("\n");
    let truncation = truncate_head(
        &output,
        &TruncationOptions {
            max_lines: Some(usize::MAX),
            max_bytes: Some(DEFAULT_MAX_BYTES),
        },
    );
    output = truncation.content;

    let mut notices = Vec::new();
    if entry_limit_reached {
        notices.push(format!(
            "{limit} entries limit reached. Use limit={} for more",
            limit * 2
        ));
    }
    if truncation.truncated {
        notices.push(format!("{} limit reached", format_size(DEFAULT_MAX_BYTES)));
    }
    if !notices.is_empty() {
        output.push_str(&format!("\n\n[{}]", notices.join(". ")));
    }

    Ok(text_block(output))
}

pub fn ls_tool(cwd: PathBuf) -> AgentTool {
    let execute: ToolFn = Arc::new(move |args| {
        let cwd = cwd.clone();
        Box::pin(async move { ls_execute(&cwd, args).await })
    });
    AgentTool {
        name: "ls".into(),
        description: DESCRIPTION.into(),
        parameters: schema(),
        execution_mode: None,
        execute,
    }
}
```

- [ ] **Step 4: Register the module temporarily enough for tests**

In `crates/pi-coding-agent/src/tools/mod.rs`, add:

```rust
pub mod ls;
```

Do not add it to `builtin_tools` until Task 5.

- [ ] **Step 5: Run green `ls` tests**

Run:

```bash
cargo test -p pi-coding-agent --test tool_ls
```

Expected: PASS.

## Task 3: Implement `find`

**Files:**
- Create: `crates/pi-coding-agent/src/tools/find.rs`
- Create: `crates/pi-coding-agent/tests/tool_find.rs`
- Modify: `crates/pi-coding-agent/src/tools/mod.rs`

- [ ] **Step 1: Write focused `find` tests**

Create `crates/pi-coding-agent/tests/tool_find.rs`:

```rust
use pi_ai::types::ContentBlock;
use pi_coding_agent::tools::find::find_execute;
use tempfile::tempdir;

fn text(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn finds_basename_matches_recursively_and_skips_common_dirs() {
    let dir = tempdir().unwrap();
    tokio::fs::create_dir_all(dir.path().join("src/nested")).await.unwrap();
    tokio::fs::create_dir_all(dir.path().join(".git")).await.unwrap();
    tokio::fs::create_dir_all(dir.path().join("node_modules/pkg")).await.unwrap();
    tokio::fs::write(dir.path().join("src/lib.rs"), "").await.unwrap();
    tokio::fs::write(dir.path().join("src/nested/main.rs"), "").await.unwrap();
    tokio::fs::write(dir.path().join(".git/hidden.rs"), "").await.unwrap();
    tokio::fs::write(dir.path().join("node_modules/pkg/index.rs"), "").await.unwrap();

    let result = find_execute(dir.path(), serde_json::json!({"pattern": "*.rs"}))
        .await
        .unwrap();

    assert_eq!(text(&result), "src/lib.rs\nsrc/nested/main.rs");
}

#[tokio::test]
async fn path_pattern_matches_relative_path() {
    let dir = tempdir().unwrap();
    tokio::fs::create_dir_all(dir.path().join("src")).await.unwrap();
    tokio::fs::create_dir_all(dir.path().join("tests")).await.unwrap();
    tokio::fs::write(dir.path().join("src/app.spec.ts"), "").await.unwrap();
    tokio::fs::write(dir.path().join("tests/app.spec.ts"), "").await.unwrap();

    let result = find_execute(
        dir.path(),
        serde_json::json!({"pattern": "src/**/*.spec.ts"}),
    )
    .await
    .unwrap();

    assert_eq!(text(&result), "src/app.spec.ts");
}

#[tokio::test]
async fn limit_and_empty_results_are_reported() {
    let dir = tempdir().unwrap();
    tokio::fs::write(dir.path().join("a.txt"), "").await.unwrap();
    tokio::fs::write(dir.path().join("b.txt"), "").await.unwrap();

    let limited = find_execute(dir.path(), serde_json::json!({"pattern": "*.txt", "limit": 1}))
        .await
        .unwrap();
    assert_eq!(
        text(&limited),
        "a.txt\n\n[1 results limit reached. Use limit=2 for more, or refine pattern]"
    );

    let empty = find_execute(dir.path(), serde_json::json!({"pattern": "*.rs"}))
        .await
        .unwrap();
    assert_eq!(text(&empty), "No files found matching pattern");
}
```

- [ ] **Step 2: Run red `find` tests**

Run:

```bash
cargo test -p pi-coding-agent --test tool_find
```

Expected: FAIL because `pi_coding_agent::tools::find` does not exist.

- [ ] **Step 3: Implement `find.rs`**

Create `crates/pi-coding-agent/src/tools/find.rs`.

Implementation requirements:

- Parse required `pattern` as string.
- Resolve `path` through `resolve_to_cwd`, defaulting to `"."`.
- Use `ignore::WalkBuilder` with hidden entries allowed and standard ignore support enabled.
- Add a filter that rejects any path component equal to `.git` or `node_modules`.
- Use `globset::GlobBuilder` to compile the pattern.
- If the raw pattern contains `/`, match against the relative POSIX path.
- If the raw pattern does not contain `/`, match against the basename.
- Collect files and directories, suffix directories with `/`.
- Sort case-insensitively.
- Apply `limit` and `truncate_head`.
- Return the exact notices from the spec.

Use this public surface:

```rust
pub async fn find_execute(
    cwd: &std::path::Path,
    args: serde_json::Value,
) -> Result<Vec<pi_ai::types::ContentBlock>, String>
```

and:

```rust
pub fn find_tool(cwd: std::path::PathBuf) -> pi_agent_core::AgentTool
```

- [ ] **Step 4: Register the module temporarily enough for tests**

In `crates/pi-coding-agent/src/tools/mod.rs`, add:

```rust
pub mod find;
```

Do not add it to `builtin_tools` until Task 5.

- [ ] **Step 5: Run green `find` tests**

Run:

```bash
cargo test -p pi-coding-agent --test tool_find
```

Expected: PASS.

## Task 4: Implement `grep`

**Files:**
- Create: `crates/pi-coding-agent/src/tools/grep.rs`
- Create: `crates/pi-coding-agent/tests/tool_grep.rs`
- Modify: `crates/pi-coding-agent/src/tools/mod.rs`

- [ ] **Step 1: Write focused `grep` tests**

Create `crates/pi-coding-agent/tests/tool_grep.rs`:

```rust
use pi_ai::types::ContentBlock;
use pi_coding_agent::tools::grep::grep_execute;
use tempfile::tempdir;

fn text(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn grep_literal_and_ignore_case_with_relative_paths() {
    let dir = tempdir().unwrap();
    tokio::fs::create_dir_all(dir.path().join("src")).await.unwrap();
    tokio::fs::write(dir.path().join("src/a.txt"), "Hello\nworld").await.unwrap();

    let result = grep_execute(
        dir.path(),
        serde_json::json!({"pattern": "hello", "literal": true, "ignoreCase": true}),
    )
    .await
    .unwrap();

    assert_eq!(text(&result), "src/a.txt:1: Hello");
}

#[tokio::test]
async fn grep_regex_glob_context_and_limit() {
    let dir = tempdir().unwrap();
    tokio::fs::create_dir_all(dir.path().join("src")).await.unwrap();
    tokio::fs::write(dir.path().join("src/a.rs"), "before\nlet alpha = 1;\nafter").await.unwrap();
    tokio::fs::write(dir.path().join("src/b.txt"), "let beta = 2;").await.unwrap();

    let result = grep_execute(
        dir.path(),
        serde_json::json!({
            "pattern": "alpha|beta",
            "glob": "*.rs",
            "context": 1,
            "limit": 1
        }),
    )
    .await
    .unwrap();

    assert_eq!(
        text(&result),
        "src/a.rs-1- before\nsrc/a.rs:2: let alpha = 1;\nsrc/a.rs-3- after\n\n[1 matches limit reached. Use limit=2 for more, or refine pattern]"
    );
}

#[tokio::test]
async fn grep_reports_no_matches_and_invalid_regex() {
    let dir = tempdir().unwrap();
    tokio::fs::write(dir.path().join("a.txt"), "abc").await.unwrap();

    let empty = grep_execute(dir.path(), serde_json::json!({"pattern": "zzz"}))
        .await
        .unwrap();
    assert_eq!(text(&empty), "No matches found");

    let err = grep_execute(dir.path(), serde_json::json!({"pattern": "["}))
        .await
        .unwrap_err();
    assert!(err.starts_with("grep: invalid regex:"), "{err}");
}
```

- [ ] **Step 2: Run red `grep` tests**

Run:

```bash
cargo test -p pi-coding-agent --test tool_grep
```

Expected: FAIL because `pi_coding_agent::tools::grep` does not exist.

- [ ] **Step 3: Implement `grep.rs`**

Create `crates/pi-coding-agent/src/tools/grep.rs`.

Implementation requirements:

- Parse required `pattern` as string.
- Parse `path`, `glob`, `ignoreCase`, `literal`, `context`, and `limit`.
- For literal mode, use `regex::escape(pattern)` before compiling.
- For ignore-case mode, use `regex::RegexBuilder::new(&pattern).case_insensitive(true).build()`.
- Resolve file candidates:
  - If `path` is a file, search only that file.
  - If `path` is a directory, traverse recursively using the same skip rules as `find`.
- If `glob` is present, compile it with basename matching when it does not contain `/`, and relative-path matching when it does.
- Normalize file contents with `\r\n` and `\r` converted to `\n`.
- Line numbers are 1-indexed.
- Context lines use `path-line- text`; match lines use `path:line: text`.
- Truncate displayed source lines to 500 Unicode scalar values, with no partial character.
- Track match count and stop once `limit` is reached.
- Apply `truncate_head` to the final output.
- Return the exact notices from the spec.

Use this public surface:

```rust
pub async fn grep_execute(
    cwd: &std::path::Path,
    args: serde_json::Value,
) -> Result<Vec<pi_ai::types::ContentBlock>, String>
```

and:

```rust
pub fn grep_tool(cwd: std::path::PathBuf) -> pi_agent_core::AgentTool
```

- [ ] **Step 4: Register the module temporarily enough for tests**

In `crates/pi-coding-agent/src/tools/mod.rs`, add:

```rust
pub mod grep;
```

Do not add it to `builtin_tools` until Task 5.

- [ ] **Step 5: Run green `grep` tests**

Run:

```bash
cargo test -p pi-coding-agent --test tool_grep
```

Expected: PASS.

## Task 5: Register All Seven Tools and Add E2E Coverage

**Files:**
- Modify: `crates/pi-coding-agent/src/tools/mod.rs`
- Modify: `crates/pi-coding-agent/src/lib.rs`
- Modify: `crates/pi-coding-agent/tests/tools_e2e.rs`

- [ ] **Step 1: Register tools in `builtin_tools`**

Update `crates/pi-coding-agent/src/tools/mod.rs`:

```rust
pub mod bash;
pub mod edit;
pub mod find;
pub mod grep;
pub mod ls;
pub mod path;
pub mod read;
pub mod truncate;
pub mod write;

pub fn builtin_tools(cwd: PathBuf) -> Vec<AgentTool> {
    vec![
        read::read_tool(cwd.clone()),
        write::write_tool(cwd.clone()),
        edit::edit_tool(cwd.clone()),
        bash::bash_tool(cwd.clone()),
        grep::grep_tool(cwd.clone()),
        find::find_tool(cwd.clone()),
        ls::ls_tool(cwd),
    ]
}
```

- [ ] **Step 2: Add an E2E search tool call**

In `crates/pi-coding-agent/tests/tools_e2e.rs`, add a helper next to `read_call`:

```rust
fn grep_call(pattern: &str) -> FauxResponse {
    FauxResponse {
        text_deltas: vec![],
        thinking_deltas: vec![],
        tool_calls: vec![FauxToolCall {
            id: "tool_1".into(),
            name: "grep".into(),
            deltas: vec![format!(r#"{{"pattern":"{pattern}","literal":true}}"#)],
            final_arguments: serde_json::json!({ "pattern": pattern, "literal": true }),
        }],
    }
}
```

Then add this test:

```rust
#[tokio::test]
async fn grep_builtin_tool_success_is_sent_back_to_model() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("input.txt"), "alpha\nbeta").unwrap();
    let contexts = Arc::new(Mutex::new(Vec::new()));
    let api = "pi-coding-tools-e2e-grep";

    registry::register(
        api,
        Arc::new(RecordingProvider::new(
            vec![
                FauxCall {
                    responses: vec![grep_call("beta")],
                    stop_reason: StopReason::ToolUse,
                },
                FauxCall {
                    responses: vec![text_response("done")],
                    stop_reason: StopReason::Stop,
                },
            ],
            contexts.clone(),
        )),
    );

    let out = run_print_mode(PrintModeOptions {
        prompt: "search".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: 5,
        tools: builtin_tools(dir.path().to_path_buf()),
        register_builtins: false,
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        invocation: pi_coding_agent::PromptInvocation::Text("search".into()),
    })
    .await
    .unwrap();
    registry::unregister(api);

    assert_eq!(out, "done");
    let contexts = contexts.lock().unwrap();
    let second_call = contexts.get(1).expect("second model call should include grep result");
    let text = second_call
        .messages
        .iter()
        .find_map(|message| match message {
            Message::ToolResult { tool_name, content, .. } if tool_name.as_deref() == Some("grep") => {
                Some(content)
            }
            _ => None,
        })
        .expect("grep tool result should be present")
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(text.contains("input.txt:2: beta"), "{text}");
}
```

- [ ] **Step 3: Run registration and E2E tests**

Run:

```bash
cargo test -p pi-coding-agent default_cli_options_include_builtin_tools
cargo test -p pi-coding-agent --test tools_e2e
```

Expected: PASS.

## Task 6: Final Verification

**Files:**
- No code changes unless verification exposes issues.

- [ ] **Step 1: Focused tool tests**

Run:

```bash
cargo test -p pi-coding-agent --test tool_ls
cargo test -p pi-coding-agent --test tool_find
cargo test -p pi-coding-agent --test tool_grep
cargo test -p pi-coding-agent --test tools_e2e
```

Expected: PASS.

- [ ] **Step 2: Full crate tests**

Run:

```bash
cargo test -p pi-coding-agent
```

Expected: PASS.

- [ ] **Step 3: Workspace checks**

Run:

```bash
cargo fmt --check
cargo test --workspace
cargo check --workspace
git diff --check
```

Expected: PASS. Existing warning output from unrelated tests is acceptable if exit code is 0.

## Self-Review Checklist

- [ ] Every spec requirement maps to a task above.
- [ ] `grep`, `find`, and `ls` each have direct unit tests.
- [ ] `builtin_tools` order is exactly `read`, `write`, `edit`, `bash`, `grep`, `find`, `ls`.
- [ ] No test requires real provider keys, `rg`, `fd`, or network access.
- [ ] Output paths are relative and use `/`.
- [ ] Workspace verification commands are listed.
