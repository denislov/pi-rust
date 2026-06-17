# Interactive TUI Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Do not commit unless the user explicitly requests a commit.

**Goal:** Add the M6 interactive-TUI readability polish: a `~`-abbreviated footer with cumulative token stats, a transcript-embedded startup welcome, a keybinding-derived key-hint helper, and a global `Ctrl+O` tool-output expand/collapse toggle.

**Architecture:** All changes live inside `crates/pi-coding-agent/src/interactive`. No `pi-tui` API changes, no `pi-agent-core` event changes, no new dependencies. The footer stays one line and keeps the `status: idle` substring. The welcome is a new `TranscriptItem::System` variant (transcript content, not a fixed row) so the height-6 editor-row anchor test is untouched. Tool expand is a root-level bool threaded into `render_transcript_lines` via a new `max_tool_result_lines` parameter.

**Tech Stack:** Rust edition 2024; existing `pi-tui` (`KeybindingsManager::get_keys`, `visible_width`, `truncate_to_width`), existing `pi-ai::types::Usage` via `AgentEvent::AgentDone`, existing faux-provider test harness in `interactive/app.rs::test_harness`.

## Global Constraints

- Every rendered line must satisfy `visible_width(line) <= width` (existing `fit_line` / `truncate_to_width` invariant).
- Tests are deterministic and offline; no real provider key, no network, no real TTY. Use the existing `VirtualTerminal` scripted harness and `FauxProvider`.
- The literal footer substrings `status: idle` and `status: running` MUST remain (existing assertions).
- The welcome must be transcript content, never a fixed UI row, so `scripted_interactive_keeps_prompt_anchored_below_transcript_viewport` (asserts `> typed` at row index 4 for a height-6 terminal) keeps passing.
- No ANSI color is introduced in this plan (deferred to a later milestone).
- Run checks from `pi-rust/` (the Cargo workspace root): `cargo fmt --check`, `cargo test -p pi-tui`, `cargo test -p pi-coding-agent`, `cargo test --workspace`, `cargo check --workspace`.

## Reference: existing signatures the plan builds on

These already exist; do not re-implement them:

```rust
// crates/pi-tui/src/input/keybindings.rs
impl KeybindingsManager {
    pub fn get_keys(&self, keybinding: &str) -> Vec<String>;
    pub fn matches(&self, event: &InputEvent, keybinding: &str) -> bool;
}

// crates/pi-coding-agent/src/interactive/app.rs (current)
const MAX_TOOL_RESULT_LINES: usize = 3;
fn render_transcript_lines(transcript: &Transcript, width: usize) -> Vec<String>;
fn render_tool_lines(call_id: &str, name: &str, result: Option<&str>, is_error: bool, width: usize) -> Vec<String>;
fn render_transcript_viewport(transcript: &Transcript, width: usize, viewport_rows: usize) -> Vec<String>;
fn fit_line(line: &str, width: usize) -> String;
fn render_tool_lines(call_id: &str, name: &str, result: Option<&str>, is_error: bool, width: usize) -> Vec<String>;
```

`InteractiveRoot` fields currently include `status: InteractiveStatus`, `cwd: PathBuf`, `model_id: String`, `session_label: String`, `viewport_width: usize`, `viewport_height: usize`, `transcript: Transcript`, `editor: Editor`, plus the action/submit/scroll plumbing.

## File Structure

- Create: `crates/pi-coding-agent/src/interactive/key_hints.rs` — keybinding-derived display text helper.
- Modify: `crates/pi-coding-agent/src/interactive/mod.rs` — declare `pub mod key_hints;` and re-export the helper.
- Modify: `crates/pi-coding-agent/src/interactive/transcript.rs` — add `TranscriptItem::System`; thread `max_tool_result_lines` is NOT here (render lives in app.rs; transcript only owns the model).
- Modify: `crates/pi-coding-agent/src/interactive/event_bridge.rs` — add `UiEvent::UsageUpdate`; accumulate `AgentDone` usage.
- Modify: `crates/pi-coding-agent/src/interactive/app.rs` — wire welcome, footer formatting, expand toggle, usage render; thread `max_tool_result_lines` through the render helpers.
- Modify: `crates/pi-coding-agent/tests/interactive_transcript.rs` — System + expand tests.
- Modify: `crates/pi-coding-agent/tests/interactive_event_bridge.rs` — UsageUpdate test.
- Modify: `crates/pi-coding-agent/tests/interactive_mode.rs` — welcome + footer stats assertions.

---

## Task 1: Key-hint helper

**Files:**
- Create: `crates/pi-coding-agent/src/interactive/key_hints.rs`
- Modify: `crates/pi-coding-agent/src/interactive/mod.rs`

**Interfaces:**
- Consumes: `pi_tui::KeybindingsManager::get_keys(action) -> Vec<String>`.
- Produces:
  - `pub fn format_key_text(keys: &[String]) -> String`
  - `pub fn key_hint(kb: &KeybindingsManager, action: &str, description: &str) -> String`
  - `pub fn app_key_hint(kb: &KeybindingsManager, action: &str, description: &str) -> String` (static fallback table for app actions not in the keybinding manager)

- [ ] **Step 1: Write failing tests**

Create `crates/pi-coding-agent/src/interactive/key_hints.rs` with only the tests and module declarations:

```rust
use pi_tui::{KeybindingsManager, TUI_KEYBINDINGS};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn kb() -> KeybindingsManager {
        KeybindingsManager::new(TUI_KEYBINDINGS.clone(), BTreeMap::new())
    }

    #[test]
    fn format_key_text_capitalizes_modifiers_and_keys() {
        assert_eq!(
            format_key_text(&["ctrl+c".to_string()]),
            "Ctrl+C"
        );
        assert_eq!(
            format_key_text(&["enter".to_string()]),
            "Enter"
        );
        assert_eq!(
            format_key_text(&["shift+enter".to_string()]),
            "Shift+Enter"
        );
    }

    #[test]
    fn format_key_text_joins_alternates_with_slash() {
        assert_eq!(
            format_key_text(&["ctrl+b".to_string(), "left".to_string()]),
            "Ctrl+B/Left"
        );
    }

    #[test]
    fn key_hint_uses_registered_binding() {
        let kb = kb();
        assert_eq!(key_hint(&kb, "tui.input.submit", "submit"), "Enter submit");
    }

    #[test]
    fn app_key_hint_uses_fallback_for_unknown_action() {
        let kb = kb();
        assert_eq!(
            app_key_hint(&kb, "app.interrupt", "interrupt"),
            "Ctrl+C interrupt"
        );
        assert_eq!(
            app_key_hint(&kb, "app.tools.expand", "expand tools"),
            "Ctrl+O expand tools"
        );
    }

    #[test]
    fn app_key_hint_falls_back_to_registered_when_present() {
        // tui.input.copy is registered to ctrl+c; app_key_hint should prefer the
        // registered binding over the static table when the action is known.
        let kb = kb();
        assert_eq!(
            app_key_hint(&kb, "tui.input.copy", "copy"),
            "Ctrl+C copy"
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent --lib key_hints
```

Expected: compile error — `format_key_text`, `key_hint`, `app_key_hint` are not defined.

- [ ] **Step 3: Implement the helper**

Append to `crates/pi-coding-agent/src/interactive/key_hints.rs` (above the test module):

```rust
use std::collections::BTreeMap;

/// Format a set of keybinding alternatives into display text.
///
/// `"ctrl+c"` -> `"Ctrl+C"`, `"shift+enter"` -> `"Shift+Enter"`.
/// Alternates are joined with `/`.
pub fn format_key_text(keys: &[String]) -> String {
    keys.iter()
        .map(|key| {
            key.split('+')
                .map(capitalize_part)
                .collect::<Vec<_>>()
                .join("+")
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn capitalize_part(part: &str) -> String {
    let mut chars = part.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Format a hint for a keybinding id known to the keybinding manager.
///
/// Falls back to the description alone if the action has no registered keys.
pub fn key_hint(kb: &KeybindingsManager, action: &str, description: &str) -> String {
    let keys = kb.get_keys(action);
    if keys.is_empty() {
        description.to_string()
    } else {
        format!("{} {}", format_key_text(&keys), description)
    }
}

/// Format a hint for an app-level action that may not be registered in
/// `TUI_KEYBINDINGS`. Falls back to a small static table, then to the
/// keybinding manager, then to the description alone.
pub fn app_key_hint(kb: &KeybindingsManager, action: &str, description: &str) -> String {
    if let Some(key) = app_fallback_key(action) {
        return format!("{} {}", format_key_text(&[key.to_string()]), description);
    }
    let keys = kb.get_keys(action);
    if keys.is_empty() {
        description.to_string()
    } else {
        format!("{} {}", format_key_text(&keys), description)
    }
}

fn app_fallback_key(action: &str) -> Option<&'static str> {
    match action {
        "app.interrupt" => Some("ctrl+c"),
        "app.exit" => Some("ctrl+c"),
        "app.tools.expand" => Some("ctrl+o"),
        _ => None,
    }
}
```

- [ ] **Step 4: Wire the module into `mod.rs`**

In `crates/pi-coding-agent/src/interactive/mod.rs`, add the module declaration and re-exports. The current file is:

```rust
pub mod app;
pub mod event_bridge;
pub mod transcript;

pub use app::run_interactive_mode;
pub use app::test_harness;
pub use event_bridge::{InteractiveEventBridge, UiEvent};
pub use transcript::{Transcript, TranscriptItem};
```

Change it to:

```rust
pub mod app;
pub mod event_bridge;
pub mod key_hints;
pub mod transcript;

pub use app::run_interactive_mode;
pub use app::test_harness;
pub use event_bridge::{InteractiveEventBridge, UiEvent};
pub use key_hints::{app_key_hint, format_key_text, key_hint};
pub use transcript::{Transcript, TranscriptItem};
```

- [ ] **Step 5: Run tests to verify they pass**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent --lib key_hints
```

Expected: PASS (5 tests).

- [ ] **Step 6: Commit**

```bash
cd pi-rust
git add crates/pi-coding-agent/src/interactive/key_hints.rs crates/pi-coding-agent/src/interactive/mod.rs
git commit -m "feat(interactive): add keybinding-derived key hint helper"
```

---

## Task 2: `TranscriptItem::System` variant

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/transcript.rs`
- Modify: `crates/pi-coding-agent/tests/interactive_transcript.rs`

**Interfaces:**
- Consumes: none new.
- Produces: `TranscriptItem::System { text: String }` and `TranscriptItem::system(impl Into<String>)`.

- [ ] **Step 1: Write failing tests**

Append to `crates/pi-coding-agent/tests/interactive_transcript.rs`:

```rust
#[test]
fn system_item_is_pushed_and_rendered_as_a_line() {
    let mut transcript = Transcript::new();
    transcript.push(TranscriptItem::system("welcome to pi"));
    assert_eq!(transcript.items().len(), 1);
    match transcript.items()[0] {
        TranscriptItem::System { ref text } => assert_eq!(text, "welcome to pi"),
        _ => panic!("expected System item"),
    }
}

#[test]
fn system_item_scrolls_like_other_items() {
    let mut transcript = Transcript::new();
    transcript.push(TranscriptItem::system("welcome"));
    transcript.scroll_page_up(2);
    assert_eq!(transcript.scroll_offset(), 2);
    transcript.scroll_to_bottom();
    assert_eq!(transcript.scroll_offset(), 0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent --test interactive_transcript
```

Expected: compile error — `TranscriptItem::system` does not exist and `System` variant is not constructible.

- [ ] **Step 3: Add the variant**

In `crates/pi-coding-agent/src/interactive/transcript.rs`, extend the `TranscriptItem` enum. The current enum starts:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum TranscriptItem {
    User {
        text: String,
    },
    Assistant {
        id: String,
        markdown: String,
        done: bool,
    },
    Tool {
        call_id: String,
        name: String,
        args: serde_json::Value,
        result: Option<String>,
        is_error: bool,
    },
    Error {
        text: String,
    },
}
```

Add a `System` variant after `Error`:

```rust
    Error {
        text: String,
    },
    System {
        text: String,
    },
}
```

Add a constructor next to the existing `error(...)` constructor:

```rust
    pub fn system(text: impl Into<String>) -> Self {
        Self::System { text: text.into() }
    }
```

- [ ] **Step 4: Handle the variant in `apply_event` (defensive)**

The `System` variant is never produced by an event in this milestone, but `apply_event` matches exhaustively on `UiEvent`, not `TranscriptItem`, so no match change is needed there. Confirm by building:

```bash
cargo check -p pi-coding-agent
```

Expected: PASS (no errors). If the compiler flags an exhaustive match elsewhere on `TranscriptItem`, add an inert `System { text } => { let _ = text; }` arm there.

- [ ] **Step 5: Run tests to verify they pass**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent --test interactive_transcript
```

Expected: PASS (4 tests: the 2 originals plus the 2 new ones).

- [ ] **Step 6: Commit**

```bash
cd pi-rust
git add crates/pi-coding-agent/src/interactive/transcript.rs crates/pi-coding-agent/tests/interactive_transcript.rs
git commit -m "feat(interactive): add TranscriptItem::System variant"
```

---

## Task 3: Thread `max_tool_result_lines` through render helpers

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`

**Interfaces:**
- Consumes: none new.
- Produces (internal, app.rs-local):
  - `fn render_transcript_lines(transcript: &Transcript, width: usize, max_tool_result_lines: usize) -> Vec<String>`
  - `fn render_tool_lines(call_id: &str, name: &str, result: Option<&str>, is_error: bool, width: usize, max_tool_result_lines: usize) -> Vec<String>`
  - `fn render_transcript_viewport(transcript: &Transcript, width: usize, viewport_rows: usize, max_tool_result_lines: usize) -> Vec<String>`

- [ ] **Step 1: Update the in-file unit test to the new signature**

In `crates/pi-coding-agent/src/interactive/app.rs`, the existing `#[cfg(test)]` test is:

```rust
    #[test]
    fn render_transcript_lines_compacts_tool_rows_and_truncates_noisy_output() {
        let mut transcript = Transcript::new();
        transcript.apply_event(UiEvent::ToolStarted {
            call_id: "tool_1".to_string(),
            name: "read".to_string(),
            args: serde_json::Value::Null,
        });

        assert_eq!(
            render_transcript_lines(&transcript, 80),
            vec!["tool read tool_1 running"]
        );

        transcript.apply_event(UiEvent::ToolFinished {
            call_id: "tool_1".to_string(),
            result: "line 1\nline 2\nline 3\nline 4\nline 5".to_string(),
            is_error: false,
        });

        assert_eq!(
            render_transcript_lines(&transcript, 80),
            vec![
                "tool read tool_1 done",
                "line 1",
                "line 2",
                "line 3",
                "... truncated 2 lines",
            ]
        );
    }
```

Replace both `render_transcript_lines(&transcript, 80)` call sites with `render_transcript_lines(&transcript, 80, 3)`:

```rust
        assert_eq!(
            render_transcript_lines(&transcript, 80, 3),
            vec!["tool read tool_1 running"]
        );

        transcript.apply_event(UiEvent::ToolFinished {
            call_id: "tool_1".to_string(),
            result: "line 1\nline 2\nline 3\nline 4\nline 5".to_string(),
            is_error: false,
        });

        assert_eq!(
            render_transcript_lines(&transcript, 80, 3),
            vec![
                "tool read tool_1 done",
                "line 1",
                "line 2",
                "line 3",
                "... truncated 2 lines",
            ]
        );
```

Also add an expanded-path assertion inside the same test, after the collapsed assertion:

```rust
        assert_eq!(
            render_transcript_lines(&transcript, 80, 20),
            vec![
                "tool read tool_1 done",
                "line 1",
                "line 2",
                "line 3",
                "line 4",
                "line 5",
            ]
        );
```

- [ ] **Step 2: Run tests to verify they fail**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent --lib render_transcript_lines_compacts_tool_rows_and_truncates_noisy_output
```

Expected: compile error — `render_transcript_lines` takes 2 args, not 3.

- [ ] **Step 3: Update the three render helpers**

In `crates/pi-coding-agent/src/interactive/app.rs`:

Change the signature and body of `render_transcript_lines` (currently at the `fn render_transcript_lines(transcript: &Transcript, width: usize) -> Vec<String>` definition). The current definition passes through to `render_tool_lines(call_id, name, result.as_deref(), *is_error, width)`. Replace the whole function with:

```rust
fn render_transcript_lines(
    transcript: &Transcript,
    width: usize,
    max_tool_result_lines: usize,
) -> Vec<String> {
    transcript
        .items()
        .iter()
        .flat_map(|item| match item {
            TranscriptItem::User { text } => vec![fit_line(&format!("user: {text}"), width)],
            TranscriptItem::System { text } => vec![fit_line(text, width)],
            TranscriptItem::Assistant { markdown, .. } => {
                let mut markdown = Markdown::new(markdown);
                markdown
                    .render(width)
                    .into_iter()
                    .map(|line| fit_line(&line, width))
                    .collect::<Vec<_>>()
            }
            TranscriptItem::Tool {
                call_id,
                name,
                result,
                is_error,
                ..
            } => render_tool_lines(
                call_id,
                name,
                result.as_deref(),
                *is_error,
                width,
                max_tool_result_lines,
            ),
            TranscriptItem::Error { text } => vec![fit_line(&format!("error: {text}"), width)],
        })
        .collect()
}
```

Change `render_tool_lines` from:

```rust
fn render_tool_lines(
    call_id: &str,
    name: &str,
    result: Option<&str>,
    is_error: bool,
    width: usize,
) -> Vec<String> {
    let status = match (result, is_error) {
        (None, _) => "running",
        (Some(_), true) => "error",
        (Some(_), false) => "done",
    };
    let mut lines = vec![fit_line(&format!("tool {name} {call_id} {status}"), width)];
    let Some(result) = result else {
        return lines;
    };

    let result_lines = result.lines().collect::<Vec<_>>();
    lines.extend(
        result_lines
            .iter()
            .take(MAX_TOOL_RESULT_LINES)
            .map(|line| fit_line(line, width)),
    );
    let omitted = result_lines.len().saturating_sub(MAX_TOOL_RESULT_LINES);
    if omitted > 0 {
        lines.push(fit_line(&format!("... truncated {omitted} lines"), width));
    }
    lines
}
```

to:

```rust
fn render_tool_lines(
    call_id: &str,
    name: &str,
    result: Option<&str>,
    is_error: bool,
    width: usize,
    max_tool_result_lines: usize,
) -> Vec<String> {
    let status = match (result, is_error) {
        (None, _) => "running",
        (Some(_), true) => "error",
        (Some(_), false) => "done",
    };
    let mut lines = vec![fit_line(&format!("tool {name} {call_id} {status}"), width)];
    let Some(result) = result else {
        return lines;
    };

    let result_lines = result.lines().collect::<Vec<_>>();
    lines.extend(
        result_lines
            .iter()
            .take(max_tool_result_lines)
            .map(|line| fit_line(line, width)),
    );
    let omitted = result_lines.len().saturating_sub(max_tool_result_lines);
    if omitted > 0 {
        lines.push(fit_line(&format!("... truncated {omitted} lines"), width));
    }
    lines
}
```

Change `render_transcript_viewport` from:

```rust
fn render_transcript_viewport(
    transcript: &Transcript,
    width: usize,
    viewport_rows: usize,
) -> Vec<String> {
    let lines = render_transcript_lines(transcript, width);
```

to:

```rust
fn render_transcript_viewport(
    transcript: &Transcript,
    width: usize,
    viewport_rows: usize,
    max_tool_result_lines: usize,
) -> Vec<String> {
    let lines = render_transcript_lines(transcript, width, max_tool_result_lines);
```

Leave the rest of `render_transcript_viewport` (the padding/slicing) unchanged.

- [ ] **Step 4: Update `InteractiveRoot::render` call site (deferred form)**

In `impl Component for InteractiveRoot`, the `render` method currently calls:

```rust
        let mut lines = render_transcript_viewport(&self.transcript, width, transcript_rows);
```

Replace with the collapsed constant for now (the expand toggle lands in Task 5):

```rust
        let mut lines =
            render_transcript_viewport(&self.transcript, width, transcript_rows, MAX_TOOL_RESULT_LINES);
```

Task 5 will rewrite this call site to choose between `MAX_TOOL_RESULT_LINES` and `EXPANDED_TOOL_RESULT_LINES` based on the expand flag.

- [ ] **Step 5: Run tests to verify they pass**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent --lib render_transcript_lines_compacts_tool_rows_and_truncates_noisy_output
cargo test -p pi-coding-agent --test interactive_transcript
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cd pi-rust
git add crates/pi-coding-agent/src/interactive/app.rs
git commit -m "refactor(interactive): thread max_tool_result_lines through render helpers"
```

---

## Task 4: `UiEvent::UsageUpdate` with cumulative totals

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/event_bridge.rs`
- Modify: `crates/pi-coding-agent/tests/interactive_event_bridge.rs`

**Interfaces:**
- Consumes: `AgentEvent::AgentDone { message: AssistantMessage }` where `AssistantMessage.usage: Usage`.
- Produces:
  - `UiEvent::UsageUpdate { input: u32, output: u32 }`
  - `InteractiveEventBridge` now carries a running `(u32, u32)` accumulator.

- [ ] **Step 1: Write failing tests**

Append to `crates/pi-coding-agent/tests/interactive_event_bridge.rs`. First read the file to mirror its existing imports. The existing imports are:

```rust
use pi_agent_core::{AgentEvent, AgentToolResult};
use pi_ai::types::{AssistantMessage, ContentBlock, StopReason, Usage};
use pi_coding_agent::interactive::{InteractiveEventBridge, UiEvent};
```

Append:

```rust
fn assistant_done_message(input: u32, output: u32) -> AssistantMessage {
    AssistantMessage {
        content: vec![ContentBlock::Text {
            text: "done".to_string(),
            cache_control: None,
        }],
        model: "faux".to_string(),
        provider: "faux".to_string(),
        api: "faux".to_string(),
        usage: Usage {
            input,
            output,
            cache_read: 0,
            cache_write: 0,
            total_tokens: input + output,
            cost: pi_ai::types::Cost::default(),
        },
        stop_reason: StopReason::Stop,
        id: None,
    }
}

#[test]
fn agent_done_emits_usage_update_with_cumulative_totals() {
    let mut bridge = InteractiveEventBridge::new();

    let first = bridge.handle(&AgentEvent::AgentDone {
        message: assistant_done_message(100, 40),
    });
    assert!(first.contains(&UiEvent::AssistantDone));
    assert!(first.contains(&UiEvent::UsageUpdate { input: 100, output: 40 }));

    let second = bridge.handle(&AgentEvent::AgentDone {
        message: assistant_done_message(250, 60),
    });
    assert!(second.contains(&UiEvent::UsageUpdate { input: 350, output: 100 }));
}
```

Note: the exact field list of `AssistantMessage` must match the real type. If the compiler reports a missing or extra field, read `crates/pi-ai/src/types.rs` around `pub struct AssistantMessage` and adjust the helper to match verbatim. `Cost::default()` is used so the test does not depend on cost fields; confirm `Cost` is in scope via `pi_ai::types::Cost` (it is re-exported there).

- [ ] **Step 2: Run tests to verify they fail**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent --test interactive_event_bridge
```

Expected: compile error — `UiEvent::UsageUpdate` does not exist and `AssistantMessage` construction may mismatch; fix field mismatches per the note above.

- [ ] **Step 3: Add the event variant**

In `crates/pi-coding-agent/src/interactive/event_bridge.rs`, extend the `UiEvent` enum. The current enum ends with:

```rust
    AgentError {
        error: String,
    },
    CompactionNotice {
        summary: String,
    },
}
```

Add a `UsageUpdate` variant:

```rust
    AgentError {
        error: String,
    },
    CompactionNotice {
        summary: String,
    },
    UsageUpdate {
        input: u32,
        output: u32,
    },
}
```

- [ ] **Step 4: Accumulate usage in the bridge**

Change the bridge struct and constructor. Currently:

```rust
#[derive(Debug, Default)]
pub struct InteractiveEventBridge;

impl InteractiveEventBridge {
    pub fn new() -> Self {
        Self
    }

    pub fn handle(&mut self, event: &AgentEvent) -> Vec<UiEvent> {
        match event {
            AgentEvent::TurnStart { .. } => vec![UiEvent::TurnStarted],
            AgentEvent::LlmEvent(event) => self.handle_llm_event(event),
            AgentEvent::ToolCallStart {
                tool_call_id,
                tool_name,
            } => vec![UiEvent::ToolStarted {
                call_id: tool_call_id.clone(),
                name: tool_name.clone(),
                args: serde_json::Value::Null,
            }],
            AgentEvent::ToolCallEnd {
                tool_call_id,
                result,
                ..
            } => vec![UiEvent::ToolFinished {
                call_id: tool_call_id.clone(),
                result: content_blocks_to_text(&result.content),
                is_error: result.is_error,
            }],
            AgentEvent::AgentDone { message } => vec![UiEvent::AssistantDone],
            AgentEvent::AgentError { error } => vec![UiEvent::AgentError {
                error: error.clone(),
            }],
            AgentEvent::SessionCompacted { summary, .. } => vec![UiEvent::CompactionNotice {
                summary: summary.clone(),
            }],
        }
    }
```

Replace the struct, constructor, and the `AgentDone` arm:

```rust
#[derive(Debug, Default)]
pub struct InteractiveEventBridge {
    total_input: u32,
    total_output: u32,
}

impl InteractiveEventBridge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle(&mut self, event: &AgentEvent) -> Vec<UiEvent> {
        match event {
            AgentEvent::TurnStart { .. } => vec![UiEvent::TurnStarted],
            AgentEvent::LlmEvent(event) => self.handle_llm_event(event),
            AgentEvent::ToolCallStart {
                tool_call_id,
                tool_name,
            } => vec![UiEvent::ToolStarted {
                call_id: tool_call_id.clone(),
                name: tool_name.clone(),
                args: serde_json::Value::Null,
            }],
            AgentEvent::ToolCallEnd {
                tool_call_id,
                result,
                ..
            } => vec![UiEvent::ToolFinished {
                call_id: tool_call_id.clone(),
                result: content_blocks_to_text(&result.content),
                is_error: result.is_error,
            }],
            AgentEvent::AgentDone { message } => {
                self.total_input = self.total_input.saturating_add(message.usage.input);
                self.total_output = self.total_output.saturating_add(message.usage.output);
                vec![
                    UiEvent::AssistantDone,
                    UiEvent::UsageUpdate {
                        input: self.total_input,
                        output: self.total_output,
                    },
                ]
            }
            AgentEvent::AgentError { error } => vec![UiEvent::AgentError {
                error: error.clone(),
            }],
            AgentEvent::SessionCompacted { summary, .. } => vec![UiEvent::CompactionNotice {
                summary: summary.clone(),
            }],
        }
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent --test interactive_event_bridge
```

Expected: PASS (5 tests: the 4 originals plus the new cumulative-usage test).

- [ ] **Step 6: Commit**

```bash
cd pi-rust
git add crates/pi-coding-agent/src/interactive/event_bridge.rs crates/pi-coding-agent/tests/interactive_event_bridge.rs
git commit -m "feat(interactive): emit cumulative UsageUpdate on AgentDone"
```

---

## Task 5: Wire welcome, footer stats, and expand toggle into `InteractiveRoot`

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`
- Modify: `crates/pi-coding-agent/tests/interactive_mode.rs`

**Interfaces:**
- Consumes (from earlier tasks):
  - `TranscriptItem::system(impl Into<String>)`
  - `UiEvent::UsageUpdate { input, output }`
  - `key_hints::key_hint` / `app_key_hint`
- Produces: end-to-end interactive behavior.

- [ ] **Step 1: Add the imports**

In `crates/pi-coding-agent/src/interactive/app.rs`, the `use pi_tui::{...}` block currently includes (among others):

```rust
use pi_tui::{
    Component, Editor, InputEvent, KeybindingsManager, Markdown, ProcessTerminal, RenderScheduler,
    StdinBuffer, TUI_KEYBINDINGS, Terminal, Tui, TuiError, is_key_release, matches_key,
    truncate_to_width, visible_width,
};
```

No new `pi_tui` import is required. The welcome text and key hints use the local module. Ensure the `use crate::interactive::{...}` line includes the new helper. The current line is:

```rust
use crate::interactive::{InteractiveEventBridge, Transcript, TranscriptItem, UiEvent};
```

Change it to:

```rust
use crate::interactive::{InteractiveEventBridge, Transcript, TranscriptItem, UiEvent};
use crate::interactive::key_hints::{app_key_hint, key_hint};
```

- [ ] **Step 2: Add the expanded-cap constant and root fields**

Near the existing `const MAX_TOOL_RESULT_LINES: usize = 3;` (top of `app.rs`), add:

```rust
const EXPANDED_TOOL_RESULT_LINES: usize = 20;
```

In `struct InteractiveRoot`, add two fields. The struct currently contains, near its end:

```rust
    viewport_width: usize,
    viewport_height: usize,
    cwd: PathBuf,
    model_id: String,
    session_label: String,
}
```

Change to:

```rust
    viewport_width: usize,
    viewport_height: usize,
    cwd: PathBuf,
    model_id: String,
    session_label: String,
    usage: (u32, u32),
    tool_output_expanded: bool,
}
```

- [ ] **Step 3: Initialize the fields and push the welcome item**

In `InteractiveRoot::new`, the struct is currently constructed ending with:

```rust
        Self {
            transcript: Transcript::new(),
            editor,
            submitted,
            scroll_command,
            pending_submit: None,
            action: InteractiveAction::None,
            status: InteractiveStatus::Idle,
            viewport_width: 80,
            viewport_height: 24,
            cwd,
            model_id,
            session_label,
        }
    }
```

Replace with (adding the welcome push, the keybindings for hints, and the new fields):

```rust
        let mut transcript = Transcript::new();
        let keybindings = KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default());
        transcript.push(TranscriptItem::system(welcome_line(&keybindings)));

        Self {
            transcript,
            editor,
            submitted,
            scroll_command,
            pending_submit: None,
            action: InteractiveAction::None,
            status: InteractiveStatus::Idle,
            viewport_width: 80,
            viewport_height: 24,
            cwd,
            model_id,
            session_label,
            usage: (0, 0),
            tool_output_expanded: false,
        }
    }
```

Note: `Editor::new(...)` earlier in this function already constructs its own `KeybindingsManager`; constructing a second one here purely for `welcome_line` is fine and keeps the welcome construction local. If you prefer, reuse a single manager — but the editor's manager is consumed by `Editor::new`, so a second instance is the minimal change.

- [ ] **Step 4: Add the `welcome_line` and `format_tokens` helpers**

Add these free functions near `fn fit_line` in `app.rs`:

```rust
fn welcome_line(keybindings: &KeybindingsManager) -> String {
    let parts = [
        key_hint(keybindings, "tui.input.submit", "submit"),
        key_hint(keybindings, "tui.input.newLine", "newline"),
        app_key_hint(keybindings, "app.interrupt", "interrupt/exit"),
        app_key_hint(keybindings, "app.tools.expand", "expand tools"),
        key_hint(keybindings, "tui.editor.pageUp", "scroll up"),
        key_hint(keybindings, "tui.editor.pageDown", "scroll down"),
    ];
    format!("pi · {}", parts.join(" · "))
}

fn format_tokens(count: u32) -> String {
    if count < 1000 {
        count.to_string()
    } else if count < 1000000 {
        format!("{}k", count / 1000)
    } else {
        format!("{}M", count / 1000000)
    }
}

fn abbreviate_cwd(cwd: &Path) -> String {
    let display = cwd.display().to_string();
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() && display.starts_with(&home) {
            return format!("~{}", &display[home.len()..]);
        }
    }
    display
}
```

- [ ] **Step 5: Rewrite the footer to include cwd + usage**

Replace the existing `footer` method:

```rust
    fn footer(&self) -> String {
        let status = match self.status {
            InteractiveStatus::Idle => "idle",
            InteractiveStatus::Running => "running",
        };
        format!(
            "status: {status} | cwd: {} | model: {} | session: {}",
            self.cwd.display(),
            self.model_id,
            self.session_label
        )
    }
```

with:

```rust
    fn footer(&self) -> String {
        let status = match self.status {
            InteractiveStatus::Idle => "idle",
            InteractiveStatus::Running => "running",
        };
        let cwd = abbreviate_cwd(&self.cwd);
        let mut parts = vec![
            format!("status: {status}"),
            format!("cwd: {cwd}"),
            format!("model: {}", self.model_id),
            format!("session: {}", self.session_label),
        ];
        if self.usage != (0, 0) {
            parts.push(format!("↑{} ↓{}", format_tokens(self.usage.0), format_tokens(self.usage.1)));
        }
        parts.join(" | ")
    }
```

- [ ] **Step 6: Store usage updates**

In `apply_events`, the method currently ends with the scrolled-view preservation block. The current method is:

```rust
    fn apply_events(&mut self, events: Vec<UiEvent>) {
        let previous_scroll_offset = self.transcript.scroll_offset();
        let previous_rows = if previous_scroll_offset > 0 {
            render_transcript_lines(&self.transcript, self.viewport_width).len()
        } else {
            0
        };
        for event in events {
            self.transcript.apply_event(event);
        }
        if previous_scroll_offset > 0 {
            let current_rows = render_transcript_lines(&self.transcript, self.viewport_width).len();
            self.transcript.preserve_scrolled_view_after_hidden_change(
                previous_scroll_offset,
                current_rows.saturating_sub(previous_rows),
            );
        }
    }
```

Replace with (handle `UsageUpdate` on the root before forwarding the rest to the transcript; the transcript's `apply_event` must not receive `UsageUpdate`):

```rust
    fn apply_events(&mut self, events: Vec<UiEvent>) {
        let previous_scroll_offset = self.transcript.scroll_offset();
        let previous_rows = if previous_scroll_offset > 0 {
            render_transcript_lines(&self.transcript, self.viewport_width, MAX_TOOL_RESULT_LINES).len()
        } else {
            0
        };
        for event in events {
            match event {
                UiEvent::UsageUpdate { input, output } => {
                    self.usage = (input, output);
                }
                other => self.transcript.apply_event(other),
            }
        }
        if previous_scroll_offset > 0 {
            let current_rows =
                render_transcript_lines(&self.transcript, self.viewport_width, MAX_TOOL_RESULT_LINES).len();
            self.transcript.preserve_scrolled_view_after_hidden_change(
                previous_scroll_offset,
                current_rows.saturating_sub(previous_rows),
            );
        }
    }
```

- [ ] **Step 7: Add the `Ctrl+O` expand toggle**

In `handle_input`, the method currently begins:

```rust
    fn handle_input(&mut self, event: &InputEvent) {
        if matches_key(event, "ctrl+c") {
            match self.status {
                InteractiveStatus::Running => {
                    self.action = InteractiveAction::AbortRunning;
                    return;
                }
                InteractiveStatus::Idle => {
                    if self.editor.text().is_empty() {
                        self.action = InteractiveAction::Exit;
                    } else {
                        self.editor.set_text("");
                    }
                    return;
                }
            }
        }

        if self.status == InteractiveStatus::Idle {
            self.editor.handle_input(event);
```

Insert a `Ctrl+O` block immediately after the `Ctrl+C` block and before the `if self.status == InteractiveStatus::Idle` block:

```rust
        if matches_key(event, "ctrl+o") {
            self.tool_output_expanded = !self.tool_output_expanded;
            return;
        }
```

- [ ] **Step 7b: Include `tool_output_expanded` in the render-state diff**

The input path triggers a re-render by comparing `render_state()` before and after `handle_input` (see `handle_input_event` in `app.rs`). If `tool_output_expanded` is not part of that state, flipping it produces `RenderRequest::NONE` and the expanded output never paints. Add it to both the struct and the accessor.

The current struct is:

```rust
#[derive(Debug, Clone, PartialEq)]
struct InteractiveRenderState {
    editor_text: String,
    editor_cursor: usize,
    transcript: Vec<TranscriptItem>,
    transcript_scroll_offset: usize,
    transcript_has_new_output_below: bool,
    status: InteractiveStatus,
}
```

Add a field at the end:

```rust
#[derive(Debug, Clone, PartialEq)]
struct InteractiveRenderState {
    editor_text: String,
    editor_cursor: usize,
    transcript: Vec<TranscriptItem>,
    transcript_scroll_offset: usize,
    transcript_has_new_output_below: bool,
    status: InteractiveStatus,
    tool_output_expanded: bool,
}
```

And the current `render_state` accessor ends with `status: self.status,`:

```rust
    fn render_state(&self) -> InteractiveRenderState {
        InteractiveRenderState {
            editor_text: self.editor.text().to_string(),
            editor_cursor: self.editor.cursor(),
            transcript: self.transcript.items().to_vec(),
            transcript_scroll_offset: self.transcript.scroll_offset(),
            transcript_has_new_output_below: self.transcript.has_new_output_below(),
            status: self.status,
        }
    }
```

Add the new field:

```rust
    fn render_state(&self) -> InteractiveRenderState {
        InteractiveRenderState {
            editor_text: self.editor.text().to_string(),
            editor_cursor: self.editor.cursor(),
            transcript: self.transcript.items().to_vec(),
            transcript_scroll_offset: self.transcript.scroll_offset(),
            transcript_has_new_output_below: self.transcript.has_new_output_below(),
            status: self.status,
            tool_output_expanded: self.tool_output_expanded,
        }
    }
```

- [ ] **Step 8: Render with the expanded/collapsed cap**

In `impl Component for InteractiveRoot`, the `render` method currently (after Task 3's deferred form) calls:

```rust
        let mut lines =
            render_transcript_viewport(&self.transcript, width, transcript_rows, MAX_TOOL_RESULT_LINES);
```

Replace with:

```rust
        let max_tool_result_lines = if self.tool_output_expanded {
            EXPANDED_TOOL_RESULT_LINES
        } else {
            MAX_TOOL_RESULT_LINES
        };
        let mut lines = render_transcript_viewport(
            &self.transcript,
            width,
            transcript_rows,
            max_tool_result_lines,
        );
```

- [ ] **Step 9a: Write failing scripted tests for welcome and footer usage**

Append to `crates/pi-coding-agent/tests/interactive_mode.rs`. The file already imports `run_scripted_idle_interactive` and `run_scripted_interactive` and defines `text_response`. Append:

```rust
#[tokio::test]
async fn scripted_interactive_shows_welcome_line_on_empty_transcript() {
    let output = run_scripted_idle_interactive("").await.unwrap();
    let frame = output.rendered_lines.join("\n");
    assert!(
        frame.contains("pi · "),
        "welcome line missing: {frame}"
    );
    assert!(
        frame.contains("submit"),
        "welcome line should mention submit: {frame}"
    );
}

#[tokio::test]
async fn scripted_interactive_footer_shows_usage_after_a_turn() {
    let provider = FauxProvider::new(vec![text_response("ok")]);
    let output = run_scripted_interactive(provider, "hi\r\x03")
        .await
        .unwrap();
    let frame = output.rendered_lines.join("\n");
    assert!(
        frame.contains("status: idle"),
        "footer must keep status: idle: {frame}"
    );
    assert!(
        frame.contains("↑") && frame.contains("↓"),
        "footer should show usage stats after a turn: {frame}"
    );
}
```

Note: the faux provider reports `Usage { input: 10, output: 20, .. }` on every `Done` event (see `crates/pi-ai/src/providers/faux.rs`), so after one turn the footer renders `↑10 ↓20` and both arrows are present.

- [ ] **Step 9b: Add an in-file unit test for the `Ctrl+O` toggle**

The scripted harness runs with `tools: Vec::new()` and `register_builtins: false`, so it cannot execute a tool call end-to-end, and the faux provider does not synthesize tool *results* (it only streams tool-call deltas; results come from real tool execution). A scripted integration test for tool expansion is therefore infeasible without a new harness variant. Instead, verify the toggle + render path directly on `InteractiveRoot` in the in-file `#[cfg(test)] mod tests` block in `crates/pi-coding-agent/src/interactive/app.rs`.

The existing in-file test module already has `use super::*;` (which brings in `InteractiveRoot`, `TranscriptItem`, `InputEvent`, `StdinBuffer`, `Transcript`, `UiEvent`) and `use super::*;` makes `PathBuf` available (it is imported at the top of `app.rs`). Append this test inside the existing `#[cfg(test)] mod tests { ... }` block (the one that currently holds `render_transcript_lines_compacts_tool_rows_and_truncates_noisy_output`):

```rust
    #[test]
    fn ctrl_o_toggles_tool_output_expansion_in_root() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_viewport_size(40, 24);
        root.transcript.push(TranscriptItem::Tool {
            call_id: "tool_1".to_string(),
            name: "read".to_string(),
            args: serde_json::Value::Null,
            result: Some("l1\nl2\nl3\nl4\nl5\nl6".to_string()),
            is_error: false,
        });

        let collapsed = root.render(40).join("\n");
        assert!(
            collapsed.contains("... truncated"),
            "collapsed tool output should show truncation: {collapsed}"
        );

        // Ctrl+O is the single byte 0x0f, which parse_control_char maps to
        // Key::Char("o") + CTRL. Feed it through StdinBuffer like the real loop.
        let mut buffer = StdinBuffer::new();
        let events = buffer.process("\x0f");
        assert_eq!(events.len(), 1, "ctrl+o should produce one input event");
        root.handle_input(&events[0]);
        assert!(root.tool_output_expanded, "ctrl+o should flip the expand flag");

        let expanded = root.render(40).join("\n");
        assert!(
            !expanded.contains("... truncated"),
            "expanded tool output should not show truncation: {expanded}"
        );
        assert!(
            expanded.contains("l6"),
            "expanded tool output should show the last line: {expanded}"
        );
    }
```

If `root.transcript` or `root.tool_output_expanded` are not accessible because of visibility, the in-file test module is inside the same crate and the fields are module-private (not `pub`), so `use super::*;` makes them reachable — no visibility change is needed. If the compiler still complains, the test is in the wrong module; ensure it is literally inside the existing `mod tests { use super::*; ... }` in `app.rs`.

- [ ] **Step 10: Run tests to verify they fail**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent --test interactive_mode
cargo test -p pi-coding-agent --lib ctrl_o_toggles_tool_output_expansion_in_root
```

Expected: the welcome and footer-usage scripted tests FAIL (behavior not wired yet), and the in-file ctrl-o test FAILS to compile (the toggle and `tool_output_expanded` field do not exist yet) until Steps 1-8 are applied.

- [ ] **Step 11: Run tests to verify they pass**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent --lib ctrl_o_toggles_tool_output_expansion_in_root
cargo test -p pi-coding-agent --test interactive_mode
cargo test -p pi-coding-agent --test interactive_abort
cargo test -p pi-coding-agent --test interactive_transcript
cargo test -p pi-coding-agent --test interactive_event_bridge
```

Expected: all PASS, including the existing height-6 anchor test (`scripted_interactive_keeps_prompt_anchored_below_transcript_viewport`) — the welcome is transcript content and scrolls out of the 6-row viewport once there is assistant output, so `> typed` stays at row 4.

- [ ] **Step 12: Commit**

```bash
cd pi-rust
git add crates/pi-coding-agent/src/interactive/app.rs crates/pi-coding-agent/tests/interactive_mode.rs
git commit -m "feat(interactive): welcome line, footer usage stats, Ctrl+O tool expand"
```

---

## Task 6: Final verification

**Files:**
- No required file changes unless verification exposes bugs.

- [ ] **Step 1: Formatting**

Run from `pi-rust/`:

```bash
cargo fmt --check
```

Expected: PASS (no diff). If it reports a diff, run `cargo fmt` and re-check.

- [ ] **Step 2: Focused crate tests**

Run from `pi-rust/`:

```bash
cargo test -p pi-tui
cargo test -p pi-coding-agent
```

Expected: PASS.

- [ ] **Step 3: Workspace tests and check**

Run from `pi-rust/`:

```bash
cargo test --workspace
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 4: Inspect git log**

Run from `pi-rust/`:

```bash
git log --oneline -8
```

Expected: the polish commits sit on top of `3f88255` (the spec commit), with clean, focused messages.

- [ ] **Step 5: Manual smoke (optional, host-dependent)**

From `pi-rust/`, in a tmux session:

```bash
tmux new-session -d -s pi-polish -x 100 -y 30
tmux send-keys -t pi-polish "cargo run -p pi-coding-agent" Enter
sleep 2
tmux capture-pane -t pi-polish -p
tmux send-keys -t pi-polish C-c
tmux kill-session -t pi-polish
```

Expected: first capture shows the welcome line in the transcript and the `status: idle | cwd: ... | model: ... | session: ...` footer (no usage stats yet). Shell prompt is not left in raw mode after Ctrl+C.
