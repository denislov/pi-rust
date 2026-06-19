# M11 Autocomplete Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a deterministic Rust autocomplete provider to `pi-tui` for slash commands, file paths, and environment variables.

**Architecture:** Implement a synchronous `CombinedAutocompleteProvider` in `crates/pi-tui/src/autocomplete.rs`. The provider returns suggestions for slash commands at the start of a line, path-like prefixes from the local filesystem, `@` file attachment prefixes, quoted path prefixes, and `$ENV` prefixes from an injectable environment map. Completion application updates the target line and cursor position in the same shape as the TS provider. Defer fd-backed recursive fuzzy file search to a later optimization slice.

**Tech Stack:** Rust 2024, `std::fs`, `std::path`, existing `fuzzy_filter_indices`, and deterministic offline tests.

---

## File Structure

- Create `crates/pi-tui/src/autocomplete.rs`: public item/command/suggestion/edit types, `CombinedAutocompleteProvider`, prefix extraction, file/env/command suggestions, completion application.
- Modify `crates/pi-tui/src/lib.rs`: expose `autocomplete` module and re-export public autocomplete types.
- Create `crates/pi-tui/tests/autocomplete.rs`: table-style tests using a temporary directory under `std::env::temp_dir()`.
- Modify `docs/roadmap/M11-interactive-ux.md`: record autocomplete progress and fd-recursive deferral.

## Task 1: Tests First

- [ ] **Step 1: Write failing tests**

Create `crates/pi-tui/tests/autocomplete.rs` covering:

- Slash command fuzzy suggestions for `/mo`.
- Applying slash command completion inserts `/model ` and moves the cursor.
- Forced file completion from a temp directory returns directories before files.
- `@` file attachment completion appends a space for files.
- `$HO` env completion uses injected env values.
- `should_trigger_file_completion` is false for bare slash commands and true for path contexts.

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test -p pi-tui --test autocomplete
```

Expected: compile failure because autocomplete types are not defined/exported yet.

## Task 2: Implementation

- [ ] **Step 1: Implement autocomplete provider**

Create `crates/pi-tui/src/autocomplete.rs` with:

- `AutocompleteItem { value, label, description }`.
- `SlashCommand { name, description, argument_hint }`.
- `AutocompleteSuggestions { items, prefix }`.
- `AutocompleteOptions { force }`.
- `CompletionEdit { lines, cursor_line, cursor_col }`.
- `CombinedAutocompleteProvider::new`, `with_env`, `get_suggestions`, `apply_completion`, `should_trigger_file_completion`.
- File completion: parse prefixes, read one directory, filter by case-insensitive starts-with, quote paths with spaces, sort directories first.
- Env completion: extract `$` token after delimiters, fuzzy filter env names, return `$NAME` values.

- [ ] **Step 2: Export symbols**

Update `crates/pi-tui/src/lib.rs` with `pub mod autocomplete;` and re-export the public autocomplete types.

- [ ] **Step 3: Update roadmap**

Under M11 item 4, add:

```markdown
> 进度：`pi-tui` 已新增同步 `CombinedAutocompleteProvider`，支持 slash command、路径/`@` 附件、`$ENV` 建议和 completion application；fd 递归 fuzzy 文件搜索后续优化。
```

## Task 3: Verification And Commit

- [ ] **Step 1: Run verification**

Run:

```bash
cargo fmt --check
env -u NO_COLOR TERM=xterm-256color cargo test -p pi-tui
env -u NO_COLOR TERM=xterm-256color cargo test --workspace
cargo check --workspace
```

Expected: all pass.

- [ ] **Step 2: Commit**

Run:

```bash
git add crates/pi-tui/src/autocomplete.rs crates/pi-tui/src/lib.rs crates/pi-tui/tests/autocomplete.rs docs/roadmap/M11-interactive-ux.md docs/superpowers/plans/2026-06-20-m11-autocomplete.md
git commit -m "feat: add tui autocomplete provider"
```
