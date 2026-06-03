# Design: Rust `pi-coding-agent` built-in tools (read / write / edit / bash)

- Date: 2026-06-04
- Status: Draft (pending review)
- Scope: M1 of the Rust port ROADMAP — give the print-mode CLI a built-in tool library.
- Depends on: `pi-ai`, `pi-agent-core` (both have working PoCs).

## 1. Context

The Rust `pi-coding-agent` print-mode PoC can send one prompt through `pi-agent-core` and
print the final assistant text, but it ships **no tools** — so the agent can talk but cannot
inspect files, run commands, or change code. The agent loop and the tool-injection seam already
exist: `pi_agent_core::Agent::add_tool`, and `PrintModeOptions.tools` / `CliRunOptions.tools`
already thread a `Vec<AgentTool>` into the run. What is missing is the tools themselves.

This design ports the four most important built-in tools from the TypeScript
`pi/packages/coding-agent/src/core/tools/` so a `pi -p "…"` invocation becomes a genuinely
useful headless coding agent: it can **read** files, run **bash** commands, **write** new
files, and **edit** existing ones. `grep` / `find` / `ls` are deferred to a follow-up spec
(bash already covers most search needs).

Behavioral reference (TS): `read.ts` (362), `bash.ts` (445), `write.ts` (267), `edit.ts` (437)
plus shared `truncate.ts` (276), `path-utils.ts` (118), `edit-diff.ts` (454). Roughly half of
each TS tool file is TUI rendering (`renderCall` / `renderResult`, syntax highlighting, diff
components); **none of that is ported** — the Rust `AgentTool` is render-free and the agent
only sees the tool's returned content blocks.

## 2. Goals & success criteria

Build a `tools` module in `pi-coding-agent` exposing four built-in tools and wire them into the
CLI by default.

Done when:

1. `cargo build -p pi-coding-agent` and the whole workspace build cleanly (Rust edition 2024).
2. `cargo fmt --check`, `cargo test -p pi-coding-agent`, and `cargo test --workspace` pass.
3. `read` / `write` / `edit` / `bash` are implemented as `AgentTool` factory functions and
   assembled by `builtin_tools(cwd)`.
4. `run_cli` enables the built-in tools by default; `run_cli_with_options` / `PrintModeOptions`
   keep an explicit-tools seam for deterministic tests.
5. The offline test suite passes with **no network access and no credentials**:
   - Per-tool unit tests over `tempfile` temp dirs:
     - read: full read, `offset`/`limit`, out-of-bounds offset error, line-limit and
       byte-limit truncation with continuation notice, first-line-exceeds-limit notice.
     - write: creates parent dirs, overwrites, returns byte count.
     - edit: exact match, fuzzy match, not-found / duplicate / empty-oldText / overlap /
       no-change errors, CRLF + BOM preservation, multiple disjoint edits in one call.
     - bash: stdout+stderr capture, non-zero exit code surfaced as error, timeout, tail
       truncation with continuation notice, missing-cwd error.
   - One end-to-end test through `run_print_mode` + the faux provider: a scripted tool-use
     turn invokes a built-in tool (e.g. `read` a temp file), the loop runs the tool, and the
     final assistant text is asserted.
6. Model-visible text (tool descriptions, truncation/continuation notices, error messages)
   matches the TS reference wording where the model relies on it to self-correct.

Non-goal: live model calls; the suite is offline via faux provider + temp-dir fixtures.

## 3. Non-goals (this increment)

- `grep` / `find` / `ls` (next tools spec).
- Image reading in `read` (text files only; image paths return a text note). Image *resize*
  and inline image content are deferred.
- `bash` full-output temp file (`OutputAccumulator`): on truncation we emit a notice only, no
  temp file is written. Streaming `onUpdate` progress is also out (print mode does not need it).
- TUI rendering, syntax highlighting, and diff/patch generation (`renderCall` / `renderResult`,
  `edit-diff` `generateDiffString` / `generateUnifiedPatch`).
- `file-mutation-queue` (tools execute sequentially in the agent loop; no concurrent writes).
- Mid-tool cancellation / abort signal passed into tools (bash self-limits via its `timeout`).
- macOS path variant fallbacks (NFD / curly-quote / AM-PM narrow-space).
- Changes to `pi-agent-core` (its `ToolFn` signature stays as-is).
- JSON / RPC / interactive modes; settings, auth, sessions, extensions.

## 4. Key decisions

- **Approach A — factory functions, zero `pi-agent-core` change.** Each tool is a function
  `xxx_tool(cwd) -> AgentTool` (mirroring TS `createXxxTool(cwd)`); the closure captures `cwd`,
  implements behavior, and returns `Result<Vec<ContentBlock>, String>`. Shared logic lives in
  small sibling modules. (Rejected: a `Tool` trait + adapter — extra indirection that collapses
  to the same closure for four tools; and extending `ToolFn` with a `CancellationToken` — larger
  blast radius across a published crate, deferred to a future increment.)
- **edit includes fuzzy matching** (NFKC + per-line trailing-whitespace + smart quote/dash/space
  normalization), matching TS, because it materially raises edit success rate.
- **read is text-only**; image paths return a short text note rather than image content.
- **bash self-limits via `timeout`**; no abort signal, no temp file for full output.
- **Built-in tools default-on**: `run_cli` assembles `builtin_tools(cwd)`; tests bypass via
  `run_cli_with_options` / `PrintModeOptions` with explicit tools.
- **Error wording matches TS** for model-visible messages.
- **`register_builtins` keeps its current meaning** (pi-ai *provider* registration), orthogonal
  to tools. Documented in code to avoid confusion; not renamed (avoids churn).

## 5. Scope

### In scope
- `tools/` module: `read.rs`, `write.rs`, `edit.rs`, `bash.rs`, shared `truncate.rs`, `path.rs`,
  and `mod.rs` with `builtin_tools(cwd) -> Vec<AgentTool>`.
- Default wiring of built-in tools into `run_cli`.
- Defensive argument parsing from `serde_json::Value` inside each tool closure, including edit's
  `prepareArguments` compatibility (legacy single `{oldText,newText}`, `edits` sent as a JSON
  string).
- Offline per-tool unit tests + one end-to-end print-mode test.

### Out of scope
See §3.

## 6. Architecture

### 6.1 Module layout
```
crates/pi-coding-agent/src/
  lib.rs            # add `pub mod tools;` + re-export builtin_tools
  tools/
    mod.rs          # builtin_tools(cwd) -> Vec<AgentTool>; re-exports
    read.rs         # read_tool(cwd)
    write.rs        # write_tool(cwd)
    edit.rs         # edit_tool(cwd)
    bash.rs         # bash_tool(cwd) (+ optional shell override)
    truncate.rs     # TruncationResult, truncate_head, truncate_tail, format_size
    path.rs         # resolve_to_cwd(path, cwd) with ~ expansion
  runtime.rs        # unchanged public API; see wiring below
  tests/
    tool_read.rs  tool_write.rs  tool_edit.rs  tool_bash.rs  tools_e2e.rs
```

### 6.2 Tool API shape
Each factory returns an `AgentTool`:
```rust
pub fn read_tool(cwd: PathBuf) -> AgentTool {
    AgentTool {
        name: "read".into(),
        description: READ_DESCRIPTION.into(),   // ported from TS, references 2000 lines / 50KB
        parameters: serde_json::json!({ /* JSON Schema mirroring TS typebox */ }),
        execute: Arc::new(move |args| {
            let cwd = cwd.clone();
            Box::pin(async move { read_execute(&cwd, args).await })
        }),
    }
}
```
- `parameters` are hand-written JSON Schema literals matching the TS typebox schemas.
- Each tool has a private `xxx_execute(cwd, args) -> Result<Vec<ContentBlock>, String>` that does
  the work; the closure is a thin wrapper. This keeps the logic plain-async-fn testable.
- Arguments are parsed defensively from `serde_json::Value`; a missing/ill-typed field returns
  `Err(...)` text (model-visible, loop continues, model self-corrects).

### 6.3 CLI wiring
- Add `builtin_tools(cwd: PathBuf) -> Vec<AgentTool>` returning `[read, write, edit, bash]`.
- `run_cli` resolves `cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))`,
  builds `CliRunOptions { model_override: None, tools: builtin_tools(cwd), register_builtins: true }`,
  and calls `run_cli_with_options`. (Production entry = tools on.)
- `run_cli_with_options` / `PrintModeOptions` are unchanged and remain the explicit-tools seam
  used by tests (no built-ins unless the caller passes them).
- `builtin_tools` is re-exported from the crate root for embedders.

### 6.4 Shared helpers
- `truncate.rs`: port `truncate.ts`. `DEFAULT_MAX_LINES = 2000`, `DEFAULT_MAX_BYTES = 50*1024`.
  `truncate_head` (file reads — keep first N) and `truncate_tail` (bash — keep last N, may keep a
  partial first line on the byte-limit edge case). `TruncationResult { content, truncated,
  truncated_by: Lines|Bytes|None, total_lines, total_bytes, output_lines, output_bytes,
  last_line_partial, first_line_exceeds_limit, max_lines, max_bytes }`. `format_size` (B/KB/MB).
  Byte counts are UTF-8 byte lengths.
- `path.rs`: `resolve_to_cwd(path, cwd)` — absolute path as-is; `~` / `~/...` expand to home
  (via `std::env::var("HOME")`, Windows `USERPROFILE`); otherwise `cwd.join(path)`. No macOS
  variant fallbacks.

### 6.5 Per-tool behavior

**read** — schema `{ path: string, offset?: number (1-indexed), limit?: number }`
1. `resolve_to_cwd`; error if not readable.
2. Image extension (`jpg/jpeg/png/gif/webp`) → return `[Text("Read image file [<mime>]\n[Image
   content is not supported in headless mode yet; omitted.]")]` (no binary read).
3. Else read UTF-8, split into lines. `offset` (1-indexed → 0-indexed); offset beyond EOF →
   `Err("Offset N is beyond end of file (M lines total)")`. Apply `limit` if given.
4. `truncate_head` on the selected slice:
   - `first_line_exceeds_limit` → `[Line K is <size>, exceeds <50KB> limit. Use bash: sed -n
     'Kp' <path> | head -c <50KB>]`.
   - truncated → content + `\n\n[Showing lines A-B of T. Use offset=B+1 to continue.]` (lines
     variant) or the bytes-limit variant.
   - user `limit` stopped early with more remaining → `[N more lines in file. Use offset=… to
     continue.]`.
5. Return `[Text(output)]`.

**write** — schema `{ path: string, content: string }`
1. `resolve_to_cwd`; create parent dirs recursively (`tokio::fs::create_dir_all`).
2. Write file (`tokio::fs::write`, UTF-8 overwrite).
3. Return `[Text("Successfully wrote <content.len()> bytes to <path>")]` (byte length of UTF-8).

**edit** — schema `{ path: string, edits: [{ oldText, newText }] }`
1. Normalize arguments (prepareArguments): if `edits` is a JSON string, parse it; if legacy
   top-level `oldText`/`newText` present, fold into one edit. Empty `edits` → invalid-input error.
2. `resolve_to_cwd`; access (read+write) → `Err("Could not edit file: <path>. <reason>.")`.
3. Read file, `strip_bom`, detect line ending (CRLF vs LF), normalize to LF.
4. `apply_edits(normalized, edits, path)` (port of `applyEditsToNormalizedContent`):
   - normalize each edit's text to LF; empty `oldText` → empty-oldText error.
   - exact match first; if any edit needs it, fall back to fuzzy-normalized content space
     (`normalize_for_fuzzy_match`: NFKC, per-line trailing-whitespace trim, smart quotes/dashes/
     spaces → ASCII).
   - each edit must be found (not-found error) and unique (duplicate error, with occurrence
     count); matches sorted by index, overlap → overlap error.
   - apply in reverse index order; if result == base → no-change error.
5. Restore line endings + BOM; write. Return `[Text("Successfully replaced N block(s) in
   <path>")]`. (No diff/patch in the result.)
   - Error messages match TS `getNotFoundError` / `getDuplicateError` / `getEmptyOldTextError`
     / `getNoChangeError` / overlap, including the single-edit vs multi-edit wording variants.

**bash** — schema `{ command: string, timeout?: number (seconds) }`
1. Verify `cwd` exists → else `Err("Working directory does not exist: <cwd>\nCannot execute bash
   commands.")`.
2. Spawn `bash -c <command>` (`tokio::process::Command`) in `cwd`, capture stdout+stderr
   **merged in arrival order** into one buffer.
3. If `timeout` set, wrap in `tokio::time::timeout`; on elapse kill the child and mark timed-out.
4. `truncate_tail` the merged output (last 2000 lines / 50KB). On truncation append
   `\n\n[Showing lines A-B of T. (output truncated)]` (full-output temp file deferred — no path).
5. Outcomes:
   - exit 0 → `Ok([Text(output_or_"(no output)")])`.
   - non-zero exit → `Err(output + "\n\nCommand exited with code N")`.
   - timeout → `Err(output + "\n\nCommand timed out after N seconds")`.
   - spawn failure → `Err("…")`.

> Merged stdout/stderr ordering: a single reader over both pipes appends chunks as they arrive
> (mirrors TS feeding both streams to one `onData`). Exact interleaving need not be
> byte-deterministic; tests assert presence of both streams and the status suffix.

## 7. Error handling

| Failure | Tool result |
|---|---|
| read: unreadable path | `Err` (access error text) |
| read: offset past EOF | `Err("Offset N is beyond end of file (M lines total)")` |
| write: mkdir/write fails | `Err(io error text)` |
| edit: file not accessible | `Err("Could not edit file: <path>. <reason>.")` |
| edit: oldText empty / not found / duplicate / overlap / no change | `Err(...)` matching TS wording |
| bash: missing cwd | `Err("Working directory does not exist: …")` |
| bash: non-zero exit / timeout | `Err(output + status suffix)` |
| any: bad/missing args | `Err(...)` describing the expected argument |

The agent loop (`pi-agent-core`) converts a tool `Err(String)` into that tool call's result
content (model-visible) and continues, so the model can self-correct — matching TS, where a tool
throwing becomes an error tool result.

## 8. Testing strategy (offline, deterministic)

- Unit tests per tool over `tempfile::tempdir()` fixtures; call the private `xxx_execute(cwd,
  args)` (or the `AgentTool.execute` closure) directly and assert returned `ContentBlock`s /
  `Err` text. Pin model-visible wording against the TS reference, except where this spec
  explicitly simplifies it (e.g. the bash truncation notice omits the deferred temp-file path).
- `truncate.rs` unit tests: no-truncation, line-limit, byte-limit, first-line-exceeds (head),
  tail partial-line edge case, `format_size`.
- `tools_e2e.rs`: register a faux provider under a unique api key (avoids the global-registry
  race seen in pi-ai faux tests), script a tool-use turn that calls a built-in tool on a temp
  file, then a final text turn; drive it through `run_print_mode` with a model bound to that
  faux provider; assert the final stdout.
- No real provider keys or network.

## 9. Dependencies

- `pi-coding-agent/Cargo.toml`:
  - extend `tokio` features to include `fs`, `process`, `time` (currently `rt-multi-thread`,
    `macros`).
  - add `unicode-normalization` (for edit's NFKC fuzzy normalization).
  - dev-dep: `tempfile` for temp-dir fixtures.
- No new workspace-wide deps; no changes to `pi-ai` or `pi-agent-core`.

## 10. Risks

- **edit fuzzy matching fidelity** — Unicode normalization edge cases may differ subtly from the
  TS `normalizeForFuzzyMatch`. Mitigation: port the exact transform set and pin behavior with
  tests covering smart quotes, dashes, NBSP, and trailing whitespace.
- **bash stdout/stderr interleaving** — not byte-deterministic across platforms. Mitigation:
  tests assert content presence + status suffix, not exact interleave.
- **cwd semantics** — print mode uses process cwd; acceptable for PoC. A future `--cwd` flag and
  per-session cwd land with session work.
- **No mid-tool cancellation** — a long bash without `timeout` blocks the turn. Mitigation: doc
  the `timeout` arg; abort-signal support deferred to a future `ToolFn` change.

## 11. Future phases

- `grep` / `find` / `ls` tools (next spec).
- Image reading (mime detect + optional resize) and inline image content.
- bash full-output temp file + throttled `onUpdate` streaming (needed by interactive mode).
- Abort signal into tools (extend `pi-agent-core::ToolFn`) + `file-mutation-queue` once parallel
  tool execution lands (ROADMAP M4).
- TUI renderers / diff display when interactive mode is built (ROADMAP M6).
