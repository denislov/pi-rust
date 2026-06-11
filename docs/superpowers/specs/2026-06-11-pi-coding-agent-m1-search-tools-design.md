# Design: pi-coding-agent M1 search tools

- Date: 2026-06-11
- Status: Ready for implementation
- Scope: Complete the remaining M1 built-in coding tools in `pi-coding-agent`: `grep`, `find`, and `ls`.
- Depends on: current `read`, `write`, `edit`, `bash` tools; `builtin_tools`; `pi-agent-core::AgentTool`; `pi-coding-agent` print/json/rpc/interactive session runner paths.

## 1. Context

`pi-coding-agent` currently has four built-in tools:

- `read`
- `write`
- `edit`
- `bash`

The roadmap M1 target is seven built-in tools:

- `read`
- `write`
- `edit`
- `bash`
- `grep`
- `find`
- `ls`

The TypeScript reference implements the remaining three under:

- `pi/packages/coding-agent/src/core/tools/grep.ts`
- `pi/packages/coding-agent/src/core/tools/find.ts`
- `pi/packages/coding-agent/src/core/tools/ls.ts`

The TypeScript implementation delegates `grep` to `rg` and `find` to `fd` through the tools-manager download path. The Rust port should keep tests deterministic and offline, so this slice will implement the behavior directly in Rust rather than requiring external binaries or network hydration.

## 2. Goal

Add deterministic Rust implementations for `grep`, `find`, and `ls`, register them as built-in tools, and prove through unit and end-to-end tests that the agent loop can use all seven M1 tools.

## 3. Non-goals

This slice does not implement:

- TypeScript extension wrapper/render components for tools.
- Downloading or shelling out to `rg` or `fd`.
- Full byte-for-byte `.gitignore` parity with every `rg`/`fd` edge case.
- Remote filesystem operations.
- Tool cancellation plumbing beyond the current agent/tool future boundaries.
- Any new CLI flags for selecting or excluding built-in tools.

## 4. Shared requirements

All three tools must follow existing Rust tool conventions:

- Each tool lives in its own module under `crates/pi-coding-agent/src/tools/`.
- Each tool exposes an async `*_execute(cwd: &Path, args: serde_json::Value) -> Result<Vec<ContentBlock>, String>` function for deterministic tests.
- Each tool exposes a `*_tool(cwd: PathBuf) -> AgentTool` wrapper.
- Tool output is a single `ContentBlock::Text`.
- Tool failures return `Err(String)` so `pi-agent-core` records an error tool result and keeps the loop semantics consistent with existing tools.
- Paths use `tools::path::resolve_to_cwd`.
- Returned paths use `/` separators for stable cross-platform output.
- Output truncates with existing `tools::truncate::truncate_head` and the 50KB default byte limit.
- Tests use temporary directories and do not require model provider keys.

Traversal behavior:

- Include dotfiles in normal directory listings and search results.
- Do not descend into `.git` or `node_modules` directories by default.
- Do not follow symlinked directories, to avoid cycles.
- Sort output deterministically with case-insensitive ordering and path string as the final tie-breaker.

## 5. `ls` behavior

Schema:

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string" },
    "limit": { "type": "number" }
  }
}
```

Behavior:

- `path` defaults to `"."`.
- `limit` defaults to `500`; values below `1` are treated as `1`.
- If the path does not exist, return `Err("ls: path not found: <path>")`.
- If the path is not a directory, return `Err("ls: not a directory: <path>")`.
- Return direct child entries only.
- Include dotfiles.
- Append `/` to directory names.
- Skip entries that cannot be statted.
- If the directory is empty, return `(empty directory)`.
- If more entries exist than `limit`, append a notice:
  `[500 entries limit reached. Use limit=1000 for more]`
- If byte truncation occurs, append a 50KB notice.

## 6. `find` behavior

Schema:

```json
{
  "type": "object",
  "properties": {
    "pattern": { "type": "string" },
    "path": { "type": "string" },
    "limit": { "type": "number" }
  },
  "required": ["pattern"]
}
```

Behavior:

- `pattern` is a glob pattern such as `*.rs`, `**/*.json`, or `src/**/*.spec.ts`.
- `path` defaults to `"."`.
- `limit` defaults to `1000`; values below `1` are treated as `1`.
- If the path does not exist, return `Err("find: path not found: <path>")`.
- If `pattern` contains `/`, match it against the full path relative to the search root.
- If `pattern` does not contain `/`, match it against the basename.
- Return files and directories that match; suffix directories with `/`.
- Return paths relative to the search root, using `/` separators.
- If no match exists, return `No files found matching pattern`.
- If `limit` is reached, append:
  `[1000 results limit reached. Use limit=2000 for more, or refine pattern]`
- If byte truncation occurs, append a 50KB notice.

## 7. `grep` behavior

Schema:

```json
{
  "type": "object",
  "properties": {
    "pattern": { "type": "string" },
    "path": { "type": "string" },
    "glob": { "type": "string" },
    "ignoreCase": { "type": "boolean" },
    "literal": { "type": "boolean" },
    "context": { "type": "number" },
    "limit": { "type": "number" }
  },
  "required": ["pattern"]
}
```

Behavior:

- `pattern` is regex by default.
- `literal=true` treats `pattern` as a literal string.
- `ignoreCase=true` applies case-insensitive matching for regex and literal modes.
- `path` defaults to `"."` and may point to a file or directory.
- `glob` filters candidate files using the same matching rule as `find`.
- `context` defaults to `0`; values below `0` are treated as `0`.
- `limit` defaults to `100`; values below `1` are treated as `1`.
- Search text files using lossy UTF-8 decoding.
- Skip files that cannot be read.
- Truncate each displayed source line to `500` characters; append a notice when any line is truncated.
- For direct match output, use:
  `relative/path:line: text`
- For context lines, use:
  `relative/path-line- text`
- If no matches exist, return `No matches found`.
- If `limit` is reached, append:
  `[100 matches limit reached. Use limit=200 for more, or refine pattern]`
- If byte truncation occurs, append a 50KB notice.
- Invalid regex patterns return an error string beginning with `grep: invalid regex:`.

## 8. Registration

`crates/pi-coding-agent/src/tools/mod.rs::builtin_tools` must return tools in this order:

```text
read, write, edit, bash, grep, find, ls
```

This preserves existing tool order and appends the remaining M1 tools.

## 9. Acceptance tests

Focused tests:

- `cargo test -p pi-coding-agent --test tool_ls`
- `cargo test -p pi-coding-agent --test tool_find`
- `cargo test -p pi-coding-agent --test tool_grep`
- `cargo test -p pi-coding-agent --test tools_e2e`

Crate/workspace verification:

- `cargo fmt --check`
- `cargo test -p pi-coding-agent`
- `cargo test --workspace`
- `cargo check --workspace`
- `git diff --check`

## 10. Risks

- Exact `rg`/`fd` ignore semantics are broad. The implementation should use the Rust `ignore` crate where practical and explicitly cover `.git` / `node_modules` in tests.
- Regex support should not force shell execution. Prefer a Rust regex dependency over invoking external commands.
- Large directory trees can be expensive. The implementation must stop traversal once `limit` is reached where possible.
- `grep` can produce multiple matches per file; tests should cover limit behavior at match count level, not just file count level.
