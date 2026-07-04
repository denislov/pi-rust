# Lua Host Workspace Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose read-only Lua host workspace metadata through `host:workspace()` without exposing raw session, runtime, filesystem, shell, provider, or Flow internals.

**Architecture:** Extend the existing Lua host API installer in `plugin_load_flow.rs` to accept the plugin entry path and install a `workspace` function beside `api_version` and `plugin`. The function returns a table with string `pluginRoot` and `entryPath`, derived only from the manifest entry location, and is installed consistently for registration, tool execution, command execution, and hook execution Lua runtimes.

**Tech Stack:** Rust 2024, `mlua`, existing `PluginLoadFlow`, `PluginService`, and deterministic tempdir-backed tests.

---

### Task 1: Add RED Lua Host Workspace Coverage

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/flow_service.rs`

- [x] **Step 1: Add registration-time workspace metadata test**

Add a `plugin_load_flow_lua_host_workspace_metadata_is_read_only_and_path_scoped` test that creates a Lua plugin whose `register(host)` calls `host:workspace()`, asserts no privileged members such as `session` or `runtime` exist, and registers a command description containing `workspace.pluginRoot` plus `workspace.entryPath`.

- [x] **Step 2: Add execution-time workspace metadata assertion**

In the same test, run the registered command and assert the command body can call `host:workspace()` again and return the same path-scoped metadata.

- [x] **Step 3: Run RED test**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent plugin_load_flow_lua_host_workspace_metadata_is_read_only_and_path_scoped -- --nocapture
```

Expected: FAIL because `host:workspace` is not installed.

### Task 2: Install Workspace Host API

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs`

- [x] **Step 1: Change `install_lua_host_api` signature**

Pass `entry_path: &Path` to `install_lua_host_api` and update all call sites in `collect_lua_plugin_specs`, `run_lua_tool`, `run_lua_command`, and `run_lua_hook`.

- [x] **Step 2: Add `host:workspace()`**

Inside `install_lua_host_api`, compute `plugin_root = entry_path.parent().unwrap_or_else(|| Path::new("."))`, convert both paths through `display().to_string()`, and install a Lua function returning a table:

```lua
{
  pluginRoot = "...",
  entryPath = "...",
}
```

Do not add filesystem handles, session ids, auth values, cwd mutation, shell access, network access, or runtime/provider objects.

- [x] **Step 3: Run focused GREEN test**

Run the focused test from Task 1 and expect PASS.

### Task 3: Update Docs And Verify

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-04-lua-host-workspace-metadata-plan.md`

- [x] **Step 1: Update TODO**

Record that Lua host APIs now include path-scoped workspace metadata while still avoiding internal operation contexts.

- [x] **Step 2: Mark plan steps complete**

Update this plan's checkboxes as implementation proceeds.

- [x] **Step 3: Run verification**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt --check
/home/whai/.cargo/bin/cargo test -p pi-coding-agent plugin_load_flow_lua_host_workspace_metadata_is_read_only_and_path_scoped -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent plugin_load_flow --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet
/home/whai/.cargo/bin/cargo check --workspace --quiet
/home/whai/.cargo/bin/cargo test --workspace --quiet
git diff --check
```
