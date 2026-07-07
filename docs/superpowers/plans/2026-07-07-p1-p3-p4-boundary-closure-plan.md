# P1/P3/P4 Boundary Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining P1 product runtime, P3 tool, and P4 UI boundary TODO items with guard-backed evidence and minimal runtime cleanup.

**Architecture:** Add focused source-guard tests around the existing Flow-centered architecture, then make the small production changes those guards expose: remove adapter-level global provider registration and validate product/plugin tools before they enter `Agent`. Keep `CodingAgentSession` as product owner, `pi-agent-core` as low-level runtime/tool owner, and `pi-tui` as generic terminal UI primitives.

**Tech Stack:** Rust 2024 workspace, cargo integration tests, source guard tests, CodeGraph-assisted audits, `cargo fmt`, `cargo test`, `cargo check`.

---

## File Structure

- Modify: `docs/TODO.md` to link the design/plan and close P1/P3/P4 when verified.
- Create: `docs/superpowers/specs/2026-07-07-p1-p3-p4-boundary-closure-design.md`.
- Create: `docs/superpowers/plans/2026-07-07-p1-p3-p4-boundary-closure-plan.md`.
- Create: `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` for P1 source guards.
- Modify: `crates/pi-coding-agent/src/protocol/rpc/state.rs` to remove adapter-level global built-in provider registration.
- Modify: `crates/pi-coding-agent/src/coding_session/runtime_service.rs` to validate tools with `Agent::try_add_tool()` before provider visibility.
- Create: `crates/pi-agent-core/tests/tool_boundary_guards.rs` for low-level tool/product separation guards.
- Create: `crates/pi-coding-agent/tests/tool_boundary_guards.rs` for product/plugin tool ingress guards.
- Create: `crates/pi-tui/tests/ui_boundary_guards.rs` for generic TUI/product separation guards.
- Create: `crates/pi-coding-agent/tests/plugin_ui_boundary_guards.rs` for plugin UI routing guards.

## Task 1: Wire Boundary Closure Docs

**Files:**
- Modify: `docs/TODO.md`
- Create: `docs/superpowers/specs/2026-07-07-p1-p3-p4-boundary-closure-design.md`
- Create: `docs/superpowers/plans/2026-07-07-p1-p3-p4-boundary-closure-plan.md`

- [ ] **Step 1: Add source document links**

Add these under `## Source Documents`:

```markdown
- [P1/P3/P4 boundary closure design](superpowers/specs/2026-07-07-p1-p3-p4-boundary-closure-design.md)
- [P1/P3/P4 boundary closure plan](superpowers/plans/2026-07-07-p1-p3-p4-boundary-closure-plan.md)
```

- [ ] **Step 2: Add a progress note**

Append this under `## Progress Log`:

```markdown
- 2026-07-07: P1/P3/P4 boundary closure started. The design and plan define guard-backed stop conditions for product runtime ownership, provider-visible tool validation/plugin ingress, and generic TUI/plugin UI routing boundaries.
```

- [ ] **Step 3: Verify docs**

Run:

```bash
git diff -- docs/TODO.md docs/superpowers/specs/2026-07-07-p1-p3-p4-boundary-closure-design.md docs/superpowers/plans/2026-07-07-p1-p3-p4-boundary-closure-plan.md
```

Expected: only the new docs, source-document links, and progress note.

- [ ] **Step 4: Commit docs slice**

```bash
git add docs/TODO.md docs/superpowers/specs/2026-07-07-p1-p3-p4-boundary-closure-design.md docs/superpowers/plans/2026-07-07-p1-p3-p4-boundary-closure-plan.md
git commit -m "docs: plan p1 p3 p4 boundary closure"
```

## Task 2: P1 Product Runtime Boundary Guard

**Files:**
- Create: `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/state.rs`

- [ ] **Step 1: Write the failing source guard**

Create `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` with source-scan helpers that:

```rust
use std::fs;
use std::path::{Path, PathBuf};

const ALLOWED_AGENT_RUNTIME_FILES: &[&str] = &[
    "crates/pi-coding-agent/src/coding_session/runtime_service.rs",
    "crates/pi-coding-agent/src/coding_session/prompt_flow.rs",
];

#[test]
fn adapters_do_not_register_global_provider_runtime() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();
    collect_source_violations(scan.repo_root(), &scan.crate_root.join("src"), &[], &mut violations, |line| {
        line.contains("register_builtin_providers_for_global_runtime(")
    });
    assert!(
        violations.is_empty(),
        "adapter/product source must not register the global provider runtime; normal product execution uses scoped AiClient runtime paths:\n{}",
        violations.join("\n")
    );
}

#[test]
fn adapter_sources_do_not_construct_or_run_low_level_agents() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();
    for relative_root in ["src/interactive", "src/protocol", "src/print_mode.rs"] {
        collect_source_violations(scan.repo_root(), &scan.crate_root.join(relative_root), ALLOWED_AGENT_RUNTIME_FILES, &mut violations, |line| {
            line.contains("Agent::new(")
                || line.contains("Agent::with_messages(")
                || line.contains(".run().await")
                || line.contains(".prompt(")
        });
    }
    assert!(
        violations.is_empty(),
        "adapters should route product execution through CodingAgentSession instead of low-level Agent construction or execution:\n{}",
        violations.join("\n")
    );
}

struct SourceScan {
    crate_root: PathBuf,
    repo_root: PathBuf,
}

impl SourceScan {
    fn new() -> Self {
        let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = crate_root.parent().and_then(Path::parent).expect("crate path").to_path_buf();
        Self { crate_root, repo_root }
    }
    fn repo_root(&self) -> &Path { &self.repo_root }
}

fn collect_source_violations(
    repo_root: &Path,
    path: &Path,
    allowed_files: &[&str],
    violations: &mut Vec<String>,
    is_violation: impl Copy + Fn(&str) -> bool,
) {
    let Ok(metadata) = fs::metadata(path) else { return; };
    if metadata.is_dir() {
        let mut entries = fs::read_dir(path).expect("read source dir").collect::<Result<Vec<_>, _>>().expect("read entries");
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect_source_violations(repo_root, &entry.path(), allowed_files, violations, is_violation);
        }
        return;
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") { return; }
    let relative = path.strip_prefix(repo_root).expect("under repo").to_string_lossy().replace('\\', "/");
    if allowed_files.contains(&relative.as_str()) { return; }
    let content = fs::read_to_string(path).expect("read source file");
    for (line_index, line) in content.lines().enumerate() {
        if is_violation(line) {
            violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
        }
    }
}
```

- [ ] **Step 2: Run the guard and verify it fails**

Run:

```bash
cargo test -p pi-coding-agent --test product_runtime_boundary_guards adapters_do_not_register_global_provider_runtime
```

Expected: FAIL showing `crates/pi-coding-agent/src/protocol/rpc/state.rs` calls `register_builtin_providers_for_global_runtime()`.

- [ ] **Step 3: Remove adapter-level global registration**

In `crates/pi-coding-agent/src/protocol/rpc/state.rs`, remove `coding_session::register_builtin_providers_for_global_runtime` from the grouped import and remove this block from `RpcState::new`:

```rust
if options.register_builtins {
    register_builtin_providers_for_global_runtime();
}
```

- [ ] **Step 4: Run focused P1 checks**

Run:

```bash
cargo test -p pi-coding-agent --test product_runtime_boundary_guards
cargo test -p pi-coding-agent --test provider_registry_boundary_guards
cargo test -p pi-coding-agent --test rpc
```

Expected: all pass.

- [ ] **Step 5: Commit P1 guard slice**

```bash
git add crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs crates/pi-coding-agent/src/protocol/rpc/state.rs
git commit -m "refactor(coding-agent): guard product runtime ownership"
```

## Task 3: P3 Validate Provider-Visible Tools

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/runtime_service.rs`
- Create: `crates/pi-coding-agent/tests/tool_boundary_guards.rs`
- Create: `crates/pi-agent-core/tests/tool_boundary_guards.rs`

- [ ] **Step 1: Write the failing product guard**

Create `crates/pi-coding-agent/tests/tool_boundary_guards.rs` with a source guard asserting `RuntimeService` uses `try_add_tool` and no longer calls `agent.add_tool(tool)` inside `build_agent_runtime_with_plugins_and_diagnostics`.

```rust
use std::fs;
use std::path::PathBuf;

#[test]
fn runtime_service_validates_tools_before_provider_visibility() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_root.join("src/coding_session/runtime_service.rs"))
        .expect("read runtime service");
    let start = source.find("fn build_agent_runtime_with_plugins_and_diagnostics(").expect("runtime builder");
    let end = source[start..].find("fn apply_tool_policy(").map(|offset| start + offset).expect("helper after builder");
    let body = &source[start..end];
    assert!(body.contains("agent.try_add_tool(tool)"), "RuntimeService must validate each provider-visible tool through Agent::try_add_tool");
    assert!(!body.contains("agent.add_tool(tool)"), "RuntimeService must not bypass tool validation with Agent::add_tool(tool)");
}

#[test]
fn plugin_tool_hosts_remain_capability_scoped() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_root.join("src/plugins/tool.rs")).expect("read tool host");
    for forbidden in ["CodingAgentSession", "SessionService", "RuntimeService", "FlowService", "AiClient", "ProviderRegistry", "ExecutionEnv", "FileSystem", "Shell"] {
        assert!(!source.contains(forbidden), "ToolRegistrationHost must not expose raw `{forbidden}` internals");
    }
}
```

- [ ] **Step 2: Verify the product guard fails**

Run:

```bash
cargo test -p pi-coding-agent --test tool_boundary_guards runtime_service_validates_tools_before_provider_visibility
```

Expected: FAIL because `RuntimeService` still uses `agent.add_tool(tool)`.

- [ ] **Step 3: Write the low-level core guard**

Create `crates/pi-agent-core/tests/tool_boundary_guards.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn agent_core_tool_runtime_has_no_coding_agent_product_imports() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root.parent().and_then(Path::parent).expect("crate path");
    let mut violations = Vec::new();
    collect_product_imports(repo_root, &crate_root.join("src"), &mut violations);
    assert!(violations.is_empty(), "pi-agent-core tool/runtime source must not import pi-coding-agent product ownership:\n{}", violations.join("\n"));
}

fn collect_product_imports(repo_root: &Path, path: &Path, violations: &mut Vec<String>) {
    let Ok(metadata) = fs::metadata(path) else { return; };
    if metadata.is_dir() {
        let mut entries = fs::read_dir(path).expect("read dir").collect::<Result<Vec<_>, _>>().expect("entries");
        entries.sort_by_key(|entry| entry.path());
        for entry in entries { collect_product_imports(repo_root, &entry.path(), violations); }
        return;
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") { return; }
    let relative = path.strip_prefix(repo_root).expect("under repo").to_string_lossy().replace('\\', "/");
    let content = fs::read_to_string(path).expect("read source");
    for (line_index, line) in content.lines().enumerate() {
        if line.contains("pi_coding_agent") || line.contains("CodingAgentSession") || line.contains("CodingAgentEvent") {
            violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
        }
    }
}
```

Run:

```bash
cargo test -p pi-agent-core --test tool_boundary_guards
```

Expected: PASS; this is a regression guard for the low-level boundary.

- [ ] **Step 4: Change RuntimeService to validate tools**

Replace this loop in `crates/pi-coding-agent/src/coding_session/runtime_service.rs`:

```rust
for tool in tools.into_iter().chain(policy_tools) {
    agent.add_tool(tool);
}
```

with:

```rust
for tool in tools.into_iter().chain(policy_tools) {
    agent.try_add_tool(tool).map_err(|error| CodingSessionError::Tool {
        message: error.to_string(),
    })?;
}
```

- [ ] **Step 5: Run focused P3 checks**

Run:

```bash
cargo test -p pi-coding-agent --test tool_boundary_guards
cargo test -p pi-agent-core --test tool_boundary_guards
cargo test -p pi-coding-agent plugin
cargo test -p pi-agent-core --test agent_turn_flow
```

Expected: all pass.

- [ ] **Step 6: Commit P3 slice**

```bash
git add crates/pi-coding-agent/src/coding_session/runtime_service.rs crates/pi-coding-agent/tests/tool_boundary_guards.rs crates/pi-agent-core/tests/tool_boundary_guards.rs
git commit -m "refactor: harden provider-visible tool boundary"
```

## Task 4: P4 Generic TUI and Plugin UI Guards

**Files:**
- Create: `crates/pi-tui/tests/ui_boundary_guards.rs`
- Create: `crates/pi-coding-agent/tests/plugin_ui_boundary_guards.rs`

- [ ] **Step 1: Add generic TUI source guard**

Create `crates/pi-tui/tests/ui_boundary_guards.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn tui_source_has_no_coding_agent_product_dependencies() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root.parent().and_then(Path::parent).expect("crate path");
    let mut violations = Vec::new();
    collect_product_terms(repo_root, &crate_root.join("src"), &mut violations);
    assert!(violations.is_empty(), "pi-tui source must remain generic and product-free:\n{}", violations.join("\n"));
}

fn collect_product_terms(repo_root: &Path, path: &Path, violations: &mut Vec<String>) {
    let Ok(metadata) = fs::metadata(path) else { return; };
    if metadata.is_dir() {
        let mut entries = fs::read_dir(path).expect("read dir").collect::<Result<Vec<_>, _>>().expect("entries");
        entries.sort_by_key(|entry| entry.path());
        for entry in entries { collect_product_terms(repo_root, &entry.path(), violations); }
        return;
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") { return; }
    let relative = path.strip_prefix(repo_root).expect("under repo").to_string_lossy().replace('\\', "/");
    let content = fs::read_to_string(path).expect("read source");
    for (line_index, line) in content.lines().enumerate() {
        for forbidden in ["pi_coding_agent", "CodingAgent", "AgentProfile", "TeamProfile", "SessionService", "CodingAgentSession", "CodingAgentEvent", "PluginUi", "plugin command", "app."] {
            if line.contains(forbidden) {
                violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
            }
        }
    }
}
```

- [ ] **Step 2: Add plugin UI routing guard**

Create `crates/pi-coding-agent/tests/plugin_ui_boundary_guards.rs`:

```rust
const INTERACTIVE_INPUT: &str = include_str!("../src/interactive/input.rs");
const INTERACTIVE_COMMANDS: &str = include_str!("../src/interactive/commands.rs");
const INTERACTIVE_ROOT: &str = include_str!("../src/interactive/root.rs");
const PROMPT_TASK: &str = include_str!("../src/interactive/prompt_task.rs");

#[test]
fn plugin_ui_routes_through_interactive_adapter_state() {
    assert!(PROMPT_TASK.contains("plugin_ui_actions(&session)"));
    assert!(PROMPT_TASK.contains("plugin_keybindings(&session)"));
    assert!(PROMPT_TASK.contains("plugin_ui_dialogs(&session)"));
    assert!(INTERACTIVE_INPUT.contains("root.handle_plugin_keybinding_input(event)"));
    assert!(INTERACTIVE_INPUT.contains("root.handle_plugin_dialog_form_input(event)"));
    assert!(INTERACTIVE_COMMANDS.contains("queue_plugin_command(root"));
    assert!(INTERACTIVE_COMMANDS.contains("validate_plugin_dialog_args"));
    assert!(INTERACTIVE_ROOT.contains("pending_plugin_command_request"));
    assert!(INTERACTIVE_ROOT.contains("active_plugin_ui_dialog"));
}

#[test]
fn plugin_ui_routing_does_not_live_in_pi_tui() {
    let crate_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root.parent().and_then(std::path::Path::parent).expect("crate path");
    let pi_tui_src = repo_root.join("crates/pi-tui/src");
    let mut violations = Vec::new();
    collect_plugin_ui_terms(repo_root, &pi_tui_src, &mut violations);
    assert!(
        violations.is_empty(),
        "plugin UI routing belongs in pi-coding-agent interactive adapters, not pi-tui:\n{}",
        violations.join("\n")
    );
}

fn collect_plugin_ui_terms(
    repo_root: &std::path::Path,
    path: &std::path::Path,
    violations: &mut Vec<String>,
) {
    let Ok(metadata) = std::fs::metadata(path) else { return; };
    if metadata.is_dir() {
        let mut entries = std::fs::read_dir(path)
            .expect("read pi-tui source directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read pi-tui entries");
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect_plugin_ui_terms(repo_root, &entry.path(), violations);
        }
        return;
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") { return; }
    let relative = path.strip_prefix(repo_root).expect("under repo").to_string_lossy().replace('\\', "/");
    let content = std::fs::read_to_string(path).expect("read pi-tui source file");
    for (line_index, line) in content.lines().enumerate() {
        for forbidden in ["PluginUi", "plugin_ui", "PendingPlugin", "active_plugin_ui_dialog", "pending_plugin_command_request"] {
            if line.contains(forbidden) {
                violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
            }
        }
    }
}
```

- [ ] **Step 3: Run focused P4 checks**

Run:

```bash
cargo test -p pi-tui --test ui_boundary_guards
cargo test -p pi-tui --test keybindings
cargo test -p pi-coding-agent --test plugin_ui_boundary_guards
cargo test -p pi-coding-agent --test interactive_sessions interactive_plugin_keybinding_opens_loaded_lua_dialog
```

Expected: all pass.

- [ ] **Step 4: Commit P4 slice**

```bash
git add crates/pi-tui/tests/ui_boundary_guards.rs crates/pi-coding-agent/tests/plugin_ui_boundary_guards.rs
git commit -m "test: guard generic tui plugin ui boundary"
```

## Task 5: Close TODO and Run Full Verification

**Files:**
- Modify: `docs/TODO.md`

- [ ] **Step 1: Update P1/P3/P4 TODO entries**

Change the three active `[~]` lines to `[x]` and summarize the evidence:

- P1: product runtime guard, removal of RPC adapter global provider registration, existing scoped runtime/provider guards.
- P3: validated product tool ingress with `try_add_tool`, plugin host scoped guard, core/product separation guard.
- P4: generic TUI source guard, product-free keybinding guard, plugin UI routing guard, existing interactive plugin UI tests.

- [ ] **Step 2: Run formatting**

```bash
cargo fmt --check
```

Expected: exit 0.

- [ ] **Step 3: Run focused boundary checks**

```bash
cargo test -p pi-coding-agent --test product_runtime_boundary_guards
cargo test -p pi-coding-agent --test provider_registry_boundary_guards
cargo test -p pi-coding-agent --test event_boundary_guards
cargo test -p pi-coding-agent --test session_boundary_guards
cargo test -p pi-coding-agent --test tool_boundary_guards
cargo test -p pi-agent-core --test tool_boundary_guards
cargo test -p pi-tui --test ui_boundary_guards
cargo test -p pi-tui --test keybindings
cargo test -p pi-coding-agent --test plugin_ui_boundary_guards
```

Expected: all pass.

- [ ] **Step 4: Run broader crate checks**

```bash
cargo test -p pi-agent-core
cargo test -p pi-coding-agent
cargo test -p pi-tui
cargo check --workspace
```

Expected: all pass.

- [ ] **Step 5: Run workspace and diff checks**

```bash
cargo test --workspace
git diff --check
```

Expected: all pass.

- [ ] **Step 6: Commit closure**

```bash
git add docs/TODO.md
git commit -m "docs: close p1 p3 p4 boundary backlog"
```

## Plan Self-Review

- Spec coverage: P1 product runtime ownership is covered by Task 2; P3 tool boundary is covered by Task 3; P4 UI boundary is covered by Task 4; TODO closure and verification are covered by Task 5.
- Placeholder scan: no unresolved placeholders are present; Task 4 Step 2 contains a real `pi-tui/src` scan for plugin UI routing terms.
- Type consistency: the plan uses existing symbols `CodingAgentSession`, `RuntimeService`, `Agent::try_add_tool`, `CodingSessionError::Tool`, `ToolRegistrationHost`, `PendingPluginUiDialog`, and `KeybindingsManager` as currently present in the workspace.
