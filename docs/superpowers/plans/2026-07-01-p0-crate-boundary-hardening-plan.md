# P0 Crate Boundary Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the intended stable and migration-private boundaries for `pi-ai`, `pi-agent-core`, `pi-coding-agent`, and `pi-tui` explicit, testable, and ready for P1-P4 implementation.

**Architecture:** Add lightweight stable facade modules where the crate does not yet have one, keep existing root exports as migration compatibility for now, and test the public symbols that downstream code should prefer. Documentation and TODO updates carry the policy; compile-time public API tests make accidental removal or expansion visible.

**Tech Stack:** Rust 2024 workspace, crate-level facade modules, integration tests, `cargo fmt`, `cargo test`, `cargo check`, project docs under `docs/superpowers`.

---

## File Structure

- Modify `docs/TODO.md` to link the P0 design/plan and mark boundary work active.
- Create `docs/superpowers/specs/2026-07-01-p0-crate-boundary-hardening-design.md` for the boundary policy.
- Create `docs/superpowers/plans/2026-07-01-p0-crate-boundary-hardening-plan.md` for execution steps.
- Modify `crates/pi-ai/src/lib.rs` to add `pub mod api` and document global registry compatibility.
- Create `crates/pi-ai/tests/public_api.rs` to compile-check the facade.
- Modify `crates/pi-agent-core/src/lib.rs` to add `pub mod api` for low-level stable runtime symbols.
- Create `crates/pi-agent-core/tests/public_api.rs` to compile-check the facade.
- Modify `crates/pi-tui/src/lib.rs` to add `pub mod api` for generic terminal UI primitives.
- Modify `crates/pi-tui/tests/public_api.rs` to import through `pi_tui::api` and preserve existing root import coverage.
- Modify `crates/pi-coding-agent/tests/public_api.rs` only if existing `api` smoke coverage does not cover the boundary symbols named in the design.

### Task 1: Wire P0 Docs Into TODO

**Files:**
- Modify: `docs/TODO.md`
- Already created: `docs/superpowers/specs/2026-07-01-p0-crate-boundary-hardening-design.md`
- Already created: `docs/superpowers/plans/2026-07-01-p0-crate-boundary-hardening-plan.md`

- [ ] **Step 1: Add source document links**

Add these two links under `## Source Documents`:

```markdown
- [P0 crate boundary hardening design](superpowers/specs/2026-07-01-p0-crate-boundary-hardening-design.md)
- [P0 crate boundary hardening plan](superpowers/plans/2026-07-01-p0-crate-boundary-hardening-plan.md)
```

- [ ] **Step 2: Mark the four boundary items active**

Change each of the four `Plan and harden` items from `[ ]` to `[~]` and append this status note:

```markdown
Boundary policy is now captured in the P0 crate boundary hardening design/plan; code-level facade and invariant tests remain in progress.
```

- [ ] **Step 3: Add a progress log entry**

Append this entry under `## Progress Log`:

```markdown
- 2026-07-01: P0 crate boundary hardening started. The design and implementation plan now define stable facade direction, migration-private surfaces, non-goals, and verification gates for `pi-ai`, `pi-agent-core`, `pi-coding-agent`, and `pi-tui` before P1-P4 code hardening proceeds.
```

- [ ] **Step 4: Verify docs diff**

Run:

```bash
. "$HOME/.cargo/env"
git diff -- docs/TODO.md docs/superpowers/specs/2026-07-01-p0-crate-boundary-hardening-design.md docs/superpowers/plans/2026-07-01-p0-crate-boundary-hardening-plan.md
```

Expected: only P0 source-document, TODO status, and progress-log changes.

- [ ] **Step 5: Commit docs slice**

```bash
git add docs/TODO.md docs/superpowers/specs/2026-07-01-p0-crate-boundary-hardening-design.md docs/superpowers/plans/2026-07-01-p0-crate-boundary-hardening-plan.md
git commit -m "docs: plan p0 crate boundary hardening"
```

### Task 2: Add `pi-ai::api` Facade

**Files:**
- Modify: `crates/pi-ai/src/lib.rs`
- Create: `crates/pi-ai/tests/public_api.rs`

- [ ] **Step 1: Write the failing public API test**

Create `crates/pi-ai/tests/public_api.rs`:

```rust
use pi_ai::api::{
    all_models, complete, env_api_key, get_model, get_models, get_providers, lookup_model,
    register, stream_model, AssistantMessage, AssistantMessageEvent, ContentBlock, Context,
    Cost, EventStream, Message, Model, ModelCost, ModelInput, ProviderResponseInfo,
    ProviderStreamHooks, StopReason, StreamOptions, ThinkingConfig, Tool, Usage,
};

#[test]
fn public_api_symbols_are_importable_from_api_facade() {
    let _ = all_models as fn() -> &'static [Model];
    let _ = get_models as fn() -> Vec<&'static Model>;
    let _ = get_providers as fn() -> Vec<String>;
    let _ = get_model as fn(&str) -> Option<&'static Model>;
    let _ = lookup_model as fn(&str) -> Option<&'static Model>;
    let _ = env_api_key as fn(&str) -> Option<String>;

    fn accepts_types(
        _assistant: Option<AssistantMessage>,
        _event: Option<AssistantMessageEvent>,
        _content: Option<ContentBlock>,
        _context: Option<Context>,
        _cost: Option<Cost>,
        _message: Option<Message>,
        _model_cost: Option<ModelCost>,
        _model_input: Option<ModelInput>,
        _provider_info: Option<ProviderResponseInfo>,
        _hooks: Option<ProviderStreamHooks>,
        _stop: Option<StopReason>,
        _options: Option<StreamOptions>,
        _thinking: Option<ThinkingConfig>,
        _tool: Option<Tool>,
        _usage: Option<Usage>,
        _stream: Option<EventStream>,
    ) {
    }

    accepts_types(
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None,
    );

    let _ = complete;
    let _ = register;
    let _ = stream_model;
}
```

- [ ] **Step 2: Verify the test fails**

Run:

```bash
. "$HOME/.cargo/env"
cargo test -p pi-ai --test public_api
```

Expected: compile failure containing `could not find api in pi_ai`.

- [ ] **Step 3: Add the minimal facade**

Append this module to `crates/pi-ai/src/lib.rs` after the existing re-exports:

```rust
/// Stable facade for embedding `pi-ai`.
///
/// The root modules remain public during migration. New downstream code should
/// prefer this module for APIs that are intended to stay stable. Global
/// `register` and `stream_model` are re-exported here as compatibility helpers
/// until the scoped provider runtime is introduced.
pub mod api {
    pub use crate::models::{
        all_models, calculate_cost, get_model, get_models, get_providers, lookup_model,
    };
    pub use crate::registry::{register, stream_model};
    pub use crate::stream::{complete, EventStream};
    pub use crate::types::{
        AssistantMessage, AssistantMessageDiagnostic, AssistantMessageEvent, ContentBlock,
        Context, Cost, DiagnosticErrorInfo, Message, Model, ModelCost, ModelInput,
        ProviderResponseInfo, ProviderStreamHooks, StopReason, StreamOptions, ThinkingConfig,
        Tool, Usage,
    };
    pub use crate::util::env_keys::env_api_key;
}
```

- [ ] **Step 4: Verify the test passes**

Run:

```bash
. "$HOME/.cargo/env"
cargo test -p pi-ai --test public_api
```

Expected: the new public API test passes.

- [ ] **Step 5: Commit pi-ai facade slice**

```bash
git add crates/pi-ai/src/lib.rs crates/pi-ai/tests/public_api.rs
git commit -m "feat(ai): add stable api facade"
```

### Task 3: Add `pi-agent-core::api` Facade

**Files:**
- Modify: `crates/pi-agent-core/src/lib.rs`
- Create: `crates/pi-agent-core/tests/public_api.rs`

- [ ] **Step 1: Write the failing public API test**

Create `crates/pi-agent-core/tests/public_api.rs`:

```rust
use pi_agent_core::api::{
    Agent, AgentConfig, AgentEvent, AgentHooks, AgentMessage, AgentResources, AgentStream,
    AgentTool, AgentToolOutput, AgentToolResult, CompactionConfig, CompactionSettings,
    ExecOptions, ExecutionEnv, ExecutionError, ExecutionOutput, FileError, FileInfo, FileKind,
    FileSystem, InMemoryExecutionEnv, PromptTemplate, ProviderRequestSnapshot, QueueMode,
    ResourceDiagnostic, Shell, Skill, SourceTag, SourcedPromptTemplate, SourcedResourceDiagnostic,
    SourcedSkill, ThinkingLevel, ToolExecutionMode, ToolFn, ToolUpdateCallback,
};

#[test]
fn low_level_runtime_symbols_are_importable_from_api_facade() {
    fn accepts_types(
        _agent: Option<Agent>,
        _config: Option<AgentConfig>,
        _event: Option<AgentEvent>,
        _hooks: Option<AgentHooks>,
        _message: Option<AgentMessage>,
        _resources: Option<AgentResources>,
        _stream: Option<AgentStream>,
        _tool: Option<AgentTool>,
        _tool_output: Option<AgentToolOutput>,
        _tool_result: Option<AgentToolResult>,
        _compaction_config: Option<CompactionConfig>,
        _compaction_settings: Option<CompactionSettings>,
        _exec_options: Option<ExecOptions>,
        _execution_output: Option<ExecutionOutput>,
        _file_info: Option<FileInfo>,
        _file_kind: Option<FileKind>,
        _prompt_template: Option<PromptTemplate>,
        _provider_snapshot: Option<ProviderRequestSnapshot>,
        _queue_mode: Option<QueueMode>,
        _diagnostic: Option<ResourceDiagnostic>,
        _skill: Option<Skill>,
        _source_tag: Option<SourceTag>,
        _sourced_template: Option<SourcedPromptTemplate>,
        _sourced_diagnostic: Option<SourcedResourceDiagnostic>,
        _sourced_skill: Option<SourcedSkill>,
        _thinking: Option<ThinkingLevel>,
        _tool_mode: Option<ToolExecutionMode>,
        _tool_fn: Option<ToolFn>,
        _tool_update: Option<ToolUpdateCallback>,
        _execution_error: Option<ExecutionError>,
        _file_error: Option<FileError>,
    ) {
    }

    accepts_types(
        None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None,
    );

    fn accepts_traits<T: ExecutionEnv + FileSystem + Shell>(_env: &T) {}
    let env = InMemoryExecutionEnv::new();
    accepts_traits(&env);
}
```

- [ ] **Step 2: Verify the test fails**

Run:

```bash
. "$HOME/.cargo/env"
cargo test -p pi-agent-core --test public_api
```

Expected: compile failure containing `could not find api in pi_agent_core`.

- [ ] **Step 3: Add the minimal facade**

Append this module to `crates/pi-agent-core/src/lib.rs` after the existing re-exports:

```rust
/// Stable low-level runtime facade for `pi-agent-core`.
///
/// Product session ownership, adapter wire events, and workflow ownership belong
/// in `pi-coding-agent`. This module intentionally exposes low-level agent,
/// tool, hook, resource, and environment contracts.
pub mod api {
    pub use crate::agent::Agent;
    pub use crate::env::{
        ExecOptions, ExecutionEnv, ExecutionOutput, FileInfo, FileKind, FileSystem,
        InMemoryExecutionEnv, Shell,
    };
    pub use crate::errors::{ExecutionError, FileError};
    pub use crate::hooks::{
        AfterToolCallContext, AfterToolCallHook, AfterToolCallResult, AgentHooks,
        BeforeToolCallContext, BeforeToolCallHook, BeforeToolCallResult, ConvertToLlmHook,
        ShouldStopAfterTurnHook, TransformContextHook,
    };
    pub use crate::types::{
        AgentConfig, AgentEvent, AgentMessage, AgentResources, AgentStream, AgentTool,
        AgentToolOutput, AgentToolResult, CompactionConfig, CompactionSettings, PromptTemplate,
        ProviderRequestSnapshot, QueueMode, ResourceDiagnostic, Skill, SourceTag,
        SourcedPromptTemplate, SourcedResourceDiagnostic, SourcedSkill, ThinkingLevel,
        ToolExecutionMode, ToolFn, ToolUpdateCallback,
    };
}
```

- [ ] **Step 4: Verify the test passes**

Run:

```bash
. "$HOME/.cargo/env"
cargo test -p pi-agent-core --test public_api
```

Expected: the new public API test passes.

- [ ] **Step 5: Commit pi-agent-core facade slice**

```bash
git add crates/pi-agent-core/src/lib.rs crates/pi-agent-core/tests/public_api.rs
git commit -m "feat(agent-core): add low-level api facade"
```

### Task 4: Add `pi-tui::api` Facade

**Files:**
- Modify: `crates/pi-tui/src/lib.rs`
- Modify: `crates/pi-tui/tests/public_api.rs`

- [ ] **Step 1: Extend the public API test first**

Add these imports to `crates/pi-tui/tests/public_api.rs`:

```rust
use pi_tui::api::{
    AutocompleteItem as ApiAutocompleteItem, Box as ApiBox, Component as ApiComponent,
    Editor as ApiEditor, InputEvent as ApiInputEvent, Key as ApiKey, KeybindingsManager as ApiKeybindingsManager,
    Markdown as ApiMarkdown, OverlayOptions as ApiOverlayOptions, ProcessTerminal as ApiProcessTerminal,
    RenderScheduler as ApiRenderScheduler, RenderStrategy as ApiRenderStrategy, Terminal as ApiTerminal,
    Text as ApiText, Tui as ApiTui, TuiTheme as ApiTuiTheme, VirtualTerminal as ApiVirtualTerminal,
};
```

Add this test:

```rust
#[test]
fn generic_tui_symbols_are_importable_from_api_facade() {
    fn accepts_types(
        _autocomplete: Option<ApiAutocompleteItem>,
        _box_component: Option<ApiBox>,
        _editor: Option<ApiEditor>,
        _input_event: Option<ApiInputEvent>,
        _key: Option<ApiKey>,
        _keybindings: Option<ApiKeybindingsManager>,
        _markdown: Option<ApiMarkdown>,
        _overlay: Option<ApiOverlayOptions>,
        _process_terminal: Option<ApiProcessTerminal>,
        _render_scheduler: Option<ApiRenderScheduler>,
        _render_strategy: Option<ApiRenderStrategy>,
        _text: Option<ApiText>,
        _theme: Option<ApiTuiTheme>,
        _virtual_terminal: Option<ApiVirtualTerminal>,
    ) {
    }

    accepts_types(
        None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    );

    fn accepts_component<T: ApiComponent>() {}
    fn accepts_terminal<T: ApiTerminal>() {}
    let _ = accepts_component::<ApiText>;
    let _ = accepts_terminal::<ApiVirtualTerminal>;
    let _ = std::any::type_name::<ApiTui<ApiVirtualTerminal>>();
}
```

- [ ] **Step 2: Verify the test fails**

Run:

```bash
. "$HOME/.cargo/env"
cargo test -p pi-tui --test public_api
```

Expected: compile failure containing `could not find api in pi_tui`.

- [ ] **Step 3: Add the minimal facade**

Append this module to `crates/pi-tui/src/lib.rs` after the existing re-exports:

```rust
/// Stable generic terminal UI facade.
///
/// Product-specific coding-agent actions, sessions, model state, tree state,
/// tools, and plugin dispatch belong in `pi-coding-agent` adapters.
pub mod api {
    pub use crate::autocomplete::{
        AutocompleteItem, AutocompleteOptions, AutocompleteProvider, AutocompleteSuggestions,
        CombinedAutocompleteProvider, CompletionEdit, SlashCommand,
    };
    pub use crate::component::{Component, ComponentId, Container};
    pub use crate::components::{
        Box, CancellableLoader, Editor, Image, Input, Loader, Markdown, SelectItem, SelectList,
        SelectorDialog, SelectorDialogOptions, SettingItem, SettingsList, SettingsListOptions,
        Spacer, Text, TruncatedText,
    };
    pub use crate::input::{
        InputEvent, Key, KeyEvent, KeyEventKind, KeyModifiers, KeybindingConflict,
        KeybindingDefinition, KeybindingsConfig, KeybindingsManager, StdinBuffer,
    };
    pub use crate::overlay::{OverlayAnchor, OverlayHandle, OverlayOptions};
    pub use crate::runtime::RenderScheduler;
    pub use crate::terminal::{ProcessTerminal, Terminal, TerminalSize};
    pub use crate::theme::{ThemeMode, ThemePalette, TuiTheme, dark_theme, light_theme};
    pub use crate::tui::{InputListenerResult, RenderOutcome, RenderStrategy, RenderSurface, Tui, TuiError};
    pub use crate::virtual_terminal::{TerminalOp, VirtualTerminal};
}
```

- [ ] **Step 4: Verify the test passes**

Run:

```bash
. "$HOME/.cargo/env"
cargo test -p pi-tui --test public_api
```

Expected: the public API test passes.

- [ ] **Step 5: Commit pi-tui facade slice**

```bash
git add crates/pi-tui/src/lib.rs crates/pi-tui/tests/public_api.rs
git commit -m "feat(tui): add generic api facade"
```

### Task 5: Verify `pi-coding-agent::api` Boundary Coverage

**Files:**
- Inspect: `crates/pi-coding-agent/tests/public_api.rs`
- Modify only if needed: `crates/pi-coding-agent/tests/public_api.rs`

- [ ] **Step 1: Inspect current coverage**

Run:

```bash
rg -n "CodingAgentSession|CodingAgentCapabilities|CapabilityStatus|CodingAgentEvent|PromptTurnOptions|PromptTurnOutcome|PluginCapabilities" crates/pi-coding-agent/tests/public_api.rs
```

Expected: public API test imports `CodingAgentSession`, capability types, prompt turn types, and event types through `pi_coding_agent::api`.

- [ ] **Step 2: Add missing imports if the grep misses required symbols**

If a required symbol is missing, add it to the existing `use pi_coding_agent::api::{ ... }` block and reference it in the relevant compile-smoke test using `Option<Symbol>` or constructor code already used in the file. Do not expose `crate::plugins` or adapter internals through the facade.

- [ ] **Step 3: Verify focused test**

Run:

```bash
. "$HOME/.cargo/env"
cargo test -p pi-coding-agent --test public_api
```

Expected: public API tests pass.

- [ ] **Step 4: Commit only if the test file changed**

```bash
git add crates/pi-coding-agent/tests/public_api.rs
git commit -m "test(coding-agent): cover stable api boundary"
```

### Task 6: Final P0 Verification

**Files:**
- All files changed by Tasks 1-5

- [ ] **Step 1: Run formatting**

```bash
. "$HOME/.cargo/env"
cargo fmt --check
```

Expected: exit 0.

- [ ] **Step 2: Run focused public API tests**

```bash
. "$HOME/.cargo/env"
cargo test -p pi-ai --test public_api
cargo test -p pi-agent-core --test public_api
cargo test -p pi-coding-agent --test public_api
cargo test -p pi-tui --test public_api
```

Expected: all public API tests pass.

- [ ] **Step 3: Run workspace check**

```bash
. "$HOME/.cargo/env"
cargo check --workspace
```

Expected: exit 0.

- [ ] **Step 4: Run diff hygiene check**

```bash
git diff --check
```

Expected: no whitespace errors.

- [ ] **Step 5: Update TODO if all P0 checks pass**

If Tasks 1-5 are implemented and Step 1-4 pass, change the four P0 boundary TODO lines from `[~]` to `[x]` with a note that facade modules and public API tests are in place.

- [ ] **Step 6: Commit verification TODO update**

```bash
git add docs/TODO.md
git commit -m "docs: mark p0 boundary facade checks complete"
```
