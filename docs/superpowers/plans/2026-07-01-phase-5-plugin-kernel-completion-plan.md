# Phase 5 Plugin Kernel Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete Phase 5 by turning the staged plugin kernel into a capability-scoped internal extension system with tool, command, hook, UI, keybind, and reserved Flow extension boundaries.

**Architecture:** Keep plugin traits internal (`pub(crate)`) until they are supportable as public API. `PluginRegistry` stores providers, `PluginService` is the only caller of provider code, `CapabilityService` reports plugin readiness, `RuntimeService` integrates tools, and `PromptTurnFlow` invokes prompt hooks through scoped context objects. Plugins must not receive `CodingAgentSession`, `SessionService`, storage, auth, raw filesystem, shell, or Flow graph mutation access.

**Tech Stack:** Rust, `pi-coding-agent`, `pi-agent-core::flow`, existing `CodingAgentSession`/`PromptTurnFlow`/`RuntimeService`, cargo test/fmt/check.

---

## Current Baseline

Already committed before this plan:

- `crate::plugins::{PluginRegistry, PluginMetadata, PluginSource, PluginError}`.
- `ToolProvider` and `ToolRegistrationHost`.
- `CommandProvider`, `CommandDefinition`, and `CommandRegistrationHost`.
- `PluginService::{collect_tools, collect_commands, diagnostics}` with returned-error and panic isolation.
- `RuntimeService::build_agent_runtime_with_plugins()` merging plugin tools into the Agent runtime.
- `PromptTurnContext` carrying the session-owned `PluginService` into `PromptTurnFlow`.

Do not rework these foundations unless the task requires a focused adjustment.

## File Structure

Create or modify only these files for Phase 5 completion:

- Modify `docs/TODO.md`: phase progress, progress log, latest verified checks.
- Modify `docs/superpowers/plans/2026-07-01-phase-5-plugin-kernel-completion-plan.md`: check off tasks as they land.
- Create `crates/pi-coding-agent/src/plugins/capability.rs`: internal plugin capability report and provider counts.
- Create `crates/pi-coding-agent/src/plugins/hook.rs`: prompt hook provider traits, hook points, hook policy, hook context, hook registration.
- Create `crates/pi-coding-agent/src/plugins/ui.rs`: minimal UI action/dialog boundary definitions.
- Create `crates/pi-coding-agent/src/plugins/keybind.rs`: minimal keybind/action registration boundary definitions.
- Create `crates/pi-coding-agent/src/plugins/flow_extension.rs`: reserved first-party Flow extension trait and named extension points.
- Modify `crates/pi-coding-agent/src/plugins/mod.rs`: export new internal plugin modules.
- Modify `crates/pi-coding-agent/src/plugins/registry.rs`: store provider vectors for hooks, UI, keybinds, Flow extensions.
- Modify `crates/pi-coding-agent/src/coding_session/plugin_service.rs`: collect capabilities, hooks, UI actions, keybindings, flow extension points; run prompt hooks with failure isolation.
- Modify `crates/pi-coding-agent/src/coding_session/capability_service.rs`: consume plugin capability reports.
- Modify `crates/pi-coding-agent/src/coding_session/context.rs`: stop hard-coding plugins as unsupported.
- Modify `crates/pi-coding-agent/src/coding_session/mod.rs`: pass `PluginService` into capability reporting.
- Modify `crates/pi-coding-agent/src/coding_session/prompt.rs`: expose only the prompt fields needed by hook contexts through `PromptTurnContext` methods.
- Modify `crates/pi-coding-agent/src/coding_session/prompt_flow.rs`: add hook node IDs and invoke hook boundaries.
- Modify `crates/pi-coding-agent/tests/public_api.rs`: update the expected plugin capability status.

## Task 1: Plugin Capability Report

**Files:**

- Create: `crates/pi-coding-agent/src/plugins/capability.rs`
- Modify: `crates/pi-coding-agent/src/plugins/mod.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/plugin_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/capability_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/context.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Test: existing unit tests in `capability_service.rs` and `plugin_service.rs`
- Test: `crates/pi-coding-agent/tests/public_api.rs`

- [x] **Step 1: Write the failing capability tests**

Add tests that expect plugin capability state to be available now that the internal kernel exists:

```rust
#[test]
fn capabilities_report_plugins_available_when_kernel_exists() {
    let plugin_report = crate::plugins::PluginCapabilities::new();
    let capabilities = CapabilityService::new().capabilities(None, &plugin_report);

    assert_eq!(capabilities.plugins, CapabilityStatus::Available);
}
```

Add a `PluginService` test that verifies command/tool provider counts are reflected in the internal report:

```rust
#[test]
fn capabilities_report_registered_plugin_capabilities() {
    let mut registry = PluginRegistry::new();
    registry.register_tool_provider(Arc::new(StaticToolProvider {
        plugin_id: "tools-plugin",
        tool_name: "plugin_echo",
    }));
    registry.register_command_provider(Arc::new(StaticCommandProvider));
    let service = PluginService::with_registry(registry);

    let capabilities = service.capabilities();

    assert_eq!(capabilities.tool_providers, 1);
    assert_eq!(capabilities.command_providers, 1);
    assert!(service.diagnostics().is_empty());
}
```

- [x] **Step 2: Run tests to verify RED**

Run:

```bash
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent capabilities_report
```

Expected: compilation/test failure because `PluginCapabilities` and the new `CapabilityService::capabilities(_, _)` signature do not exist.

- [x] **Step 3: Implement minimal capability report**

Create `plugins/capability.rs`:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PluginCapabilities {
    pub(crate) tool_providers: usize,
    pub(crate) command_providers: usize,
    pub(crate) hook_providers: usize,
    pub(crate) ui_providers: usize,
    pub(crate) keybind_providers: usize,
    pub(crate) flow_extensions: usize,
    pub(crate) diagnostics: usize,
}

impl PluginCapabilities {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}
```

Update `PluginService::capabilities()` to fill the report from `PluginRegistry` and diagnostics. Update `CapabilityService::capabilities(active_operation, plugin_report)` so `CodingAgentCapabilities.plugins` is `CapabilityStatus::Available`.

- [x] **Step 4: Run tests to verify GREEN**

Run:

```bash
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent capabilities_report
```

Expected: the new plugin capability tests pass, and existing capability tests are updated to expect `CapabilityStatus::Available` for plugins.

- [x] **Step 5: Update docs and commit**

Update `docs/TODO.md` Phase 5 capability/failure-isolation notes, then commit:

```bash
git add crates/pi-coding-agent/src/plugins/capability.rs crates/pi-coding-agent/src/plugins/mod.rs crates/pi-coding-agent/src/coding_session/plugin_service.rs crates/pi-coding-agent/src/coding_session/capability_service.rs crates/pi-coding-agent/src/coding_session/context.rs crates/pi-coding-agent/src/coding_session/mod.rs crates/pi-coding-agent/tests/public_api.rs docs/TODO.md docs/superpowers/plans/2026-07-01-phase-5-plugin-kernel-completion-plan.md
git commit -m "feat(coding-agent): report plugin kernel capabilities"
```

## Task 2: Prompt Hook Provider Boundary

**Files:**

- Create: `crates/pi-coding-agent/src/plugins/hook.rs`
- Modify: `crates/pi-coding-agent/src/plugins/mod.rs`
- Modify: `crates/pi-coding-agent/src/plugins/registry.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/plugin_service.rs`
- Test: `plugin_service.rs`

- [x] **Step 1: Write failing hook collection tests**

Add tests to `plugin_service.rs` for returned hook registrations and returned-error isolation:

```rust
#[test]
fn collect_prompt_hooks_returns_registered_hook_definitions() {
    let mut registry = PluginRegistry::new();
    registry.register_hook_provider(Arc::new(StaticHookProvider));
    let service = PluginService::with_registry(registry);

    let hooks = service.collect_prompt_hooks();

    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0].point, PromptHookPoint::BeforeAgentTurn);
    assert!(service.diagnostics().is_empty());
}
```

- [x] **Step 2: Run tests to verify RED**

Run:

```bash
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent collect_prompt_hooks
```

Expected: compilation failure because hook types do not exist.

- [x] **Step 3: Implement hook types and registry storage**

Create `plugins/hook.rs` with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptHookPoint {
    BeforePromptPrepare,
    AfterInputPrepared,
    AfterResourcesLoaded,
    BeforeAgentTurn,
    AfterAgentTurn,
    BeforeSessionCommit,
    AfterSessionCommit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HookFailurePolicy {
    FailOpen,
    FailClosed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HookRegistration {
    pub(crate) point: PromptHookPoint,
    pub(crate) policy: HookFailurePolicy,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct HookRegistrationHost;

pub(crate) trait HookProvider: Send + Sync {
    fn metadata(&self) -> PluginMetadata;
    fn hooks(&self, host: &HookRegistrationHost) -> Result<Vec<HookRegistration>, PluginError>;
}
```

Add `hook_providers` to `PluginRegistry`, plus `register_hook_provider()` and `hook_providers()`.

- [x] **Step 4: Run tests to verify GREEN**

Run:

```bash
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent collect_prompt_hooks
```

Expected: hook collection tests pass.

- [x] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/plugins/hook.rs crates/pi-coding-agent/src/plugins/mod.rs crates/pi-coding-agent/src/plugins/registry.rs crates/pi-coding-agent/src/coding_session/plugin_service.rs docs/TODO.md docs/superpowers/plans/2026-07-01-phase-5-plugin-kernel-completion-plan.md
git commit -m "feat(coding-agent): add plugin prompt hook boundary"
```

## Task 3: PromptTurnFlow Hook Execution

**Files:**

- Modify: `crates/pi-coding-agent/src/plugins/hook.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/plugin_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/prompt.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/prompt_flow.rs`
- Test: `prompt_flow.rs`
- Test: `plugin_service.rs`

- [x] **Step 1: Write failing hook execution tests**

Add a `prompt_flow.rs` test that registers a hook provider and verifies hook diagnostics are recorded as coding diagnostics without exposing session storage:

```rust
#[tokio::test]
async fn prompt_turn_flow_runs_noncritical_prompt_hooks_as_diagnostics() {
    let api = "prompt-flow-plugin-hook";
    let flow = PromptTurnFlow::new().unwrap();
    let mut context = context_with_runtime(api, "done");
    let _session = attach_session_boundary(&mut context);
    context.set_plugin_service(plugin_service_with_hook(PromptHookPoint::BeforeAgentTurn, HookFailurePolicy::FailOpen));

    flow.run(&mut context).await.unwrap();

    assert!(context.diagnostics().iter().any(|diagnostic| {
        diagnostic.message.contains("hook before agent turn")
    }));
    registry::unregister(api);
}
```

Add fail-open returned-error and fail-closed tests:

```rust
#[tokio::test]
async fn prompt_turn_flow_aborts_for_fail_closed_hook_error() {
    let api = "prompt-flow-plugin-critical-hook";
    let flow = PromptTurnFlow::new().unwrap();
    let mut context = context_with_runtime(api, "done");
    let _session = attach_session_boundary(&mut context);
    context.set_plugin_service(plugin_service_with_failing_hook(PromptHookPoint::BeforeAgentTurn, HookFailurePolicy::FailClosed));

    let error = flow.run(&mut context).await.unwrap_err();

    assert!(error.to_string().contains("plugin hook"));
    registry::unregister(api);
}
```

- [x] **Step 2: Run tests to verify RED**

Run:

```bash
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent prompt_turn_flow_runs_noncritical_prompt_hooks_as_diagnostics
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent prompt_turn_flow_aborts_for_fail_closed_hook_error
```

Expected: compilation/test failure because hook execution is not wired. Actual RED was verified before implementation with the noncritical hook test failing on missing hook execution types and trait method.

- [x] **Step 3: Implement scoped hook execution**

Extend `plugins/hook.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PromptHookContext {
    pub(crate) operation_id: String,
    pub(crate) turn_id: String,
    pub(crate) session_id: Option<String>,
    pub(crate) point: PromptHookPoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HookDiagnostic {
    pub(crate) message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct HookOutcome {
    pub(crate) diagnostics: Vec<HookDiagnostic>,
}
```

Add `HookProvider::run_hook(&self, ctx: &PromptHookContext) -> Result<HookOutcome, PluginError>`. `PluginService::run_prompt_hook(point, context)` catches panics and returned errors. Fail-open errors become plugin diagnostics and `CodingDiagnostic::warn`; fail-closed errors return `CodingSessionError::Plugin`.

- [x] **Step 4: Wire PromptTurnFlow hook points**

Call hooks at these stable points:

- `BeforePromptPrepare`: immediately before `prepare_input`.
- `AfterInputPrepared`: immediately after `prepare_input`.
- `AfterResourcesLoaded`: immediately after `load_resources`.
- `BeforeAgentTurn`: immediately before `run_agent_turn` starts.
- `AfterAgentTurn`: after agent turn succeeds.
- `BeforeSessionCommit`: inside `finalize_turn` before readiness returns success.
- `AfterSessionCommit`: after `emit_completion` records prompt completion. This is still before owner-level session finalization; document that Phase 5 hook cannot observe durable commit results yet.

- [x] **Step 5: Run tests to verify GREEN**

Run:

```bash
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent prompt_turn_flow_runs_noncritical_prompt_hooks_as_diagnostics
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent prompt_turn_flow_continues_for_fail_open_hook_error_as_diagnostic
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent prompt_turn_flow_aborts_for_fail_closed_hook_error
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent prompt_turn_flow
```

Expected: hook execution and existing prompt flow tests pass.

- [x] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/plugins/hook.rs crates/pi-coding-agent/src/coding_session/plugin_service.rs crates/pi-coding-agent/src/coding_session/prompt.rs crates/pi-coding-agent/src/coding_session/prompt_flow.rs docs/TODO.md docs/superpowers/plans/2026-07-01-phase-5-plugin-kernel-completion-plan.md
git commit -m "feat(coding-agent): run plugin hooks in prompt flow"
```

## Task 4: UI and Keybind Provider Boundaries

**Files:**

- Create: `crates/pi-coding-agent/src/plugins/ui.rs`
- Create: `crates/pi-coding-agent/src/plugins/keybind.rs`
- Modify: `crates/pi-coding-agent/src/plugins/mod.rs`
- Modify: `crates/pi-coding-agent/src/plugins/registry.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/plugin_service.rs`
- Test: `plugin_service.rs`

- [x] **Step 1: Write failing UI/keybind collection tests**

Add tests for `collect_ui_actions()` and `collect_keybindings()`. They should verify definitions are collected and provider failures become diagnostics.

- [x] **Step 2: Run tests to verify RED**

Run:

```bash
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent collect_ui collect_keybindings
```

Expected: compilation failure because UI/keybind provider types do not exist. Actual RED was verified with `collect_ui_actions_returns_registered_action_definitions` failing on missing UI/keybind imports.

- [x] **Step 3: Implement minimal provider boundaries**

`plugins/ui.rs` must define `UiActionDefinition`, `UiProvider`, and `UiRegistrationHost`. `plugins/keybind.rs` must define `KeybindDefinition`, `KeybindProvider`, and `KeybindRegistrationHost`. Definitions include only stable IDs, labels, descriptions, and action IDs. Do not render terminal output or wire real key handlers in this task.

- [x] **Step 4: Run tests to verify GREEN**

Run:

```bash
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent collect_ui
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent collect_keybindings
```

Expected: UI/keybind boundary tests pass.

- [x] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/plugins/ui.rs crates/pi-coding-agent/src/plugins/keybind.rs crates/pi-coding-agent/src/plugins/mod.rs crates/pi-coding-agent/src/plugins/registry.rs crates/pi-coding-agent/src/coding_session/plugin_service.rs docs/TODO.md docs/superpowers/plans/2026-07-01-phase-5-plugin-kernel-completion-plan.md
git commit -m "feat(coding-agent): add plugin ui and keybind boundaries"
```

## Task 5: Reserved FlowExtension Boundary

**Files:**

- Create: `crates/pi-coding-agent/src/plugins/flow_extension.rs`
- Modify: `crates/pi-coding-agent/src/plugins/mod.rs`
- Modify: `crates/pi-coding-agent/src/plugins/registry.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/plugin_service.rs`
- Test: `plugin_service.rs`

- [x] **Step 1: Write failing FlowExtension tests**

Add a test proving only named extension points can be registered:

```rust
#[test]
fn collect_flow_extension_points_returns_named_points_without_graph_rewrites() {
    let mut registry = PluginRegistry::new();
    registry.register_flow_extension(Arc::new(StaticFlowExtension));
    let service = PluginService::with_registry(registry);

    let points = service.collect_flow_extension_points();

    assert_eq!(points, vec![FlowExtensionPoint::PromptBeforeAgentTurn]);
}
```

- [x] **Step 2: Run tests to verify RED**

Run:

```bash
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent collect_flow_extension_points
```

Expected: compilation failure because FlowExtension types do not exist. Actual RED was verified with unresolved imports for `FlowExtension` and `FlowExtensionPoint` before the boundary module was added.

- [x] **Step 3: Implement reserved FlowExtension types**

Create `plugins/flow_extension.rs` with a closed enum such as:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowExtensionPoint {
    PromptBeforePrepare,
    PromptAfterResourcesLoaded,
    PromptBeforeAgentTurn,
    PromptAfterAgentTurn,
    PromptBeforeSessionCommit,
    AgentBeforeProviderRequest,
    AgentAfterToolResult,
}
```

The trait exposes only `extension_points() -> Result<Vec<FlowExtensionPoint>, PluginError>`. Do not expose node insertion, graph rewrites, or Lua registration.

- [x] **Step 4: Run tests to verify GREEN**

Run:

```bash
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent collect_flow_extension_points
```

Expected: reserved FlowExtension tests pass. Actual GREEN was verified with `cargo test -p pi-coding-agent collect_flow_extension_points`, plus `plugin`, `coding_session`, `cargo check --workspace`, and full `cargo test -p pi-coding-agent` checks.

- [x] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/plugins/flow_extension.rs crates/pi-coding-agent/src/plugins/mod.rs crates/pi-coding-agent/src/plugins/registry.rs crates/pi-coding-agent/src/coding_session/plugin_service.rs docs/TODO.md docs/superpowers/plans/2026-07-01-phase-5-plugin-kernel-completion-plan.md
git commit -m "feat(coding-agent): reserve plugin flow extension points"
```

## Task 6: Phase 5 Completion Audit and Cleanup

**Files:**

- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-01-phase-5-plugin-kernel-completion-plan.md`
- Modify code files only if verification exposes issues.

- [ ] **Step 1: Audit Phase 5 against the guide**

Verify each guide handoff item:

- plugin registry exists for tool, command, hook, UI, keybind, FlowExtension providers;
- capability-scoped host pattern exists for all provider families;
- plugin tools integrate into runtime service;
- plugin hooks integrate into prompt flow;
- FlowExtension is reserved and controlled;
- Lua cannot register arbitrary Flow graph mutations;
- plugin failures become diagnostics/errors, not panics;
- plugin contexts do not expose `CodingAgentSession`, `SessionService`, storage, auth, raw shell, or raw filesystem.

- [ ] **Step 2: Run focused checks**

Run:

```bash
PATH=$HOME/.cargo/bin:$PATH cargo fmt --check
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent plugin
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent coding_session
PATH=$HOME/.cargo/bin:$PATH cargo check --workspace
```

Expected: all pass with exit code 0.

- [ ] **Step 3: Run broad package checks**

Run:

```bash
PATH=$HOME/.cargo/bin:$PATH cargo test -p pi-coding-agent
PATH=$HOME/.cargo/bin:$PATH git diff --check
```

Expected: package tests pass, and `git diff --check` exits 0.

- [ ] **Step 4: Mark Phase 5 complete**

Update `docs/TODO.md`:

- current north star Phase 5 item becomes `[x]`;
- all Phase 5 checklist items become `[x]`;
- progress log records the completed Phase 5 handoff;
- latest verified checks list the exact commands from Steps 2 and 3.

- [ ] **Step 5: Commit completion**

```bash
git add docs/TODO.md docs/superpowers/plans/2026-07-01-phase-5-plugin-kernel-completion-plan.md
git commit -m "docs: mark phase 5 plugin kernel complete"
```

- [ ] **Step 6: Final goal audit**

Run:

```bash
git status --short
git log --oneline -8
```

Expected: worktree is clean, and the latest commits show the Phase 5 completion sequence.

Only after this audit proves all guide requirements should the persistent goal be marked complete.
