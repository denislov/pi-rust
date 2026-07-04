# Lua Host Capabilities Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose read-only Lua host API feature metadata through `host:capabilities()` so Lua plugins can feature-detect supported registration and metadata helpers without receiving raw session, runtime, provider, filesystem, shell, or operation-context access.

**Architecture:** Extend the existing Lua host API installer in `plugin_load_flow.rs` to add a `capabilities` function beside `api_version`, `plugin`, and `workspace`. The function returns a fresh table of boolean feature flags named after stable Lua host methods. The same installer is used for registration, tool execution, command execution, and hook execution runtimes, so the feature table remains consistent across all plugin phases.

**Tech Stack:** Rust 2024, `mlua`, existing `PluginLoadFlow`, `PluginService`, and deterministic tempdir-backed tests.

---

### Task 1: Add RED Lua Host Capabilities Coverage

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/flow_service.rs`

- [x] **Step 1: Add registration-time capabilities metadata test**

Add a `plugin_load_flow_lua_host_capabilities_metadata_is_feature_scoped` test that creates a Lua plugin whose `register(host)` calls `host:capabilities()`, verifies expected feature flags are true, and verifies privileged members such as `session`, `runtime`, `provider`, `filesystem`, `shell`, and `operationContext` are absent from both `host` and the returned capabilities table.

- [x] **Step 2: Add execution-time capabilities metadata assertion**

In the same test, register a command and assert command execution can call `host:capabilities()` again and observe the same feature-scoped metadata.

- [x] **Step 3: Run RED test**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent plugin_load_flow_lua_host_capabilities_metadata_is_feature_scoped -- --nocapture
```

Expected: FAIL because `host:capabilities` is not installed.

### Task 2: Install Capabilities Host API

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs`

- [x] **Step 1: Add `host:capabilities()`**

Inside `install_lua_host_api`, install a Lua function returning a fresh table with boolean flags for the supported host methods:

```lua
{
  api_version = true,
  plugin = true,
  workspace = true,
  capabilities = true,
  tool = true,
  command = true,
  hook = true,
  ui_action = true,
  dialog = true,
  keybind = true,
}
```

Do not add session ids, auth values, operation ids, filesystem handles, shell/network access, or runtime/provider objects.

- [x] **Step 2: Run focused GREEN test**

Run the focused test from Task 1 and expect PASS.

### Task 3: Update Docs And Verify

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-04-lua-host-capabilities-metadata-plan.md`

- [x] **Step 1: Update TODO**

Record that Lua host APIs now include feature-scoped capabilities metadata while continuing to avoid internal operation contexts and privileged handles.

- [x] **Step 2: Mark plan steps complete**

Update this plan's checkboxes as implementation proceeds.

- [x] **Step 3: Run verification**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt --check
/home/whai/.cargo/bin/cargo test -p pi-coding-agent plugin_load_flow_lua_host_capabilities_metadata_is_feature_scoped -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent plugin_load_flow --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet
/home/whai/.cargo/bin/cargo check --workspace --quiet
/home/whai/.cargo/bin/cargo test --workspace --quiet
git diff --check
```
