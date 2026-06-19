# M11 Box And TruncatedText Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add reusable `Box` and `TruncatedText` components to `pi-tui` so later M11 selector/dialog components can share padding, background, and single-line truncation behavior.

**Architecture:** Keep each component in a focused file under `crates/pi-tui/src/components/`. `TruncatedText` renders only the first source line, applies optional padding, truncates to visible terminal width, and pads to the requested render width. `Box` owns child components, renders them at an inner width, applies horizontal/vertical padding, and optionally applies a background callback to each padded line.

**Tech Stack:** Rust 2024, existing `pi-tui::Component`, `truncate_to_width`, `visible_width`, and existing component/public API tests.

---

## File Structure

- Create `crates/pi-tui/src/components/truncated_text.rs`: `TruncatedText` state, padding constructor, text mutation, width-safe render.
- Create `crates/pi-tui/src/components/box_component.rs`: public `Box` component with children, padding, optional background callback, invalidation, and width-safe render.
- Modify `crates/pi-tui/src/components/mod.rs`: export `Box` and `TruncatedText`.
- Modify `crates/pi-tui/src/lib.rs`: re-export `Box` and `TruncatedText` from the crate root.
- Modify `crates/pi-tui/tests/components.rs`: add focused behavior tests for both components.
- Modify `crates/pi-tui/tests/public_api.rs`: import and instantiate `Box` and `TruncatedText`.
- Modify `docs/roadmap/M11-interactive-ux.md`: record component-library progress for `Box` / `TruncatedText`.

## Task 1: TruncatedText Tests And Component

- [ ] **Step 1: Write failing tests**

Add these tests to `crates/pi-tui/tests/components.rs`:

```rust
#[test]
fn truncated_text_renders_first_line_padded_to_width() {
    let mut text = TruncatedText::new("alpha\nbeta");
    assert_eq!(text.render(8), vec!["alpha   ".to_string()]);
}

#[test]
fn truncated_text_applies_padding_and_truncates_to_available_width() {
    let mut text = TruncatedText::with_padding("abcdef", 1, 1);
    let lines = text.render(6);

    assert_eq!(
        lines,
        vec![
            "      ".to_string(),
            " abcd ".to_string(),
            "      ".to_string(),
        ]
    );
    assert!(lines.iter().all(|line| visible_width(line) <= 6));
}
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test -p pi-tui --test components truncated_text
```

Expected: compile failure because `TruncatedText` is not defined/exported yet.

- [ ] **Step 3: Implement `TruncatedText`**

Create `crates/pi-tui/src/components/truncated_text.rs`:

```rust
use crate::{Component, truncate_to_width, visible_width};

pub struct TruncatedText {
    text: String,
    padding_x: usize,
    padding_y: usize,
}

impl TruncatedText {
    pub fn new(text: impl Into<String>) -> Self {
        Self::with_padding(text, 0, 0)
    }

    pub fn with_padding(text: impl Into<String>, padding_x: usize, padding_y: usize) -> Self {
        Self {
            text: text.into(),
            padding_x,
            padding_y,
        }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }
}

impl Component for TruncatedText {
    fn render(&mut self, width: usize) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }

        let mut lines = Vec::new();
        let empty = " ".repeat(width);
        for _ in 0..self.padding_y {
            lines.push(empty.clone());
        }

        let padding_x = self.padding_x.min(width.saturating_sub(1) / 2);
        let available_width = width.saturating_sub(padding_x * 2);
        let first_line = self.text.split('\n').next().unwrap_or("");
        let display = truncate_to_width(first_line, available_width);
        let mut line = format!("{}{}{}", " ".repeat(padding_x), display, " ".repeat(padding_x));
        let line_width = visible_width(&line);
        if line_width < width {
            line.push_str(&" ".repeat(width - line_width));
        }
        lines.push(truncate_to_width(&line, width));

        for _ in 0..self.padding_y {
            lines.push(empty.clone());
        }
        lines
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
```

## Task 2: Box Tests And Component

- [ ] **Step 1: Write failing tests**

Add these tests to `crates/pi-tui/tests/components.rs`:

```rust
#[test]
fn box_component_adds_padding_around_children() {
    let mut panel = TuiBox::with_padding(1, 1);
    panel.add_child(std::boxed::Box::new(TruncatedText::new("alpha")));

    assert_eq!(
        panel.render(8),
        vec![
            "        ".to_string(),
            " alpha  ".to_string(),
            "        ".to_string(),
        ]
    );
}

#[test]
fn box_component_applies_background_to_padded_lines() {
    let mut panel = TuiBox::with_padding(1, 0);
    panel.set_background_fn(Some(std::boxed::Box::new(|line| format!("<{line}>"))));
    panel.add_child(std::boxed::Box::new(TruncatedText::new("ok")));

    assert_eq!(panel.render(6), vec!["< ok  >".to_string()]);
}

#[test]
fn box_component_clear_removes_children() {
    let mut panel = TuiBox::new();
    panel.add_child(std::boxed::Box::new(TruncatedText::new("alpha")));
    panel.clear();

    assert!(panel.render(8).is_empty());
}
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test -p pi-tui --test components box_component
```

Expected: compile failure because `Box` is not defined/exported yet.

- [ ] **Step 3: Implement `Box`**

Create `crates/pi-tui/src/components/box_component.rs` with:

- `pub type BackgroundFn = std::boxed::Box<dyn Fn(&str) -> String>;`
- `pub struct Box { children: Vec<std::boxed::Box<dyn Component>>, padding_x, padding_y, background_fn }`
- `new()`, `with_padding(padding_x, padding_y)`, `add_child`, `clear`, `set_background_fn`, `invalidate`.
- `render(width)` returns `Vec::new()` for width 0 or no children, renders children at an inner width, pads each line to the outer width, applies the background callback after padding, and preserves `visible_width(line) <= width` before background decoration.

Use `std::boxed::Box` explicitly inside this file so the component name does not conflict with Rust's allocation type.

## Task 3: Exports, Roadmap, Verification, Commit

- [ ] **Step 1: Export public symbols**

Update `crates/pi-tui/src/components/mod.rs`:

```rust
mod box_component;
mod truncated_text;

pub use box_component::{BackgroundFn, Box};
pub use truncated_text::TruncatedText;
```

Update `crates/pi-tui/src/lib.rs` component re-export list to include `BackgroundFn`, `Box`, and `TruncatedText`.

- [ ] **Step 2: Add public API smoke import**

Update `crates/pi-tui/tests/public_api.rs` to import `Box as TuiBox` and `TruncatedText`, then instantiate both:

```rust
let mut panel = TuiBox::new();
panel.add_child(std::boxed::Box::new(TruncatedText::new("Loading")));
let _ = panel.render(20);
```

- [ ] **Step 3: Update roadmap**

Under M11 item 3, add:

```markdown
> 进度：`pi-tui` 已新增 `Box` / `TruncatedText` 基础组件，覆盖 padding、单行截断、宽度约束和背景回调；后续 selectors/dialogs 可直接复用。
```

- [ ] **Step 4: Run verification**

Run:

```bash
cargo fmt --check
env -u NO_COLOR TERM=xterm-256color cargo test -p pi-tui
env -u NO_COLOR TERM=xterm-256color cargo test --workspace
cargo check --workspace
```

Expected: all pass. Use the color-enabled environment because existing markdown tests assert ANSI output.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/pi-tui/src/components/box_component.rs crates/pi-tui/src/components/truncated_text.rs crates/pi-tui/src/components/mod.rs crates/pi-tui/src/lib.rs crates/pi-tui/tests/components.rs crates/pi-tui/tests/public_api.rs docs/roadmap/M11-interactive-ux.md docs/superpowers/plans/2026-06-20-m11-box-truncated-text.md
git commit -m "feat: add tui box and truncated text"
```

Expected: commit succeeds with only this component-library slice included.
