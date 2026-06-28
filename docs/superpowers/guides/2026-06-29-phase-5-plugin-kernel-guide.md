# Phase 5 Guide: Plugin Kernel on Session and Flow Boundaries

## Phase Goal

Add the Rust trait plugin kernel on top of stable `CodingAgentSession`, `PromptTurnFlow`, `AgentTurnFlow`, and capability boundaries.

Phase 5 should not expose arbitrary Lua node/subflow registration. It should establish safe Rust extension points first.

## Preconditions

Phase 1:

- `CodingAgentSession` exists.
- `SessionService` and transaction finalization exist.

Phase 2:

- `PromptTurnFlow` exists and runs the product prompt path.

Phase 3:

- adapters consume `CodingAgentEvent`.
- `CapabilityService` exists.

Phase 4 is helpful but not strictly required for product-level tool/command/hook plugins. Agent-level flow hooks should wait for Phase 4.

## Non-Negotiable Constraints

- Plugins do not receive `&mut CodingAgentSession`.
- Plugins do not receive raw `SessionService`.
- Plugins cannot direct-commit session events.
- Plugins cannot access auth secrets by default.
- Lua cannot register arbitrary Flow nodes/subflows in this phase.
- Plugin failures must become typed diagnostics/errors, not panics.

## Target Module Layout

Add:

```text
crates/pi-coding-agent/src/plugins/
  mod.rs
  registry.rs
  host.rs
  capability.rs
  tool.rs
  command.rs
  hook.rs
  ui.rs
  keybind.rs
  flow_extension.rs
  error.rs
```

Wire into:

```text
crates/pi-coding-agent/src/coding_session/plugin_service.rs
crates/pi-coding-agent/src/coding_session/runtime_service.rs
crates/pi-coding-agent/src/coding_session/flow_service.rs
crates/pi-coding-agent/src/coding_session/capability_service.rs
```

Keep plugin traits public only when they are ready to be supported. Internal first-party traits can remain `pub(crate)` initially.

## Plugin Registry

Recommended shape:

```rust
pub struct PluginRegistry {
    tool_providers: Vec<Arc<dyn ToolProvider>>,
    command_providers: Vec<Arc<dyn CommandProvider>>,
    hook_providers: Vec<Arc<dyn HookProvider>>,
    ui_providers: Vec<Arc<dyn UiProvider>>,
    keybind_providers: Vec<Arc<dyn KeybindProvider>>,
    flow_extensions: Vec<Arc<dyn FlowExtension>>,
}
```

Plugin identity:

```rust
pub struct PluginId(String);

pub struct PluginMetadata {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub source: PluginSource,
}
```

`PluginSource`:

- first-party;
- project;
- user/global;
- Lua later.

## Host Capability Model

Plugins get scoped hosts.

Examples:

```rust
pub struct ToolHost<'a> { ... }
pub struct CommandHost<'a> { ... }
pub struct PromptHookHost<'a> { ... }
pub struct FlowExtensionHost<'a> { ... }
```

Allowed capabilities should be explicit:

```text
read session view
emit diagnostic
request cancellation
read resources snapshot
register tool
register command
request filesystem read through ExecutionEnv
request shell execution through ExecutionEnv
append approved pending event through transaction handle
```

Forbidden by default:

```text
raw CodingAgentSession
raw SessionService
raw AuthStore
raw provider internals
raw std::fs outside ExecutionEnv
raw shell process spawn
direct session commit
```

## ToolProvider

Trait sketch:

```rust
pub trait ToolProvider: Send + Sync {
    fn metadata(&self) -> PluginMetadata;
    fn tools(&self, host: &ToolRegistrationHost) -> Result<Vec<AgentTool>, PluginError>;
}
```

Integration:

- `RuntimeService` asks `PluginService` for plugin tools.
- plugin tools are merged with built-ins before tool filtering.
- tool filter applies uniformly to built-in and plugin tools.
- plugin tool execution uses existing `AgentTool`.

Tests:

- plugin tool appears in tool list;
- tool filter can include/exclude plugin tool;
- faux provider can call plugin tool;
- plugin tool error becomes tool error result, not panic.

## CommandProvider

Trait sketch:

```rust
pub trait CommandProvider: Send + Sync {
    fn metadata(&self) -> PluginMetadata;
    fn commands(&self, host: &CommandRegistrationHost) -> Result<Vec<CommandDefinition>, PluginError>;
}
```

Command execution should call high-level session methods:

```rust
pub trait CommandHandler: Send + Sync {
    fn run<'a>(&'a self, ctx: CommandContext<'a>) -> CommandFuture<'a>;
}
```

CommandContext should not expose storage. It can:

- read session view;
- emit diagnostics;
- call allowed owner methods;
- request UI action if interactive.

Tests:

- command appears in capability list;
- command failure emits diagnostic;
- command cannot mutate session except through allowed methods.

## HookProvider

Initial hook points around `PromptTurnFlow`:

```text
before_prompt_prepare
after_input_prepared
after_resources_loaded
before_agent_turn
after_agent_turn
before_session_commit
after_session_commit
```

Trait sketch:

```rust
pub trait HookProvider: Send + Sync {
    fn metadata(&self) -> PluginMetadata;
    fn hooks(&self) -> Vec<HookRegistration>;
}
```

Hook context views:

- `PromptPrepareHookContext`;
- `ResourcesLoadedHookContext`;
- `AgentTurnHookContext`;
- `SessionCommitHookContext`.

Rules:

- hooks may emit diagnostics;
- hooks may request approved mutations through context methods;
- hooks cannot call storage commit;
- hooks can be fail-open or fail-closed depending on hook type.

Tests:

- hook diagnostic appears as `CodingAgentEvent::Diagnostic`;
- failing noncritical hook does not abort prompt;
- failing critical hook aborts with typed plugin error if configured.

## UiProvider and KeybindProvider

Phase 5 can keep UI/keybind minimal.

UiProvider:

- registers simple dialogs/actions;
- does not render arbitrary terminal output directly;
- uses `pi-tui` primitives through controlled adapter.

KeybindProvider:

- registers app-level action IDs;
- `pi-tui` remains generic and does not learn coding-agent semantics.

Tests:

- plugin keybinding appears in capability/keybinding registry;
- unavailable UI action is reported through capability status.

## FlowExtension

Reserve trait:

```rust
pub trait FlowExtension: Send + Sync {
    fn metadata(&self) -> PluginMetadata;
    fn extension_points(&self) -> Vec<FlowExtensionPoint>;
}
```

Phase 5 restrictions:

- first-party Rust only;
- no Lua arbitrary nodes;
- extension points are named, not arbitrary graph rewrites;
- extension receives a scoped host/context;
- extension cannot replace core nodes unless a later design explicitly allows it.

Extension point examples:

```text
prompt.before_prepare
prompt.after_resources_loaded
prompt.before_agent_turn
prompt.after_agent_turn
prompt.before_session_commit
agent.before_provider_request
agent.after_tool_result
```

Agent-level extension points should wait until `AgentTurnFlow` is stable.

## PluginService Integration

`PluginService` owns:

- registry;
- enable/disable state;
- diagnostics;
- failure isolation policy.

Methods:

```rust
pub(crate) fn collect_tools(&self, host: ToolRegistrationHost) -> Vec<AgentTool>;
pub(crate) fn commands(&self) -> Vec<CommandDefinition>;
pub(crate) fn run_hook(&self, point: HookPoint, ctx: HookContext) -> HookResult;
pub(crate) fn capabilities(&self) -> PluginCapabilities;
```

`RuntimeService` calls `collect_tools`.

`FlowService` calls `run_hook` at prompt hook nodes.

`CapabilityService` includes plugin capability state.

## Error Isolation

Add `PluginError`:

```rust
pub enum PluginError {
    Registration { plugin_id: String, message: String },
    Execution { plugin_id: String, message: String },
    PermissionDenied { plugin_id: String, capability: String },
    Panic { plugin_id: String, message: String },
}
```

Use `catch_unwind` only at clear plugin call boundaries if plugin code can panic.

Map plugin failures to:

- `CodingAgentEvent::Diagnostic`;
- `CodingSessionError::Plugin` when fail-closed;
- disabled plugin state if repeated failure policy is implemented.

## Lua Boundary

Lua bridge is not implemented in early Phase 5 unless the Rust trait kernel is already stable.

When Lua starts:

- expose tool/command/hook first;
- do not expose arbitrary `FlowExtension`;
- no raw filesystem/shell/network;
- use capability whitelist;
- version Lua API.

## Tests

Recommended files:

```text
crates/pi-coding-agent/tests/plugin_registry.rs
crates/pi-coding-agent/tests/plugin_tools.rs
crates/pi-coding-agent/tests/plugin_commands.rs
crates/pi-coding-agent/tests/plugin_hooks.rs
crates/pi-coding-agent/tests/plugin_capabilities.rs
```

Coverage:

- first-party plugin registration;
- plugin tool execution through faux provider;
- plugin hook diagnostic;
- plugin command capability;
- plugin failure isolation;
- no direct session commit from plugin context.

## Phase 5 Handoff to Phase 6

Phase 5 must leave:

- plugin registry;
- capability-scoped host pattern;
- plugin tools integrated into runtime service;
- plugin hooks integrated into prompt flow;
- `FlowExtension` reserved but controlled;
- no arbitrary Lua flow graph mutation.

Phase 6 can build advanced workflows that use the same extension and operation-context patterns.

## Stop Conditions

Stop and reassess if:

- plugin trait requires `&mut CodingAgentSession`;
- plugin can directly access `SessionLogStore`;
- plugin errors panic through prompt execution;
- Lua API needs internal context types;
- FlowExtension starts allowing arbitrary graph replacement before Phase 2/4 boundaries are stable.

## Suggested Checks

Focused:

```text
cargo fmt --check
cargo test -p pi-coding-agent plugin
cargo test -p pi-coding-agent coding_session
```

Broader:

```text
cargo test -p pi-coding-agent
cargo check --workspace
```
