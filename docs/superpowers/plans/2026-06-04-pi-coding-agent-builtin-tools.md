# pi-coding-agent built-in tools (read/write/edit/bash) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (inline) — implement task-by-task. Steps use checkbox (`- [ ]`) syntax. TDD throughout; commit after each task.

**Goal:** Give the Rust `pi-coding-agent` print-mode CLI four built-in tools (read, write, edit, bash) so it can inspect files, run commands, create files, and edit code, wired on by default.

**Architecture:** Each tool is a factory function `xxx_tool(cwd) -> AgentTool` (mirrors TS `createXxxTool`); the closure parses `serde_json::Value` args and returns `Result<Vec<ContentBlock>, String>`. Shared helpers (`truncate`, `path`) live in sibling modules. `builtin_tools(cwd)` assembles all four; `run_cli` wires them in by default. No changes to `pi-ai`/`pi-agent-core`.

**Tech Stack:** Rust edition 2024, tokio (fs/process/time/io-util), unicode-normalization (edit fuzzy NFKC), tempfile (dev tests), serde_json. Behavioral reference: `pi/packages/coding-agent/src/core/tools/*.ts`. Exact strings: spec §12.1.

**Spec:** `docs/superpowers/specs/2026-06-04-pi-coding-agent-builtin-tools-design.md`

---

## File Structure

- Create `crates/pi-coding-agent/src/tools/mod.rs` — `builtin_tools(cwd) -> Vec<AgentTool>`, re-exports.
- Create `crates/pi-coding-agent/src/tools/truncate.rs` — `TruncationResult`, `truncate_head`, `truncate_tail`, `format_size`.
- Create `crates/pi-coding-agent/src/tools/path.rs` — `resolve_to_cwd(path, cwd)`.
- Create `crates/pi-coding-agent/src/tools/read.rs` — `read_tool(cwd)` + `read_execute`.
- Create `crates/pi-coding-agent/src/tools/write.rs` — `write_tool(cwd)` + `write_execute`.
- Create `crates/pi-coding-agent/src/tools/edit.rs` — `edit_tool(cwd)` + `edit_execute` + fuzzy apply.
- Create `crates/pi-coding-agent/src/tools/bash.rs` — `bash_tool(cwd)` + `bash_execute`.
- Modify `crates/pi-coding-agent/src/lib.rs` — `pub mod tools;`, re-export `builtin_tools`, wire `run_cli`.
- Modify `crates/pi-coding-agent/Cargo.toml` — tokio features `fs,process,time,io-util`; add `unicode-normalization`; dev-dep `tempfile`.
- Create tests `crates/pi-coding-agent/tests/{tool_read,tool_write,tool_edit,tool_bash,tools_e2e}.rs`.

Helpers are unit-tested inline (`#[cfg(test)]`); tool behavior is tested both inline and via the integration tests calling `xxx_execute` directly over `tempfile::tempdir()`.

---

## Task 1: Scaffold module + deps (compiles, empty)

**Files:** Modify `Cargo.toml`, `src/lib.rs`; Create `src/tools/mod.rs`.

- [ ] **Step 1: Update Cargo.toml**

```toml
[dependencies]
futures = "0.3"
pi-agent-core = { path = "../pi-agent-core" }
pi-ai = { path = "../pi-ai" }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "fs", "process", "time", "io-util"] }
unicode-normalization = "0.1"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Create `src/tools/mod.rs`**

```rust
use pi_agent_core::AgentTool;
use std::path::PathBuf;

pub mod bash;
pub mod edit;
pub mod path;
pub mod read;
pub mod truncate;
pub mod write;

/// All built-in tools bound to `cwd`, in a stable order.
pub fn builtin_tools(cwd: PathBuf) -> Vec<AgentTool> {
    vec![
        read::read_tool(cwd.clone()),
        write::write_tool(cwd.clone()),
        edit::edit_tool(cwd.clone()),
        bash::bash_tool(cwd),
    ]
}
```

- [ ] **Step 3: Add `pub mod tools;` and re-export in `src/lib.rs`** (top, after existing `pub mod` lines)

```rust
pub mod tools;
pub use tools::builtin_tools;
```

(Each submodule file must exist before this compiles; create empty stubs with a single `pub fn xxx_tool(_cwd: std::path::PathBuf) -> pi_agent_core::AgentTool { unimplemented!() }` placeholder if needed to compile, replaced in later tasks. Prefer creating real files in the order below so the workspace compiles after each task.)

- [ ] **Step 4: `cargo build -p pi-coding-agent`** — Expected: builds (stubs may warn).

- [ ] **Step 5: Commit** `git add -A && git commit -m "feat(pi-coding-agent): scaffold tools module + deps"`

---

## Task 2: `truncate.rs` (port truncate.ts)

**Files:** Create `src/tools/truncate.rs`.

- [ ] **Step 1: Write failing tests** (inline `#[cfg(test)]`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn no_truncation() {
        let r = truncate_head("a\nb\nc", &Default::default());
        assert!(!r.truncated); assert_eq!(r.content, "a\nb\nc"); assert_eq!(r.total_lines, 3);
    }
    #[test] fn head_line_limit() {
        let content = (0..10).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        let r = truncate_head(&content, &TruncationOptions { max_lines: Some(3), max_bytes: None });
        assert!(r.truncated); assert_eq!(r.truncated_by, TruncatedBy::Lines);
        assert_eq!(r.output_lines, 3); assert_eq!(r.content, "0\n1\n2");
    }
    #[test] fn head_byte_limit() {
        let content = "aaaa\nbbbb\ncccc"; // 4 bytes/line
        let r = truncate_head(content, &TruncationOptions { max_lines: None, max_bytes: Some(6) });
        assert!(r.truncated); assert_eq!(r.truncated_by, TruncatedBy::Bytes);
        assert_eq!(r.content, "aaaa"); // 2nd line would push to 9 bytes > 6
    }
    #[test] fn head_first_line_exceeds() {
        let r = truncate_head("aaaaaaaaaa\nb", &TruncationOptions { max_lines: None, max_bytes: Some(5) });
        assert!(r.first_line_exceeds_limit); assert_eq!(r.content, "");
    }
    #[test] fn tail_line_limit() {
        let content = (0..10).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        let r = truncate_tail(&content, &TruncationOptions { max_lines: Some(3), max_bytes: None });
        assert!(r.truncated); assert_eq!(r.content, "7\n8\n9");
    }
    #[test] fn tail_partial_last_line() {
        let r = truncate_tail("héllo-world", &TruncationOptions { max_lines: None, max_bytes: Some(5) });
        assert!(r.last_line_partial);
        assert!(r.content.len() <= 5);
        assert!(std::str::from_utf8(r.content.as_bytes()).is_ok());
    }
    #[test] fn size_fmt() {
        assert_eq!(format_size(512), "512B");
        assert_eq!(format_size(51200), "50.0KB");
    }
}
```

- [ ] **Step 2: Run** `cargo test -p pi-coding-agent --lib tools::truncate` — Expected: FAIL (items undefined).

- [ ] **Step 3: Implement** (port `truncate.ts` exactly; byte counts are UTF-8 `.len()`)

```rust
pub const DEFAULT_MAX_LINES: usize = 2000;
pub const DEFAULT_MAX_BYTES: usize = 50 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncatedBy { Lines, Bytes, None }

#[derive(Debug, Clone, Default)]
pub struct TruncationOptions { pub max_lines: Option<usize>, pub max_bytes: Option<usize> }

#[derive(Debug, Clone)]
pub struct TruncationResult {
    pub content: String, pub truncated: bool, pub truncated_by: TruncatedBy,
    pub total_lines: usize, pub total_bytes: usize, pub output_lines: usize, pub output_bytes: usize,
    pub last_line_partial: bool, pub first_line_exceeds_limit: bool,
    pub max_lines: usize, pub max_bytes: usize,
}

pub fn format_size(bytes: usize) -> String {
    if bytes < 1024 { format!("{bytes}B") }
    else if bytes < 1024 * 1024 { format!("{:.1}KB", bytes as f64 / 1024.0) }
    else { format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0)) }
}

fn split_lines_for_counting(content: &str) -> Vec<&str> {
    if content.is_empty() { return Vec::new(); }
    let mut lines: Vec<&str> = content.split('\n').collect();
    if content.ends_with('\n') { lines.pop(); }
    lines
}

fn no_trunc(content: &str, lines: usize, bytes: usize, max_lines: usize, max_bytes: usize) -> TruncationResult {
    TruncationResult { content: content.to_string(), truncated: false, truncated_by: TruncatedBy::None,
        total_lines: lines, total_bytes: bytes, output_lines: lines, output_bytes: bytes,
        last_line_partial: false, first_line_exceeds_limit: false, max_lines, max_bytes }
}

pub fn truncate_head(content: &str, opts: &TruncationOptions) -> TruncationResult {
    let max_lines = opts.max_lines.unwrap_or(DEFAULT_MAX_LINES);
    let max_bytes = opts.max_bytes.unwrap_or(DEFAULT_MAX_BYTES);
    let total_bytes = content.len();
    let lines = split_lines_for_counting(content);
    let total_lines = lines.len();
    if total_lines <= max_lines && total_bytes <= max_bytes {
        return no_trunc(content, total_lines, total_bytes, max_lines, max_bytes);
    }
    let first_line_bytes = lines.first().map(|l| l.len()).unwrap_or(0);
    if first_line_bytes > max_bytes {
        return TruncationResult { content: String::new(), truncated: true, truncated_by: TruncatedBy::Bytes,
            total_lines, total_bytes, output_lines: 0, output_bytes: 0, last_line_partial: false,
            first_line_exceeds_limit: true, max_lines, max_bytes };
    }
    let mut out: Vec<&str> = Vec::new();
    let mut bytes_count = 0usize;
    let mut truncated_by = TruncatedBy::Lines;
    for (i, line) in lines.iter().enumerate() {
        if i >= max_lines { break; }
        let line_bytes = line.len() + if i > 0 { 1 } else { 0 };
        if bytes_count + line_bytes > max_bytes { truncated_by = TruncatedBy::Bytes; break; }
        out.push(line); bytes_count += line_bytes;
    }
    if out.len() >= max_lines && bytes_count <= max_bytes { truncated_by = TruncatedBy::Lines; }
    let content_out = out.join("\n");
    let output_bytes = content_out.len();
    TruncationResult { content: content_out, truncated: true, truncated_by, total_lines, total_bytes,
        output_lines: out.len(), output_bytes, last_line_partial: false, first_line_exceeds_limit: false,
        max_lines, max_bytes }
}

fn truncate_bytes_from_end(s: &str, max_bytes: usize) -> String {
    let b = s.as_bytes();
    if b.len() <= max_bytes { return s.to_string(); }
    let mut start = b.len() - max_bytes;
    while start < b.len() && (b[start] & 0xC0) == 0x80 { start += 1; }
    String::from_utf8_lossy(&b[start..]).into_owned()
}

pub fn truncate_tail(content: &str, opts: &TruncationOptions) -> TruncationResult {
    let max_lines = opts.max_lines.unwrap_or(DEFAULT_MAX_LINES);
    let max_bytes = opts.max_bytes.unwrap_or(DEFAULT_MAX_BYTES);
    let total_bytes = content.len();
    let lines = split_lines_for_counting(content);
    let total_lines = lines.len();
    if total_lines <= max_lines && total_bytes <= max_bytes {
        return no_trunc(content, total_lines, total_bytes, max_lines, max_bytes);
    }
    let mut out: Vec<String> = Vec::new();
    let mut bytes_count = 0usize;
    let mut truncated_by = TruncatedBy::Lines;
    let mut last_line_partial = false;
    for line in lines.iter().rev() {
        if out.len() >= max_lines { break; }
        let line_bytes = line.len() + if !out.is_empty() { 1 } else { 0 };
        if bytes_count + line_bytes > max_bytes {
            truncated_by = TruncatedBy::Bytes;
            if out.is_empty() {
                let t = truncate_bytes_from_end(line, max_bytes);
                bytes_count = t.len(); out.insert(0, t); last_line_partial = true;
            }
            break;
        }
        out.insert(0, (*line).to_string()); bytes_count += line_bytes;
    }
    if out.len() >= max_lines && bytes_count <= max_bytes { truncated_by = TruncatedBy::Lines; }
    let content_out = out.join("\n");
    let output_bytes = content_out.len();
    TruncationResult { content: content_out, truncated: true, truncated_by, total_lines, total_bytes,
        output_lines: out.len(), output_bytes, last_line_partial, first_line_exceeds_limit: false,
        max_lines, max_bytes }
}
```

- [ ] **Step 4: Run** `cargo test -p pi-coding-agent --lib tools::truncate` — Expected: PASS.
- [ ] **Step 5: Commit** `git add -A && git commit -m "feat(pi-coding-agent): add tool output truncation helpers"`

---

## Task 3: `path.rs` (resolve_to_cwd)

**Files:** Create `src/tools/path.rs`.

- [ ] **Step 1: Failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    #[test] fn relative_joins_cwd() {
        assert_eq!(resolve_to_cwd("a/b.txt", &PathBuf::from("/work")), PathBuf::from("/work/a/b.txt"));
    }
    #[test] fn absolute_kept() {
        assert_eq!(resolve_to_cwd("/etc/hosts", &PathBuf::from("/work")), PathBuf::from("/etc/hosts"));
    }
    #[test] fn tilde_expands_or_keeps() {
        let r = resolve_to_cwd("~/x", &PathBuf::from("/work"));
        match std::env::var("HOME") {
            Ok(h) if !h.is_empty() => assert_eq!(r, PathBuf::from(h).join("x")),
            _ => assert_eq!(r, PathBuf::from("~/x")),
        }
    }
}
```

- [ ] **Step 2: Run** `cargo test -p pi-coding-agent --lib tools::path` — Expected: FAIL.
- [ ] **Step 3: Implement**

```rust
use std::path::{Path, PathBuf};

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from).filter(|p| !p.as_os_str().is_empty())
}

pub fn resolve_to_cwd(path: &str, cwd: &Path) -> PathBuf {
    if path == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(path));
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return match home_dir() { Some(h) => h.join(rest), None => PathBuf::from(path) };
    }
    let p = Path::new(path);
    if p.is_absolute() { p.to_path_buf() } else { cwd.join(p) }
}
```

- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `git add -A && git commit -m "feat(pi-coding-agent): add path resolution helper"`

---

## Task 4: `write.rs` (simplest tool)

**Files:** Create `src/tools/write.rs`; Create `tests/tool_write.rs`.

- [ ] **Step 1: Failing integration test** (`tests/tool_write.rs`)

```rust
use pi_ai::types::ContentBlock;
use pi_coding_agent::tools::write::write_execute;
use tempfile::tempdir;

fn text(blocks: &[ContentBlock]) -> String {
    blocks.iter().filter_map(|b| match b { ContentBlock::Text { text, .. } => Some(text.clone()), _ => None }).collect::<Vec<_>>().join("\n")
}

#[tokio::test]
async fn writes_and_creates_parents() {
    let dir = tempdir().unwrap();
    let args = serde_json::json!({ "path": "sub/dir/out.txt", "content": "héllo" });
    let r = write_execute(dir.path(), args).await.unwrap();
    let written = std::fs::read_to_string(dir.path().join("sub/dir/out.txt")).unwrap();
    assert_eq!(written, "héllo");
    assert!(text(&r).contains("Successfully wrote 6 bytes to sub/dir/out.txt")); // 'é' = 2 bytes
}

#[tokio::test]
async fn missing_args_error() {
    let dir = tempdir().unwrap();
    let err = write_execute(dir.path(), serde_json::json!({ "path": "x" })).await.unwrap_err();
    assert!(err.contains("content"));
}
```

- [ ] **Step 2: Run** `cargo test -p pi-coding-agent --test tool_write` — Expected: FAIL.
- [ ] **Step 3: Implement**

```rust
use crate::tools::path::resolve_to_cwd;
use pi_agent_core::{AgentTool, ToolFn};
use pi_ai::types::ContentBlock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const DESCRIPTION: &str = "Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Automatically creates parent directories.";

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "Path to the file to write (relative or absolute)" },
            "content": { "type": "string", "description": "Content to write to the file" }
        },
        "required": ["path", "content"], "additionalProperties": false
    })
}

fn arg_str(args: &serde_json::Value, key: &str) -> Result<String, String> {
    args.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
        .ok_or_else(|| format!("write: missing or non-string '{key}' argument"))
}

pub async fn write_execute(cwd: &Path, args: serde_json::Value) -> Result<Vec<ContentBlock>, String> {
    let path = arg_str(&args, "path")?;
    let content = arg_str(&args, "content")?;
    let abs = resolve_to_cwd(&path, cwd);
    if let Some(parent) = abs.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| format!("write: failed to create {}: {e}", parent.display()))?;
    }
    tokio::fs::write(&abs, &content).await.map_err(|e| format!("write: failed to write {}: {e}", abs.display()))?;
    let n = content.as_bytes().len();
    Ok(vec![ContentBlock::Text { text: format!("Successfully wrote {n} bytes to {path}"), text_signature: None }])
}

pub fn write_tool(cwd: PathBuf) -> AgentTool {
    let execute: ToolFn = Arc::new(move |args| {
        let cwd = cwd.clone();
        Box::pin(async move { write_execute(&cwd, args).await })
    });
    AgentTool { name: "write".into(), description: DESCRIPTION.into(), parameters: schema(), execute }
}
```

(Requires `pub mod write;` already in `tools/mod.rs` and `tools` public in lib.rs.)

- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `git add -A && git commit -m "feat(pi-coding-agent): add write tool"`

---

## Task 5: `read.rs`

**Files:** Create `src/tools/read.rs`; Create `tests/tool_read.rs`.

- [ ] **Step 1: Failing integration tests** (`tests/tool_read.rs`) — cover full read, offset/limit, offset OOB, line truncation continuation, offset≤0, image note.

```rust
use pi_ai::types::ContentBlock;
use pi_coding_agent::tools::read::read_execute;
use tempfile::tempdir;

fn text(b: &[ContentBlock]) -> String {
    b.iter().filter_map(|x| match x { ContentBlock::Text { text, .. } => Some(text.clone()), _ => None }).collect::<Vec<_>>().join("\n")
}
async fn write(dir: &std::path::Path, name: &str, body: &str) { std::fs::write(dir.join(name), body).unwrap(); }

#[tokio::test]
async fn reads_full_file() {
    let d = tempdir().unwrap(); write(d.path(), "a.txt", "l1\nl2\nl3").await;
    let r = read_execute(d.path(), serde_json::json!({"path":"a.txt"})).await.unwrap();
    assert_eq!(text(&r), "l1\nl2\nl3");
}
#[tokio::test]
async fn offset_and_limit() {
    let d = tempdir().unwrap();
    let body = (1..=5).map(|i| format!("line{i}")).collect::<Vec<_>>().join("\n");
    write(d.path(), "a.txt", &body).await;
    let r = read_execute(d.path(), serde_json::json!({"path":"a.txt","offset":2,"limit":2})).await.unwrap();
    let t = text(&r);
    assert!(t.starts_with("line2\nline3"));
    assert!(t.contains("more lines in file. Use offset=4 to continue."));
}
#[tokio::test]
async fn offset_out_of_bounds() {
    let d = tempdir().unwrap(); write(d.path(), "a.txt", "x\ny").await;
    let e = read_execute(d.path(), serde_json::json!({"path":"a.txt","offset":99})).await.unwrap_err();
    assert_eq!(e, "Offset 99 is beyond end of file (2 lines total)");
}
#[tokio::test]
async fn offset_zero_reads_from_start() {
    let d = tempdir().unwrap(); write(d.path(), "a.txt", "x\ny").await;
    let r = read_execute(d.path(), serde_json::json!({"path":"a.txt","offset":0})).await.unwrap();
    assert_eq!(text(&r), "x\ny");
}
#[tokio::test]
async fn missing_file_errors() {
    let d = tempdir().unwrap();
    assert!(read_execute(d.path(), serde_json::json!({"path":"nope.txt"})).await.is_err());
}
#[tokio::test]
async fn missing_image_file_errors() {
    let d = tempdir().unwrap();
    assert!(read_execute(d.path(), serde_json::json!({"path":"missing.png"})).await.is_err());
}
#[tokio::test]
async fn image_returns_note() {
    let d = tempdir().unwrap(); write(d.path(), "a.png", "not really").await;
    let r = read_execute(d.path(), serde_json::json!({"path":"a.png"})).await.unwrap();
    assert_eq!(text(&r), "Read image file [image/png]\n[Image content is not supported in headless mode yet; omitted.]");
}
#[tokio::test]
async fn limit_larger_than_remaining_clips_to_eof() {
    let d = tempdir().unwrap(); write(d.path(), "a.txt", "x\ny").await;
    let r = read_execute(d.path(), serde_json::json!({"path":"a.txt","offset":2,"limit":99})).await.unwrap();
    assert_eq!(text(&r), "y");
}
#[tokio::test]
async fn line_truncation_has_continuation() {
    let d = tempdir().unwrap();
    let body = (1..=2005).map(|i| format!("line{i}")).collect::<Vec<_>>().join("\n");
    write(d.path(), "a.txt", &body).await;
    let r = read_execute(d.path(), serde_json::json!({"path":"a.txt"})).await.unwrap();
    let t = text(&r);
    assert!(t.contains("line2000"));
    assert!(t.contains("[Showing lines 1-2000 of 2005. Use offset=2001 to continue.]"));
}
#[tokio::test]
async fn byte_truncation_has_continuation() {
    let d = tempdir().unwrap();
    let body = (1..=60).map(|i| format!("{i}:{}", "x".repeat(1000))).collect::<Vec<_>>().join("\n");
    write(d.path(), "a.txt", &body).await;
    let r = read_execute(d.path(), serde_json::json!({"path":"a.txt"})).await.unwrap();
    let t = text(&r);
    assert!(t.contains("(50.0KB limit). Use offset="), "{t}");
}
#[tokio::test]
async fn first_line_exceeds_limit_has_bash_hint() {
    let d = tempdir().unwrap();
    write(d.path(), "a.txt", &"x".repeat(51201)).await;
    let r = read_execute(d.path(), serde_json::json!({"path":"a.txt"})).await.unwrap();
    assert_eq!(text(&r), "[Line 1 is 50.0KB, exceeds 50.0KB limit. Use bash: sed -n '1p' a.txt | head -c 51200]");
}
```

- [ ] **Step 2: Run** `cargo test -p pi-coding-agent --test tool_read` — Expected: FAIL.
- [ ] **Step 3: Implement** (uses `truncate_head`; strings per spec §12.1)

```rust
use crate::tools::path::resolve_to_cwd;
use crate::tools::truncate::{format_size, truncate_head, TruncationOptions, TruncatedBy, DEFAULT_MAX_BYTES};
use pi_agent_core::{AgentTool, ToolFn};
use pi_ai::types::ContentBlock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const DESCRIPTION: &str = "Read the contents of a text file. Output is truncated to 2000 lines or 50KB (whichever is hit first). Use offset/limit for large files; continue with offset until complete. Image files are not read in this mode.";
fn image_mime(path: &Path) -> Option<&'static str> {
    let ext = path.extension().and_then(|e| e.to_str())?.to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type":"object",
        "properties":{
            "path":{"type":"string","description":"Path to the file to read (relative or absolute)"},
            "offset":{"type":"number","description":"Line number to start reading from (1-indexed)"},
            "limit":{"type":"number","description":"Maximum number of lines to read"}
        },
        "required":["path"]
    })
}

fn text_block(t: String) -> Vec<ContentBlock> { vec![ContentBlock::Text { text: t, text_signature: None }] }

pub async fn read_execute(cwd: &Path, args: serde_json::Value) -> Result<Vec<ContentBlock>, String> {
    let path = args.get("path").and_then(|v| v.as_str()).ok_or("read: missing or non-string 'path' argument")?.to_string();
    let offset = args.get("offset").and_then(|v| v.as_u64()).map(|n| n as usize);
    let limit = args.get("limit").and_then(|v| v.as_u64()).map(|n| n as usize);
    let abs = resolve_to_cwd(&path, cwd);

    let _readable = tokio::fs::File::open(&abs).await.map_err(|e| format!("read: cannot read {}: {e}", abs.display()))?;
    if let Some(mime) = image_mime(&abs) {
        return Ok(text_block(format!("Read image file [{mime}]\n[Image content is not supported in headless mode yet; omitted.]")));
    }

    let raw = tokio::fs::read(&abs).await.map_err(|e| format!("read: cannot read {}: {e}", abs.display()))?;
    let content = String::from_utf8_lossy(&raw).into_owned();
    let all: Vec<&str> = content.split('\n').collect();
    let total = all.len();

    let start = offset.unwrap_or(1).saturating_sub(1);
    let start_display = start + 1;
    if start >= all.len() {
        return Err(format!("Offset {} is beyond end of file ({} lines total)", offset.unwrap_or(1), total));
    }

    let (selected, user_limited): (String, Option<usize>) = match limit {
        Some(l) => {
            let end = (start + l).min(all.len());
            (all[start..end].join("\n"), Some(end - start))
        }
        None => (all[start..].join("\n"), None),
    };

    let tr = truncate_head(&selected, &TruncationOptions::default());
    let out = if tr.first_line_exceeds_limit {
        let first_line_bytes = all[start].len();
        format!("[Line {start_display} is {}, exceeds {} limit. Use bash: sed -n '{start_display}p' {path} | head -c {DEFAULT_MAX_BYTES}]",
            format_size(first_line_bytes), format_size(DEFAULT_MAX_BYTES))
    } else if tr.truncated {
        let end_display = start_display + tr.output_lines - 1;
        let next = end_display + 1;
        if tr.truncated_by == TruncatedBy::Lines {
            format!("{}\n\n[Showing lines {start_display}-{end_display} of {total}. Use offset={next} to continue.]", tr.content)
        } else {
            format!("{}\n\n[Showing lines {start_display}-{end_display} of {total} ({} limit). Use offset={next} to continue.]", tr.content, format_size(DEFAULT_MAX_BYTES))
        }
    } else if let Some(ul) = user_limited {
        if start + ul < all.len() {
            let remaining = all.len() - (start + ul);
            let next = start + ul + 1;
            format!("{}\n\n[{remaining} more lines in file. Use offset={next} to continue.]", tr.content)
        } else { tr.content }
    } else { tr.content };

    Ok(text_block(out))
}

pub fn read_tool(cwd: PathBuf) -> AgentTool {
    let execute: ToolFn = Arc::new(move |args| { let cwd = cwd.clone(); Box::pin(async move { read_execute(&cwd, args).await }) });
    AgentTool { name: "read".into(), description: DESCRIPTION.into(), parameters: schema(), execute }
}
```

- [ ] **Step 4: Run** — Expected: PASS.
- [ ] **Step 5: Commit** `git add -A && git commit -m "feat(pi-coding-agent): add read tool"`

---

## Task 6: `edit.rs` (exact + fuzzy match, port edit-diff.ts)

**Files:** Create `src/tools/edit.rs`; Create `tests/tool_edit.rs`.

- [ ] **Step 1: Failing tests** — exact, multi-edit, fuzzy (smart quote), not-found, duplicate, overlap, empty, no-change, CRLF preserve, legacy args.

```rust
use pi_ai::types::ContentBlock;
use pi_coding_agent::tools::edit::edit_execute;
use tempfile::tempdir;
fn text(b: &[ContentBlock]) -> String { b.iter().filter_map(|x| match x { ContentBlock::Text{text,..}=>Some(text.clone()),_=>None}).collect::<Vec<_>>().join("\n") }

#[tokio::test] async fn exact_replace() {
    let d=tempdir().unwrap(); let p=d.path().join("f.txt"); std::fs::write(&p,"hello world").unwrap();
    let r=edit_execute(d.path(), serde_json::json!({"path":"f.txt","edits":[{"oldText":"world","newText":"rust"}]})).await.unwrap();
    assert_eq!(std::fs::read_to_string(&p).unwrap(),"hello rust");
    assert!(text(&r).contains("Successfully replaced 1 block(s) in f.txt."));
}
#[tokio::test] async fn multi_edit_success() {
    let d=tempdir().unwrap(); let p=d.path().join("f.txt"); std::fs::write(&p,"a\nb\nc").unwrap();
    edit_execute(d.path(), serde_json::json!({"path":"f.txt","edits":[{"oldText":"a","newText":"A"},{"oldText":"c","newText":"C"}]})).await.unwrap();
    assert_eq!(std::fs::read_to_string(&p).unwrap(),"A\nb\nC");
}
#[tokio::test] async fn not_found_single() {
    let d=tempdir().unwrap(); std::fs::write(d.path().join("f.txt"),"abc").unwrap();
    let e=edit_execute(d.path(), serde_json::json!({"path":"f.txt","edits":[{"oldText":"xyz","newText":"q"}]})).await.unwrap_err();
    assert_eq!(e, "Could not find the exact text in f.txt. The old text must match exactly including all whitespace and newlines.");
}
#[tokio::test] async fn duplicate_single() {
    let d=tempdir().unwrap(); std::fs::write(d.path().join("f.txt"),"x x").unwrap();
    let e=edit_execute(d.path(), serde_json::json!({"path":"f.txt","edits":[{"oldText":"x","newText":"y"}]})).await.unwrap_err();
    assert_eq!(e, "Found 2 occurrences of the text in f.txt. The text must be unique. Please provide more context to make it unique.");
}
#[tokio::test] async fn overlap_errors() {
    let d=tempdir().unwrap(); std::fs::write(d.path().join("f.txt"),"abcdef").unwrap();
    let e=edit_execute(d.path(), serde_json::json!({"path":"f.txt","edits":[{"oldText":"abc","newText":"X"},{"oldText":"bcd","newText":"Y"}]})).await.unwrap_err();
    assert!(e.contains("overlap in f.txt"));
}
#[tokio::test] async fn empty_oldtext_single() {
    let d=tempdir().unwrap(); std::fs::write(d.path().join("f.txt"),"abc").unwrap();
    let e=edit_execute(d.path(), serde_json::json!({"path":"f.txt","edits":[{"oldText":"","newText":"q"}]})).await.unwrap_err();
    assert_eq!(e, "oldText must not be empty in f.txt.");
}
#[tokio::test] async fn empty_oldtext_multi() {
    let d=tempdir().unwrap(); std::fs::write(d.path().join("f.txt"),"abc").unwrap();
    let e=edit_execute(d.path(), serde_json::json!({"path":"f.txt","edits":[{"oldText":"a","newText":"A"},{"oldText":"","newText":"q"}]})).await.unwrap_err();
    assert_eq!(e, "edits[1].oldText must not be empty in f.txt.");
}
#[tokio::test] async fn no_change_errors() {
    let d=tempdir().unwrap(); std::fs::write(d.path().join("f.txt"),"abc").unwrap();
    let e=edit_execute(d.path(), serde_json::json!({"path":"f.txt","edits":[{"oldText":"abc","newText":"abc"}]})).await.unwrap_err();
    assert!(e.starts_with("No changes made to f.txt."));
}
#[tokio::test] async fn fuzzy_smart_quote() {
    let d=tempdir().unwrap(); let p=d.path().join("f.txt"); std::fs::write(&p,"say \u{2018}hi\u{2019} now").unwrap(); // curly quotes in file
    let r=edit_execute(d.path(), serde_json::json!({"path":"f.txt","edits":[{"oldText":"say 'hi' now","newText":"done"}]})).await; // straight quotes from model
    assert!(r.is_ok(), "fuzzy match should succeed: {r:?}");
    assert_eq!(std::fs::read_to_string(&p).unwrap(),"done");
}
#[tokio::test] async fn fuzzy_preserves_replacement_text() {
    let d=tempdir().unwrap(); let p=d.path().join("f.txt"); std::fs::write(&p,"say \u{2018}hi\u{2019} now").unwrap();
    edit_execute(d.path(), serde_json::json!({"path":"f.txt","edits":[{"oldText":"say 'hi' now","newText":"done \u{2013} now"}]})).await.unwrap();
    assert_eq!(std::fs::read_to_string(&p).unwrap(),"done \u{2013} now");
}
#[tokio::test] async fn crlf_preserved() {
    let d=tempdir().unwrap(); let p=d.path().join("f.txt"); std::fs::write(&p,"a\r\nb\r\nc").unwrap();
    edit_execute(d.path(), serde_json::json!({"path":"f.txt","edits":[{"oldText":"b","newText":"B"}]})).await.unwrap();
    assert_eq!(std::fs::read_to_string(&p).unwrap(),"a\r\nB\r\nc");
}
#[tokio::test] async fn bom_preserved() {
    let d=tempdir().unwrap(); let p=d.path().join("f.txt"); std::fs::write(&p,"\u{feff}a\nb").unwrap();
    edit_execute(d.path(), serde_json::json!({"path":"f.txt","edits":[{"oldText":"b","newText":"B"}]})).await.unwrap();
    assert_eq!(std::fs::read_to_string(&p).unwrap(),"\u{feff}a\nB");
}
#[tokio::test] async fn legacy_single_edit_args() {
    let d=tempdir().unwrap(); let p=d.path().join("f.txt"); std::fs::write(&p,"abc").unwrap();
    edit_execute(d.path(), serde_json::json!({"path":"f.txt","oldText":"abc","newText":"xyz"})).await.unwrap();
    assert_eq!(std::fs::read_to_string(&p).unwrap(),"xyz");
}
```

- [ ] **Step 2: Run** `cargo test -p pi-coding-agent --test tool_edit` — Expected: FAIL.
- [ ] **Step 3: Implement** (port `edit-diff.ts`: line endings, BOM, fuzzy, apply; errors per spec §12.1; **no diff/patch**)

```rust
use crate::tools::path::resolve_to_cwd;
use pi_agent_core::{AgentTool, ToolFn};
use pi_ai::types::ContentBlock;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use unicode_normalization::UnicodeNormalization;

const DESCRIPTION: &str = "Edit a single file using exact text replacement. Every edits[].oldText must match a unique, non-overlapping region of the original file. Merge nearby changes into one edit; do not include large unchanged regions.";

struct Edit { old_text: String, new_text: String }

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type":"object",
        "properties":{
            "path":{"type":"string"},
            "edits":{"type":"array","items":{"type":"object",
                "properties":{"oldText":{"type":"string"},"newText":{"type":"string"}},
                "required":["oldText","newText"],"additionalProperties":false}}
        },
        "required":["path","edits"],"additionalProperties":false
    })
}

fn detect_crlf(s: &str) -> bool {
    // first line ending wins (TS detectLineEnding)
    match (s.find("\r\n"), s.find('\n')) {
        (Some(rn), Some(n)) => rn == n - 1 && rn <= s.find('\n').unwrap(),
        _ => s.contains("\r\n"),
    }
}
fn normalize_to_lf(s: &str) -> String { s.replace("\r\n", "\n").replace('\r', "\n") }
fn restore_crlf(s: &str, crlf: bool) -> String { if crlf { s.replace('\n', "\r\n") } else { s.to_string() } }
fn strip_bom(s: &str) -> (&str, &str) { if let Some(r) = s.strip_prefix('\u{feff}') { ("\u{feff}", r) } else { ("", s) } }

fn normalize_for_fuzzy(text: &str) -> String {
    let nfkc: String = text.nfkc().collect();
    let trimmed: String = nfkc.split('\n').map(|l| l.trim_end()).collect::<Vec<_>>().join("\n");
    trimmed.chars().map(|c| match c {
        '\u{2018}'|'\u{2019}'|'\u{201A}'|'\u{201B}' => '\'',
        '\u{201C}'|'\u{201D}'|'\u{201E}'|'\u{201F}' => '"',
        '\u{2010}'|'\u{2011}'|'\u{2012}'|'\u{2013}'|'\u{2014}'|'\u{2015}'|'\u{2212}' => '-',
        '\u{00A0}'|'\u{2002}'..='\u{200A}'|'\u{202F}'|'\u{205F}'|'\u{3000}' => ' ',
        other => other,
    }).collect()
}

fn count_occurrences(content: &str, old: &str) -> usize {
    let fc = normalize_for_fuzzy(content); let fo = normalize_for_fuzzy(old);
    if fo.is_empty() { return 0; }
    fc.matches(&fo).count()
}

// Returns (base_content, new_content) or Err(message).
fn apply_edits(normalized: &str, edits: &[Edit], path: &str) -> Result<(String, String), String> {
    let total = edits.len();
    let norm: Vec<Edit> = edits.iter().map(|e| Edit { old_text: normalize_to_lf(&e.old_text), new_text: normalize_to_lf(&e.new_text) }).collect();
    for (i, e) in norm.iter().enumerate() {
        if e.old_text.is_empty() {
            return Err(if total == 1 { format!("oldText must not be empty in {path}.") } else { format!("edits[{i}].oldText must not be empty in {path}.") });
        }
    }
    let any_fuzzy = norm.iter().any(|e| !normalized.contains(&e.old_text) && normalize_for_fuzzy(normalized).contains(&normalize_for_fuzzy(&e.old_text)));
    let base = if any_fuzzy { normalize_for_fuzzy(normalized) } else { normalized.to_string() };

    let mut matched: Vec<(usize, usize, usize, String)> = Vec::new(); // (edit_index, match_index, match_len, new_text)
    for (i, e) in norm.iter().enumerate() {
        let (idx, len, new) = if any_fuzzy {
            let fo = normalize_for_fuzzy(&e.old_text);
            match base.find(&fo) { Some(ix) => (ix, fo.len(), e.new_text.clone()), None => return Err(not_found(path, i, total)) }
        } else {
            match base.find(&e.old_text) { Some(ix) => (ix, e.old_text.len(), e.new_text.clone()), None => return Err(not_found(path, i, total)) }
        };
        let occ = count_occurrences(&base, &e.old_text);
        if occ > 1 { return Err(duplicate(path, i, total, occ)); }
        matched.push((i, idx, len, new));
    }
    matched.sort_by_key(|m| m.1);
    for w in matched.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if a.1 + a.2 > b.1 {
            return Err(format!("edits[{}] and edits[{}] overlap in {path}. Merge them into one edit or target disjoint regions.", a.0, b.0));
        }
    }
    let mut new_content = base.clone();
    for (_, idx, len, new) in matched.iter().rev() {
        new_content.replace_range(*idx..*idx + *len, new);
    }
    if base == new_content {
        return Err(if total == 1 {
            format!("No changes made to {path}. The replacement produced identical content. This might indicate an issue with special characters or the text not existing as expected.")
        } else { format!("No changes made to {path}. The replacements produced identical content.") });
    }
    Ok((base, new_content))
}

fn not_found(path: &str, i: usize, total: usize) -> String {
    if total == 1 { format!("Could not find the exact text in {path}. The old text must match exactly including all whitespace and newlines.") }
    else { format!("Could not find edits[{i}] in {path}. The oldText must match exactly including all whitespace and newlines.") }
}
fn duplicate(path: &str, i: usize, total: usize, n: usize) -> String {
    if total == 1 { format!("Found {n} occurrences of the text in {path}. The text must be unique. Please provide more context to make it unique.") }
    else { format!("Found {n} occurrences of edits[{i}] in {path}. Each oldText must be unique. Please provide more context to make it unique.") }
}

fn parse_edits(args: &serde_json::Value) -> Result<Vec<Edit>, String> {
    // edits as JSON string -> parse
    let mut edits_val = args.get("edits").cloned().unwrap_or(serde_json::Value::Null);
    if let Some(s) = edits_val.as_str() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) { edits_val = v; }
    }
    let mut out: Vec<Edit> = Vec::new();
    if let Some(arr) = edits_val.as_array() {
        for e in arr {
            let o = e.get("oldText").and_then(|v| v.as_str());
            let n = e.get("newText").and_then(|v| v.as_str());
            if let (Some(o), Some(n)) = (o, n) { out.push(Edit { old_text: o.into(), new_text: n.into() }); }
        }
    }
    // legacy single edit appended
    if let (Some(o), Some(n)) = (args.get("oldText").and_then(|v| v.as_str()), args.get("newText").and_then(|v| v.as_str())) {
        out.push(Edit { old_text: o.into(), new_text: n.into() });
    }
    if out.is_empty() { return Err("Edit tool input is invalid. edits must contain at least one replacement.".into()); }
    Ok(out)
}

pub async fn edit_execute(cwd: &Path, args: serde_json::Value) -> Result<Vec<ContentBlock>, String> {
    let path = args.get("path").and_then(|v| v.as_str()).ok_or("edit: missing or non-string 'path' argument")?.to_string();
    let edits = parse_edits(&args)?;
    let abs = resolve_to_cwd(&path, cwd);
    let raw = tokio::fs::read(&abs).await.map_err(|e| format!("Could not edit file: {path}. {e}."))?;
    let content = String::from_utf8_lossy(&raw).into_owned();
    let (bom, body) = strip_bom(&content);
    let crlf = detect_crlf(body);
    let normalized = normalize_to_lf(body);
    let (_base, new_content) = apply_edits(&normalized, &edits, &path)?;
    let final_content = format!("{bom}{}", restore_crlf(&new_content, crlf));
    tokio::fs::write(&abs, final_content).await.map_err(|e| format!("edit: failed to write {}: {e}", abs.display()))?;
    Ok(vec![ContentBlock::Text { text: format!("Successfully replaced {} block(s) in {path}.", edits.len()), text_signature: None }])
}

pub fn edit_tool(cwd: PathBuf) -> AgentTool {
    let execute: ToolFn = Arc::new(move |args| { let cwd = cwd.clone(); Box::pin(async move { edit_execute(&cwd, args).await }) });
    AgentTool { name: "edit".into(), description: DESCRIPTION.into(), parameters: schema(), execute }
}
```

- [ ] **Step 4: Run** — Expected: PASS. (If fuzzy index/len math against original vs normalized causes off-by issues, adjust to match TS: when any fuzzy, all matching+replacement happens in `base` = fuzzy-normalized content, which the code above already does.)
- [ ] **Step 5: Commit** `git add -A && git commit -m "feat(pi-coding-agent): add edit tool with fuzzy matching"`

---

## Task 7: `bash.rs`

**Files:** Create `src/tools/bash.rs`; Create `tests/tool_bash.rs`.

- [ ] **Step 1: Failing tests** — stdout, stderr, exit code, timeout, missing cwd.

```rust
use pi_ai::types::ContentBlock;
use pi_coding_agent::tools::bash::bash_execute;
use tempfile::tempdir;
fn text(b: &[ContentBlock]) -> String { b.iter().filter_map(|x| match x { ContentBlock::Text{text,..}=>Some(text.clone()),_=>None}).collect::<Vec<_>>().join("\n") }

#[tokio::test] async fn captures_stdout() {
    let d=tempdir().unwrap();
    let r=bash_execute(d.path(), serde_json::json!({"command":"echo hello"})).await.unwrap();
    assert!(text(&r).contains("hello"));
}
#[tokio::test] async fn captures_stderr() {
    let d=tempdir().unwrap();
    let r=bash_execute(d.path(), serde_json::json!({"command":"echo oops 1>&2"})).await.unwrap();
    assert!(text(&r).contains("oops"));
}
#[tokio::test] async fn keeps_stdout_stderr_arrival_order_for_chunks() {
    let d=tempdir().unwrap();
    let r=bash_execute(d.path(), serde_json::json!({"command":"printf 'out1\\n'; sleep 0.05; printf 'err1\\n' 1>&2; sleep 0.05; printf 'out2\\n'"})).await.unwrap();
    let t=text(&r);
    let out1=t.find("out1").unwrap();
    let err1=t.find("err1").unwrap();
    let out2=t.find("out2").unwrap();
    assert!(out1 < err1 && err1 < out2, "{t}");
}
#[tokio::test] async fn nonzero_exit_is_error() {
    let d=tempdir().unwrap();
    let e=bash_execute(d.path(), serde_json::json!({"command":"echo bad; exit 3"})).await.unwrap_err();
    assert!(e.contains("bad")); assert!(e.contains("Command exited with code 3"));
}
#[tokio::test] async fn timeout_errors() {
    let d=tempdir().unwrap();
    let e=bash_execute(d.path(), serde_json::json!({"command":"sleep 5","timeout":1})).await.unwrap_err();
    assert!(e.contains("Command timed out after 1 seconds"));
}
#[tokio::test] async fn truncates_tail_output() {
    let d=tempdir().unwrap();
    let r=bash_execute(d.path(), serde_json::json!({"command":"seq 1 2005"})).await.unwrap();
    let t=text(&r);
    assert!(t.contains("2005"));
    assert!(t.contains("[Output truncated: showing last 2000 of 2005 lines (50KB/2000-line limit).]"), "{t}");
}
#[tokio::test] async fn missing_cwd_errors() {
    let e=bash_execute(std::path::Path::new("/no/such/dir/xyz"), serde_json::json!({"command":"echo hi"})).await.unwrap_err();
    assert!(e.contains("Working directory does not exist"));
}
```

- [ ] **Step 2: Run** `cargo test -p pi-coding-agent --test tool_bash` — Expected: FAIL.
- [ ] **Step 3: Implement** (merge stdout+stderr; `tokio::time::timeout`; `truncate_tail`)

```rust
use crate::tools::truncate::{truncate_tail, TruncationOptions};
use pi_agent_core::{AgentTool, ToolFn};
use pi_ai::types::ContentBlock;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncReadExt;

const DESCRIPTION: &str = "Execute a bash command in the working directory. Returns merged stdout and stderr. Output is truncated to the last 2000 lines or 50KB (whichever is hit first). Optionally provide a timeout in seconds.";

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type":"object",
        "properties":{
            "command":{"type":"string","description":"Bash command to execute"},
            "timeout":{"type":"number","description":"Timeout in seconds (optional)"}
        },
        "required":["command"]
    })
}

pub async fn bash_execute(cwd: &Path, args: serde_json::Value) -> Result<Vec<ContentBlock>, String> {
    let command = args.get("command").and_then(|v| v.as_str()).ok_or("bash: missing or non-string 'command' argument")?.to_string();
    let timeout = args.get("timeout").and_then(|v| v.as_u64()).filter(|secs| *secs > 0);
    let workdir = cwd.to_path_buf();
    if !tokio::fs::try_exists(&workdir).await.unwrap_or(false) {
        return Err(format!("Working directory does not exist: {}\nCannot execute bash commands.", workdir.display()));
    }

    let mut cmd = tokio::process::Command::new("bash");
    cmd.arg("-c").arg(&command).current_dir(&workdir)
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("bash: failed to spawn: {e}"))?;

    let mut stdout = child.stdout.take().ok_or_else(|| "bash: failed to capture stdout".to_string())?;
    let mut stderr = child.stderr.take().ok_or_else(|| "bash: failed to capture stderr".to_string())?;
    let mut buf: Vec<u8> = Vec::new();

    let collect = async {
        let mut merged = Vec::new();
        let mut stdout_open = true;
        let mut stderr_open = true;
        let mut stdout_buf = vec![0u8; 8192];
        let mut stderr_buf = vec![0u8; 8192];
        while stdout_open || stderr_open {
            tokio::select! {
                read = stdout.read(&mut stdout_buf), if stdout_open => {
                    match read {
                        Ok(0) => stdout_open = false,
                        Ok(n) => merged.extend_from_slice(&stdout_buf[..n]),
                        Err(_) => stdout_open = false,
                    }
                }
                read = stderr.read(&mut stderr_buf), if stderr_open => {
                    match read {
                        Ok(0) => stderr_open = false,
                        Ok(n) => merged.extend_from_slice(&stderr_buf[..n]),
                        Err(_) => stderr_open = false,
                    }
                }
            }
        }
        merged
    };

    let status_res = if let Some(secs) = timeout {
        match tokio::time::timeout(std::time::Duration::from_secs(secs), async {
            buf = collect.await;
            child.wait().await
        }).await {
            Ok(s) => s.map_err(|e| format!("bash: wait failed: {e}")).map(Some),
            Err(_) => { let _ = child.kill().await; Ok(None) } // timed out
        }
    } else {
        buf = collect.await;
        child.wait().await.map_err(|e| format!("bash: wait failed: {e}")).map(Some)
    };

    let merged = String::from_utf8_lossy(&buf).into_owned();
    let tr = truncate_tail(&merged, &TruncationOptions::default());
    let mut text = tr.content.clone();
    if tr.truncated {
        text = format!("{text}\n\n[Output truncated: showing last {} of {} lines (50KB/2000-line limit).]", tr.output_lines, tr.total_lines);
    }
    let success_text = if text.is_empty() && !tr.truncated { "(no output)".to_string() } else { text.clone() };

    match status_res? {
        None => Err(format!("{}{}Command timed out after {} seconds", text, if text.is_empty() {""} else {"\n\n"}, timeout.unwrap())),
        Some(status) => {
            let code = status.code();
            match code {
                Some(0) => Ok(vec![ContentBlock::Text { text: success_text, text_signature: None }]),
                Some(c) => Err(format!("{}{}Command exited with code {c}", success_text, if success_text.is_empty() {""} else {"\n\n"})),
                None => Err(format!("{}{}Command terminated by signal", success_text, if success_text.is_empty() {""} else {"\n\n"})),
            }
        }
    }
}

pub fn bash_tool(cwd: PathBuf) -> AgentTool {
    let execute: ToolFn = Arc::new(move |args| { let cwd = cwd.clone(); Box::pin(async move { bash_execute(&cwd, args).await }) });
    AgentTool { name: "bash".into(), description: DESCRIPTION.into(), parameters: schema(), execute }
}
```

> Note: on timeout the `collect` future is dropped, so `buf` may be empty; that's acceptable (the
> error still reports the timeout). If partial output on timeout matters later, refactor to read
> into a shared buffer via a spawned task. Keep simple for M1.

- [ ] **Step 4: Run** — Expected: PASS. (timeout test needs `sleep`; available on Linux/macOS CI.)
- [ ] **Step 5: Commit** `git add -A && git commit -m "feat(pi-coding-agent): add bash tool"`

---

## Task 8: Wire `builtin_tools` into `run_cli`

**Files:** Modify `src/lib.rs`.

- [ ] **Step 1: Failing unit test** (add to `src/lib.rs`, bottom of file) — this verifies the default `run_cli` path gets built-in tools without making a live provider call.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cli_options_include_builtin_tools() {
        let options = default_cli_options(std::path::PathBuf::from("."));
        let names: Vec<_> = options.tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["read", "write", "edit", "bash"]);
        assert!(options.register_builtins);
        assert!(options.model_override.is_none());
    }
}
```

- [ ] **Step 2: Run** `cargo test -p pi-coding-agent --lib default_cli_options_include_builtin_tools` — Expected: FAIL (`default_cli_options` is not defined).

- [ ] **Step 3: Add helper + modify `run_cli`** in `src/lib.rs`:

```rust
fn default_cli_options(cwd: std::path::PathBuf) -> CliRunOptions {
    CliRunOptions {
        model_override: None,
        tools: builtin_tools(cwd),
        register_builtins: true,
    }
}

pub async fn run_cli(args: impl IntoIterator<Item = String>) -> CliOutput {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    run_cli_with_options(args, default_cli_options(cwd)).await
}
```

- [ ] **Step 4: Run** `cargo test -p pi-coding-agent --lib default_cli_options_include_builtin_tools` — Expected: PASS.

- [ ] **Step 5: Run** `cargo test -p pi-coding-agent` — Expected: PASS (existing `run_cli_with_options` tests still use explicit empty tools).

- [ ] **Step 6: Commit** `git add -A && git commit -m "feat(pi-coding-agent): enable built-in tools by default in run_cli"`

---

## Task 9: End-to-end tool loop test (recording faux provider)

**Files:** `tests/tools_e2e.rs`.

- [ ] **Step 1: Write e2e tests** — success (`read`) + error (`read` missing file), driven through `run_print_mode` with a local provider that records the context of each model call.

```rust
use futures::stream;
use pi_ai::providers::faux::{FauxCall, FauxResponse, FauxToolCall};
use pi_ai::registry::{self, ApiProvider};
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Message, Model, StopReason,
    StreamOptions,
};
use pi_ai::EventStream;
use pi_coding_agent::{builtin_tools, PrintModeOptions, run_print_mode};
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

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

fn read_call(path: &str) -> FauxResponse {
    FauxResponse {
        text_deltas: vec![],
        thinking_deltas: vec![],
        tool_calls: vec![FauxToolCall {
            id: "tool_1".into(),
            name: "read".into(),
            deltas: vec![format!(r#"{{"path":"{path}"}}"#)],
            final_arguments: serde_json::json!({ "path": path }),
        }],
    }
}

struct RecordingProvider {
    calls: Mutex<Vec<FauxCall>>,
    contexts: Arc<Mutex<Vec<Context>>>,
}

impl RecordingProvider {
    fn new(calls: Vec<FauxCall>, contexts: Arc<Mutex<Vec<Context>>>) -> Self {
        Self { calls: Mutex::new(calls), contexts }
    }
}

fn message_for_call(model_id: &str, call: &FauxCall) -> AssistantMessage {
    let mut message = AssistantMessage::empty("recording-faux", model_id);
    for response in &call.responses {
        if !response.text_deltas.is_empty() {
            message.content.push(ContentBlock::Text {
                text: response.text_deltas.join(""),
                text_signature: None,
            });
        }
        for tool_call in &response.tool_calls {
            message.content.push(ContentBlock::ToolCall {
                id: tool_call.id.clone(),
                name: tool_call.name.clone(),
                arguments: tool_call.final_arguments.clone(),
                thought_signature: None,
            });
        }
    }
    message.stop_reason = call.stop_reason.clone();
    message
}

impl ApiProvider for RecordingProvider {
    fn stream(&self, model: &Model, ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        self.contexts.lock().unwrap().push(ctx);
        let call = self.calls.lock().unwrap().remove(0);
        let message = message_for_call(&model.id, &call);
        Box::pin(stream::iter(vec![AssistantMessageEvent::Done {
            reason: call.stop_reason,
            message,
        }]))
    }
}

async fn run_scripted_read(api: &str, path: &str, cwd: std::path::PathBuf, contexts: Arc<Mutex<Vec<Context>>>) -> String {
    registry::register(
        api,
        Arc::new(RecordingProvider::new(
            vec![
                FauxCall { responses: vec![read_call(path)], stop_reason: StopReason::ToolUse },
                FauxCall { responses: vec![text_response("done")], stop_reason: StopReason::Stop },
            ],
            contexts,
        )),
    );
    let out = run_print_mode(PrintModeOptions {
        prompt: "read it".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: 5,
        tools: builtin_tools(cwd),
        register_builtins: false,
    })
    .await
    .unwrap();
    registry::unregister(api);
    out
}

#[test]
fn builtin_tools_has_four() {
    let tools = pi_coding_agent::builtin_tools(std::path::PathBuf::from("."));
    let names: Vec<_> = tools.iter().map(|t| t.name.clone()).collect();
    assert_eq!(names, vec!["read","write","edit","bash"]);
}

#[tokio::test]
async fn read_builtin_tool_success_loop_completes() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("input.txt"), "hello").unwrap();
    let contexts = Arc::new(Mutex::new(Vec::new()));

    let out = run_scripted_read(
        "pi-coding-tools-e2e-success",
        "input.txt",
        dir.path().to_path_buf(),
        contexts.clone(),
    )
    .await;

    assert_eq!(out, "done");
    let contexts = contexts.lock().unwrap();
    assert_eq!(contexts.len(), 2);
}

#[tokio::test]
async fn read_builtin_tool_error_is_sent_back_to_model_and_loop_completes() {
    let dir = tempdir().unwrap();
    let contexts = Arc::new(Mutex::new(Vec::new()));

    let out = run_scripted_read(
        "pi-coding-tools-e2e-error",
        "missing.txt",
        dir.path().to_path_buf(),
        contexts.clone(),
    )
    .await;

    assert_eq!(out, "done");
    let contexts = contexts.lock().unwrap();
    let second_call = contexts.get(1).expect("second model call should include tool result");
    let tool_result = second_call.messages.iter().find_map(|message| match message {
        Message::ToolResult { tool_name, is_error, content, .. } if tool_name.as_deref() == Some("read") => {
            Some((is_error, content))
        }
        _ => None,
    }).expect("read tool result should be present in second call context");
    assert_eq!(*tool_result.0, Some(true));
    let text = tool_result.1.iter().filter_map(|block| match block {
        ContentBlock::Text { text, .. } => Some(text.as_str()),
        _ => None,
    }).collect::<Vec<_>>().join("\n");
    assert!(text.contains("read: cannot read"), "{text}");
}
```

- [ ] **Step 2: Run** `cargo test -p pi-coding-agent --test tools_e2e` — Expected: PASS. If it fails, inspect the recorded second-call `Context` first; the likely issue is tool-result propagation, not the read tool itself.
- [ ] **Step 3: Run** `cargo test -p pi-coding-agent --test print_mode` — Expected: PASS (existing faux-provider tests still green).
- [ ] **Step 4: Run** `cargo test -p pi-coding-agent --test tools_e2e` — Expected: PASS after any import/format cleanup.
- [ ] **Step 5: Commit** `git add -A && git commit -m "test(pi-coding-agent): end-to-end built-in tool loop via faux provider"`

---

## Task 10: Verify + polish

- [ ] **Step 1:** `cargo fmt` then `cargo fmt --check` — Expected: clean.
- [ ] **Step 2:** `cargo test --workspace` — Expected: all green.
- [ ] **Step 3:** `cargo check --workspace` — Expected: clean (address warnings in new files).
- [ ] **Step 4:** Quick manual smoke (optional, needs key): `cargo run -p pi-coding-agent -- -p "list files with ls via bash"` (skip if no key).
- [ ] **Step 5: Commit** any fmt/warning fixes `git add -A && git commit -m "chore(pi-coding-agent): fmt + clippy cleanups for tools"`

---

## Self-Review notes
- Spec §6.5 behaviors all map to Tasks 4–7; §12.1 exact strings are embedded in test assertions.
- §12.3 wiring → Task 8 with a real default-options unit test. §12.4 added tests → Tasks 5,6,9; bash interleave/truncation coverage → Task 7.
- Deferred per spec (no tasks, intentional): images binary, bash temp file, mutation queue, abort signal, TUI/diff.
- Type consistency: `xxx_execute(cwd: &Path, args: Value) -> Result<Vec<ContentBlock>, String>` and `xxx_tool(cwd: PathBuf) -> AgentTool` used uniformly; `TruncationOptions`/`TruncatedBy` names consistent across truncate/read/bash.
