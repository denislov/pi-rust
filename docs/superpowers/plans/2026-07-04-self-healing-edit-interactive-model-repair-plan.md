# Self-Healing Edit Interactive Model Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose bounded session-owned self-healing edit model repair through the interactive `/self-healing-edit` command.

**Architecture:** Extend the existing narrow interactive command shape with product-level flags: `--model-repair` and `--model-repair-attempts N`. The command parser stores only a bounded policy on `PendingSelfHealingEditRequest`; the interactive loop maps that policy to `SelfHealingEditModelRepairOptions` from the current `PromptContext`, using a fixed repair prompt and without exposing provider, runtime, or session internals.

**Tech Stack:** Rust 2024, `pi-coding-agent` interactive command parser, `PromptRunOptions`, `PromptTurnOptions`, `SelfHealingEditRequest`, `SelfHealingEditModelRepairOptions`, faux provider interactive tests.

---

### Task 1: Add RED Parser Coverage

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`
- Modify: `crates/pi-coding-agent/src/interactive/root.rs`

- [x] **Step 1: Add pending policy expectations**

Add fields to the existing app-level self-healing edit tests after the parser has a desired shape:

```rust
assert_eq!(request.model_repair, None);
```

For the check-command test, keep the existing check assertion and add:

```rust
assert_eq!(request.model_repair, None);
```

- [x] **Step 2: Add model repair parser test**

Add `self_healing_edit_command_queues_model_repair_policy`:

```rust
#[test]
fn self_healing_edit_command_queues_model_repair_policy() {
    let mut root = InteractiveRoot::new(
        PathBuf::from("."),
        "faux-model".to_string(),
        "session".to_string(),
    );

    root.handle_slash_command(ParsedSlashCommand {
        name: "self-healing-edit".to_string(),
        args: "src/app.txt old value => new value --model-repair-attempts 2 --check cargo test --quiet".to_string(),
        original: "/self-healing-edit src/app.txt old value => new value --model-repair-attempts 2 --check cargo test --quiet".to_string(),
    });

    assert_eq!(root.take_action(), InteractiveAction::SelfHealingEdit);
    let request = root
        .take_pending_self_healing_edit_request()
        .expect("self-healing edit request should be queued");
    assert_eq!(request.path, "src/app.txt");
    assert_eq!(request.replacements[0].old_text, "old value");
    assert_eq!(request.replacements[0].new_text, "new value");
    assert_eq!(request.check_command.as_deref(), Some("cargo test --quiet"));
    let model_repair = request
        .model_repair
        .expect("model repair policy should be queued");
    assert_eq!(model_repair.max_attempts, 2);
}
```

- [x] **Step 3: Add invalid attempts parser test**

Add `self_healing_edit_command_rejects_invalid_model_repair_attempts`:

```rust
#[test]
fn self_healing_edit_command_rejects_invalid_model_repair_attempts() {
    let mut root = InteractiveRoot::new(
        PathBuf::from("."),
        "faux-model".to_string(),
        "session".to_string(),
    );

    root.handle_slash_command(ParsedSlashCommand {
        name: "self-healing-edit".to_string(),
        args: "src/app.txt old => new --model-repair-attempts nope".to_string(),
        original: "/self-healing-edit src/app.txt old => new --model-repair-attempts nope".to_string(),
    });

    assert_eq!(root.take_action(), InteractiveAction::None);
    assert!(root.take_pending_self_healing_edit_request().is_none());
    let text = last_system_text(&root);
    assert!(
        text.contains("Usage: /self-healing-edit <path> <oldText> => <newText>"),
        "{text}"
    );
}
```

- [x] **Step 4: Run RED parser tests**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent self_healing_edit_command_ --lib -- --nocapture
```

Expected: compile failure because the pending request does not have `model_repair` yet.

### Task 2: Parse Interactive Policy

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/root.rs`
- Modify: `crates/pi-coding-agent/src/interactive/commands.rs`
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`

- [x] **Step 1: Add pending policy type**

Add this type near `PendingSelfHealingEditRequest`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingSelfHealingEditModelRepair {
    pub(super) max_attempts: usize,
}
```

Extend `PendingSelfHealingEditRequest`:

```rust
pub(super) model_repair: Option<PendingSelfHealingEditModelRepair>,
```

- [x] **Step 2: Extend usage text**

Use this usage string in `parse_self_healing_edit_args`:

```rust
let usage = "Usage: /self-healing-edit <path> <oldText> => <newText> [--model-repair] [--model-repair-attempts N] [--check <command>]";
```

- [x] **Step 3: Parse suffix policy**

Add a parser helper that accepts model-repair flags before or after `--check` while preserving `--check` as a command-with-spaces suffix:

```rust
fn parse_self_healing_edit_model_repair_suffix(
    value: &str,
    usage: &str,
) -> Result<(String, Option<PendingSelfHealingEditModelRepair>), String> {
    let mut value = value.trim().to_string();
    let mut max_attempts = None;
    let mut enabled = false;
    loop {
        if let Some(before) = value.strip_suffix(" --model-repair") {
            enabled = true;
            value = before.trim_end().to_string();
            continue;
        }
        if let Some((before, attempts)) = value.rsplit_once(" --model-repair-attempts ") {
            let attempts = attempts.trim();
            if attempts.is_empty() {
                return Err(usage.to_string());
            }
            if attempts.chars().any(char::is_whitespace) {
                break;
            }
            let attempts = attempts
                .parse::<usize>()
                .ok()
                .filter(|attempts| *attempts > 0)
                .ok_or_else(|| usage.to_string())?;
            enabled = true;
            max_attempts = Some(attempts);
            value = before.trim_end().to_string();
            continue;
        }
        break;
    }
    if value == "--model-repair-attempts"
        || value.ends_with(" --model-repair-attempts")
        || value.ends_with(" --model-repair-attempts ")
    {
        return Err(usage.to_string());
    }
    let policy = enabled.then_some(PendingSelfHealingEditModelRepair {
        max_attempts: max_attempts.unwrap_or(1),
    });
    Ok((value, policy))
}
```

- [x] **Step 4: Merge suffixes into request**

In `parse_self_healing_edit_args`, parse model repair flags once before `--check` extraction and once after, then merge the policy:

```rust
let (new_text, model_repair_after_check) =
    parse_self_healing_edit_model_repair_suffix(new_text.trim(), usage)?;
let (new_text, check_command) = parse_self_healing_edit_check_suffix(&new_text, usage)?;
let (new_text, model_repair_before_check) =
    parse_self_healing_edit_model_repair_suffix(&new_text, usage)?;
let model_repair = model_repair_after_check.or(model_repair_before_check);
```

Construct `PendingSelfHealingEditRequest` with `model_repair`.

- [x] **Step 5: Run parser tests GREEN**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent self_healing_edit_command_ --lib -- --nocapture
```

Expected: both tests pass.

### Task 3: Map Policy To Session API

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/loop.rs`

- [x] **Step 1: Import API types**

Update imports:

```rust
use pi_agent_core::{AgentResources, session::create_session_id};
use crate::coding_session::{
    CodingAgentSession, PluginLoadOutcome, PromptTurnOptions, PromptTurnOutcome,
    SelfHealingEditModelRepairOptions, SelfHealingEditRequest,
};
```

- [x] **Step 2: Add mapping helper**

Add helper near `start_self_healing_edit_task`:

```rust
fn interactive_self_healing_model_repair_options(
    prompt_context: &PromptContext,
    max_attempts: usize,
) -> SelfHealingEditModelRepairOptions {
    let prompt = "repair self-healing edit".to_string();
    let prompt_options = PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
        prompt: prompt.clone(),
        model: prompt_context.model.clone(),
        api_key: prompt_context.api_key.clone(),
        system_prompt: Some("Return only self-healing edit repair JSON.".to_string()),
        max_turns: Some(1),
        tools: prompt_context.tools.clone(),
        register_builtins: false,
        session: prompt_context.session.clone(),
        session_target: None,
        session_name: prompt_context.session_name.clone(),
        thinking_level: prompt_context.thinking_level,
        tool_execution: None,
        resources: AgentResources::default(),
        settings: Some(prompt_context.settings.clone()),
        invocation: PromptInvocation::Text(prompt),
    });
    SelfHealingEditModelRepairOptions::new(prompt_options).with_max_attempts(max_attempts)
}
```

- [x] **Step 3: Attach model repair to request**

In `start_self_healing_edit_task`, after applying `check_command`, add:

```rust
if let Some(model_repair) = request.model_repair {
    edit_request = edit_request.with_model_repair(
        interactive_self_healing_model_repair_options(prompt_context, model_repair.max_attempts),
    );
}
```

### Task 4: Add GREEN End-To-End Coverage

**Files:**
- Modify: `crates/pi-coding-agent/tests/interactive_mode.rs`

- [x] **Step 1: Import scripted helper**

Add `run_scripted_interactive_with_session_dir_size_and_waits` if it is not already imported.

- [x] **Step 2: Add scripted repair test**

Add:

```rust
#[tokio::test]
async fn scripted_interactive_self_healing_edit_uses_model_repair_policy() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("src")).unwrap();
    std::fs::write(temp.path().join("src/app.txt"), "one\ntwo\nthree\n").unwrap();
    let provider = FauxProvider::simple_text(
        r#"{"edits":[{"oldText":"deux","newText":"dos"}]}"#,
    );

    let output = run_scripted_interactive_with_session_dir_size_and_waits(
        provider,
        temp.path(),
        vec![
            (
                "/self-healing-edit src/app.txt two => deux --model-repair --check grep -q dos src/app.txt\r",
                "self_healing_edit.completed",
            ),
            ("\x03", ""),
        ],
        80,
        24,
    )
    .await
    .expect("scripted interactive self-healing edit should succeed");

    assert_eq!(
        std::fs::read_to_string(temp.path().join("src/app.txt")).unwrap(),
        "one\ndos\nthree\n"
    );
    assert!(output.contains("Successfully replaced"), "{output:?}");
    assert_eq!(output.exit_code, 0);
}
```

- [x] **Step 3: Run focused scripted test**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test interactive_mode scripted_interactive_self_healing_edit_uses_model_repair_policy -- --nocapture
```

Expected: pass after Task 3 implementation.

### Task 5: Update Docs And Verify

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-04-self-healing-edit-interactive-model-repair-plan.md`

- [x] **Step 1: Update TODO**

Add this plan to Source Documents and update the Phase 6 self-healing edit note to say interactive mode exposes bounded model repair through `/self-healing-edit --model-repair`.

- [x] **Step 2: Mark plan steps complete**

Mark checkboxes as each step completes.

- [x] **Step 3: Run verification**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt --check
/home/whai/.cargo/bin/cargo test -p pi-coding-agent self_healing_edit_command_ --lib -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test interactive_mode scripted_interactive_self_healing_edit_uses_model_repair_policy -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test interactive_mode --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet
/home/whai/.cargo/bin/cargo check --workspace --quiet
git diff --check
```

Expected: all commands exit 0.
