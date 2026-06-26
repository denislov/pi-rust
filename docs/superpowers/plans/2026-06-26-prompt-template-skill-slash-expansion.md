# Prompt Template & Skill Slash Command Expansion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Enable /templatename args and /skill:name args slash commands in the Rust interactive TUI, expanding them to their full content before sending to the model, with autocomplete support.

**Architecture:** Expand prompt text in handle_slash_command's catch-all branch (commands.rs) when no builtin command matches — this is the only path where /templatename and /skill:name text arrives. InteractiveRoot holds prompt_templates and skills from PromptContext. slash.rs suggestion functions are generalized to accept dynamic command lists. pi-agent-core gains full TS-compatible substituteArgs and parseCommandArgs.

**Tech Stack:** Rust 2024 edition, pi-agent-core, pi-coding-agent, pi-tui

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| pi-agent-core/Cargo.toml | Modify | Add regex dependency |
| pi-agent-core/src/resources/system_prompt.rs | Modify | parse_command_args, substitute_args, enhanced format_prompt_template_invocation |
| pi-agent-core/src/resources/mod.rs | Modify | Export parse_command_args, substitute_args |
| pi-agent-core/src/lib.rs | Modify | Re-export parse_command_args, substitute_args |
| pi-coding-agent/src/interactive/slash.rs | Modify | BuiltinSlashCommand String fields, dynamic suggestion lists |
| pi-coding-agent/src/interactive/root.rs | Modify | Store prompt_templates/skills, expand_prompt_text, dynamic slash commands |
| pi-coding-agent/src/interactive/commands.rs | Modify | expand_skill_command, expand_prompt_template functions + expand in slash dispatch catch-all |
| pi-coding-agent/src/interactive/app.rs | Modify | Tests for expansion and autocomplete + fix BUILTIN_SLASH_COMMANDS import |

---

### Task 1: Enhance prompt template arg substitution with full TS parity

**Files:**
- Modify: pi-rust/crates/pi-agent-core/Cargo.toml
- Modify: pi-rust/crates/pi-agent-core/src/resources/system_prompt.rs
- Modify: pi-rust/crates/pi-agent-core/src/resources/mod.rs
- Modify: pi-rust/crates/pi-agent-core/src/lib.rs

- [ ] **Step 1: Add regex dependency**

In pi-rust/crates/pi-agent-core/Cargo.toml, add under [dependencies]:
```toml
regex = "1"
```

- [ ] **Step 2: Implement parse_command_args and substitute_args**

In pi-rust/crates/pi-agent-core/src/resources/system_prompt.rs, add two new public functions after the existing format_prompt_template_invocation:

1. parse_command_args(args_string: &str) -> Vec<String> - bash-style quoted argument parser (mirrors TS parseCommandArgs)
2. substitute_args(content: &str, args: &[String]) -> String - replaces $1, $2, $@, $ARGUMENTS, ${N:-default}, ${@:N}, ${@:N:L} placeholders using regex

Update format_prompt_template_invocation to delegate to substitute_args.

See the design doc for exact implementation code.

- [ ] **Step 3: Add tests**

Append to the existing test module in system_prompt.rs (keep all existing tests). Add tests for:
- $@ and $ARGUMENTS replacement
- ${N:-default} with missing, empty, and present values
- ${@:N} slice from N
- ${@:N:L} slice from N with length L
- ${@:0} treats 0 as 1
- parse_command_args: whitespace split, double quotes, single quotes, empty input, whitespace-only

- [ ] **Step 4: Export new functions**

In pi-rust/crates/pi-agent-core/src/resources/mod.rs, update pub use to export parse_command_args and substitute_args.

In pi-rust/crates/pi-agent-core/src/lib.rs, add re-exports for parse_command_args and substitute_args.

- [ ] **Step 5: Run tests**

Run: cargo test -p pi-agent-core --lib resources::system_prompt::tests
Expected: all tests PASS

- [ ] **Step 6: Commit**

```bash
cd pi-rust
git add crates/pi-agent-core/Cargo.toml crates/pi-agent-core/src/resources/system_prompt.rs crates/pi-agent-core/src/resources/mod.rs crates/pi-agent-core/src/lib.rs
git commit -m "feat(agent-core): enhance prompt template arg substitution with full TS parity"
```

---

### Task 2: Generalize BuiltinSlashCommand to support dynamic command lists

**Files:**
- Modify: pi-rust/crates/pi-coding-agent/src/interactive/slash.rs
- Modify: pi-rust/crates/pi-coding-agent/src/interactive/app.rs (test imports)

- [ ] **Step 1: Change BuiltinSlashCommand to use String fields**

Replace the struct definition - change name and description from &'static str to String.

Replace BUILTIN_SLASH_COMMANDS const with a function builtin_slash_commands() -> Vec<BuiltinSlashCommand> that returns the same 22 commands.

- [ ] **Step 2: Update suggestion_indices to accept a dynamic command list**

Add commands: &[BuiltinSlashCommand] parameter. Replace BUILTIN_SLASH_COMMANDS reference with commands.

- [ ] **Step 3: Update render_suggestions to accept a dynamic command list**

Add commands: &[BuiltinSlashCommand] parameter. Replace all BUILTIN_SLASH_COMMANDS references with commands.

- [ ] **Step 4: Update handle_suggestion_input to accept a dynamic command list**

Add commands: &[BuiltinSlashCommand] parameter. Replace all BUILTIN_SLASH_COMMANDS references with commands.

- [ ] **Step 5: Update help_text to use the function**

Change help_text() to call builtin_slash_commands() instead of referencing BUILTIN_SLASH_COMMANDS.

- [ ] **Step 6: Update app.rs test imports**

In app.rs, change imports and test references from BUILTIN_SLASH_COMMANDS to builtin_slash_commands():
- Line 37-38: change `BUILTIN_SLASH_COMMANDS` to `builtin_slash_commands` in the use statement
- Line 910: change `BUILTIN_SLASH_COMMANDS.iter().map(|command| command.name)` to `builtin_slash_commands().iter().map(|command| command.name.clone())` (returns Vec<String> instead of Vec<&str>)
- Line 1478: change `for command in BUILTIN_SLASH_COMMANDS` to `for command in builtin_slash_commands()`

- [ ] **Step 7: Run tests to verify no regressions**

Run: cargo test -p pi-coding-agent --lib interactive::app
Expected: all existing slash tests PASS

- [ ] **Step 8: Commit**

```bash
cd pi-rust
git add crates/pi-coding-agent/src/interactive/slash.rs
git commit -m "feat(coding-agent): generalize BuiltinSlashCommand to support dynamic command lists"
```

---

### Task 3: Add prompt_templates and skills to InteractiveRoot

**Files:**
- Modify: pi-rust/crates/pi-coding-agent/src/interactive/root.rs

- [ ] **Step 1: Add fields to InteractiveRoot struct**

After pub(super) resolved_theme (line 117), add:
```rust
pub(super) prompt_templates: Vec<pi_agent_core::PromptTemplate>,
pub(super) skills: Vec<pi_agent_core::Skill>,
```

- [ ] **Step 2: Initialize in constructor**

In new_with_theme_models_and_settings, add after resolved_theme: None,:
```rust
prompt_templates: Vec::new(),
skills: Vec::new(),
```

- [ ] **Step 3: Populate in apply_prompt_context**

In apply_prompt_context (line 319), add after self.git_branch.set_cwd(&self.cwd);:
```rust
self.prompt_templates = prompt_context.resources.prompt_templates.clone();
self.skills = prompt_context.resources.skills.clone();
```

- [ ] **Step 4: Add expand_prompt_text method**

After apply_prompt_context, add:
```rust
pub(super) fn expand_prompt_text(&self, text: &str) -> String {
    let text = crate::interactive::commands::expand_skill_command(text, &self.skills);
    crate::interactive::commands::expand_prompt_template(text, &self.prompt_templates)
}
```

- [ ] **Step 5: Add all_slash_commands method**

Add method to build the combined command list for autocomplete:
```rust
pub(super) fn all_slash_commands(&self) -> Vec<slash::BuiltinSlashCommand> {
    let mut commands = slash::builtin_slash_commands();
    for t in &self.prompt_templates {
        commands.push(slash::BuiltinSlashCommand {
            name: t.name.clone(),
            description: t.description.clone(),
        });
    }
    for s in &self.skills {
        commands.push(slash::BuiltinSlashCommand {
            name: format!("skill:{}", s.name),
            description: s.description.clone(),
        });
    }
    commands
}
```

- [ ] **Step 6: Update render_slash_suggestions to pass dynamic commands**

Change render_slash_suggestions (line 732) to pass self.all_slash_commands() as the commands parameter to slash::render_suggestions.

- [ ] **Step 7: Update handle_slash_suggestion_input to pass dynamic commands**

Change handle_slash_suggestion_input (line 876) to pass self.all_slash_commands() as the commands parameter to slash::handle_suggestion_input.

- [ ] **Step 8: Run tests to verify no regressions**

Run: cargo test -p pi-coding-agent --lib interactive::app
Expected: all existing slash tests PASS

- [ ] **Step 9: Commit**

```bash
cd pi-rust
git add crates/pi-coding-agent/src/interactive/root.rs
git commit -m "feat(coding-agent): add prompt_templates and skills to InteractiveRoot"
```

---

### Task 4: Implement expand_skill_command and expand_prompt_template

**Files:**
- Modify: pi-rust/crates/pi-coding-agent/src/interactive/commands.rs

- [ ] **Step 1: Add imports**

Add at top of commands.rs:
```rust
use pi_agent_core::resources::parse_command_args;
use pi_agent_core::{PromptTemplate, Skill, substitute_args};
```

- [ ] **Step 2: Implement expand_skill_command**

Add function that:
1. Checks if text starts with /skill:
2. Extracts skill name and args
3. Looks up skill by name
4. Returns skill content wrapped in XML skill block (mirrors TS _expandSkillCommand)
5. Appends args after skill block if present
6. Returns original text unchanged if not a skill command or skill not found

- [ ] **Step 3: Implement expand_prompt_template**

Add function that:
1. Checks if text starts with /
2. Extracts template name and args string
3. Looks up template by name
4. Parses args with parse_command_args
5. Substitutes args into template content with substitute_args
6. Returns expanded content
7. Returns original text unchanged if not a template match

- [ ] **Step 4: Run tests to verify no regressions**

Run: cargo test -p pi-coding-agent --lib interactive::app
Expected: all existing slash tests PASS

- [ ] **Step 5: Commit**

```bash
cd pi-rust
git add crates/pi-coding-agent/src/interactive/commands.rs
git commit -m "feat(coding-agent): implement expand_skill_command and expand_prompt_template"
```

---

### Task 5: Insert expand step in handle_slash_command catch-all

> **Why here instead of input.rs?** The input.rs submit path splits into two branches:
> slash commands (`/...`) go to `handle_slash_command`, everything else goes straight
> to submit. `/templatename` starts with `/` so it enters the slash command path, and
> the `_ =>` catch-all would report "unknown command" before expansion ever runs.
> Placing expansion in the catch-all **after** the builtin command match ensures
> templates/skills are only tried when no builtin command matched.

**Files:**
- Modify: pi-rust/crates/pi-coding-agent/src/interactive/commands.rs

- [ ] **Step 1: Modify handle_slash_command catch-all**

In handle_slash_command (commands.rs), replace the `_ =>` branch to try template/skill expansion before reporting unknown command:

Before:
```rust
_ => {
    root.transcript.push(TranscriptItem::system(format!(
        "unknown command: {} - type /help for available commands",
        command.original
    )));
}
```

After:
```rust
_ => {
    let expanded = root.expand_prompt_text(&command.original);
    if expanded != command.original {
        root.editor.add_to_history(&expanded);
        root.pending_submit = Some(expanded);
        root.action = InteractiveAction::Submit;
    } else {
        root.transcript.push(TranscriptItem::system(format!(
            "unknown command: {} - type /help for available commands",
            command.original
        )));
    }
}
```

- [ ] **Step 2: Run tests to verify no regressions**

Run: cargo test -p pi-coding-agent --lib interactive::app
Expected: all existing tests PASS

- [ ] **Step 3: Commit**

```bash
cd pi-rust
git add crates/pi-coding-agent/src/interactive/commands.rs
git commit -m "feat(coding-agent): expand prompt templates and skills in slash command dispatch"
```

---

### Task 6: Add tests for expansion and autocomplete

**Files:**
- Modify: pi-rust/crates/pi-coding-agent/src/interactive/app.rs

- [ ] **Step 1: Add test for expand_skill_command**

Test that /skill:name expands to XML skill block with content.

- [ ] **Step 2: Add test for expand_prompt_template**

Test that /templatename args expands with arg substitution.

- [ ] **Step 3: Add test for non-matching text passes through**

Test that non-slash text and unknown commands pass through unchanged.

- [ ] **Step 4: Add test for autocomplete includes templates and skills**

Test that slash suggestions include prompt template names and skill: prefixed names when templates/skills are loaded.

- [ ] **Step 5: Run all tests**

Run: cargo test -p pi-coding-agent --lib interactive::app
Expected: all tests PASS

- [ ] **Step 6: Run workspace tests**

Run: cargo test --workspace
Expected: all tests PASS

- [ ] **Step 7: Commit**

```bash
cd pi-rust
git add crates/pi-coding-agent/src/interactive/app.rs
git commit -m "test(coding-agent): add tests for prompt template and skill slash expansion"
```
