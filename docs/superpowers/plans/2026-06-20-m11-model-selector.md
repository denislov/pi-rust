# M11 Model Selector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Rust interactive `/model` as the first real M11 selector vertical slice.

**Architecture:** Keep slash dispatch in `crates/pi-coding-agent/src/interactive/app.rs`, but add a small model-selection state boundary so `/model` can update local UI state without submitting to the provider. The outer interactive loop updates its mutable `PromptContext.model` after a confirmed switch, ensuring subsequent prompts use the selected model.

**Tech Stack:** Rust 2024, `pi-coding-agent` scripted interactive harness, `pi-tui::SelectList`, `pi_ai::lookup_model`, `pi_ai::all_models`, faux provider tests.

---

## File Structure

- Modify `crates/pi-coding-agent/src/interactive/app.rs`: add model selector state, direct `/model <id>` handling, selector dispatch, and prompt context model updates.
- Modify `crates/pi-coding-agent/tests/interactive_mode.rs`: add scripted tests for direct switch, invalid id, selector confirmation, selector cancellation, and subsequent prompt model usage.
- Modify `docs/roadmap/M11-interactive-ux.md`: record that `/model` has moved beyond a placeholder once tests pass.

## Task 1: Direct `/model <id>` Switch

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`
- Modify: `crates/pi-coding-agent/tests/interactive_mode.rs`

- [ ] **Step 1: Write failing scripted test for direct model switch**

Add a test near the current slash command tests in `crates/pi-coding-agent/tests/interactive_mode.rs`:

```rust
#[tokio::test]
async fn scripted_interactive_model_command_switches_footer_model() {
    let output = run_scripted_idle_interactive("/model claude-haiku-4-5\r")
        .await
        .unwrap();
    assert!(
        output.contains("Model set: claude-haiku-4-5"),
        "{output:?}"
    );
    assert!(output.contains("model: claude-haiku-4-5"), "{output:?}");
    assert_eq!(output.exit_code, 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p pi-coding-agent scripted_interactive_model_command_switches_footer_model
```

Expected: FAIL because `/model claude-haiku-4-5` still reports "not implemented".

- [ ] **Step 3: Implement minimal direct model switch**

In `InteractiveRoot`, add a `selected_model: Option<Model>` field and route `"model"` with non-empty args to a new `handle_model_command(&command.args)` method. The method must:

```rust
fn handle_model_command(&mut self, args: &str) {
    if args.is_empty() {
        self.open_model_selector();
        return;
    }

    match pi_ai::lookup_model(args) {
        Some(model) => {
            self.model_id = model.id.clone();
            self.selected_model = Some(model);
            self.transcript.push(TranscriptItem::system(format!(
                "Model set: {}",
                self.model_id
            )));
        }
        None => {
            self.transcript.push(TranscriptItem::system(format!(
                "Unknown model: {args}"
            )));
        }
    }
}
```

Add a `take_selected_model(&mut self) -> Option<Model>` helper. In `process_input_event`, after `root.handle_input(&event)`, take this selected model and return it through loop control or another small local update path so `PromptContext.model` is changed before the next prompt.

- [ ] **Step 4: Run direct switch test to verify it passes**

Run:

```bash
cargo test -p pi-coding-agent scripted_interactive_model_command_switches_footer_model
```

Expected: PASS.

## Task 2: Direct Switch Updates Subsequent Prompt Model

**Files:**
- Modify: `crates/pi-coding-agent/tests/interactive_mode.rs`
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`

- [ ] **Step 1: Write failing provider model assertion test**

Use the existing faux provider scripted harness to send `/model claude-haiku-4-5`, then a prompt. Assert the faux provider saw `claude-haiku-4-5`. If the provider request log does not expose model ids yet, add a small test helper in the existing test harness rather than changing production provider APIs.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p pi-coding-agent scripted_interactive_model_command_changes_next_prompt_model
```

Expected: FAIL until `PromptContext.model` is updated.

- [ ] **Step 3: Wire selected model into `PromptContext`**

Change `run_started_interactive_loop` to accept `mut prompt_context: PromptContext`. Extend `LoopControl::Continue` or `RenderRequest` handling so a confirmed model update from `InteractiveRoot` assigns:

```rust
prompt_context.model = model;
```

Keep the render request forced after a model switch so the footer redraws immediately.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p pi-coding-agent scripted_interactive_model_command_changes_next_prompt_model
```

Expected: PASS.

## Task 3: `/model` Selector Open, Confirm, Cancel

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`
- Modify: `crates/pi-coding-agent/tests/interactive_mode.rs`

- [ ] **Step 1: Write failing selector confirmation test**

Add a scripted test that sends `/model`, types a fuzzy query for `claude-haiku-4-5`, presses Enter, and asserts the footer and transcript show the selected model.

- [ ] **Step 2: Write failing selector cancellation test**

Add a scripted test that sends `/model`, presses Escape, and asserts the original model remains in the footer and no "Model set:" message appears.

- [ ] **Step 3: Run selector tests to verify they fail**

Run:

```bash
cargo test -p pi-coding-agent scripted_interactive_model_selector
```

Expected: FAIL because `/model` still does not open a selector.

- [ ] **Step 4: Implement selector state**

Add an enum to `InteractiveRoot`:

```rust
enum ActiveModal {
    ModelSelector(SelectList),
}
```

When `/model` has no args, build a deterministic `SelectList` from `pi_ai::all_models()`. Labels should be model ids; descriptions should include provider and display name. While the modal is active, `InteractiveRoot::handle_input` should send input to the selector first. Confirm should resolve the selected item back to a `Model`, set `selected_model`, close the modal, and push `Model set: <id>`. Cancel should close the modal without changing the model.

- [ ] **Step 5: Render selector**

Render the selector above the footer, using bounded width and existing `SelectList::render`. Keep this minimal; do not introduce a full dialog framework in this task.

- [ ] **Step 6: Run selector tests to verify they pass**

Run:

```bash
cargo test -p pi-coding-agent scripted_interactive_model_selector
```

Expected: PASS.

## Task 4: Roadmap And Verification

**Files:**
- Modify: `docs/roadmap/M11-interactive-ux.md`

- [ ] **Step 1: Update roadmap progress**

Add a progress note under M11 item 1 stating that `/model <id>` and `/model` selector are implemented in Rust interactive mode, while other selector commands remain pending.

- [ ] **Step 2: Run focused checks**

Run:

```bash
cargo fmt --check
cargo test -p pi-coding-agent interactive
```

Expected: both pass.

- [ ] **Step 3: Run `pi-tui` selector-related checks**

Run:

```bash
cargo test -p pi-tui select_list settings_list autocomplete
```

Expected: selector, settings list, and autocomplete filtered tests pass.

---

## Self-Review

Spec coverage: covers `/model <id>`, `/model` selector, prompt model propagation, cancellation, and roadmap update.

Intentional gaps: `/settings`, `/scoped-models`, `/resume`, `/tree`, auth, theme, terminal image, and TUI-7 smoke remain future M11 work.

Risk: `app.rs` is already large. If selector code grows beyond a small helper enum and builders, split model selector code into `crates/pi-coding-agent/src/interactive/model_selector.rs` during implementation.
